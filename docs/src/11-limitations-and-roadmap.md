# 11. Limitations, Known Issues, and Roadmap

This chapter is an honest accounting of what QIndex does *today* versus what its
structure suggests it is *meant* to do. Throughout, "QIndex" refers to the
project as a whole; "QIndex" (the metric) and the `qindex` field on the
`Scholar` struct refer to the per-scholar score produced by the ranking
algorithm. Where a feature is a fallback, a stub, or aspirational, this chapter
says so plainly and describes what actually happens when the code runs. Every
claim is tied to a source path or line and, where relevant, to the sibling
chapter that owns the topic in full.

The README mentions Bootstrap, Chart.js, and a WASM build and shows sample
ranking tables. The CDN-loaded Bootstrap/Chart.js claim is real (see
`base_template` in `src/web/templates.rs:80`), but the sample tables and any
WASM target are not reproducible from the repository and are not relied on here.

A useful one-line summary of the current state comes from the running server:
`GET /api/stats` reports roughly 19,954 papers, 44 venues, and 43,942 scholars,
with `total_citations = 0`. That last zero is the root cause of most of the
algorithmic limitations below.

## 11.1 Algorithmic limitations

### 11.1.1 The citation graph is empty, so PageRank never runs

Chapter 5 (*The Ranking Algorithm*) describes a venue-level PageRank with power
iteration, damping, and convergence. That code exists
(`src/algorithm.rs:255-378`) but is **dead at runtime**. The parser initializes
every paper with `citations: Vec::new()` and `cited_by: Vec::new()`
(`src/parser.rs:166-167`) and never writes `Paper.citations` from any BibTeX
field — `parse_entry` (`src/parser.rs:151-219`) has no case that populates it.
`build_citation_network` (`src/models.rs:174-192`) derives `cited_by` by
iterating `citations`, which is empty, so `cited_by` stays empty too.

The result: `build_venue_graph` produces zero edges in its BibTeX branch
(`src/algorithm.rs:188-249`), and the singular cache file `./cache/citations.json`
that `load_citation_cache` looks for (`src/algorithm.rs:84-97`) does not exist
on disk — only the directory `cache/citations/` with differently named files is
present. With `venue_graph` empty, `has_citations` is false and execution falls
into the **prestige fallback** (`src/algorithm.rs:334-367`):

$$
\text{score}(v) = \frac{\ln(\text{papers}_v + 1)}{10}\cdot
\bigl(\text{is\_csrankings}(v)\,?\,2:1\bigr)
+ \frac{\bigl(\sum \text{charcodes}(\text{name}_v)\bigr)\bmod 100}{10000}
$$

followed by normalization and the tier bonus (`src/algorithm.rs:380-403`). The
name-hash term exists only to break ties between venues with identical paper
counts. In other words, the "PageRank" shown in the UI today is a paper-count
prestige heuristic, not a link-analysis score. The convergence/normalization
block in `calculate_venue_pagerank` also has a brace-nesting quirk
(`src/algorithm.rs:293-333`) that entangles the convergence check with the
per-`from_venue` loop; because the branch is never reached, the bug is latent
rather than observed.

Consequently `h_index` and `citation_count` are `0` for every scholar (the
h-index loop reads `cited_by.len()`, which is always zero;
`src/algorithm.rs:486-512`), and venue `impact_factor` reduces to
`pagerank * ln(paper_count + 1)`.

### 11.1.2 The reference year is hardcoded to 2024

The scholar QIndex applies a recency decay using a literal `current_year = 2024`
(`src/algorithm.rs:426`):

$$
\text{decay}(p) = \begin{cases}
0.95^{(2024 - \text{year}_p)/5} & 2024 - \text{year}_p > 0\\
1 & \text{otherwise}
\end{cases}
$$

Two issues follow. First, the exponent is divided by 5, so this is *not* a plain
$0.95^{\text{age}}$ — it is a much gentler decay than the parameter name
`year_decay` implies. Second, the constant is stale *and unsafe*: `current_year`
is inferred as `u32` (matching `paper.year: Option<u32>`), so `current_year -
year` is an unsigned subtraction performed *before* the `> 0` guard. Any paper
from 2025 or later underflows — a debug build panics
(`attempt to subtract with overflow`) and the release build wraps to ~`4.29e9`,
which drives the decay factor to ~0 and effectively zeroes that paper's
contribution (the opposite of leaving it undecayed). The fix is to derive the
reference year from the system clock or a CLI parameter **and** compute the
difference in a signed type or behind a guard, as the BibTeX path already does
with an `as i32` cast (`src/algorithm.rs:211`). See Chapter 5,
"The Ranking Algorithm".

### 11.1.3 Web and CLI scoring diverge

The web server and the CLI compute different rankings from the same bib data.
The CLI read commands (`calculate`, `venues`, `scholars`, `search`) call
`calculator.load_citation_cache("./cache/citations.json")`
(`src/main.rs:89-90,118-119,136-137,154-155`) before `calculate()`. The web path
constructs `PageRankCalculator::new(&*graph)` in `src/web/state.rs:70,99` and
calls `calculate()` **without ever calling `load_citation_cache`** — that
function is invoked nowhere in the web codebase. So in the web path
`citation_cache` stays `None` and `has_cached_citations` is false unconditionally
(`src/algorithm.rs:124-127`).

In practice, because `./cache/citations.json` is also missing, *both* paths land
in the prestige fallback today, so the divergence is currently masked. But the
moment that file is provided, the CLI and web UI will silently disagree. This is
a structural hazard, documented here against Chapters 8 (*Web Interface and HTTP
API*) and 9 (*The Command-Line Interface*).

## 11.2 Data limitations

### 11.2.1 Coverage: ~731 of ~21,000 papers have external citation data

The S2AG integration (Chapter 6, *Citation Data Integration*) matched only
**731** papers to Semantic Scholar, carrying ~41,468 total citations, stored in
`cache/citations/s2ag_citations.json` (`cache/citations/s2ag_summary.json`).
Against a corpus of roughly 21,000 BibTeX entries (`bib/` totals ~20,898
`@inproceedings` + 122 `@article`), that is about a 3.5% citation-data hit rate.
These counts feed the **per-paper display only** — `get_real_citations`
(`src/web/templates.rs:5-7`) calls `s2ag_citations::get_citation_count`, rendering
`Citations: N (S2AG)` on venue/scholar detail pages
(`src/web/templates.rs:1093-1095, 1293-1295`) and falling back to the
(always-zero) internal `cited_by.len()` otherwise.

### 11.2.2 Multiple inconsistent citation sources

At least four pipelines coexist, and three different numbers are reported for
"total citations" depending on which endpoint you hit:

| Surface | Source | What it counts |
|---|---|---|
| `/api/stats`, `/statistics`, `/api/homepage` | `graph.edges.len()` (internal) | 0 (edges never populated) |
| `/api/citation-status` | `combined_cache.json` | sum of 599 papers' counts (~4,145) |
| Per-paper detail pages | `s2ag_citations.json` | S2AG count for 731 papers (~41,468 total) |

These sets do not overlap cleanly: the homepage stats come from the
DBLP+OpenAlex `combined_cache.json` (599 papers), per-paper numbers from S2AG
(731 papers), and the internal `edges` count is always zero
(`src/web/handlers.rs:669-720`, `src/web/handlers.rs:543`,
`src/web/templates.rs:5-7`). A user comparing the homepage total to a paper's
displayed citations will see inconsistent figures. The S2AG citation *graph*
artifact `cache/citations/s2ag_citation_graph.json` has 190 nodes but only one
`cites` edge and one `cited_by` edge total — it is an orphan written by no
current script and read by no code. The CLAUDE.md claim of "285 citation
relationships" is not reflected in any present artifact.

### 11.2.3 Fuzzy title matching can mismatch

S2AG lookups normalize the title (lowercase, alphanumeric + whitespace only;
`src/s2ag_citations.rs:108-116`), try an exact `HashMap` hit, then fall back to a
**linear scan** over all 731 entries using `titles_match`
(`src/s2ag_citations.rs:118-148`). That predicate returns true on substring
containment of either normalized title, or on >70% overlap of significant
(length-> 3) words. Substring containment can produce false positives between
papers sharing a generic prefix or suffix, and the scan is $O(N)$ per unmatched
title — run per paper on every detail-page render (up to 100 papers per page).
This is correctness *and* performance debt; see Chapter 6.

## 11.3 Implementation limitations

### 11.3.1 Reference extraction is a stub

The `extract-papers` subcommand (Chapter 9) cannot extract references.
`extract_references_from_pdf` logs `"PDF extraction not yet implemented"` and
returns `Ok(Vec::new())` (`src/paper_extractor.rs:374-382`), so `reference_count`
is always 0 even when a PDF is found. Most PDF-finder backends are also stubbed
to return `Ok(None)`: `search_usenix` and `search_google_scholar`
(`src/paper_extractor.rs:324-347`), `find_acm_paper` and `find_on_author_page`
(`src/paper_finder.rs:147-156, 240-248`). Only ArXiv title matching and CrossRef
DOI/metadata are functional. The command runs to completion but produces no
reference data.

### 11.3.2 Placeholder fields in `/api/citation-status`

`api_citation_status` (`src/web/handlers.rs:623`) returns a `CitationStatus`
whose progress metrics are hardcoded (`src/web/handlers.rs:801-816`):
`success_rate = 100.0` when `total_papers > 0`, `papers_per_minute = 2.0`
(commented "Estimated rate"), `active_sources = ["DBLP", "OpenAlex"]` always,
`current_conference = None` always, and `successful == total_attempts ==
total_papers` (which makes `success_rate` definitionally meaningless).
Per-conference `papers_fetched` and `citations` are hardcoded to 0
(`src/web/handlers.rs:748-749`, commented "Would need to track per-conference").
The endpoint also shells out to `ps aux | grep ... fetch_citations`
(`src/web/handlers.rs:766-768`) to guess whether a fetch is running. The
citation-status dashboard therefore shows mostly fabricated metrics, not live
progress (Chapter 8).

### 11.3.3 `unwrap`/`expect` panics and lock poisoning

Failure handling leans on panics. At startup, `parse_directory(...).expect(...)`
and `APP_STATE.set(...).expect(...)` abort the process on error
(`src/web/server.rs:16,31`). Per request, `api_citation_status` calls
`std::fs::read_dir(bib_dir).unwrap()` (`src/web/handlers.rs:730`), which panics
the worker if `bib/` becomes unreadable. Every handler takes
`state.graph.read().unwrap()` and the cache locks use `.read().unwrap()` /
`.write().unwrap()` (`src/web/handlers.rs:92,111,175,...`; `src/web/state.rs:58,
69,76,...`). Because these are `RwLock`s, a single panic while a write lock is
held poisons the lock and turns every subsequent request into a panic — a
cascading failure mode. Separately, `calculate().ok()`
(`src/web/state.rs:71,100`) silently swallows ranking errors and still caches
whatever the (possibly empty) result was for the 5-minute staleness window.

### 11.3.4 Dead code, warnings, and unused dependencies

- `HashSet` is imported in `src/models.rs` but unused.
- A free function `calculate_h_index` (`src/algorithm.rs:631-651`) duplicates the
  method `calculate_scholar_h_index` and is never called; the discarded
  `_impact` local in `apply_tier_bonus` (`src/algorithm.rs:391`) is computed and
  thrown away.
- `CitationGraph.edges` (`Vec<CitationEdge>`) is never populated, so
  `CitationEdge.weight/year/cross_venue` are dead fields (Chapter 3).
- `Scholar.affiliations` is declared but never filled by the parser.
- `src/citations.rs` (the legacy async Semantic Scholar fetcher) persists to a
  `citations.json` that does not exist in `cache/citations/` and is never invoked
  for display.
- `exportData()` in `static/app.js:226-239` targets `?format=` query params that
  no route honors; the footer links to `/api/docs`
  (`src/web/templates.rs:149`), which is not a registered route (404).
- The dependency footprint is large (363 packages in `Cargo.lock`) including
  `actix-session`, `petgraph`, and `ndarray`, several of which do not appear in
  the active code paths and inflate the LTO release build (Chapter 10).

## 11.4 Prioritized roadmap

The items below are ordered by impact-to-effort. Each names the chapter/section it
affects so a contributor can find the surrounding design.

### P0 — Make the rankings real and consistent

1. **Populate a paper-level citation graph and feed PageRank.** Wire
   `s2ag_citations.json` (and a real `s2ag_citation_graph.json`) into
   `Paper.citations`/`cited_by`, or populate `CitationGraph.edges` directly, so
   `build_venue_graph` produces non-empty edges and the power-iteration path in
   `src/algorithm.rs:255-378` actually runs. This retires the prestige fallback
   (§11.1.1) and enables non-zero `h_index`/`citation_count`. *Affects Ch. 5
   §PageRank, Ch. 6 §Graph extraction.* Fix the brace-nesting bug at
   `src/algorithm.rs:293-333` as part of this, since it becomes live.

2. **Unify the web and CLI scoring paths.** Either call `load_citation_cache` in
   the web path (`src/web/state.rs:70,99`) or remove it from the CLI, and settle
   on one canonical cache location instead of the singular
   `./cache/citations.json` the CLI hardcodes vs. the `cache/citations/`
   directory everything else uses. *Affects Ch. 8 §State, Ch. 9 §Cache paths.*

3. **Unify citation sources.** Pick one pipeline (S2AG is the most complete) and
   route the homepage stats, `/api/stats`, and per-paper display through it so
   the three "total citations" numbers in §11.2.2 agree. Deprecate the legacy
   `src/citations.rs` and the dead S2ORC pipeline. *Affects Ch. 6.*

### P1 — Correctness and safety

4. **Parameterize the reference year.** Replace the hardcoded `2024`
   (`src/algorithm.rs:426`) with `chrono::Utc::now().year()` or a CLI/config
   value, and document the $/5$ decay exponent. *Affects Ch. 5 §Recency.*

5. **Add authentication before any network exposure.** `qindex web --host
   0.0.0.0` binds to all interfaces (`src/web/server.rs:62`) with no auth. Add at
   least a token or basic-auth layer before recommending `0.0.0.0`. Also rotate
   the API key that is committed in plaintext in CLAUDE.md and rely solely on the
   `S2_API_KEY` env var (`.env.example`). *Affects Ch. 8, Ch. 10 §Deployment.*

6. **Replace request-path panics with graceful errors.** Convert the
   `.unwrap()`/`.expect()` calls in handlers and locks (§11.3.3) to `Result`
   returns or `actix` error responses, and consider `parking_lot` locks or
   poison recovery to avoid cascading failure. *Affects Ch. 7 §Architecture,
   Ch. 8.*

### P2 — Feature completion and hygiene

7. **Finish reference extraction** or remove `extract-papers` until it works:
   implement the PDF backends and `extract_references_from_pdf`
   (`src/paper_extractor.rs:374-382`). *Affects Ch. 9.*

8. **Replace placeholder `/api/citation-status` fields** (§11.3.2) with real
   per-conference tracking, or label them as estimates in the UI. *Affects Ch. 8.*

9. **Index S2AG lookups.** Replace the $O(N)$ fuzzy scan
   (`src/s2ag_citations.rs:118-148`) with a precomputed index and tighten the
   substring rule to reduce false matches. *Affects Ch. 6.*

10. **Add tests and benchmarks.** There is no observed test coverage for the
    parser, ranking, or matching logic. At minimum: golden tests for
    `normalize_author_name`/`normalize_venue_id`, a small fixture graph that
    exercises the live PageRank path once §P0.1 lands, and a benchmark for
    detail-page render time under the fuzzy-match load. *Affects all of Part III
    and IV.*

11. **Prune dead code and unused deps** (§11.3.4): drop the unused `HashSet`
    import, the duplicate `calculate_h_index`, the discarded `_impact`, the
    non-functional `exportData()`/`/api/docs` link, and audit the 363-package
    dependency tree to shrink the LTO release build. *Affects Ch. 10 §Build.*

## 11.5 Summary

QIndex parses a substantial bibliographic corpus correctly and serves a
functional web and CLI surface, but its headline algorithm does not yet run as
designed: the citation graph is empty, PageRank is short-circuited to a
paper-count prestige heuristic, and the three citation numbers exposed across the
UI come from three unreconciled sources. The reference year is frozen at 2024,
the web and CLI scoring paths are structurally divergent, reference extraction is
stubbed, and several request paths panic on error. None of these are
architectural dead ends — the roadmap above turns the existing scaffolding into a
working link-analysis ranking, primarily by populating the citation graph
(§P0.1) and unifying the data sources (§P0.3). Until then, readers and evaluators
should treat the venue and scholar rankings as prestige-weighted paper counts,
not citation-based scores.
