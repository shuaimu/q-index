# 8. The Static Website and Its Data Files

QIndex (both the project and, separately, the per-scholar QIndex metric defined in
Chapter 5, *The Ranking Algorithm*) publishes its results as a **static website**.
`qindex build-site` renders every page and every data file once, into a directory
that any static host can serve; the reference deployment is the GitHub Pages
project site at <https://shuaimu.github.io/q-index/>. There is no server process:
the actix-web server, its `/api/*` JSON endpoints, the shared `AppState` and its
5-minute ranking cache, and the live-reload script were all removed in favour of
this design.

This chapter documents the build, the output layout and URL scheme, every page,
the JSON files the site publishes (which replace the old HTTP API), the
browser-side script that powers search, filters, and scholar profiles, and the
per-paper citation display. It also records, honestly, where the numbers on the
site come from and where they still disagree.

Cross-references: the ranking structs serialized into the data files are defined
in Chapter 3, *The Data Model*; the scoring functions invoked here live in
Chapter 5; the citation caches consumed for per-paper counts are described in
Chapter 6, *Citation Data Integration*. Chapter 7 places the generator in the
overall architecture, Chapter 9 documents the `build-site` command line, and
Chapter 10 covers deployment to GitHub Pages.

## The build

The entry point is `build_site(opts)` in `src/site/mod.rs`, called by the
`build-site` subcommand through `run_build_site` (`src/main.rs`). It runs
these steps in order:

1. Parse the corpus with `BibParser::parse_directory(bib_dir)`
   (`src/site/mod.rs`). Unlike the old server, which `.expect`ed here and
   panicked, a parse failure is returned as an error with context.
2. Run `PageRankCalculator::new(&graph).calculate()?` once and collect *all*
   venue and scholar rankings with `get_top_venues(usize::MAX, None, None)` and
   `get_top_scholars(usize::MAX, None)` (`src/site/mod.rs`). As on the old
   web path, `load_citation_cache` is not called, so the prestige fallback
   produces the venue scores (Chapter 5).
3. Look up a citation count for every paper (`paper_citations`,
   `src/site/mod.rs`; see "Per-paper citation display" below).
4. Build a `Ctx` (link builder; see "URLs and the base path"), then clear and
   recreate the output directory (`prepare_out_dir`, `src/site/mod.rs`).
5. Write the pre-rendered HTML pages, then every venue page, then the scholar
   shards and the other JSON files (`src/site/mod.rs`).
6. Copy `static/` to `static/` and, if `<book-dir>/index.html` exists, the
   rendered mdBook to `book/` (`src/site/mod.rs`); write an empty
   `.nojekyll` so GitHub Pages serves the files without Jekyll processing.

`prepare_out_dir` deletes the previous build before writing. As a guard against a
mistyped `--out-dir`, it refuses to delete a non-empty directory that does not
contain `.nojekyll` (the marker every build writes), and the build fails instead.

`run_build_site` prints a one-line report (`BuildReport`: HTML, JSON, and other
file counts plus total bytes), warns when no rendered book was found, and suggests
a local preview command. On the reference corpus a build observed in this
session wrote 1,030 HTML pages, 260 JSON files, and 59 other files, about 62 MB
in total, in roughly 13 seconds.

## Output layout

| Path | Kind | Contents |
|------|------|----------|
| `index.html` | pre-rendered | dashboard: totals, top-10 venues and scholars, field chart |
| `venues/index.html` | pre-rendered + JS filter | every ranked venue; `?field=` / `?tier=` filters |
| `venue/<slug>/index.html` | pre-rendered | venue page 1: header stats, top-10 authors, 20 papers |
| `venue/<slug>/page/<n>/index.html` | pre-rendered | venue page *n* (n ≥ 2) |
| `scholars/index.html` | pre-rendered + JS filter | top 100 scholars by QIndex; `?min_papers=` filter |
| `scholar/index.html` | JS shell | scholar profile for `?id=<scholar id>&page=<n>` |
| `search/index.html` | JS shell | search results for `?q=` |
| `statistics/index.html` | pre-rendered | dataset statistics tables |
| `about/index.html` | pre-rendered | project description and the "Open Data" list (`#open-data`) |
| `404.html` | pre-rendered | GitHub Pages serves it for any missing path |
| `data/search-index.json` | data | every venue and scholar with scores (search, autocomplete) |
| `data/scholars/00.json` … `ff.json` | data | scholar profiles, 256 shards |
| `data/venues.json` | data | all ranked venues (`Vec<VenueRanking>`) |
| `data/scholars.json` | data | top 100 scholars (`Vec<ScholarRanking>`) |
| `data/stats.json` | data | dataset statistics |
| `static/` | assets | `app.js`, `style.css` copied from the repository's `static/` |
| `book/` | assets | this book, copied from `docs/book` when it has been built |
| `.nojekyll` | marker | disables Jekyll on GitHub Pages; also marks the directory as a build |

Every page is an `index.html` inside a directory, so links end in `/` and work
unchanged on GitHub Pages, on `python3 -m http.server`, and on any other static
host. Detail pages exist for all 44 venues in the graph (not just the
CSRankings venues that appear in the rankings), so every venue name the site can
show has a page to link to.

The built `site/` directory is a build artifact. It is gitignored and never
committed; CI rebuilds it from `bib/` and `cache/citations/` on every push
(Chapter 10).

## URLs and the base path

A GitHub Pages *project* site lives under a path prefix — here `/q-index/` —
not at the domain root, so a link written as `/venues/` would point outside the
site. Every internal link is therefore built from a configurable base URL:

- `Base::new` (`src/site/mod.rs`) normalizes the `--base-url` argument to
  start and end with `/` (`""` and `"/"` become `/`; `q-index`, `/q-index`, and
  `/q-index/` all become `/q-index/`). A full `https://host/path` URL is also
  accepted and given a trailing slash.
- `Ctx` (`src/site/mod.rs`) is passed to every template and is the only
  way templates build links: `ctx.url("venues/")`, `ctx.venue_url(id)`,
  `ctx.venue_page_url(id, page)`, `ctx.scholar_url(id)`, and
  `ctx.author_url(name)`.
- The page template writes the base into `<meta name="qindex-base">`
  (`base_template` in `src/site/templates.rs`), and `static/app.js` reads it
  into its `BASE` constant so links built in the browser use the same prefix.

**Venue slugs.** The old server addressed venues by their internal id
(`/venue/USENIX_ATC`). The static site uses a lowercase ASCII slug instead:
`slugify` (`src/site/mod.rs`) keeps ASCII letters and digits, lowercases
them, and collapses every other run of characters into one `-` (so `USENIX_ATC`
becomes `usenix-atc`). `venue_slugs` (`src/site/mod.rs`) assigns slugs
in sorted id order and appends `-2`, `-3`, … on a collision, so every venue gets
a unique, filesystem-safe directory name.

**Scholar ids.** Scholar ids are derived from names and can contain non-ASCII
letters and apostrophes, so they are never used as file names. They travel in
the query string instead: `ctx.scholar_url` percent-encodes the id with
`urlencoding::encode` (`src/site/mod.rs`), and the browser decodes it.

The old routes map onto the new layout as follows:

| Old server route | Static site |
|------------------|-------------|
| `GET /` (AJAX shell + `/api/homepage`) | `index.html`, fully pre-rendered |
| `GET /venues?field=&tier=&limit=` | `venues/?field=&tier=` (filtered in the browser; `limit` ignored) |
| `GET /venue/{id}?page=&per_page=` | `venue/<slug>/`, `venue/<slug>/page/<n>/` (fixed 20 per page) |
| `GET /scholars?min_papers=&limit=` | `scholars/?min_papers=` (top 100; filtered in the browser) |
| `GET /scholar/{id}?page=&per_page=` | `scholar/?id=<id>&page=<n>` (fixed 20 per page) |
| `GET /search?q=` | `search/?q=` (computed in the browser) |
| `GET /statistics`, `GET /about` | `statistics/`, `about/` |
| `GET /api/venues`, `/api/scholars`, `/api/stats` | `data/venues.json`, `data/scholars.json`, `data/stats.json` |
| `GET /api/search?q=` | `data/search-index.json`, searched in the browser |
| `GET /api/homepage`, `/api/stats/fields` | rendered into `index.html` |
| `GET /api/citation-status` | removed |
| `/static/live-reload.js` | removed |

## Pre-rendered pages

All pages share `base_template(ctx, title, page, content)`
(`src/site/templates.rs`), which provides the navbar (Dashboard, Venues,
Scholars, Statistics, About, Book, and a search box that submits to `search/`),
the footer, and the asset links. The `page` argument is written to
`<body data-page="…">`; `static/app.js` uses it to decide which browser-side
behaviour to run.

**Dashboard (`index.html`).** `index_page` (`src/site/templates.rs`)
renders the four total cards (papers, venues, scholars, and
`total_citations = graph.edges.len()`), the top 10 venues by PageRank, the top
10 scholars by QIndex, and the "Venue Distribution by Field" bar chart. The old
homepage was an empty shell that fetched `/api/homepage` and `/api/stats/fields`
after load; now everything is in the HTML. The chart data — paper counts per
`venue.field`, largest first (`field_counts`, `src/site/mod.rs`) — is
embedded as an inline `FIELD_DATA` JSON literal by `json_script`
(`src/site/templates.rs`), which escapes `</` so a field name can never
close the `<script>` element.

**Venues (`venues/`).** `venues_page` (`src/site/templates.rs`) renders
every venue returned by `get_top_venues` — the CSRankings venues, in PageRank
order — with rank, tier badge, field, PageRank, impact factor, and paper count.
Each row carries `data-field` and `data-tier` attributes. The filter form
submits to the same page as a GET request; `initVenueFilters`
(`static/app.js`) reads `?field=` and `?tier=`, sets the two `<select>`s
to match, hides rows that fail the filter (case-insensitive substring match on
the field, exact match on the tier — the same rules as the old server), and
renumbers the visible ranks. If nothing matches it shows "No venues match these
filters."

**Venue pages (`venue/<slug>/…`).** `write_venue_pages`
(`src/site/mod.rs`) sorts each venue's papers newest first, sums
`cited_by.len()` into a total-citations figure, counts papers per author for a
top-10 author list (ties broken alphabetically so builds are deterministic), and
writes one page per 20 papers. `venue_detail_page` (`src/site/templates.rs`)
renders the header stats, the top authors, the papers, and the pagination bar;
the pagination window (first, last, and two pages either side of the current
one, with ellipses) is the same as before, but each link now points at a
pre-rendered page rather than `?page=`. Each paper (`paper_item`,
`src/site/templates.rs`) shows its title, authors, year, citation count,
a Google Scholar search link, a publisher or DOI link, and a copy-DOI button.

Author names in the top-authors list and in every paper's author line are now
links to the author's scholar profile. `ctx.author_url` maps a name to its
scholar id exactly as the parser does (`generate_scholar_id(normalize_author_name(name))`,
`src/site/mod.rs`) and only emits a link when that id exists in the
graph.

**Scholars (`scholars/`).** `scholars_page` (`src/site/templates.rs`)
renders the top 100 scholars by QIndex (the old page showed 50 by default) with
QIndex, h-index, CSRankings paper count, citation count, and top venues. Rows
carry `data-papers`; `initScholarFilters` (`static/app.js`) applies an
optional `?min_papers=` filter and renumbers the ranks. The old server had no
form for this parameter, and neither does the static page; it is reachable by
URL only.

**Statistics (`statistics/`).** `statistics_page` (`src/site/templates.rs`)
renders the same tables as before from `statistics_data`
(`src/site/mod.rs`): totals, the ten most recent years by paper count,
the ten largest venues, and the ten "most cited" papers by `cited_by.len()`.
Because `graph.edges` is empty (Chapters 3 and 6), the citation total is 0 and
the most-cited table degenerates to ties at zero, broken alphabetically by title.

**About (`about/`) and 404.** `about_page` (`src/site/templates.rs`)
keeps the old static content and adds an "Open Data" card (`id="open-data"`,
linked from the footer) listing the four public JSON files. `not_found_page`
renders `404.html`, which GitHub Pages returns for any path that does not exist.

## Browser-rendered pages

Two pages cannot reasonably be pre-rendered. Search depends on an arbitrary
query, and the corpus has about 43,942 scholars, so one HTML file per scholar
would add tens of thousands of files for profiles that are mostly one or two
papers long. Both are therefore *shells*: the template renders the page frame and
a loading spinner (with a `<noscript>` notice), and `static/app.js` fills in the
content from JSON.

### Search (`search/?q=`)

`search_shell_page` (`src/site/templates.rs`) renders the heading and an
empty `#search-results`. `renderSearchPage` (`static/app.js`) loads
`data/search-index.json` and runs `searchIndex` (`static/app.js`), which
keeps the old server's matching rules:

- the query is lowercased (and, new, trimmed; an empty query matches nothing);
- a venue matches if the query is a substring of its name, full name, or field;
- a scholar matches if the query is a substring of the lowercased display name
  or of the id with `_` turned back into spaces — the id is derived from the
  normalized name, so this covers the old `normalized_name` match.

Venues are listed in PageRank order and scholars in QIndex order, because the
index is stored that way. Scholar results are capped at 200 rows
(`MAX_SEARCH_RESULTS`, `static/app.js`) with a "Showing the top 200 of N"
notice; a two-letter query such as `li` matches over 5,000 scholars on the
current corpus. The navbar autocomplete (`initSearchAutocomplete`,
`static/app.js`) uses the same index and matcher, debounced by 300 ms,
and ignores results for a query the user has since changed.

### Scholar profiles (`scholar/?id=<id>&page=<n>`)

`scholar_shell_page` (`src/site/templates.rs`) renders the breadcrumb and
an empty `#scholar-root`. `renderScholarPage` (`static/app.js`) computes
the scholar's shard (see "Scholar shards" below), fetches
`data/scholars/<shard>.json`, looks up the id, and renders the profile with the
same layout as the old server's scholar page: affiliation, paper count, citation
count, h-index, a venues list, and the publications, 20 per page, with the same
pagination window as the Rust template (`paginationHtml`,
`static/app.js`). It updates `document.title` and the breadcrumb, and it
shows "Scholar not found." for an unknown id or "No scholar selected." when `id`
is missing. All text from the JSON is inserted through `escapeHtml`
(`static/app.js`), so names and titles cannot inject markup.

## The JSON data files

The site publishes its data as plain JSON files. There is no `ApiResponse`
envelope any more: each file is the bare value.

| File | Shape | Notes |
|------|-------|-------|
| `data/venues.json` | `Vec<VenueRanking>` | every ranked (CSRankings) venue, PageRank order |
| `data/scholars.json` | `Vec<ScholarRanking>` | top 100 by QIndex |
| `data/stats.json` | object | `total_papers`, `total_venues`, `total_scholars`, `total_citations`, `papers_by_year`, `papers_by_tier`, `papers_by_field` |
| `data/search-index.json` | object | see below |
| `data/scholars/<xx>.json` | object | see below |

`data/stats.json` (`stats_json`, `src/site/mod.rs`) has the same fields
as the `Statistics` payload the old `/api/stats` wrapped in its envelope; the
three maps are now `BTreeMap`s, so their keys come out sorted.

### `data/search-index.json`

`search_index` (`src/site/mod.rs`) writes:

```json
{
  "venues":   [ { "id": "SOSP", "name": "SOSP", "full_name": "...", "field": "Operating Systems",
                  "tier": "A*", "pagerank": 0.12, "url": "venue/sosp/" }, ... ],
  "scholars": [ [ "ion_stoica", "Ion Stoica", 61.2345, 0, 114 ], ... ]
}
```

(values illustrative). `venues` covers all venues in the graph, sorted by
PageRank, with `pagerank` taken from the computed rankings (0 for venues outside
the CSRankings list) and `url` relative to the base. `scholars` covers every
scholar as a compact `[id, name, qindex, h_index, paper_count]` array — about
44,000 of them, so the array form saves a lot over objects — sorted by QIndex
descending, then id. `qindex` is rounded to four decimals; `paper_count` is the
scholar's total paper count, as on the old search page. On the reference corpus
the file is about 2.0 MB, or about 0.73 MB gzip-compressed as GitHub Pages
serves it.

### Scholar shards

`write_scholar_shards` (`src/site/mod.rs`) writes one record per scholar,
split across 256 files named by two hex digits (`data/scholars/00.json` …
`ff.json`). Each file is an object mapping scholar id to:

```json
{
  "name": "Ion Stoica",
  "affiliations": [ ... ],          // omitted when empty
  "qindex": 61.2345,
  "h_index": 0,
  "citations": 0,                   // internal cited_by, summed over all papers
  "venues": [ [ "NSDI", 30 ], ... ],// venue name, paper count; most papers first
  "papers": [                       // newest first
    { "title": "...", "venue": "NSDI", "year": 2024, "citations": 12,
      "s2ag": true,                 // omitted when false
      "first_author": "...",        // for the Google Scholar link
      "doi": "...",                 // omitted when absent
      "publisher_url": "..." }      // omitted when there is a DOI
  ]
}
```

The browser derives the Google Scholar link from `title` and `first_author`
and the DOI link from `doi`, so those URLs are not stored. On the reference
corpus the shards hold about 92,400 paper entries (a paper appears once per
author) and total about 24 MB; a single shard is 66–139 KB, about 30–50 KB
compressed, so a profile costs one small download.

**Shard function.** A scholar's shard is the 32-bit FNV-1a hash of the UTF-8
bytes of its id, modulo 256 (`scholar_shard`, `src/site/mod.rs`;
`SCHOLAR_SHARDS`, `src/site/mod.rs`). The browser must compute the identical
value, so the function exists three times: in Rust, in `static/app.js`
(`shardOf`, using `TextEncoder` for the UTF-8 bytes and
`Math.imul(...) >>> 0` for unsigned 32-bit multiplication), and in the link
checker `scripts/check_site_links.py`. A unit test pins the Rust function to the
standard FNV-1a test vectors (`shard_matches_js_implementation` in
`src/site/mod.rs`). Changing the shard count or
the hash means changing all three together.

## Templating with maud

Pages are built with `maud`, a compile-time HTML macro, so markup is
type-checked and HTML-escaped by default. `base_template` links Bootstrap 5.3,
Bootstrap Icons, and Chart.js 4.4 from the jsDelivr CDN, plus the site's own
`static/style.css` and `static/app.js` through `ctx.url(...)`
(`src/site/templates.rs`). Compared with the server-era template:
jQuery is no longer loaded (nothing used it); the `RUST_ENV`-controlled
`live-reload.js` is gone; the footer's dead `/api/docs` link is replaced by
"Open Data (JSON)", and the footer states the build date
(`Ctx.generated`, `src/site/mod.rs`). The copyright still reads "© 2024".

The copy-DOI button no longer builds an inline `onclick` handler from the DOI
string. It carries the DOI in a `data-copy` attribute
(`src/site/templates.rs`), and one delegated click handler in `app.js`
(`initCopyButtons`, `static/app.js`) copies it, which is safe for any
characters a DOI may contain.

### `static/app.js`

`static/app.js` is the only script the site ships. On `DOMContentLoaded` it runs
the site-wide features — Bootstrap tooltips, the scroll-to-top button, search
autocomplete, sorting for `table.sortable`, and the copy buttons — and then
dispatches on `document.body.dataset.page` to `initVenueFilters`,
`initScholarFilters`, `renderSearchPage`, or `renderScholarPage`
(`static/app.js`). It keeps a few older helpers (`setupRealtimeSearch`,
`formatNumber`, `copyToClipboard`, `showToast`). The server-era
`exportData()` (which targeted `?format=` parameters no route honored) and the
`fetch`-monkeypatching loading indicator were removed.

## Per-paper citation display (S2AG)

Every paper on a venue page or scholar profile shows one citation number with its
source. The number is decided once, at build time, by `paper_citations`
(`src/site/mod.rs`): it calls `s2ag_citations::get_citation_count(title)`
for every paper (in parallel with rayon) and records the S2AG count when it is
non-zero, otherwise the internal `cited_by.len()`. Venue pages render this in
Rust; scholar shards store it as `citations` plus an `s2ag` flag, and `app.js`
renders it the same way:

```text
Citations: N (S2AG)        when the S2AG lookup found the paper
Citations: N (internal)    otherwise (N = cited_by.len(), currently always 0)
```

The backing store is still the global `Lazy<RwLock<S2AGCitationIndex>>`
(`src/s2ag_citations.rs:152`), which loads `cache/citations/s2ag_citations.json`
from a hardcoded relative path on first access. The build must therefore run from
the repository root; anywhere else the index loads empty and every paper falls
back to `(internal)` without an error. The lookup (`src/s2ag_citations.rs:71-148`)
tries an exact match on the normalized title and otherwise scans every entry with
`titles_match` (substring containment or more than 70% significant-word overlap).
That scan used to run for every paper on every detail-page request. It now runs
once per paper per build. Its generosity is visible in the output: about 23,900
of the 92,400 paper entries in the shards carry an S2AG count, far more than the
731 entries in the S2AG file, so some matches are surely loose. See Chapter 6 for
the matching rules.

### Two citation totals, two sources

"Total citations" still means different things in different places, though one
of the three sources the server used is gone:

| Surface | Source | Code |
|---------|--------|------|
| dashboard card, `statistics/`, `data/stats.json` | `graph.edges.len()` (internal, 0 today) | `src/site/mod.rs` |
| per paper on venue pages and scholar profiles | S2AG count from `s2ag_citations.json`, else `cited_by.len()` | `src/site/mod.rs` |

The third source, the DBLP/OpenAlex `combined_cache.json` summed by the old
`/api/citation-status` endpoint, is no longer read by any Rust code. That
endpoint — a process monitor that shelled out to `ps aux` and reported several
hardcoded placeholder metrics — was dropped, since a static site has no running
fetch process to observe.

## Behaviour changes from the server

Most pages look and behave as before. The differences, all deliberate:

- **Scores are the computed ones.** The server's search results and scholar
  pages read `Scholar.qindex`, `Scholar.h_index`, and `Venue.pagerank` from the
  graph, which the calculator never fills in, so they always showed 0. The site
  uses the computed rankings everywhere (Chapter 7). h-index is still 0 because
  the corpus has no internal citation edges.
- **Author names link to scholar profiles** on venue pages.
- **Venue URLs use slugs** (`venue/sosp/`) instead of internal ids.
- **Page sizes are fixed** at 20 papers; `per_page` and `limit` parameters are
  gone. The venues page shows every ranked venue and the scholars page the top 100.
- **Search trims the query and caps scholar results at 200.**
- **The dashboard is pre-rendered** instead of loading over AJAX.
- **`/api/citation-status` and live reload were removed.**

## Verifying a build

Because every link is computed, a wrong base path or a missing file shows up as a
broken link rather than an error, so the build is checked mechanically.
`scripts/check_site_links.py <site-dir> <base-url>` parses every HTML file in the
output (including the copied book), resolves each `href`, `src`, and form
`action` against the page's own URL (so relative links count too), and fails if
an internal link leaves the base path or points to a missing file (a trailing `/`
means `index.html`). For links to `scholar/?id=…` it also opens the right shard,
using its own copy of the shard function, and checks that the scholar exists.
On the reference build it checked 124,698 internal links in 1,047 HTML files
with no failures. CI runs it before every deployment (Chapter 10). It does not
execute JavaScript, so links that `app.js` builds at runtime are covered only
indirectly, through the shared base path and shard function.

## Summary

The website is a set of files generated in one pass by `qindex build-site`.
Rankings, venue pages, statistics, and the about page are pre-rendered HTML;
search and scholar profiles are rendered in the browser from a search index and
256 scholar shards; and the four public JSON files replace the old HTTP API.
Every link goes through one base path, so the same build logic serves a GitHub
Pages project site under `/q-index/` or a local preview at `/`. The honest
caveats carried over from the server era still apply: venue scores come from the
prestige fallback because the citation cache is not loaded, graph-based citation
totals are zero, and the only non-zero per-paper citation numbers come from the
separate, loosely matched S2AG index. What is gone is the server's operational
risk — panicking handlers, lock poisoning, the shell-out, and the stale-cache
window — along with any need to run, secure, or monitor a process.
