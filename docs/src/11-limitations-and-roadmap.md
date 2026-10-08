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

A useful one-line summary of the current state: a build parses roughly 19,954
papers, 44 venues, and 43,942 scholars, with **zero internal citation edges**.
That zero is the root cause of most of the algorithmic limitations below. (The
website's citation figures do not show it: they come from per-paper Semantic
Scholar counts matched offline, which the ranking never sees.)

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

Consequently the algorithm's `h_index` and `citation_count` are `0` for every
scholar (the h-index loop reads `cited_by.len()`, which is always zero;
`src/algorithm.rs:486-512`), and venue `impact_factor` reduces to
`pagerank * ln(paper_count + 1)`. The website replaces the two scholar figures
with ones derived from Semantic Scholar counts (`Citations` in
`src/site/mod.rs`), so the h-indices it shows are real — but they are display
values only and do not influence any ranking.

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

### 11.2.1 Coverage: 82.5% of papers have S2AG citation counts

The S2AG integration (Chapter 6, *Citation Data Integration*) matches
**16,462 of 19,954** papers to Semantic Scholar and stores their counts in
`cache/citations/s2ag_paper_citations.json`, keyed by paper id. Coverage is
uneven: 97 percent of papers with a DOI are matched, but only 48 percent of the
6,018 without one. Those are concentrated in OSDI, NSDI, USENIX ATC, FAST,
NeurIPS, and ICML, whose BibTeX carries no DOIs, so they can only be matched by
title against the downloaded S2AG `papers` files — and only 32 of that release's
60 files were downloaded (one of them truncated). Unmatched papers show
"Citations: n/a" and contribute nothing to scholar and venue totals or
h-indices, so scholars who publish mainly at the DOI-less venues are
under-counted relative to the rest. Closing the gap needs an S2 API key (the
datasets API and the title-match endpoint require one; Chapter 6, §6.4).

The counts are a snapshot (the file's `generated` date) and go stale until the
pipeline is rerun; papers added to `bib/` show "n/a" until then.

### 11.2.2 Displayed citations and the ranking disagree

Every citation figure on the website — per paper, per scholar, per venue, the
dashboard and `data/stats.json` totals — is derived from the same per-paper S2AG
counts, so the site is internally consistent. The ranking is not part of that:
`PageRankCalculator` still sees an empty citation graph, so a scholar's displayed
citation total and h-index have no effect on their QIndex, and the "Top Scholars"
table is ordered by a score that ignores the citation numbers printed next to it.
The S2AG citation *graph* artifact `cache/citations/s2ag_citation_graph.json` has
190 nodes but only one `cites` edge and one `cited_by` edge total — it is an
orphan written by no current script and read by no code. The CLAUDE.md claim of
"285 citation relationships" is not reflected in any present artifact.

Older pipelines add noise around this: the superseded `parse_s2ag.py` outputs
(`s2ag_citations.json` and friends) and the DBLP+OpenAlex `combined_cache.json`
(599 papers) are still committed but read by no Rust code, and the CLI
subcommands print the algorithm's zero h-indices and citation counts rather than
the website's S2AG-based ones.

### 11.2.3 Duplicate BibTeX entries and matching residue

Some papers appear in `bib/` more than once under different keys (Ion Stoica's
114 papers include the vLLM/PagedAttention paper three times). Citation totals and
h-indices count each Semantic Scholar corpus id once (`distinct_counts` in
`src/site/mod.rs`), but paper listings, paper counts, and the "Most Cited Papers"
table still show every duplicate, and an unmatched duplicate cannot be recognized
at all. Matching itself is conservative — DOI equality, or a normalized title of
at least three words with the year within ±1 — and spot checks found no false
matches, but S2 occasionally stores a garbled title for a USENIX paper
("This paper is included in the Proceedings of …"), and a DOI-less paper whose S2
record carries a different title will stay unmatched.

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
First, the S2AG counts load from a CWD-relative default path
(`--citations-file`, default `cache/citations/s2ag_paper_citations.json`), and a
missing file only logs a warning, so a build run from the wrong directory succeeds
but shows "Citations: n/a" everywhere (Chapter 10). Second, the ranking code
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
  the actix stack was removed, but `petgraph`, `ndarray`, `rayon`, and
  `lazy_static` still have no call sites and still inflate the LTO release build
  (Chapter 10). `rayon` lost its only call site when the fuzzy S2AG lookup was
  replaced by exact lookups.
- `scripts/parse_s2ag.py` and its outputs (`s2ag_citations.json`,
  `s2ag_id_mapping.json`, `s2ag_summary.json`) are superseded by
  `scripts/match_s2ag.py` and read by nothing (Chapter 6). (The server-era
  `exportData()` helper and dead `/api/docs` footer link were removed with the
  server.)

## 11.4 Prioritized roadmap

The items below are ordered by impact-to-effort. Each names the chapter/section it
affects so a contributor can find the surrounding design.

### P0 — Make the rankings real and consistent

1. **Populate a paper-level citation graph and feed PageRank.** Extract
   paper-to-paper edges from the S2AG `citations` release — the corpus ids in
   `s2ag_paper_citations.json` already map S2 papers to ours — into
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

3. **Close the S2AG coverage gap.** With an S2 API key, download the complete
   `papers` release or run `match_s2ag.py resolve --title-match`, so the DOI-less
   USENIX and ML venues (§11.2.1) stop being under-counted. While there, retire
   the superseded `parse_s2ag.py`, the legacy `src/citations.rs`, and the dead
   S2ORC pipeline. *Affects Ch. 6.*

### P1 — Correctness and safety

4. **Parameterize the reference year.** Replace the hardcoded `2024`
   (`src/algorithm.rs:426`) with `chrono::Utc::now().year()` or a CLI/config
   value, and document the $/5$ decay exponent. *Affects Ch. 5 §Recency.*

5. **Fail the build when citation data is missing.** The site build only warns
   when `cache/citations/s2ag_paper_citations.json` is not found from the working
   directory (§11.3.3). Resolving the path
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

8. **Deduplicate the corpus.** Merge duplicate BibTeX entries for the same paper
   (§11.2.3) — the S2 corpus ids already identify most of them — so listings and
   paper counts stop showing duplicates. *Affects Ch. 4, Ch. 6.*

9. **Add tests and benchmarks.** The site generator has tests
    (`tests/site_test.rs` and the unit tests in `src/site/mod.rs`), but there is
    no observed coverage for the parser, ranking, or matching logic, and
    `test_get_venue_field` is stale (§11.3.4). At minimum: fix that assertion,
    golden tests for `normalize_author_name`/`normalize_venue_id`, a small
    fixture graph that exercises the live PageRank path once §P0.1 lands, and
    tests for `scripts/match_s2ag.py`'s title normalization and candidate choice.
    *Affects all of Part III and IV.*

10. **Prune dead code and unused deps** (§11.3.4): drop the unused `HashSet`
    import, the duplicate `calculate_h_index`, the discarded `_impact`, and the
    unused `petgraph`/`ndarray`/`rayon`/`lazy_static` dependencies to shrink the
    LTO release build.
    *Affects Ch. 10 §Build.*

## 11.5 Summary

QIndex parses a substantial bibliographic corpus correctly and publishes a
functional static website and CLI, but its headline algorithm does not yet run as
designed: the citation graph is empty, PageRank is short-circuited to a
paper-count prestige heuristic, and the citation numbers the website shows —
now consistent, S2AG-based, and covering 82.5 percent of papers — have no effect
on the rankings. The reference year is frozen at 2024, the website
and CLI scoring paths are structurally divergent, and reference extraction is
stubbed. None of these are
architectural dead ends — the roadmap above turns the existing scaffolding into a
working link-analysis ranking, primarily by populating the citation graph
(§P0.1) and closing the citation-coverage gap (§P0.3). Until then, readers and evaluators
should treat the venue and scholar rankings as prestige-weighted paper counts,
not citation-based scores.
