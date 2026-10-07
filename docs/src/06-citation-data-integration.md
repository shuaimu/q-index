# 6. Citation Data Integration

The QIndex project ranks venues and scholars; the QIndex metric (the per-scholar
score, disambiguated here from the project name) is computed by the ranking
algorithm described in Chapter 5, *The Ranking Algorithm*. That algorithm wants a
citation graph — who cites whom — and the data model in Chapter 3, *The Data
Model*, reserves space for it: `Paper.citations`, `Paper.cited_by`, and
`CitationGraph.edges`. This chapter is the honest account of how citation data is,
and is not, populated into those fields.

The short version: the BibTeX corpus (Chapter 4, *The Bibliographic Database*)
contains no `cites`/`reference` fields, so the parser leaves every paper's
citation lists empty. Citation *counts* are instead injected from an external
Semantic Scholar dataset (S2AG) when the static website is generated. The
citation *graph* extraction failed, so no edges flow into PageRank. Several
parallel, partly redundant fetch pipelines exist; they disagree on totals because
they are computed over different paper sets. The rest of this chapter substantiates
each of those claims against the source.

## 6.1 Why the graph is empty by default

The parser never writes `Paper.citations`. `parse_entry` (`src/parser.rs:151-219`)
maps BibTeX tags `title`, `author`, `booktitle`/`journal`, `year`, `month`, `doi`,
`url`, `abstract`, and `keywords`; there is no branch that reads a reference or
citation field, and the paper's `citations` vector is initialized empty at
`src/parser.rs:166`. `CitationGraph::build_citation_network`
(`src/models.rs:174-192`) derives `cited_by` by iterating each paper's `citations`
list, so with that list always empty, `cited_by` stays empty too.
`CitationGraph.edges` is initialized empty in `CitationGraph::new`
(`src/models.rs:151-156`) and is never appended to anywhere in `models.rs` or
`parser.rs`.

The runtime consequence is observable in every build: the site's `data/stats.json`
(and the dashboard and statistics pages) report `total_citations = 0` because that
field is computed as `graph.edges.len()` (`stats_json` and `statistics_data` in
`src/site/mod.rs`). The corpus parses to roughly 19,954 papers, 44
venues, and 43,942 scholars, but zero internal citation edges.

Citation information therefore must come from *outside* the BibTeX path. As
documented in `CLAUDE.md` and implemented in `src/s2ag_citations.rs`, the external
source is the Semantic Scholar Academic Graph (S2AG). Crucially, that source feeds
per-paper *counts* into the display layer; it does not feed *edges* into the graph
that PageRank traverses. The two are wired separately, and only one of them works.

## 6.2 The S2AG bulk pipeline

S2AG integration is an offline, three-stage pipeline that ends in a committed JSON
file consumed by the Rust site generator.

```
download_s2ag.py   ->  data/s2ag/*.jsonl.gz   (raw bulk dataset, gitignored)
       |
parse_s2ag.py      ->  cache/citations/s2ag_citations.json   (committed)
                       cache/citations/s2ag_id_mapping.json
                       cache/citations/s2ag_summary.json
       |
src/s2ag_citations.rs  ->  loads s2ag_citations.json at first access (Lazy)
       |
src/site/mod.rs        ->  paper_citations(): get_citation_count(title) once per paper,
                           at build time; shown on venue pages and scholar profiles
```

### 6.2.1 Download

`scripts/download_s2ag.py` fetches the S2AG bulk *datasets* (the `papers` and
`citations` releases) into `data/`. The API key is resolved from the environment:

```python
# scripts/download_s2ag.py:258 (paraphrased)
api_key = args.api_key or os.environ.get("S2_API_KEY")
```

`CLAUDE.md` records that the raw download was approximately 138 GB across the
`papers` (~48 GB) and `citations` (~90 GB) releases. That raw data lives under
`data/` and is gitignored; it is not committed and not required to build the site.
A caveat worth recording for anyone re-running the pipeline: the only S2AG raw
artifacts actually present in the working tree are sample files (`papers-sample.jsonl.gz`,
`citations-sample.jsonl.gz`) plus empty `papers/`/`citations/` subdirectories, so
the 138 GB figure is from `CLAUDE.md`'s history and is not verifiable from the repo
checkout alone.

### 6.2.2 Parse and match

`scripts/parse_s2ag.py` walks the raw S2AG `papers` shards and matches each
QIndex BibTeX paper to an S2AG record by normalized title, with year and DOI as
secondary signals. Its `normalize_title` (`scripts/parse_s2ag.py:84`) strips LaTeX
commands and braces, lowercases, removes punctuation
(`re.sub(r'[^\w\s]', '', t)`), and collapses whitespace — the Python counterpart
to the Rust normalizer described in §6.3.

The parser writes three committed artifacts (and only these three):

- `s2ag_citations.json` — the data the Rust site generator reads.
- `s2ag_id_mapping.json` — normalized title -> S2AG corpus id.
- `s2ag_summary.json` — aggregate statistics.

There is no graph-building code in `parse_s2ag.py`; a grep for `graph` over the
script returns nothing. This is the root cause of the missing edges, addressed in
§6.4.

### 6.2.3 Coverage

`cache/citations/s2ag_summary.json` records the matched coverage:

| Metric | Value |
| --- | --- |
| Papers matched to S2AG | 731 |
| Total citations across matched papers | 41,468 |
| Top cited | "Wait-free synchronization" (toplas, 1991): 1,966 |
| Second | "FlashAttention-2" (iclr, 2024): 1,456 |

Against a corpus of roughly 19,954 papers, 731 matches is about 3.7 percent
coverage. Sample per-venue paper counts from the summary include ccs:122, stoc:67,
icde:61, eurosys:56, sigcomm:54; sample per-venue citation totals include ccs:7183,
sigcomm:4170, stoc:3248, icml:3015. These figures were confirmed against the file
on disk during the writing of this chapter.

## 6.3 The Rust loader: `src/s2ag_citations.rs`

The loader is a small, self-contained module. Its data record,
`S2AGCitationData` (`src/s2ag_citations.rs:9-26`), mirrors the JSON value:

```rust
pub struct S2AGCitationData {
    pub title: String,
    pub year: Option<String>,
    pub venue: String,
    pub corpus_id: Option<u64>,
    pub s2ag_title: Option<String>,
    pub s2ag_year: Option<u32>,
    pub doi: Option<String>,
    pub citation_count: usize,
    pub reference_count: usize,
    pub authors: Vec<String>,
    pub venue_info: Option<String>,
    pub fields: Option<Vec<serde_json::Value>>,
    #[serde(rename = "abstract")]      // 'abstract' is a Rust keyword
    pub abstract_text: Option<String>,
    pub url: Option<String>,
}
```

`S2AGCitationIndex` (`src/s2ag_citations.rs:28-33`) wraps a
`HashMap<String, S2AGCitationData>` keyed by normalized title, plus two
precomputed aggregates, `total_citations` and `papers_with_citations`.
`load_from_file` (`src/s2ag_citations.rs:44`) deserializes the entire JSON object
into that map, sums `citation_count` for the total, and counts entries with a
positive count. If the file is missing it logs a warning and returns an empty index
rather than erroring (`src/s2ag_citations.rs:45-48`).

### 6.3.1 Global state and lifecycle

The index is a process-global lazy singleton:

```rust
// src/s2ag_citations.rs:152-160
pub static S2AG_CITATIONS: Lazy<RwLock<S2AGCitationIndex>> = Lazy::new(|| {
    let path = Path::new("cache/citations/s2ag_citations.json");
    let index = S2AGCitationIndex::load_from_file(path)
        .unwrap_or_else(|e| {
            log::error!("Failed to load S2AG citations: {}", e);
            S2AGCitationIndex::new()
        });
    RwLock::new(index)
});
```

The path is a hardcoded *relative* path. The index loads on first access of
`S2AG_CITATIONS`, which happens lazily when the site build first looks up a
paper's citation count (`paper_citations` in `src/site/mod.rs`). `reload_s2ag_citations` (`src/s2ag_citations.rs:162`) re-reads
the same hardcoded path and swaps the `RwLock` contents in place. The public free
function `get_citation_count` (`src/s2ag_citations.rs:173`) takes the read lock and
delegates to `get_citation_count_for_paper`.

Because the path is relative, S2AG counts only load when `qindex build-site` runs
from the repository root. Run from any other directory, the file is "not found",
the loader silently returns an empty index, and every paper on the generated site
shows the internal fallback (zero) instead of a real count, without failing the
build. Chapter 10, *Building, Running, and Deployment*, lists this among the
build's CWD-relative inputs.

### 6.3.2 Title matching

Lookups are by title, normalized identically on both sides.

```rust
// src/s2ag_citations.rs:108-116
fn normalize_title(&self, title: &str) -> String {
    title.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
```

`get_citation_count_for_paper` (`src/s2ag_citations.rs:71`) first attempts an exact
`HashMap` hit on the normalized title (the map keys are already normalized), then
falls back to a linear scan over every entry, calling `titles_match`
(`src/s2ag_citations.rs:118-148`). The fuzzy rule returns true if either
normalized title is a substring of the other (to absorb subtitles), and otherwise
computes a significant-word overlap: both titles must be at least three words, and
the share of words of length greater than three from one title also present in the
other must exceed 0.7.

$$\text{match}(t_1, t_2) = \big(t_1 \supseteq t_2 \;\lor\; t_2 \supseteq t_1\big) \;\lor\; \frac{|\{w \in t_1 : |w| > 3 \land w \in t_2\}|}{|\{w \in t_1 : |w| > 3\}|} > 0.7$$

Two costs follow. First, performance: on any title that misses the exact map, the
lookup is an O(N) scan of all 731 entries. The removed web server paid this per
paper on every detail-page render; the site generator pays it once per paper per
build (about 20,000 lookups, spread over threads with rayon), which keeps it
affordable at the current scale. Second, correctness: the substring rule can
false-match short or generic titles that share a prefix or suffix, attributing the
wrong citation count to a paper. The generated site shows this is not
hypothetical: in the reference build about 23,900 of the 92,400 paper entries in
the scholar shards (each paper appears once per author) carry an S2AG count — far
more distinct papers than the index's 731 entries could legitimately match. Both
costs bound how far this design extends.

### 6.3.3 Where counts are displayed

The site generator calls into the loader once per paper, at build time:

```rust
// src/site/mod.rs, paper_citations()
let s2ag = crate::s2ag_citations::get_citation_count(&paper.title);
let citations = if s2ag > 0 {
    PaperCitations { count: s2ag, s2ag: true }
} else {
    PaperCitations { count: paper.cited_by.len(), s2ag: false }
};
```

Venue pages render the result in Rust (`paper_item` in `src/site/templates.rs`);
scholar profiles store it in their JSON shard as `citations` plus an `s2ag` flag and
`static/app.js` renders it the same way (Chapter 8). A paper with an S2AG match shows
`Citations: N (S2AG)`; otherwise it shows
`Citations: <paper.cited_by.len()> (internal)`. Since `cited_by` is always empty
(§6.1), the "internal" branch always shows zero. In effect, a paper either has an
S2AG match (real count) or shows zero.

## 6.4 The citation graph extraction failed

The single most important fact in this chapter: S2AG feeds counts, not edges.

`parse_s2ag.py` contains no graph code and never writes a graph file. The file
`cache/citations/s2ag_citation_graph.json` exists but is an orphan, produced by
some other (now absent) process and read by nothing — no Rust module and no script
references it. Its contents confirm the failure. It has 190 nodes keyed by corpus
id, and every node is essentially `{"cites": [], "cited_by": []}`. Inspected during
the writing of this chapter, the entire file contains 2 edges total across 190
nodes (one `cites` entry and one `cited_by` entry). It is not a citation graph in
any usable sense.

The other graph-shaped artifacts are equally empty. `citation_graph.json` is the
two-byte literal `{}`. `citation_data.json` (produced by
`scripts/build_citation_database.py`) reports `metadata.total_citation_links = 0`
and carries an empty `citations` list over 103 papers.

This means the `CitationGraph.edges` field and the entire PageRank
power-iteration path in `src/algorithm.rs` receive nothing. As Chapter 5 details,
the algorithm therefore runs its *prestige fallback* (`src/algorithm.rs:334-367`),
scoring venues from paper counts and a CSRankings flag rather than from real
citation flow. The `CLAUDE.md` note of "285 citation relationships" is not
reflected in any current on-disk artifact and should be treated as stale.

To make the graph real, a future change would need `parse_s2ag.py` (or a new
script) to emit genuine paper-to-paper edges from the S2AG `citations` release,
plus a Rust loader that populates `Paper.citations`/`Paper.cited_by` (or
`CitationGraph.edges`) before `PageRankCalculator::calculate` runs. None of that
exists today.

## 6.5 The competing pipelines

Four pipelines fetch or derive citation data. They were built at different times
and overlap; today only the first feeds the website. The second fed the removed web
server's citation-status endpoint and is now read by no Rust code.

| # | Pipeline | Scripts | Output cache | Consumed by |
| --- | --- | --- | --- | --- |
| 1 | S2AG bulk | `download_s2ag.py`, `parse_s2ag.py` | `s2ag_citations.json` | per-paper counts (`src/site/mod.rs`) |
| 2 | DBLP + OpenAlex | `fetch_citations_combined.py` | `combined_cache.json` | nothing since the server's removal (formerly citation-status) |
| 3 | Legacy SS / CrossRef | `fetch_citations.py`, `src/citations.rs` | `paper_cache.json` | nothing (formerly a citation-status fallback) |
| 4 | S2ORC (dead) | `download_s2orc.py`, `parse_s2orc.py` | none usable | nothing |

### 6.5.1 DBLP + OpenAlex (formerly the citation-status panel)

`scripts/fetch_citations_combined.py` queries DBLP and OpenAlex and writes
`combined_cache.json` (599 papers; per-paper records of
`{title, year, venue, dblp_key, doi, source, citation_count, openalex_id,
fetched_at}`; source distribution dblp:597, openalex_only:2). Its only consumer
was the removed web server's `/api/citation-status` endpoint, which read this file
(falling back to `paper_cache.json`), summed `citation_count` into a total, bucketed
the counts into a `0-10` / `11-50` / `51-100` / `101-500` / `500+` histogram, and
padded the result with hardcoded "live progress" fields (success rate,
papers-per-minute, active sources) and a `ps aux` check for a running fetch
process. A static site has no fetch process to observe, so the endpoint was
dropped rather than ported, and no Rust code reads `combined_cache.json`,
`paper_cache.json`, or `dblp_cache.json` today. The files remain committed under
`cache/citations/` as pipeline outputs.

### 6.5.2 Legacy SemanticScholar / CrossRef

`src/citations.rs` is an async Semantic Scholar fetcher (`CitationFetcher`,
`CitationCache`, `PaperCitation`) wired to the `fetch-citations` CLI subcommand
(Chapter 9, *The Command-Line Interface*). It rate-limits at one request per
second, caches for 30 days, and only fetches a few major venues
(OSDI/SOSP/SIGMOD/VLDB/NSDI/PLDI/POPL). Critically, it reads and writes
`cache_dir.join("citations.json")` — a singular file that does not exist in
`cache/citations/`. The site generator and its templates never invoke it. For
display purposes it is dead code; its companion `paper_cache.json` (103 papers) was
read only as the citation-status fallback noted above, and is now read by nothing.

### 6.5.3 The dead S2ORC path

`scripts/download_s2orc.py` and `scripts/parse_s2orc.py` target the S2ORC
full-text corpus. The path produced nothing usable: the only artifact,
`data/s2orc/s2orc-metadata-000.jsonl.gz`, is 311 bytes and is not valid gzip — a
placeholder. No Rust code reads any S2ORC output.

## 6.6 Source-of-truth inconsistencies

Two different kinds of citation number surface on the website, computed by
independent pipelines over different paper sets. They are not meant to agree, and
they do not. (The removed web server had a third, the citation-status total.)

| Surface | Source | Formula | Approx. value |
| --- | --- | --- | --- |
| dashboard, `statistics/`, `data/stats.json` | internal graph | `graph.edges.len()` | 0 |
| Per-paper listings (venue pages, scholar profiles) | `s2ag_citations.json` | S2AG `citation_count` per matched title | ~41,468 total across 731 entries |
| *(removed)* `/api/citation-status` | `combined_cache.json` | sum of `citation_count` over 599 papers | ~4,145 |

A reader who visits the dashboard sees zero total citations, then clicks into a
venue and sees individual papers with hundreds or thousands of citations each. Both
are "correct" for their own data source; there is no reconciliation layer. An extender
who wants one consistent number must pick a single pipeline as the source of truth
and route every surface through it — the natural choice being S2AG, given it has
the broadest coverage, with the open work being to also derive edges from it
(§6.4).

The match-rate caveats reinforce that the data is sparse. The failure caches are
large relative to the successes: `combined_failed.json` holds 3,170 entries against
599 successes, `failed_lookups.json` 266, and `openalex_failed.json` 178. Citation
coverage in QIndex is the exception, not the rule.

## 6.7 Cache-file inventory

All files live in `cache/citations/` and are committed (dated Aug 13 2025). The raw
S2AG/S2ORC bulk data under `data/` is gitignored.

| File | Top-level schema | Entries | Read by |
| --- | --- | --- | --- |
| `s2ag_citations.json` | `dict[norm_title -> S2AGCitationData]` | 731 | `src/s2ag_citations.rs` |
| `s2ag_id_mapping.json` | `dict[norm_title -> corpus_id:int]` | 731 | (reference only) |
| `s2ag_summary.json` | `{total_papers_matched, total_citations, papers_by_venue, citations_by_venue, top_cited_papers}` | — | (reference only) |
| `s2ag_citation_graph.json` | `dict[corpus_id_str -> {cites:[], cited_by:[]}]` | 190 (2 edges total) | nothing (orphan) |
| `combined_cache.json` | `dict[md5 -> {title,year,venue,dblp_key,doi,source,citation_count,openalex_id,fetched_at}]` | 599 | nothing (formerly the removed citation-status endpoint) |
| `combined_failed.json` | `dict[md5 -> failure record]` | 3,170 | (scripts) |
| `paper_cache.json` | `dict[md5 -> {title,authors,year,doi,ss_id,citation_count,venue,fetched_at}]` | 103 | nothing (formerly that endpoint's fallback) |
| `openalex_cache.json` | OpenAlex records | 152 | (scripts) |
| `openalex_failed.json` | failures | 178 | (scripts) |
| `failed_lookups.json` | SS failures | 266 | (scripts) |
| `dblp_cache.json` | DBLP records | 1 | nothing |
| `dblp_mapping.json` | DBLP id map | 1 | (scripts) |
| `dblp_failed.json` | `{}` | 0 | (scripts) |
| `citation_data.json` | `{papers:[...], citations:[], metadata:{...}}` | 103 papers / 0 links | nothing in Rust |
| `citation_graph.json` | `{}` | 0 | nothing (empty) |
| `citation_graph.graphml` | GraphML export | — | nothing in Rust |

Only `s2ag_citations.json`, `combined_cache.json`, and `paper_cache.json` are read
by Rust at runtime. The rest are either intermediate fetch caches, reference
artifacts, or dead exports.

## 6.8 API key and configuration

The Semantic Scholar API key is read from the `S2_API_KEY` environment variable, or
a `--api-key` flag, by the two scripts that need it (`download_s2ag.py` and
`fetch_citations_multi_api.py`). The repository ships `.env.example` with a
template line:

```bash
# .env.example
S2_API_KEY=your-api-key-here
```

There is no real `.env` file in the tree, and no script loads one (`python-dotenv`
is not used); the variable must be exported in the shell before running a fetch:

```bash
export S2_API_KEY=...        # do not commit the real key
python3 scripts/download_s2ag.py --dataset papers
python3 scripts/parse_s2ag.py
```

One security note for maintainers: `CLAUDE.md` historically embedded a literal
sample API key in checked-in text. A live credential committed to version control
should be rotated and removed; the key belongs only in the environment or an
uncommitted `.env`.

## 6.9 Summary and extension points

What works today: a per-paper citation *count* for the 731 BibTeX papers that
matched S2AG, looked up by normalized title from a committed JSON file at site-build time and
rendered on venue pages and scholar profiles. What does not work: the citation *graph* — every
graph artifact is empty, `CitationGraph.edges` stays at zero, and PageRank runs its
prestige fallback rather than on real citation flow (Chapter 5). The system has
multiple competing pipelines whose totals do not reconcile, and the fuzzy title
match attaches counts to far more papers than the index can legitimately cover.

The highest-leverage extensions, in order:

1. **Build real edges.** Extend `parse_s2ag.py` to emit paper-to-paper edges from
   the S2AG `citations` release, and add a Rust loader that populates
   `Paper.citations`/`Paper.cited_by` before ranking. This is what would turn the
   dead PageRank path live.
2. **Unify the source of truth.** Route the dashboard and statistics totals and
   the per-paper display through one pipeline (S2AG) so the totals agree.
3. **Index the lookup.** Replace the O(N) fuzzy scan in
   `get_citation_count_for_paper` with a pre-built normalized-title index and an
   explicit DOI key, removing both the per-paper scan cost at build time and the
   substring false-match risk.
4. **Improve coverage.** At ~3.7 percent matched, better title/DOI matching (and
   reducing the 3,170-entry failure cache) is where most of the missing citation
   data is.

Chapter 11, *Limitations, Known Issues, and Roadmap*, revisits these as
project-level work items.
