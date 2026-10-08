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
citation lists empty. Citation *counts* are instead matched offline against the
Semantic Scholar Academic Graph (S2AG), written to a committed JSON file keyed by
paper id, and looked up exactly when the static website is generated. Every
citation number on the site — per paper, per scholar, per venue, and the dataset
total — derives from those per-paper counts. The citation *graph* extraction
failed, so no edges flow into PageRank. Several older, partly redundant fetch
pipelines also exist; none of them feeds the site. The rest of this chapter
substantiates each of those claims against the source.

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
`parser.rs`. The corpus parses to roughly 19,954 papers, 44 venues, and 43,942
scholars, but zero internal citation edges.

Citation information therefore must come from *outside* the BibTeX path. The
external source is S2AG. Crucially, that source feeds per-paper *counts* into the
display layer; it does not feed *edges* into the graph that PageRank traverses.
The two are wired separately, and only one of them works (§6.5).

## 6.2 The S2AG pipeline

S2AG integration is an offline pipeline that ends in one committed JSON file,
keyed by the paper ids the Rust parser assigns, which the site generator reads
with exact lookups.

```
qindex export-papers         ->  cache/citations/papers.jsonl         (gitignored)
   (src/main.rs run_export_papers: id, title, year, doi, venue as parsed)
        |
download_s2ag.py             ->  data/s2ag/papers/*.gz                 (gitignored)
        |
match_s2ag.py scan           ->  data/s2ag/work/scan_matches.json      (gitignored)
   (parallel scan of the local S2AG papers files: DOI or title+year -> corpus id)
        |
match_s2ag.py resolve        ->  cache/citations/s2ag_paper_citations.json  (committed)
   (Graph API /paper/batch by CorpusId or DOI -> current citation counts;
    responses cached in data/s2ag/work/api_cache.json)
        |
src/s2ag_citations.rs        ->  S2agCitations::load: HashMap<paper id, S2agPaper>
        |
src/site/mod.rs              ->  Citations::of(paper): exact lookup by paper.id;
                                 totals and h-indexes derived from the same counts
```

### 6.2.1 Exporting our papers

The matcher works from the papers exactly as the Rust parser sees them, rather than
re-parsing BibTeX in Python. `qindex export-papers` (`run_export_papers` in
`src/main.rs`; CLI in Chapter 9) parses `bib/` with `BibParser` and writes one JSON
object per paper — `{id, title, year, doi, venue}` — to
`cache/citations/papers.jsonl`. The `id` is the parser's paper id (the BibTeX cite
key), which is the key the site generator looks up, so the output of the matcher
can be joined back without any title comparison at build time. The file is
regenerable and gitignored.

Of the 19,954 exported papers, 13,936 carry a DOI. The 6,018 without one are
concentrated in exactly the venues whose BibTeX came from hand-curated or USENIX
sources: OSDI, NSDI, USENIX ATC, and FAST have essentially no DOIs, nor do NeurIPS
and ICML. Those papers can only be matched by title.

### 6.2.2 Download

`scripts/download_s2ag.py` fetches the S2AG bulk *datasets* (the `papers` and
`citations` releases) into `data/s2ag/`. The datasets API requires an API key to
hand out download links (unauthenticated requests get
`{"error":"A valid API key is required"}`); the key is resolved from the
environment:

```python
# scripts/download_s2ag.py:258 (paraphrased)
api_key = args.api_key or os.environ.get("S2_API_KEY")
```

The working tree holds a partial download of release 2025-08-08: 32 of the 60
`papers` files (27 GB gzipped) and 122 of the 236 `citations` files (112 GB). One
of the 32 papers files was truncated mid-download; the matcher reads the 807,798
records before the cut and reports the file. All of `data/` is gitignored and is
not needed to build the site.

### 6.2.3 Local scan (`match_s2ag.py scan`)

`scan` (`cmd_scan` in `scripts/match_s2ag.py`) streams every `data/s2ag/papers/*.gz`
file in parallel, one worker process per file (`multiprocessing.Pool`, workers
initialized with the set of our DOIs and normalized titles). On the reference run
it read 117,481,744 S2AG records. A record is a candidate for one of our papers if

- its `externalids.DOI`, lowercased, equals the paper's DOI (`local-doi`), or
- its normalized title equals the paper's normalized title (`local-title`), for
  titles of at least `MIN_TITLE_WORDS` = 3 words. Shorter titles ("Front Matter",
  "Keynote") match only by DOI.

Title normalization (`norm_title`) is applied identically to both sides:
HTML entities are unescaped (some BibTeX titles contain `&apos;`), the text is
NFKD-normalized and stripped of combining marks, LaTeX commands such as `\textsc`
are removed, and the result is lowercased with every run of non-alphanumeric
characters (including stray `{`/`}` from BibTeX) collapsed to a single space.

When several S2AG records hit the same paper — typically an arXiv preprint and the
conference version — `pick` keeps candidates whose year is within ±1 of ours (or
either year is missing), then prefers one whose S2 venue contains our venue name
(`venue_affinity`), then the most-cited. A DOI hit always takes precedence over a
title hit. The result is `data/s2ag/work/scan_matches.json`: paper id → corpus id,
the S2 title/year/venue, and the method.

### 6.2.4 Resolving current counts (`match_s2ag.py resolve`)

`resolve` (`cmd_resolve`) issues one query per paper to the Semantic Scholar Graph
API's batch endpoint (`POST /graph/v1/paper/batch`, `BATCH_SIZE` = 500 ids per
request): `CorpusId:<id>` for papers the scan found, `DOI:<doi>` for the rest. The
bulk files are therefore only an *id resolver*; every citation count on the site
comes from one API snapshot, dated in the output file's `generated` field
(2026-10-08 on the reference run). The batch endpoint works without a key with
retry and backoff (`Api.request` retries 429/5xx up to eight times, doubling a
5-second delay); the ~35 requests of a full run take a few minutes. Responses are
cached in `data/s2ag/work/api_cache.json`, so a rerun only queries ids it has not
seen.

With `S2_API_KEY` set and `--title-match`, `resolve` also tries the title-match
endpoint (`/paper/search/match`) for papers still unmatched, accepting a result
only if the titles agree (word-set Jaccard ≥ 0.9, `titles_agree`) and the years
are within ±1 (method `api-title`). Without a key this is impractical: a probe of
20 requests at one per second was rate-limited 18 times.

The output, `cache/citations/s2ag_paper_citations.json` (about 3.3 MB, committed),
is keyed by paper id:

```json
{"generated": "2026-10-08",
 "source": "Semantic Scholar Graph API citation counts; S2AG papers dataset for title matching",
 "matched": 16462, "total": 19954,
 "by_method": {"local-doi": 6981, "local-title": 3171, "doi": 6310},
 "papers": {
   "000110": {"corpus_id": 50626, "citations": 91, "influential": 7,
              "s2_title": "Higher-order multi-parameter tree transducers ...",
              "s2_year": 2010, "match": "local-doi"}, ...}}
```

### 6.2.5 Coverage

| Metric | Value |
| --- | --- |
| Papers matched | 16,462 of 19,954 (82.5%) |
| — by DOI in the local scan (`local-doi`) | 6,981 |
| — by title + year in the local scan (`local-title`) | 3,171 |
| — by DOI through the API (`doi`) | 6,310 |
| Papers with a DOI matched | 13,579 of 13,936 (97%) |
| Papers without a DOI matched | 2,883 of 6,018 (48%) |

Per venue, the DOI-bearing venues (CCS, STOC, SIGCOMM, ICDE, EuroSys, ASPLOS,
SIGMOD, SOSP, PLDI, POPL, …) sit between 85 and 99 percent. The DOI-less venues
(OSDI, NSDI, USENIX ATC, FAST, NeurIPS, ICML) sit around 43–52 percent, and that
ceiling is structural: title matching can only find papers whose S2AG record is in
one of the 32 downloaded files, about 53 percent of the dataset. Within that limit
it performs about as well as DOI matching.

Checks run on the reference output:

- **No fan-out.** Among title matches the most common citation count accounts for
  2.5 percent of papers — the shape of a real distribution. (The previous pipeline,
  §6.3.3, gave 1,776 papers the identical count 14.)
- **Years.** Every match whose year differs from ours by more than one is a DOI
  match (S2 sometimes records the preprint year); title matches are within ±1 by
  construction.
- **Titles.** Spot checks of random title matches agree with S2's title. Where the
  current S2 title differs (21 of 3,171), S2 has replaced it with a garbled
  extraction such as "This paper is included in the Proceedings of the …" for some
  USENIX papers; the corpus id, and so the count, is still the right paper's.

## 6.3 The Rust side

### 6.3.1 Loader: `src/s2ag_citations.rs`

The loader is a plain deserializer with no matching logic. It reads only what the
site shows; the S2 title/year and match method stay in the file for auditing:

```rust
// src/s2ag_citations.rs
pub struct S2agPaper {
    pub corpus_id: Option<u64>,
    pub citations: usize,
}

pub struct S2agCitations {
    pub generated: Option<String>,             // date the counts were fetched
    pub papers: HashMap<String, S2agPaper>,    // keyed by paper id
}
```

`S2agCitations::load` reads the file and logs how many papers it covers and the
fetch date (a missing file logs a warning and yields an empty map, so a site can
still be built without S2AG data); `get(paper_id)` is a single `HashMap` lookup.
`DEFAULT_PATH` names the file `match_s2ag.py` writes. There is no global state: `build_site` loads the file from
`SiteOptions::citations_file`, which `qindex build-site --citations-file`
populates (default `cache/citations/s2ag_paper_citations.json`, `DEFAULT_PATH`; the integration
tests pass `None`).

### 6.3.2 Displayed counts: `Citations` in `src/site/mod.rs`

`Citations::of(paper)` returns the S2AG count when the paper id is in the file,
otherwise the internal `cited_by.len()`, tagged with which source it came from
(`PaperCitations { count, s2ag }`). Venue pages render it in Rust (`paper_item` in
`src/site/templates.rs`); scholar profiles store it in their JSON shard as
`citations` plus an `s2ag` flag, and `paperHtml` in `static/app.js` renders it the
same way (Chapter 8). The three outcomes are:

| Case | Shown as |
| --- | --- |
| Matched in S2AG | `Citations: N (S2AG)` |
| Not matched, cited inside the dataset | `Citations: N (internal)` |
| Not matched, no internal citations (`PaperCitations::unknown`) | `Citations: n/a` |

Since `cited_by` is always empty (§6.1), the middle case does not occur today; an
unmatched paper shows "n/a" rather than a misleading zero.

Every aggregate on the site is derived from these same per-paper counts:

- a scholar's total citations and h-index (scholar shards, the top-scholars table's
  `citation_count`/`h_index`, the search index's `h_index`), overwritten on the
  `ScholarRanking` values after `PageRankCalculator` has produced them;
- a venue page's "Total Citations";
- the dashboard and `statistics/` totals, `data/stats.json`'s `total_citations`
  (about 1.8 million on the reference build), and the "Most Cited Papers" table.

`Citations::total` and `Citations::h_index` both go through `distinct_counts`, which
counts each S2 corpus id once. Duplicate BibTeX entries for the same paper are
common (Ion Stoica's 114 papers include the vLLM/PagedAttention paper three times
under different keys); without the deduplication its 8,958 citations would be
counted three times in his total and h-index. On the reference build his profile
shows about 25.9 thousand citations and an h-index of about 42.

These are display values only. The QIndex score and the ranking order still come
from `PageRankCalculator` unchanged (Chapter 5), which does not see S2AG counts.

### 6.3.3 What this replaced

Before this pipeline, `scripts/parse_s2ag.py` re-parsed BibTeX with a regex whose
title capture stopped at the first `}` (and could match the `booktitle` field
instead of `title`), and wrote `cache/citations/s2ag_citations.json`: 731 entries
keyed by normalized title. The Rust module held them in a process-global `Lazy`
index and matched by title at lookup time, falling back to a fuzzy rule that
accepted any title containing (or contained in) an index title. That rule attached
counts to 3,551 papers — 1,776 of them the identical count 14 — so most displayed
counts were false positives, while the large majority of papers showed zero.
`parse_s2ag.py` and its outputs (`s2ag_citations.json`, `s2ag_id_mapping.json`,
`s2ag_summary.json`, and the orphan `s2ag_citation_graph.json`) remain in the
repository but nothing reads them any more.

## 6.4 Rerunning the pipeline

```bash
cargo build --release
./target/release/qindex export-papers          # cache/citations/papers.jsonl
python3 -I scripts/match_s2ag.py scan          # needs data/s2ag/papers/*.gz; minutes on many cores
python3 -I scripts/match_s2ag.py resolve       # Graph API; cached in data/s2ag/work/
./target/release/qindex build-site             # picks up the new counts
```

Commit the regenerated `cache/citations/s2ag_paper_citations.json`; CI has no access
to the bulk files and rebuilds the site from the committed file. `resolve` alone
(without a new `scan`) refreshes counts for already-matched papers from the
cached corpus ids. Python's TLS verification falls back to the system CA bundle
(`/etc/ssl/certs/ca-certificates.crt`) when its own default has no CA file
(`ssl_context`), which is the case for some Homebrew Pythons.

What remains unmatched is mostly the 3,135 DOI-less papers whose records sit in the
28 S2AG `papers` files that were never downloaded. Two routes close that gap, and
both need an S2 API key:

1. download the complete latest `papers` release (all 60 files, from one release so
   the shards don't overlap) and rerun `scan`; or
2. run `resolve --title-match` with `S2_API_KEY` set, which title-matches the
   leftovers one request at a time (about an hour for ~3,500 papers at the keyed
   rate).

## 6.5 The citation graph extraction failed

The single most important fact in this chapter: S2AG feeds counts, not edges.

`parse_s2ag.py` contains no graph code and never writes a graph file, and
`match_s2ag.py` only resolves counts. The file
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

To make the graph real, a future change would need a script to emit genuine
paper-to-paper edges from the S2AG `citations` release (whose records reference
corpus ids, which `s2ag_paper_citations.json` now maps to our paper ids), plus a
Rust loader that populates `Paper.citations`/`Paper.cited_by` (or
`CitationGraph.edges`) before `PageRankCalculator::calculate` runs. None of that
exists today.

## 6.6 The competing pipelines

Five pipelines fetch or derive citation data. They were built at different times
and overlap; today only the first feeds the website.

| # | Pipeline | Scripts | Output cache | Consumed by |
| --- | --- | --- | --- | --- |
| 1 | S2AG match | `qindex export-papers`, `download_s2ag.py`, `match_s2ag.py` | `s2ag_paper_citations.json` | every citation number on the site (`src/site/mod.rs`) |
| 2 | Old S2AG parse (superseded) | `parse_s2ag.py` | `s2ag_citations.json` | nothing since pipeline 1 replaced it |
| 3 | DBLP + OpenAlex | `fetch_citations_combined.py` | `combined_cache.json` | nothing since the server's removal (formerly citation-status) |
| 4 | Legacy SS / CrossRef | `fetch_citations.py`, `src/citations.rs` | `paper_cache.json` | nothing (formerly a citation-status fallback) |
| 5 | S2ORC (dead) | `download_s2orc.py`, `parse_s2orc.py` | none usable | nothing |

### 6.6.1 DBLP + OpenAlex (formerly the citation-status panel)

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

### 6.6.2 Legacy SemanticScholar / CrossRef

`src/citations.rs` is an async Semantic Scholar fetcher (`CitationFetcher`,
`CitationCache`, `PaperCitation`) wired to the `fetch-citations` CLI subcommand
(Chapter 9, *The Command-Line Interface*). It rate-limits at one request per
second, caches for 30 days, and only fetches a few major venues
(OSDI/SOSP/SIGMOD/VLDB/NSDI/PLDI/POPL). Critically, it reads and writes
`cache_dir.join("citations.json")` — a singular file that does not exist in
`cache/citations/`. The site generator and its templates never invoke it. For
display purposes it is dead code; its companion `paper_cache.json` (103 papers) was
read only as the citation-status fallback noted above, and is now read by nothing.

### 6.6.3 The dead S2ORC path

`scripts/download_s2orc.py` and `scripts/parse_s2orc.py` target the S2ORC
full-text corpus. The path produced nothing usable: the only artifact,
`data/s2orc/s2orc-metadata-000.jsonl.gz`, is 311 bytes and is not valid gzip — a
placeholder. No Rust code reads any S2ORC output.

## 6.7 One source of truth

Every citation number the website shows now comes from one place,
`s2ag_paper_citations.json`, through `Citations` (§6.3.2):

| Surface | Formula |
| --- | --- |
| Paper listings (venue pages, scholar profiles) | S2AG count, else internal, else "n/a" |
| Scholar profile, top-scholars table, search index | sum and h-index of the paper counts, each corpus id once |
| Venue page "Total Citations" | sum of its papers' counts, each corpus id once |
| Dashboard, `statistics/`, `data/stats.json`, "Most Cited Papers" | sum / ranking of all papers' counts, each corpus id once |

Previously the dashboard reported `graph.edges.len()` (always 0) while venue pages
showed per-paper S2AG counts, so a visitor saw zero total citations next to papers
with thousands. That inconsistency is gone. The old caches (`combined_cache.json`
and friends) still disagree with S2AG, and the CLI subcommands (Chapter 9) still
know nothing about S2AG, but none of that reaches the site.

## 6.8 Cache-file inventory

All files live in `cache/citations/`. The raw S2AG/S2ORC bulk data and the
matcher's work files under `data/` are gitignored.

| File | Top-level schema | Entries | Read by |
| --- | --- | --- | --- |
| `s2ag_paper_citations.json` | `{generated, source, matched, total, by_method, papers: dict[paper_id -> {corpus_id, citations, influential, s2_title, s2_year, match}]}` | 16,462 | `src/s2ag_citations.rs` (site build) |
| `papers.jsonl` (gitignored) | JSON lines `{id, title, year, doi, venue}` | 19,954 | `scripts/match_s2ag.py` |
| `s2ag_citations.json` | `dict[norm_title -> S2AGCitationData]` | 731 | nothing (superseded) |
| `s2ag_id_mapping.json` | `dict[norm_title -> corpus_id:int]` | 731 | nothing (superseded) |
| `s2ag_summary.json` | `{total_papers_matched, total_citations, ...}` | — | nothing (superseded) |
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

Only `s2ag_paper_citations.json` is read by Rust. The rest are intermediate fetch
caches, superseded outputs, or dead exports.

## 6.9 API key and configuration

The Semantic Scholar API key is read from the `S2_API_KEY` environment variable by
the scripts that need it: `download_s2ag.py` (also `--api-key`),
`fetch_citations_multi_api.py`, and `match_s2ag.py`, which sends it as the
`x-api-key` header when present. Only the datasets download and the title-match
step actually require it; `match_s2ag.py resolve`'s batch lookups work without one.
The repository ships `.env.example` with a template line:

```bash
# .env.example
S2_API_KEY=your-api-key-here
```

There is no real `.env` file in the tree, and no script loads one (`python-dotenv`
is not used); the variable must be exported in the shell (for example
`set -a; source .env; set +a`) before running a fetch:

```bash
export S2_API_KEY=...        # do not commit the real key
python3 scripts/download_s2ag.py --dataset papers
python3 -I scripts/match_s2ag.py resolve --title-match
```

## 6.10 Summary and extension points

What works today: a citation count for 16,462 of 19,954 papers (82.5 percent),
matched to S2AG by DOI or by title and year, refreshed from one Graph API snapshot,
committed as a file keyed by paper id, and looked up exactly at site-build time.
Every citation number on the site — per paper, per scholar (total and h-index), per
venue, and the dataset totals — derives from those counts, counting each S2 paper
once. What does not work: the citation *graph* — every graph artifact is empty,
`CitationGraph.edges` stays at zero, and PageRank runs its prestige fallback rather
than on real citation flow (Chapter 5), so citation counts affect what the site
displays but not how it ranks.

The highest-leverage extensions, in order:

1. **Build real edges.** Emit paper-to-paper edges from the S2AG `citations`
   release, using `s2ag_paper_citations.json` to map corpus ids to our paper ids,
   and populate `Paper.citations`/`Paper.cited_by` before ranking. This is what
   would turn the dead PageRank path live.
2. **Close the no-DOI gap.** With an API key, download the full `papers` release or
   run `resolve --title-match` (§6.4); the DOI-less USENIX and ML venues are where
   nearly all of the remaining unmatched papers are.
3. **Clean duplicate BibTeX entries.** Totals already count each corpus id once,
   but paper listings still show the duplicates.

Chapter 11, *Limitations, Known Issues, and Roadmap*, revisits these as
project-level work items.
