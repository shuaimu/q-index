# 2. Background and Related Work

This chapter surveys the ideas QIndex builds on and, for each, names the design
choice the system actually makes today. Two senses of the word "QIndex" appear
throughout the book: **QIndex (the project)** is this codebase and the website it generates;
**QIndex (the metric)** is the per-scholar score it computes (the `qindex` field
on the `Scholar` struct, `src/models.rs:46-58`). When the distinction matters we
qualify it; elsewhere context disambiguates.

The implemented link-analysis path loads a real directed graph from the local
S2AG snapshot before ranking. It is a bibliography-only graph, not the full
academic graph; counts and graph edges measure different scopes. Fallbacks and
unfinished PDF extraction remain separate from this working offline path.

## 2.1 Eigenvector Centrality and PageRank

### The idea

Counting citations treats every citation as worth the same. Eigenvector
centrality refines this: a node is important if it is pointed to by other
important nodes. Formally, for an adjacency relation over nodes, the centrality
vector $x$ satisfies $x = \frac{1}{\lambda} A^\top x$ for the principal
eigenvalue $\lambda$ — a node's score is proportional to the sum of its
in-neighbors' scores.

PageRank is the variant that made this practical on the web graph. With a
*damping factor* $d$ (the probability a random surfer follows an edge rather than
teleporting), the score of node $t$ is

$$
\mathrm{PR}(t) = \frac{1-d}{n} + d \sum_{f \to t}
                 \mathrm{PR}(f) \cdot \frac{w(f,t)}{\sum_{e} w(f,e)},
$$

Here $n$ is the number of nodes and $w$ are edge weights. For dangling sources
add $dD/n$, where $D$ is their total mass. The teleport term alone does not
redistribute dangling mass. Power iteration starts at $1/n$ and stops when the
maximum change is below tolerance or the iteration cap is reached.

### What "PageRank over an academic graph" means

QIndex applies this not to web pages but to a bibliographic graph. The intended
nodes are **venues** (conferences and journals), and an edge from venue *A* to
venue *B* means a paper published in *A* cites a paper in *B*. The result is a
*prestige* score per venue that captures how the citing community values it,
rather than a raw citation tally. This venue prestige then feeds the per-scholar
QIndex (Section 2.3). Running PageRank over venues rather than papers is a
deliberate aggregation: it smooths sparse per-paper citation data and yields a
ranking comparable to venue-level reputation lists.

### What the implementation does today

The `PageRankCalculator` (`src/algorithm.rs`) implements this update faithfully:
`calculate_venue_pagerank` (`src/algorithm.rs:255-378`) seeds each venue to
$1/n$, applies the damping update with per-node weight normalisation, distributes
dangling-node mass uniformly, and breaks when `max_diff < tolerance`. The
defaults come from `AlgorithmParams::default()` (`src/models.rs:108-125`):

| Parameter         | Default | Role                                  |
| ----------------- | ------- | ------------------------------------- |
| `damping_factor`  | 0.85    | PageRank $d$                          |
| `max_iterations`  | 100     | power-iteration cap                   |
| `tolerance`       | 1e-6    | convergence threshold on `max_diff`   |
| `venue_weight`    | 0.7     | (declared; see Chapter 5)             |
| `year_decay`      | 0.95    | edge-age discount; scholar exponent is age/5 |
| `reference_year` | UTC year by default | valid loaded snapshot release year takes precedence |

The default CLI and site load the local S2AG count and graph artifacts described
in Chapter 6. The parser alone supplies no references; the snapshot integration
populates the internal graph before scoring. Directed cross-venue citations feed
PageRank once each, with year-decayed edge weights. Dangling venues distribute
their mass uniformly. Published Venue Score is normalized, unadjusted PageRank,
without manual prestige, name or eligibility boosts. With no usable
cross-venue citations, every venue receives 1/N.

## 2.2 The h-index and Its Limitations

The **h-index** summarises a scholar's output: it is the largest $h$ such that
$h$ of their papers each have at least $h$ citations. It rewards sustained,
broadly cited work over a single hit or a long tail of uncited papers. Its known
weaknesses motivate QIndex's broader metric:

- It is monotone non-decreasing and grows with career length, so it conflates
  productivity with seniority.
- It ignores venue quality: a paper in a top conference and a paper in an obscure
  one count identically.
- It is insensitive to citations above the threshold (a 10-citation and a
  10,000-citation paper contribute the same to an h of 10).
- It depends entirely on a reliable citation count per paper.

An internal-graph h-index measures citations from the bibliography only; it is
not interchangeable with an h-index based on whole-S2 snapshot citation counts.
The ranking CLI and website use the local snapshot count artifact for reported
citation totals and h-indices, deduplicating multiple BibTeX aliases for one S2
corpus id. QIndex remains venue-weighted publication credit rather than a direct
function of those citation totals.

## 2.3 CSRankings Methodology: What QIndex Borrows and Changes

[CSRankings](https://csrankings.org) ranks institutions by counting publications
in a curated list of top venues, with uniform fractional coauthor credit rather
than QIndex's first/last-author position heuristic. Its
deliberate design choices are: a hand-picked venue allowlist (to resist
gaming via low-quality venues) and per-author fractional credit (so a 20-author
paper does not inflate everyone equally).

### What QIndex borrows

1. **A venue allowlist.** `is_csrankings_venue` (`src/algorithm.rs:15-60`)
   uppercases the venue name, strips a trailing four-digit year, and matches
   against a hardcoded list of roughly 85 venue tokens spanning the standard
   CSRankings subareas (e.g. `OSDI`, `SOSP`, `SIGMOD`, `NEURIPS`, `STOC`,
   `CHI`). The QIndex computation *skips any venue not on this list*
   (`src/algorithm.rs:410-416`), mirroring CSRankings' filtering philosophy.
   A parallel field taxonomy lives in `get_venue_field` (`src/models.rs:391-468`),
   which maps the same subareas (Operating Systems, Databases, Machine Learning &
   Data Mining, and so on) for browsing and charts.

2. **Author-position weighting.** Unlike CSRankings' uniform fractional split,
   QIndex weights by *role* (`src/algorithm.rs:434-450`). When a paper has more
   than one author, the contribution to a scholar's score is scaled by position:

   | Position           | Multiplier            |
   | ------------------ | --------------------- |
   | First author       | 1.0                   |
   | Last author        | 0.8                   |
   | Middle author      | 0.6 / (author_count − 2) |
   | Single author      | (no factor applied)   |

   This encodes the convention that first and last authors carry primary credit.

### Where QIndex differs

QIndex departs from CSRankings on two axes:

- **PageRank/prestige weighting instead of equal venue weight.** CSRankings
  treats all listed venues alike (a publication is a publication). QIndex weights
  each paper by its venue's prestige score (`venue_score`, derived from the
  internal citation graph, with a fallback only when usable edges are absent). A paper in a
  higher-scored venue contributes more.

- **Year decay.** QIndex applies $0.95^{\max(Y_{\mathrm{ref}}-y_p,0)/5}$.
  `AlgorithmParams.reference_year` defaults to the current UTC year and becomes
  the valid snapshot release year in `PageRankCalculator::with_citations`.
  Future and unknown-year papers remain undecayed.

The full QIndex metric is implemented by `calculate_scholar_scores` in
`src/algorithm.rs`:

$$
\mathrm{raw}(s) = \!\!\sum_{\substack{v \in \text{CSRankings} \\ p \in s}}\!\!
  \mathrm{venue\_score}(v)\cdot \mathrm{decay}(p)\cdot \mathrm{pos}(p,s),
\qquad
\mathrm{QIndex}(s) = \frac{\mathrm{raw}(s)\cdot \ln(N_s+1)}{\max_{s'}(\cdots)}\cdot 100,
$$

Here $N_s$ counts the same unique eligible authored publications used in the
sum. Corpus-id aliases share one representative; unmatched works deduplicate
only by local id. Missing venue scores contribute zero. All-zero inputs keep
all QIndices zero; otherwise scores normalize to 0–100 (Chapter 5).

## 2.4 Citation Data Sources Compared

QIndex's central dependency is per-paper citation counts and citation edges.
Several public sources supply these, with different trade-offs. The system has
scripts for most of them; only one feeds the published website today.

| Source | What it provides | How QIndex uses it |
| ------ | ---------------- | ------------------ |
| **DBLP** | Clean conference/journal proceedings metadata (titles, authors, venues, years) via a search API. No citation counts. | Source of the `bib/` corpus. `scripts/fetch_main_conference.py` queries the DBLP `publ` API and filters out workshops/posters. |
| **Semantic Scholar / S2AG** | Bulk academic-graph corpus: paper records, citation counts, and citation edges. | Local release 2026-10-06 supplies snapshot-wide counts and the bibliography-only ranking graph through two committed artifacts. |
| **OpenAlex** | Open scholarly index with citation counts and IDs. | `scripts/fetch_citations_combined.py` populates `combined_cache.json`; no longer read by any Rust code (it fed the removed server's citation-status endpoint), and never fed rankings. |
| **Crossref** | DOI registration metadata and reference lists. | Used by the legacy fetcher and the (stubbed) paper extractor for DOI lookups. |

### DBLP: the metadata backbone

DBLP supplies bibliographic metadata but **no citations**. Corpus size changes
with cleaning and additions; use the current parser export and generated site
statistics rather than older chapter figures. DBLP's metadata role is distinct
from the snapshot citation evidence.

### Semantic Scholar / S2AG: local snapshot inputs

The complete release `2026-10-06` is stored under
`data/s2ag/2026-10-06/`: 60 paper shards and 395 citation shards. The local-only
`scripts/match_s2ag.py` pipeline matches DOI or normalized title/year, resolves
counts from paper records, and extracts edges whose endpoints both match our
bibliography. It writes `cache/citations/s2ag_paper_citations.json` and
`cache/citations/s2ag_citation_graph.json`; both are committed for CLI and CI site
builds. Coverage and edge totals are recorded by the generated artifacts rather
than fixed in this chapter.

Counts include citations from all of S2; the internal graph includes only matched
bibliography papers. The graph, not a count-derived synthetic aggregate node,
feeds venue PageRank. `release_id` identifies the snapshot date; `generated`
identifies processing time, not a live count refresh. Extraction and site builds
need no API key or network requests. `S2_API_KEY` is needed only to download
datasets; raw shards and work files remain gitignored.

### OpenAlex and Crossref: the secondary pipelines

`combined_cache.json` (599 papers, from a DBLP + OpenAlex pipeline), with
`paper_cache.json` (103 papers) as its fallback, used to back the old web
server's citation-status endpoint. That endpoint was removed with the server, so
no Rust code reads either file today. A legacy Semantic
Scholar/Crossref fetcher (`src/citations.rs`) persists to a `citations.json` that
does not exist on disk and is not invoked at render time — it is effectively
dead. Crossref also underpins the largely stubbed reference extractor
(`src/paper_extractor.rs`), covered in Chapter 9.

### Display numbers versus ranking input

The CLI and website use the same count artifact and graph release. Whole-S2
counts supply reported totals and h-indices; bibliography-only directed edges
drive venue PageRank. The published `pagerank` field is normalized, unadjusted
PageRank (**Venue Score**). Paper citation counts are not an additional direct
QIndex term. The `impact_factor` field is venue score times
$\ln(\text{unique venue works}+1)$, labelled
**Count-Scaled Score**, not a journal impact factor.

## 2.5 Summary

QIndex combines normalized, unadjusted citation-derived venue centrality with
a separate CSRankings-style scholar allowlist, position credit and gentle recency
decay. Scholar credit and productivity scaling share one unique eligible
publication set. Without usable cross-venue edges, venue PageRank is uniform
1/N. Neither eligibility nor the byline heuristic is an objective fairness
guarantee. Name identity,
partial bibliography coverage and snapshot matching constrain interpretation.
Chapters 3–6 explain the model, algorithm and local integration in detail.
