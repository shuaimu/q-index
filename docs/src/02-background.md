# 2. Background and Related Work

This chapter surveys the ideas QIndex builds on and, for each, names the design
choice the system actually makes today. Two senses of the word "QIndex" appear
throughout the book: **QIndex (the project)** is this codebase and the website it generates;
**QIndex (the metric)** is the per-scholar score it computes (the `qindex` field
on the `Scholar` struct, `src/models.rs:46-58`). When the distinction matters we
qualify it; elsewhere context disambiguates.

A recurring theme is the gap between the *intended* design and the *implemented*
behavior. QIndex is built around link-analysis over a citation graph, but the
graph is empty at runtime. We therefore describe each technique twice: what it
contributes in principle, and what the code does in its place. Where a feature is
a fallback, a stub, or aspirational, this chapter says so plainly rather than
restating the project README's claims.

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

where $n$ is the number of nodes and $w$ are edge weights. The
$\frac{1-d}{n}$ term is the teleport (random-restart) probability that keeps the
iteration well-defined even on dangling nodes. PageRank is computed by power
iteration: initialise every node to $1/n$, apply the update repeatedly, and stop
when the maximum per-node change falls below a tolerance.

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
| `year_decay`      | 0.95    | per-year discount on edges/papers     |

However, **this power-iteration path is dead code at runtime.** The venue graph
is built from `Paper.citations`, and the BibTeX parser never populates that field
— every paper is created with `citations: Vec::new()` (`src/parser.rs:166-167`),
and no BibTeX tag maps to it in `parse_entry` (`src/parser.rs:151-219`).
`build_citation_network` (`src/models.rs:174-192`) derives `cited_by` by
traversing `citations`, so it too stays empty. The only on-disk cache the read
commands try to load is the singular file `./cache/citations.json`, which does not
exist (the repo has a `cache/citations/` *directory* with differently named
files). With no edges and no cache, `has_cached_citations` is false and the venue
graph is empty.

What actually computes venue scores is the **prestige fallback**
(`src/algorithm.rs:334-367`), confirmed in source above. For each venue it
assigns a base score $\ln(\text{papers}+1)/10$, doubles it for CSRankings venues,
adds a small deterministic perturbation from a hash of the venue name (to break
ties), and normalises so the scores sum to 1:

$$
\mathrm{score}(v) = \frac{\ln(\text{papers}_v + 1)}{10}
                    \cdot \big(\mathrm{csrankings}(v)\,?\,2:1\big)
                    + \frac{\mathrm{hash}(name_v) \bmod 100}{10000}.
$$

This is a paper-count heuristic dressed in PageRank's clothing: it never consults
the citation structure. A `tier_bonus` multiplier
(`A*`=2.0, `A`=1.5, `B`=1.2, `C`=1.0) is then applied and the scores
re-normalised (`apply_tier_bonus`, `src/algorithm.rs:380-403`). Chapter 5, *The
Ranking Algorithm*, dissects the full pipeline, including a control-flow brace
quirk in the dead PageRank branch.

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

QIndex computes an h-index per scholar (`calculate_scholar_h_index`,
`src/algorithm.rs:486-512`): it gathers `cited_by.len()` for the scholar's
papers, **restricted to CSRankings venues**, sorts descending, and finds the
largest $h$ with `citations[h-1] >= h`. A second, unused free function
`calculate_h_index` (`src/algorithm.rs:631-651`) computes the unfiltered version.
Because `cited_by` is empty for every paper (Section 2.1), **the algorithm's
h-index is 0 for every scholar**, and its `citation_count` is likewise 0. The
h-index machinery is correct but starved of input; it will produce meaningful
values only once a real citation graph is loaded (Chapter 6). The website does
not show those zeros: it recomputes each scholar's citation total and h-index
from the per-paper Semantic Scholar counts (`Citations::h_index` in
`src/site/mod.rs`), so the h-indices displayed on the site are real, while the
ranking itself still sees none of it.

## 2.3 CSRankings Methodology: What QIndex Borrows and Changes

[CSRankings](https://csrankings.org) ranks institutions by counting publications
in a curated list of top venues, with credit split among authors by position on
the paper (fractional/position-based attribution rather than whole counting). Its
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
  each paper by its venue's prestige score (`venue_score`, intended to be the
  PageRank from Section 2.1, in practice the prestige fallback). A paper in a
  higher-scored venue contributes more.

- **Year decay.** CSRankings uses fixed publication windows; QIndex applies a
  continuous discount to older papers. The scholar score multiplies each paper by
  $0.95^{(2024 - \text{year})/5}$ — note the exponent is divided by 5, so this is
  *not* a plain per-year decay, and `current_year` is **hardcoded to 2024**
  (`src/algorithm.rs:426-430`), so 2025+ papers receive no decay.

The full QIndex (the metric) for a scholar is, per `calculate_scholar_scores`
(`src/algorithm.rs:405-476`, read directly above):

$$
\mathrm{raw}(s) = \!\!\sum_{\substack{v \in \text{CSRankings} \\ p \in s}}\!\!
  \mathrm{venue\_score}(v)\cdot \mathrm{decay}(p)\cdot \mathrm{pos}(p,s),
\qquad
\mathrm{QIndex}(s) = \frac{\mathrm{raw}(s)\cdot \ln(N_s+1)}{\max_{s'}(\cdots)}\cdot 100,
$$

where $N_s = \texttt{scholar.papers.len()}$ is the scholar's *total* paper count
across all venues — an inconsistency, since the summation itself includes only
CSRankings venues. Final scores are normalised to a 0–100 scale. A missing
venue score defaults to 0.01. Chapter 5 covers the formula in full.

## 2.4 Citation Data Sources Compared

QIndex's central dependency is per-paper citation counts and citation edges.
Several public sources supply these, with different trade-offs. The system has
scripts for most of them; only one feeds the published website today.

| Source | What it provides | How QIndex uses it |
| ------ | ---------------- | ------------------ |
| **DBLP** | Clean conference/journal proceedings metadata (titles, authors, venues, years) via a search API. No citation counts. | Source of the `bib/` corpus. `scripts/fetch_main_conference.py` queries the DBLP `publ` API and filters out workshops/posters. |
| **Semantic Scholar / S2AG** | Bulk academic-graph corpus: paper records, citation counts, and citation edges. | The only live citation source. `s2ag_paper_citations.json` (16,462 matched papers) supplies every citation figure on the website. |
| **OpenAlex** | Open scholarly index with citation counts and IDs. | `scripts/fetch_citations_combined.py` populates `combined_cache.json`; no longer read by any Rust code (it fed the removed server's citation-status endpoint), and never fed rankings. |
| **Crossref** | DOI registration metadata and reference lists. | Used by the legacy fetcher and the (stubbed) paper extractor for DOI lookups. |

### DBLP: the metadata backbone

DBLP supplies the bibliographic records but **no citations**. The `bib/` folder
holds roughly 21,000 BibTeX entries across about 44 venue files (largest: `ccs`
≈ 2,789, `stoc` ≈ 1,923, `sigcomm` ≈ 1,687). The parser produces
roughly 19,954 papers, 44 venues, and 43,942 scholars (observed via
`qindex stats` and the site's `data/stats.json`). DBLP is the right tool for clean proceedings metadata and the
wrong tool for impact: that gap is exactly why a second source is needed.

### Semantic Scholar / S2AG: the live citation feed

S2AG (the Semantic Scholar Academic Graph) is the only citation source the
site build consults. An offline matcher, `scripts/match_s2ag.py`, pairs each
parsed paper with its Semantic Scholar record — by DOI, or by normalized title and
year against the downloaded S2AG `papers` files — and fetches current counts from
the Semantic Scholar Graph API. It writes `cache/citations/s2ag_paper_citations.json`,
keyed by our paper ids: 16,462 of 19,954 papers are matched (97 percent of those
with a DOI, about half of the DOI-less USENIX and ML papers). The loader
`src/s2ag_citations.rs` reads it into a map, and the site generator looks each
paper up by id (`Citations::of` in `src/site/mod.rs`), rendering
`Citations: N (S2AG)` on venue pages and scholar profiles, or "n/a" when a paper
was not matched. Scholar and venue totals, scholar h-indices, and the dataset total
are all derived from these counts.

Crucially, S2AG feeds **per-paper counts, not the ranking graph.** The companion
`s2ag_citation_graph.json` has 190 nodes but effectively one cites edge and one
cited_by edge total — graph extraction never succeeded (neither the superseded
`scripts/parse_s2ag.py` nor `match_s2ag.py` builds a graph). So even the live
source contributes display numbers, not the venue-to-venue edges PageRank needs.
The partial S2AG bulk download (32 of 60 `papers` files, 122 of 236 `citations`
files, about 139 GB) lives under `data/` and is gitignored; only the derived
`cache/citations/*.json` artifacts are committed. The API key is read
from the `S2_API_KEY` environment variable (`.env.example`); no real `.env` is
committed.

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

The website's citation figures are now consistent with one another: the
dashboard, the statistics page, `data/stats.json`, venue and scholar pages all
derive from the same per-paper S2AG counts. What still does not agree is the
website and the ranking algorithm. The algorithm sees no citations at all (its
graph is empty), so a scholar's displayed h-index and citation total have no
influence on their QIndex. Turning the S2AG data into a citation graph that
actually drives PageRank is the core open problem; see Chapter 6, *Citation Data
Integration*, and Chapter 11, *Limitations, Known Issues, and Roadmap*.

## 2.5 Summary

QIndex is designed as a PageRank-over-venues prestige model fused with a
CSRankings-style venue allowlist and position-weighted, year-decayed authorship
credit. In its current state the design's link-analysis core does not run:
absent a populated citation graph, venue scores come from a paper-count prestige
fallback, and the algorithm's own h-indices and internal citation counts are
uniformly 0 (the website displays S2AG-based ones instead). The
techniques surveyed here — eigenvector centrality, the h-index, CSRankings
methodology, and the DBLP/S2AG/OpenAlex/Crossref data landscape — define both the
ambition and the honest current limits of the system. The remaining chapters
trace these from data model (Chapter 3) through algorithm (Chapter 5) and
integration (Chapter 6) to the website and CLI surfaces.
