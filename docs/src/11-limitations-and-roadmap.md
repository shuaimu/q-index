# 11. Limitations, Known Issues, and Roadmap

This chapter is an honest accounting of what QIndex does *today* versus what its
structure suggests it is *meant* to do. Throughout, "QIndex" refers to the
project as a whole; "QIndex" (the metric) and the `qindex` field on the
`Scholar` struct refer to the per-scholar score produced by the ranking
algorithm. Where a feature is a fallback, a stub, or aspirational, this chapter
says so plainly and describes what actually happens when the code runs. Every
claim is tied to a source path or line and, where relevant, to the sibling
chapter that owns the topic in full.

The static frontend loads Bootstrap and Chart.js from CDNs (see `base_template`
in `src/site/templates.rs`). A WASM target is not implemented; the README lists it
only as a prospective enhancement. Ranking examples must come from generated
data rather than invented scholar/venue measurements.

The default CLI and website now use local snapshot counts and a real internal
citation graph. The graph is restricted to matched bibliography papers; this
sampling boundary, matching uncertainty and scholar-ranking policy remain
important limitations. Current coverage and edge totals are in generated outputs.

## 11.1 Algorithmic limitations

### 11.1.1 A bibliography-only graph is not the whole academic graph

The complete S2AG release `2026-10-06` supplies directed citation records, but the
ranking graph retains only edges whose endpoints both match our bibliography.
External citations contribute to snapshot paper counts, not additional venue
nodes or edges. Consequently PageRank measures prestige within this selected
corpus, and coverage changes can change scores even when S2 records are stable.

The loader canonicalizes duplicate BibTeX aliases by corpus id. Venue aggregation
counts each directed edge once and retains only cross-venue citations with the
existing year-decay policy. Dangling venues distribute mass uniformly. Venue
Score is normalized, unadjusted PageRank of this bibliography-only graph,
without manual prestige, name or eligibility boosts. Without usable cross-venue
edges, each venue receives 1/N. CSRankings scholar eligibility remains separate.

Reported counts and h-indices use whole-S2 snapshot counts, not internal
in-degree. QIndex is still venue-weighted publication credit; a citation total
is not an additional direct term in its formula.

### 11.1.2 Chronology and recency are policy choices

`AlgorithmParams.reference_year` defaults to the current UTC year.
`PageRankCalculator::with_citations` replaces it with the year of a valid
snapshot release date. Scholar decay is
$0.95^{\max(Y_{\mathrm{ref}}-y_p,0)/5}$; saturating subtraction avoids future-year
underflow, and future/unknown-year works are undecayed. Citation edge decay uses
$0.95^{\max(y_{\mathrm{citing}}-y_{\mathrm{cited}},0)}$, not absolute year distance.
The gentle scholar exponent remains a deliberate heuristic, not evidence of
individual contribution or a guarantee of career-stage fairness.

### 11.1.3 Field classification and byline assumptions

Automatic venue field classification uses substring matching and can misclassify
names. It is separate from the CSRankings scholar-eligibility allowlist and does
not adjust scores. The published `pagerank` field is normalized, unadjusted
PageRank (**Venue Score**). `impact_factor` is that score times
$\ln(\text{unique venue works}+1)$, a **Count-Scaled Score**, not a journal impact factor.

First/last-author weights assume roles that do not hold in every field
(especially alphabetical bylines). The ranking sum and its logarithmic $N$
now use the same unique eligible authored works, so excluded output cannot
boost productivity scaling. This consistency does not establish objective
fairness or make relative scores comparable across different corpora.

### 11.1.4 Input overrides must preserve provenance

Default CLI and site scoring use the same count and graph paths. Site overrides
`--citations-file` and `--citation-graph-file` must identify one release; mixing
releases is rejected when the graph loads. Paths remain relative to the working
directory, so deployments should run from the repository root or explicitly
relocate both inputs. Rebuilding the website alone cannot refresh an older
snapshot or include newly added bibliography papers in the derived graph.

## 11.2 Data limitations

### 11.2.1 Matching coverage and snapshot age

DOI or normalized title/year matching against complete local paper shards can
still miss records or select ambiguous versions. No search/API fallback is used.
Unmatched papers are unknown, not verified zero-citation papers, and can
understate aggregate counts. Use generated `matched`, `total` and `by_method`
fields for current coverage, not figures from the superseded partial download.

`release_id` dates the snapshot; `generated` dates processing. Rerunning extraction
against release `2026-10-06` does not make its counts current. New bibliography
entries require export and all three local stages to refresh both derived inputs.

### 11.2.2 Counts and graph answer different questions

Counts include external S2 citations while the ranking graph includes only
bibliography endpoints. A scholar's total and h-index therefore need not track
QIndex ordering. That is a deliberate measurement distinction, not a disconnected
ingestion path: CLI and site use the same two local artifacts, whose releases
must agree. Legacy fetcher outputs do not supply default rankings.

### 11.2.3 Duplicate BibTeX entries and matching residue

Matched aliases share a publication identity based on S2 corpus id; unmatched
works deduplicate only by local id. The representative prefers venue, DOI,
year and author completeness, then smallest local id. Ranking credit requires
that representative to contain the scholar and resolve to an eligible venue.
Scoring, eligible publication counts, h-indices, top venues, graph endpoints
and publication cards therefore do not inflate from matched aliases.
Different-key unmatched duplicates and conflicting metadata still need audit.
Profiles distinguish `all_indexed` from eligible `ranking` metrics.

Matching uncertainty remains: title/year heuristics cannot prove work identity,
and garbled S2 titles can leave DOI-less papers unmatched. Author identity is
still name-based; shared names can merge people and name variants can split one
person. S2 author metadata may be incomplete or wrong, and is not grounds for
automatic author-ID merging or fabricated affiliations. Two publisher-confirmed
wrong-author aliases were removed during the 2026-10-10 OSDI/SOSP cleanup;
other questionable records require evidence, not a general policy redesign.

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
but shows unavailable citation counts (Chapter 10). The computed ranking sorts
now use `total_cmp` with id tie-breaks; low-level metadata search helpers in
`CitationGraph` still use `partial_cmp().unwrap()` and initial model scores.
Do not confuse those helpers with the computed ranking/search surface.

### 11.3.4 Dead code, warnings, and unused dependencies

- `AlgorithmParams.venue_weight` is retained but unused by the ranking formula.
- `Scholar.affiliations` is declared but not populated by the parser.
- The legacy fetcher in `src/citations.rs` is not the offline ranking input.
- `scripts/parse_s2ag.py` and its old outputs are superseded by the local-only
  `scripts/match_s2ag.py` path.

Warnings and dependency hygiene should be assessed against current source and
clippy evidence, not the old server-era inventory. `lazy_static`, for example,
is used by parser accent cleaning. Source behavior tests now cover parser
metadata/index rebuilding, eligible/logarithmic counting, safe chronology,
alias invariance, author-position credit, directed/dangling PageRank and
deterministic ranking. `tests/s2ag_graph_test.rs` covers provenance and canonical
aliases; `tests/test_match_s2ag.py` covers candidate selection and stale/partial
local snapshot inputs. The earlier lack of parser/ranking behavior tests is
no longer a current limitation.

## 11.4 Prioritized roadmap

The items below are ordered by impact-to-effort. Each names the chapter/section it
affects so a contributor can find the surrounding design.

### P0 — Improve evidence quality without changing ranking policy

1. **Audit ambiguous matches and corpus coverage.** Use generated match summaries
   and candidate metadata to examine false positives and omissions. Keep both
   derived artifacts tied to one complete release and rebuild them together.
   Extraction remains local-only; online title search is not required.

2. **Assess bibliography selection bias.** Compare coverage across venues and
   explain the effect of restricting citation endpoints to the corpus. Adding
   papers requires regenerating count matches and graph edges, not manufacturing
   aggregate-cache links.

### P1 — Evidence audits and deployment safety

3. **Audit author and work metadata.** Compare disputed records with publisher
   bylines and preserve evidence for corrections. Review unresolved name
   collisions, unmatched aliases and conflicting matched representatives.
   Better input evidence matters even with score deduplication.

4. **Make missing citation inputs an explicit deployment decision.** Counts and
   graph loaders allow bibliography-only operation with warnings. Decide whether
   publication should require both artifacts, without removing intentional
   offline bibliography-only use. Provenance mismatches already fail.
   *Affects Ch. 6, Ch. 10.*

5. **Measure ranking sensitivity.** Quantify how bibliography selection, unmatched
   works, eligibility restrictions and byline conventions affect results. Disclose
   differences between all-indexed profile metrics and eligible ranking metrics.
   Any future policy change needs explicit justification.

### P2 — Feature completion and maintainability

6. **Finish reference extraction or remove the unusable command.** PDF backends
   and `extract_references_from_pdf` remain incomplete. This is independent of
   working S2AG graph extraction. *Affects Ch. 9.*

7. **Extend behavior evidence where needed.** Existing tests exercise meaningful
   parser, ranking and matching behavior. Add representative real-world fixtures,
   metadata conflict audits and benchmarks for full-corpus extraction; do not
   replace behavior coverage with tests that pin wording, defaults or source text.

8. **Review remaining metadata helpers and legacy paths.** Low-level graph search
   helpers use initial score fields and unordered venue selection. If adding
   callers, use computed ranking surfaces or explicitly define metadata-only
   semantics. Audit dependencies and clippy findings against actual callsites
   before pruning; already removed dead computations are not roadmap items.

## 11.5 Summary

QIndex publishes a static website and CLI using two committed local snapshot
artifacts. Real directed bibliography citations feed venue PageRank and scholar
QIndex; whole-S2 counts feed reported totals and h-indices. Graph restrictions,
matching uncertainty, snapshot age and ranking policy still constrain
interpretation. PDF reference extraction remains stubbed and independent of the
working local S2AG path. Improvements should address those boundaries without
confusing whole-S2 counts with internal graph structure.
