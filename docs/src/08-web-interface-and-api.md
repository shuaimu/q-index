# 8. The Web Interface and HTTP API

QIndex (both the project and, separately, the per-scholar QIndex metric defined in
Chapter 5, *The Ranking Algorithm*) ships a single `actix-web` HTTP server that
serves a server-rendered HTML site and a small JSON API over the in-memory
`CitationGraph`. This chapter documents the server lifecycle, every page and
endpoint, the templating and AJAX approach, and the per-paper citation display.
It also records, honestly, several behaviors that diverge from what the README
and dashboards suggest: the web ranking path uses a fallback scoring scheme,
three different citation totals are computed from three unrelated data sources,
and several dashboard fields are hardcoded placeholders.

Cross-references: the data structures returned by the API are defined in
Chapter 3, *The Data Model*; the scoring functions invoked here live in
Chapter 5; the citation caches consumed for per-paper counts are described in
Chapter 6, *Citation Data Integration*. The CLI that shares the same scoring
code (but loads different data) is Chapter 9.

## Server lifecycle and shared state

The entry point is `start_server(bib_dir, host, port)` in
`src/web/server.rs:10`. It runs entirely before the HTTP listener opens:

1. Construct a `BibParser` and call `parser.parse_directory(bib_dir)`
   (`src/web/server.rs:14-16`). The result is unwrapped with
   `.expect("Failed to parse bibliography")`, so a parse failure aborts
   startup with a panic rather than a graceful error.
2. Log paper, venue, and scholar counts, then log every venue name and its
   paper count (`src/web/server.rs:18-27`). Observed on the current corpus:
   roughly 19,954 papers, 44 venues, and 43,942 scholars.
3. Wrap the graph in `AppState::new(graph)` inside an `Arc`
   (`src/web/server.rs:30`) and publish it into the process-global
   `OnceCell` via `APP_STATE.set(state.clone()).expect("Failed to set
   application state")` (`src/web/server.rs:31`). A second `set` would panic;
   in practice it is called exactly once.
4. Build the `HttpServer` with `Logger` and `Compress` middleware, register the
   routes, mount static files, and `.bind((host, port))` (`src/web/server.rs:36-62`).

`APP_STATE` is a `static OnceCell<Arc<AppState>>` (`src/web/state.rs:6`). Handlers
do not receive state through actix's extractor; they read the global directly.
`AppState` (`src/web/state.rs:8-13`) holds three `Arc<RwLock<...>>` fields:

| Field | Type | Role |
|-------|------|------|
| `graph` | `Arc<RwLock<CitationGraph>>` | the parsed corpus, read on every request |
| `last_update` | `Arc<RwLock<DateTime<Utc>>>` | set at construction, never read by the web code |
| `cache` | `Arc<RwLock<Cache>>` | memoized top-100 venue and scholar rankings |

### Host, port, and network exposure

The `Web` subcommand (`src/cli.rs:90-102`) defines `--bib-dir`/`-b`
(default `./bib`), a long-only `--host` (default `127.0.0.1`), and `--port`/`-p`
(default `8080`, `u16`). `main.rs:46` destructures these and forwards them
through `run_web_server` (`src/main.rs:341`) into `start_server`, which binds
`(host, port)` at `src/web/server.rs:62`. The default binding is loopback only;
`qindex web --host 0.0.0.0 -p 8080` exposes the site on all interfaces. There is
no authentication, TLS, or rate limiting in the server, so `0.0.0.0` publishes
the full corpus and all endpoints (including `/api/citation-status`, which shells
out to `ps aux`) to the network.

All filesystem paths used at runtime — `./static`, `cache/citations/*.json`, and
`bib/` — are relative to the process working directory. The server therefore only
works correctly when launched from the repository root; from elsewhere it silently
serves no static assets and reads empty caches.

### The 5-minute rank cache

Ranking is expensive relative to a page render, so `AppState` memoizes it.
`Cache` (`src/web/state.rs:15-44`) stores `Option<Vec<VenueRanking>>`,
`Option<Vec<ScholarRanking>>`, and a `last_refresh` timestamp. Staleness is a
fixed window:

```rust
pub fn is_stale(&self) -> bool {
    let now = chrono::Utc::now();
    (now - self.last_refresh).num_minutes() > 5
}
```

`get_or_calculate_venues(top)` (`src/web/state.rs:55-82`) returns the cached
slice when the cache is fresh *and* the stored vector already holds at least
`top` entries; otherwise it takes the graph read lock, builds a
`PageRankCalculator`, calls `calculate().ok()`, fetches `get_top_venues(100, None, None)`,
stores the full 100 entries, and refreshes the timestamp.
`get_or_calculate_scholars(top)` (`src/web/state.rs:84-111`) mirrors this with
`get_top_scholars(100, None)`.

Two consequences follow. First, the cache is keyed only on the global top-100
ranking — there is no per-field, per-tier, or per-`min_papers` key. Filtering by
those parameters happens in the handlers *after* the cache returns, so any request
asking for `top <= 100` is served from the shared cache. Second,
`calculate().ok()` discards errors silently (`src/web/state.rs:71,100`); a failed
calculation still caches whatever `get_top_venues`/`get_top_scholars` returned
(possibly empty) and refreshes the timestamp, so a transient failure can be
"stuck" for up to five minutes. `Cache::invalidate()` exists
(`src/web/state.rs:36-39`) but is never called, so there is no way to force a
refresh short of waiting out the timer or restarting.

### Fallback scoring on the web path

The most important behavioral note: the web ranking path does **not** load the
external citation cache. `get_or_calculate_*` construct the calculator with
`PageRankCalculator::new(&*graph)` (`src/web/state.rs:70,99`) and call
`calculate()` without ever calling `load_citation_cache`. That method is invoked
only from CLI commands (`src/main.rs:90,119,137,155`). As a result, inside
`calculate()` the citation cache stays `None`, `has_cached_citations` is false
(`src/algorithm.rs:124-127`), and — because the parser never populates
`Paper.citations` (`src/parser.rs:166`) — the BibTeX-derived venue graph is empty.
The active branch is therefore the prestige fallback at `src/algorithm.rs:334-367`,
which scores each venue by `ln(paper_count + 1)/10`, doubles CSRankings venues,
adds a small name-hash perturbation, normalizes, and applies tier bonuses. See
Chapter 5 for the full fallback formula. The practical upshot is that the web UI
and the CLI `venues`/`scholars` commands can produce different rankings from the
same corpus, because the CLI does load `cache/citations`.

## HTML pages

Eight HTML routes are registered (`src/web/server.rs:42-49`); each handler returns
a `maud::Markup`.

| Route | Handler | Shows |
|-------|---------|-------|
| `GET /` | `index_handler` → `index_page_async()` | shell page that AJAX-loads the dashboard (`src/web/handlers.rs:45-48`) |
| `GET /venues` | `venues_handler` | ranked venue table, filterable by field/tier (`src/web/handlers.rs:50`) |
| `GET /venue/{id}` | `venue_detail_handler` | one venue: papers (year desc), top-10 authors, paginated (`src/web/handlers.rs:101`) |
| `GET /scholars` | `scholars_handler` | ranked scholar table, filterable by min papers (`src/web/handlers.rs:69`) |
| `GET /scholar/{id}` | `scholar_detail_handler` | one scholar: papers, venues, paginated (`src/web/handlers.rs:166`) |
| `GET /search` | `search_handler` | venue + scholar matches for `?q=` (`src/web/handlers.rs:87`) |
| `GET /about` | `about_handler` | static maud card content (`src/web/handlers.rs:225`) |
| `GET /statistics` | `statistics_handler` | server-rendered stats tables (`src/web/handlers.rs:287`) |

`venues_handler` reads `VenueQuery { field, tier, limit }` (limit default 50),
calls `get_or_calculate_venues(limit)`, then applies a case-insensitive substring
filter on `v.field` and an exact tier match before rendering. `scholars_handler`
reads `ScholarQuery { min_papers, limit }`, fetches `get_or_calculate_scholars(limit)`,
and filters by `paper_count >= min_papers`.

The two detail handlers share a pagination convention:

```text
per_page = query.per_page.unwrap_or(20).min(100)
page     = query.page.unwrap_or(1).max(1)
total_pages = (total_papers + per_page - 1) / per_page
start = (page - 1) * per_page
end   = min(start + per_page, total_papers)
```

Both return HTTP 404 if the `{id}` does not resolve in the graph. `venue_detail_handler`
sums `cited_by.len()` across the venue's papers for a "total citations" figure and
computes a top-10 author list; `scholar_detail_handler` groups the scholar's papers
by venue and sums `cited_by.len()`. Because the parser never fills `cited_by`
(Chapter 3), these `cited_by`-derived totals are effectively zero on the detail
pages; the per-paper S2AG numbers described below are the only non-zero citation
figures rendered.

`statistics_handler` computes `total_papers`/`total_venues`/`total_scholars`,
sets `total_citations = graph.edges.len()`, builds a `papers_by_year` `BTreeMap`,
a top-10 `papers_by_venue` list, and a top-10 "most cited" list ordered by
`cited_by.len()`. Since `graph.edges` is empty (Chapter 3, Chapter 6), the
citation total on this page is 0 and the most-cited ranking degenerates.

## JSON API endpoints

Seven JSON routes are registered (`src/web/server.rs:51-57`). Most wrap their
payload in a generic envelope, `ApiResponse<T>` (`src/web/handlers.rs:35-41`):

```json
{ "success": true, "data": <T>, "error": "<omitted when null>" }
```

The `error` field uses `skip_serializing_if = "Option::is_none"`, so it is absent
on success.

| Route | Method | Params | Response |
|-------|--------|--------|----------|
| `/api/homepage` | GET | none | `ApiResponse<HomepageData>` (`handlers.rs:455`) |
| `/api/venues` | GET | none | `ApiResponse<Vec<VenueRanking>>`, top 100 (`handlers.rs:490`) |
| `/api/scholars` | GET | none | `ApiResponse<Vec<ScholarRanking>>`, top 100 (`handlers.rs:504`) |
| `/api/search` | GET | `q` | `ApiResponse<SearchResults>` (`handlers.rs:518`) |
| `/api/stats` | GET | none | `ApiResponse<Statistics>` (`handlers.rs:543`) |
| `/api/stats/fields` | GET | none | **raw** `FieldData` (no envelope) (`handlers.rs:594`) |
| `/api/citation-status` | GET | none | **raw** `CitationStatus` (no envelope) (`handlers.rs:623`) |

`HomepageData` bundles a `Stats` block (`total_papers`, `total_venues`,
`total_scholars`, and `total_citations = graph.edges.len()`) with
`get_or_calculate_venues(10)` and `get_or_calculate_scholars(10)`.
`/api/search` returns `SearchResults { venues, scholars }` from
`graph.search_venues(q)` and `graph.search_scholars(q)`.

`/api/stats` returns a richer `Statistics` struct: `total_papers`, `total_venues`,
`total_scholars`, `total_citations = graph.edges.len()`, plus
`papers_by_year: HashMap<u32,usize>`, `papers_by_tier`, and `papers_by_field`
(the latter two summed from `venue.papers.len()` per tier/field). On the current
corpus `/api/stats` reports `total_citations = 0` because the in-memory edge list
is empty.

Two endpoints deliberately break the envelope. `/api/stats/fields` returns a bare
`FieldData { labels: Vec<String>, values: Vec<usize> }` sorted by count descending,
consumed directly by the homepage's Chart.js field chart. `/api/citation-status`
returns a flat `CitationStatus` struct. This inconsistency is load-bearing for
the front end: `static/app.js` reads `data.data` from the search endpoint
(envelope-shaped), while the homepage JS reads `/api/stats/fields` as raw — so a
future refactor that unifies envelopes would break one or the other consumer.

### Three citation totals, three sources

A recurring source of confusion is that "total citations" means three different
things depending on where it is read:

| Surface | Source | Code |
|---------|--------|------|
| `/api/stats`, `/api/homepage`, `/statistics` | `graph.edges.len()` (internal, empty today) | `handlers.rs:543,455,287` |
| `/api/citation-status` | sum of `citation_count` in `combined_cache.json` | `handlers.rs:684-687` |
| per-paper on detail pages | S2AG count from `s2ag_citations.json`, else `cited_by.len()` | `templates.rs:1093-1097` |

These pipelines are independent and span different paper sets (the in-memory
graph, the ~599-entry DBLP/OpenAlex combined cache, and the ~731-entry S2AG
index — see Chapter 6), so the numbers do not agree and are not expected to.

## A detailed look at `/api/citation-status`

`api_citation_status` (`src/web/handlers.rs:623-819`) is a status dashboard
backend that mixes real data, process inspection, and hardcoded placeholders.

**Citation totals and distribution.** It reads `cache/citations/combined_cache.json`
first (`handlers.rs:669,679`); if absent it falls back to
`cache/citations/paper_cache.json` (`handlers.rs:711`). A `dblp_cache.json` path
is declared but never used (`handlers.rs:670`). From the chosen cache it sets
`total_papers = cache.len()`, sums `citation_count` into `total_citations`, and
bins each paper into a `citation_distribution` histogram:

```rust
let bucket = match count {
    0..=10   => "0-10",
    11..=50  => "11-50",
    51..=100 => "51-100",
    101..=500 => "101-500",
    _        => "500+",
};
```

It also collects the first five papers iterated as `recent_papers`
(`handlers.rs:700-707`) — "recent" only by `HashMap` iteration order, not by date.

**Per-conference rows.** It scans `bib/` with `std::fs::read_dir(bib_dir).unwrap()`
(`handlers.rs:730`) and, for each `*.bib` file, counts
`@inproceedings` + `@article` occurrences as that conference's `total_papers`
(`handlers.rs:737-741`). The `unwrap()` here is a panic risk on every request: if
`bib/` exists but is unreadable, the request worker panics.

**Process detection.** To report whether a fetch is live, it shells out:

```sh
ps aux | grep -E 'python.*fetch_citations' | grep -v grep | head -1
```

(`handlers.rs:766-768`). A non-empty result sets `running = true` and parses the
PID (second whitespace field) and process name (fields 10+). Separately,
`last_update_seconds_ago` is derived from the mtime of `combined_cache.json`
(`handlers.rs:790-796`, default `999999`), and the final flag is
`is_running = process_info.running || last_update_seconds_ago < 60`
(`handlers.rs:799`).

**Hardcoded placeholders.** Several fields are fabricated rather than measured
(`handlers.rs:801-816`):

| Field | Value | Note |
|-------|-------|------|
| `success_rate` | `100.0` if `total_papers > 0` else `0.0` | meaningless — see below |
| `papers_per_minute` | `2.0` | comment marks it "Estimated rate" |
| `active_sources` | `["DBLP", "OpenAlex"]` | always, regardless of any run |
| `current_conference` | `None` | always |
| `successful` / `total_attempts` | both `= total_papers` | so `success_rate` is tautological |
| per-conference `papers_fetched` | `0` | comment: "Would need to track per-conference" |
| per-conference `citations` | `0` | never populated |

Because `successful == total_attempts` by construction, `success_rate` carries no
information. The dashboard fed by this endpoint therefore presents a mix of one
real number (the combined-cache citation total), live process detection, and
otherwise placeholder metrics. Treat it as a status *mock-up* with a real backing
number, not as live fetch telemetry.

## Templating with maud

Pages are built with `maud`, a compile-time HTML macro (`src/web/templates.rs:1`),
so markup is type-checked and HTML-escaped by default. `base_template`
(`templates.rs:80`) provides the navbar/footer shell and pulls Bootstrap 5.3,
Bootstrap Icons, Chart.js 4.4, and jQuery 3.7 from CDNs, plus `/static/style.css`
and `/static/app.js` (`templates.rs:88-92,164-165`). It conditionally injects
`/static/live-reload.js` when the `RUST_ENV` environment variable is not
`"production"` (defaulting to `"development"`, `templates.rs:167`). Note that the
footer links to `/api/docs`, which is not a registered route (it 404s), and the
copyright reads "© 2024".

The `static/` directory holds exactly three files: `app.js`, `live-reload.js`, and
`style.css`. (The README mentions a WebAssembly module; no such asset exists in
the served tree, and nothing in the code references one. Treat that claim as
aspirational.)

### The AJAX homepage

`GET /` returns a lightweight shell (`index_page_async()`) that fetches its data
client-side rather than rendering server-side. The shell embeds inline JavaScript
via `PreEscaped` (`templates.rs:255+`). On `DOMContentLoaded` it calls
`fetch('/api/homepage')`, checks `result.success`, and passes `result.data` to a
`renderDashboard` function that builds the stats cards, the top-venues table, and
the top-scholars table in the browser, then hides `#loading` and shows
`#dashboard-content` (`templates.rs:258-351`). A second call, `loadFieldChart()`,
fetches `/api/stats/fields` and renders a Chart.js bar chart from the raw
`{labels, values}` payload (`templates.rs:353+`). This homepage JavaScript is
inline in the template; it is **not** part of `static/app.js`.

### `static/app.js`

`static/app.js` is the site-wide script loaded by `base_template`. It initializes
Bootstrap tooltips, a scroll-to-top button, and search autocomplete: a 300 ms
debounced `fetch('/api/search?q=...')` that reads `data.data.venues` and
`data.data.scholars` (`app.js:67-76`) — relying on the `ApiResponse` envelope. It
also implements client-side sorting for `table.sortable`, and an AJAX
loading-indicator that monkeypatches `window.fetch` (`app.js:210-222`). One helper,
`exportData()` (`app.js:226-239`), targets `/api/venues?format=`,
`/api/scholars?format=`, and `/api/stats?format=`, but no route honors a `format`
query parameter, so export is non-functional today — an aspirational stub.

## Per-paper citation display (S2AG)

Detail pages show a real citation count per paper through `get_real_citations`
(`src/web/templates.rs:5-7`), a thin wrapper over
`crate::s2ag_citations::get_citation_count(title)`. On both the venue and scholar
detail pages (`templates.rs:1093-1097` and `1293-1295`) the template computes the
count and chooses a label:

```rust
@let real_citations = get_real_citations(&paper.title);
@if real_citations > 0 {
    "Citations: " strong.text-primary { (real_citations) } " (S2AG)"
} @else {
    "Citations: " (paper.cited_by.len()) " (internal)"
}
```

So a paper with an S2AG match shows `Citations: N (S2AG)`; otherwise it falls back
to the (currently always zero) internal `cited_by.len()` labeled `(internal)`.

The backing store is a global `Lazy<RwLock<S2AGCitationIndex>>`
(`src/s2ag_citations.rs:152`) that loads
`cache/citations/s2ag_citations.json` on first access. The path is a hardcoded
relative path, so citations only load when the server runs from the repository
root; a load error logs and falls back to an empty index (no display citations).
Lookup (`src/s2ag_citations.rs:71-148`) first normalizes the title (lowercase,
alphanumeric and spaces only) and tries an exact `HashMap` hit; on a miss it runs
a fuzzy linear scan over every entry using `titles_match` (substring containment
or > 70% significant-word overlap). The fuzzy path is O(*index size*) per missed
title, executed once per paper rendered. On a detail page showing up to 100 papers
this is O(papers × index size) per page render. See Chapter 6 for the matching
rules and the S2AG pipeline.

## Reliability notes and panic risks

The web server uses `unwrap()`/`expect()` in several reachable places, and a
poisoned `RwLock` propagates failure:

- **Startup panics.** `parse_directory(...).expect(...)` (`src/web/server.rs:16`)
  and `APP_STATE.set(...).expect(...)` (`src/web/server.rs:31`) abort startup on
  failure instead of returning an error.
- **Per-request panic.** `std::fs::read_dir(bib_dir).unwrap()` in
  `api_citation_status` (`src/web/handlers.rs:730`) panics the worker if `bib/`
  is unreadable, and this path runs on every `/api/citation-status` hit.
- **Lock poisoning cascade.** Every handler takes the graph read lock with
  `state.graph.read().unwrap()` (e.g. `handlers.rs:92,111,175,292,463,523,548,599`),
  and the cache uses `.read().unwrap()`/`.write().unwrap()`
  (`src/web/state.rs:58,69,76,87,98,104`). If any earlier panic occurs while a
  write lock is held, the `RwLock` becomes poisoned and every subsequent request
  panics — a cascading failure. Within `calculate()` itself, the ranking code also
  contains `partial_cmp().unwrap()` sorts that would panic on a `NaN` score
  (Chapter 5).

These are acceptable for a single-tenant research tool on loopback but are real
concerns under the no-auth `--host 0.0.0.0` exposure, where untrusted clients can
repeatedly hit the shell-out and `read_dir` paths.

## Summary

The web layer is a thin, single-process actix-web server over an in-memory graph,
with a 5-minute global rank cache and a deliberately minimal API. The honest
caveats matter for anyone extending it: the web path scores via the prestige
fallback because it never loads the citation cache (unlike the CLI); the
in-memory edge count is zero, so `graph.edges.len()`-based citation totals read
zero; the only real per-paper citation numbers come from the separate S2AG index;
`/api/citation-status` is largely placeholder telemetry over one real number; the
API envelope is intentionally inconsistent between three endpoints; the
`exportData` and `/api/docs` features are dead; and all paths are CWD-relative.
Builders should treat the dashboards as illustrative of structure, not as live,
mutually consistent metrics, and should harden the `unwrap()` sites before any
networked deployment.
