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
`base_template` in `src/site/templates.rs`), but the sample tables and any
WASM target are not reproducible from the repository and are not relied on here.

A useful one-line summary of the current state comes from any site build:
`data/stats.json` reports roughly 19,954 papers, 44 venues, and 43,942 scholars,
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

### 11.1.3 Website and CLI scoring diverge

The site build and the CLI compute rankings from the same bib data along
different paths. The CLI read commands (`calculate`, `venues`, `scholars`,
`search`) call `calculator.load_citation_cache("./cache/citations.json")`
(`src/main.rs:116-118,145-147,163-165,181-183`) before `calculate()`. The site
generator (`build_site` in `src/site/mod.rs`) constructs
`PageRankCalculator::new(&graph)` and calls `calculate()` **without ever calling
`load_citation_cache`**, exactly as the removed web server did. So on the website
`citation_cache` stays `None` and `has_cached_citations` is false unconditionally
(`src/algorithm.rs:124-127`).

In practice, because `./cache/citations.json` is also missing, *both* paths land
in the prestige fallback today, so the divergence is currently masked. But the
moment that file is provided, the CLI and the website will silently disagree. This
is a structural hazard, documented here against Chapters 8 (*The Static Website
and Its Data Files*) and 9 (*The Command-Line Interface*).

## 11.2 Data limitations

### 11.2.1 Coverage: ~731 of ~21,000 papers have external citation data

The S2AG integration (Chapter 6, *Citation Data Integration*) matched only
**731** papers to Semantic Scholar, carrying ~41,468 total citations, stored in
`cache/citations/s2ag_citations.json` (`cache/citations/s2ag_summary.json`).
Against a corpus of roughly 21,000 BibTeX entries (`bib/` totals ~20,898
`@inproceedings` + 122 `@article`), that is about a 3.5% citation-data hit rate.
These counts feed the **per-paper display only** — the site build's
`paper_citations` (`src/site/mod.rs`) calls `s2ag_citations::get_citation_count`
once per paper, and venue pages and scholar profiles render
`Citations: N (S2AG)`, falling back to the (always-zero) internal
`cited_by.len()` otherwise.

### 11.2.2 Multiple inconsistent citation sources

At least four pipelines coexist. The website reports two different kinds of
"citation" number, and the removed web server reported a third:

| Surface | Source | What it counts |
|---|---|---|
| dashboard, `statistics/`, `data/stats.json` | `graph.edges.len()` (internal) | 0 (edges never populated) |
| Per-paper listings (venue pages, scholar profiles) | `s2ag_citations.json` | S2AG count for 731 entries (~41,468 total) |
| *(removed)* `/api/citation-status` | `combined_cache.json` | sum of 599 papers' counts (~4,145) |

These sets do not overlap cleanly: per-paper numbers come from S2AG (731
entries), the internal `edges` count is always zero (`stats_json` and
`statistics_data` in `src/site/mod.rs`), and the DBLP+OpenAlex
`combined_cache.json` (599 papers) is no longer read by any Rust code. A user
comparing the dashboard total to a paper's displayed citations will see
inconsistent figures. The S2AG citation *graph*
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
title. The site build runs it once per paper (about 20,000 lookups, parallelized
with rayon) rather than on every page render as the old server did, so the
performance cost is now a build-time cost. The false positives are visible in the
output: about 23,900 of the 92,400 author-paper entries in the scholar shards
carry an S2AG count, far more papers than 731 index entries can legitimately
match. This is correctness *and* performance debt; see Chapter 6.

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

### 11.3.2 Removed: placeholder fields in `/api/citation-status`

The old web server's `/api/citation-status` endpoint reported mostly hardcoded
progress metrics (a tautological `success_rate`, a fixed `papers_per_minute = 2.0`,
fixed `active_sources`, per-conference counts pinned to 0) plus a `ps aux` check
for a running fetch. It was removed with the server rather than ported, since a
static site has no fetch process to observe (Chapter 8). It is listed here only
so that older references to it can be traced.

### 11.3.3 Error handling

The server-era failure modes — `.expect` panics at startup, a per-request
`read_dir(...).unwrap()`, `RwLock` poisoning cascading across requests, and
`calculate().ok()` caching a failed ranking for five minutes — left with the
server. The site build (`build_site` in `src/site/mod.rs`) returns errors with
context instead: a missing bib directory, an unwritable output directory, or a
ranking error fails the build, and CI does not deploy. Two weaker spots remain.
First, the S2AG index still loads from a hardcoded CWD-relative path and silently
falls back to an empty index, so a build run from the wrong directory succeeds
but shows only `(internal)` citation counts (Chapter 10). Second, the ranking code
still sorts with `partial_cmp().unwrap()`, which would panic on a `NaN` score
(Chapter 5).

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
- `tests/integration_test.rs` has a stale assertion: `test_get_venue_field`
  expects the field "Systems" for SOSP/OSDI, but the code now returns
  "Operating Systems", so the test fails. CI runs only the library and site tests
  (Chapter 10).
- The dependency footprint shrank from 363 to 278 packages in `Cargo.lock` when
  the actix stack was removed, but `petgraph` and `ndarray` still have no call
  sites and still inflate the LTO release build (Chapter 10). (The server-era
  `exportData()` helper and dead `/api/docs` footer link were removed with the
  server.)

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

2. **Unify the website and CLI scoring paths.** Either call `load_citation_cache`
   in the site build (`build_site` in `src/site/mod.rs`) or remove it from the CLI, and settle
   on one canonical cache location instead of the singular
   `./cache/citations.json` the CLI hardcodes vs. the `cache/citations/`
   directory everything else uses. *Affects Ch. 8 §The build, Ch. 9 §Cache paths.*

3. **Unify citation sources.** Pick one pipeline (S2AG is the most complete) and
   route the dashboard totals, `data/stats.json`, and per-paper display through it
   so the "total citations" numbers in §11.2.2 agree. Deprecate the legacy
   `src/citations.rs` and the dead S2ORC pipeline. *Affects Ch. 6.*

### P1 — Correctness and safety

4. **Parameterize the reference year.** Replace the hardcoded `2024`
   (`src/algorithm.rs:426`) with `chrono::Utc::now().year()` or a CLI/config
   value, and document the $/5$ decay exponent. *Affects Ch. 5 §Recency.*

5. **Fail the build when citation data is missing.** The site build silently
   falls back to an empty S2AG index when `cache/citations/s2ag_citations.json`
   is not found from the working directory (§11.3.3). Resolving the path
   relative to the repository, or failing loudly, would stop a misconfigured
   build from publishing a site with every citation count at zero. *Affects
   Ch. 6, Ch. 10.*

6. **Remove the remaining panic paths.** Replace the `partial_cmp().unwrap()`
   sorts in the ranking code (§11.3.3) with `total_cmp` or explicit `NaN`
   handling. *Affects Ch. 5.*

### P2 — Feature completion and hygiene

7. **Finish reference extraction** or remove `extract-papers` until it works:
   implement the PDF backends and `extract_references_from_pdf`
   (`src/paper_extractor.rs:374-382`). *Affects Ch. 9.*

8. **Index S2AG lookups.** Replace the $O(N)$ fuzzy scan
   (`src/s2ag_citations.rs:118-148`) with a precomputed index and tighten the
   substring rule to reduce false matches. *Affects Ch. 6.*

9. **Add tests and benchmarks.** The site generator has tests
    (`tests/site_test.rs` and the unit tests in `src/site/mod.rs`), but there is
    no observed coverage for the parser, ranking, or matching logic, and
    `test_get_venue_field` is stale (§11.3.4). At minimum: fix that assertion,
    golden tests for `normalize_author_name`/`normalize_venue_id`, a small
    fixture graph that exercises the live PageRank path once §P0.1 lands, and a
    benchmark for the build-time citation lookup under the fuzzy-match load.
    *Affects all of Part III and IV.*

10. **Prune dead code and unused deps** (§11.3.4): drop the unused `HashSet`
    import, the duplicate `calculate_h_index`, the discarded `_impact`, and the
    unused `petgraph`/`ndarray` dependencies to shrink the LTO release build.
    *Affects Ch. 10 §Build.*

## 11.5 Summary

QIndex parses a substantial bibliographic corpus correctly and publishes a
functional static website and CLI, but its headline algorithm does not yet run as
designed: the citation graph is empty, PageRank is short-circuited to a
paper-count prestige heuristic, and the citation numbers exposed on the website
come from unreconciled sources. The reference year is frozen at 2024, the website
and CLI scoring paths are structurally divergent, and reference extraction is
stubbed. None of these are
architectural dead ends — the roadmap above turns the existing scaffolding into a
working link-analysis ranking, primarily by populating the citation graph
(§P0.1) and unifying the data sources (§P0.3). Until then, readers and evaluators
should treat the venue and scholar rankings as prestige-weighted paper counts,
not citation-based scores.
