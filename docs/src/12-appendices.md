# 12. Appendices

Throughout this book, "QIndex" names both the project and the per-scholar
prestige metric it computes; where ambiguity is possible below, the metric is
written as the *QIndex metric* and the project simply as *QIndex*. These
appendices collect reference material that the preceding chapters point to:
the on-disk JSON cache formats (Appendix A), BibTeX corpus conventions
(Appendix B), configuration and runtime environment (Appendix C), a glossary
(Appendix D), and external references (Appendix E). All claims here are
grounded in the source tree; file paths are absolute-from-repo-root and code
references name files and symbols. Where a feature is a fallback, stub, or
aspirational, it is labelled as such rather than presented as working.

## Appendix A: cache/citations JSON file formats

Every file discussed here lives under `cache/citations/`. The directory is
committed to the repository; the multi-gigabyte raw S2AG bulk download and the
S2AG matcher's work files under `data/` are gitignored and are *not* the subject
of this appendix. Rust consumes two local snapshot artifacts:
`s2ag_paper_citations.json` and `s2ag_citation_graph.json`. Legacy pipeline files
are not inputs to default rankings or site builds.

### A.1 `s2ag_paper_citations.json` (consumed by Rust)

Written by local-only `scripts/match_s2ag.py resolve` (Chapter 6). Provenance
fields are `release_id` (`2026-10-06`), `generated` (processing date) and `source`
(local snapshot). Coverage fields are `matched`, `total` and `by_method`; consult
the generated file for observed values.

The `papers` map is keyed by the Rust parser's paper id. Each entry has integer
`corpus_id`, `citations` from the snapshot's `citationcount`, `influential` where
available, `s2_title`, `s2_year` and `match` (`local-doi`, `local-dblp` or `local-title`). Counts
include citations from all of S2, not just our bibliography. Builds use exact
paper-id lookup, not title search. Corpus ids let consumers deduplicate aliases.
`release_id`, not `generated`, is the count provenance date.

### A.1.1 `s2ag_citation_graph.json` (consumed by Rust)

Written by local-only `scripts/match_s2ag.py graph`. It has `release_id`,
`generated`, `source`, scan metadata and `edges`: sorted unique arrays
`[citing_corpus_id, cited_corpus_id]` of unsigned integers. Both endpoints must be
matched bibliography corpus ids. External endpoints and same-corpus self edges
are excluded. Alias and repeated-row deduplication prevents inflated edges.
The release must agree with the count artifact.

Rust canonicalizes corpus ids to deterministic bibliography ids and populates
the internal adjacency lists and edge vector. Venue PageRank uses these edges,
not a synthetic graph inferred from whole-S2 counts.

### A.2 `s2ag_citations.json` and `s2ag_summary.json` (superseded)

Outputs of the superseded `scripts/parse_s2ag.py`; no code reads them any more.
`s2ag_citations.json` maps a normalized title to an S2AG record
(`title`, `year`, `venue`, `corpus_id`, `s2ag_title`, `s2ag_year`, `doi`,
`citation_count`, `reference_count`, `authors`, `venue_info`, `fields`,
`abstract`, `url`) from the old partial extraction; the Rust site used to
fuzzy-match titles against it, which could misattribute counts (Chapter 6).
`s2ag_summary.json` holds aggregate statistics. Top-level keys
(confirmed from the file): `total_papers_matched`, `total_citations`,
`papers_by_venue`, `citations_by_venue`, `top_cited_papers`.

```json
{
  "total_papers_matched": 731,
  "total_citations": 41468,
  "papers_by_venue": { "eurosys": 56, "atc": 30, "icde": 61, "ccs": 122 },
  "citations_by_venue": { "ccs": 7183, "sigcomm": 4170, "stoc": 3248 },
  "top_cited_papers": [
    { "title": "Wait-free synchronization", "venue": "toplas",
      "year": "1991", "citations": 1966 }
  ]
}
```

The values shown are the observed counts for that old extraction: 731 matched
papers, 41,468 total citations, top paper "Wait-free synchronization" (1,966).
No code reads this file.

### A.3 `combined_cache.json` (DBLP + OpenAlex; formerly consumed by Rust)

Output of the DBLP/OpenAlex pipeline (`scripts/fetch_citations_combined.py`).
It was read by the removed web server's `/api/citation-status` endpoint; no Rust
code reads it today. The key is an MD5 hash
of the paper identity; 599 entries are present, with `source` distributed
as `dblp` (597) and `openalex_only` (2). The summed `citation_count` over all
entries is 4,145.

```json
{
  "2af381710f3e474968e3c488cdb3d5a6": {
    "title": "Multicoordinated Paxos.",
    "year": "2007",
    "venue": "PODC",
    "dblp_key": "conf/podc/CamargosSP07",
    "doi": "10.1145/1281100.1281150",
    "source": "dblp",
    "citation_count": 30,
    "openalex_id": "https://openalex.org/W2293633413",
    "fetched_at": 1755100872.010785
  }
}
```

This is a verbatim entry from the committed file.

### A.4 Other files (not consumed by Rust)

| File | Shape | Count | Status |
|---|---|---|---|
| `s2ag_id_mapping.json` | `{normalized_title: corpus_id}` | 731 | superseded (old `parse_s2ag.py`); read by nothing |
| `paper_cache.json` | `{md5: {title,authors,year,doi,ss_id,citation_count,venue,fetched_at}}` | 103 | read by no Rust (formerly the removed `/api/citation-status` fallback) |
| `citation_data.json` | `{papers:[...], citations:[], metadata:{...}}` | 103 papers, 0 links | read by no Rust |
| `citation_graph.json` | `{}` | empty | read by no Rust |
| `dblp_cache.json` | `{md5: {...}}` | 1 | read by no Rust |
| `combined_failed.json` | failure cache | 3170 | scripts only |
| `failed_lookups.json` | failure cache | 266 | legacy fetcher |
| `openalex_cache.json` / `openalex_failed.json` | caches | 152 / 178 | scripts only |

A `paper_cache.json` value carries author lists and may have `ss_id: null`
and `venue: null` (verified from the file). The legacy fetcher
`src/citations.rs` reads and writes `cache_dir.join("citations.json")`, a
filename that does **not** exist under `cache/citations/`; that path is
effectively dead for the website (`src/citations.rs:124-145`).

Honesty note: the citation *graph* artifacts are non-functional.
`s2ag_citation_graph.json` has 190 nodes but 2 total edges,
`citation_graph.json` is `{}`, and `citation_data.json` reports
`total_citation_links: 0`. The earlier "285 citation relationships" claim in
`CLAUDE.md` is not reflected in any current artifact. Consequently S2AG feeds
citation *counts* only — every citation figure on the website — never the
PageRank graph; see Chapter 5, "The Ranking Algorithm."

## Appendix B: BibTeX entry conventions

The corpus lives in `bib/`, one file per venue named `{venue}.bib` lowercase
(Chapter 4, "The Bibliographic Database"). Historical counts from the earlier
book snapshot, before the 2026-10-10 cleanup and new `tos.bib`, were 49 `.bib`
files, 20,898 `@inproceedings`, 122 `@article`, 47 `@string`, and 8 `@techreport`.
These are not current publication counts; use parser exports and site statistics.

- **Entry types.** Conference papers use `@inproceedings`; journal papers use
  `@article` (e.g. `bib/tods.bib`, `bib/jacm.bib`). `parse_entry` maps the
  entry type to `VenueType` (`src/parser.rs`): `inproceedings`/
  `conference` -> Conference, `article` -> Journal, `inworkshop`/`workshop` ->
  Workshop, `symposium` -> Symposium, `techreport` -> TechnicalReport, else Unknown.
  Technical reports retain their issuing institution as their venue.
- **Cite key.** The BibTeX citation key becomes `Paper.id` (and a duplicate
  `Paper.bib_key`); it is the primary key in the in-memory paper map
  (`src/parser.rs:160-161`).
- **Minimal fields.** Most entries carry only `booktitle`/`journal` and
  `year`. A representative entry:

  ```bibtex
  @inproceedings{lampson83hints,
    booktitle = "SOSP",
    year = {1983}
  }
  ```

  However, `parse_entry` only keeps an entry when both `title` and `author`
  are non-empty (`src/parser.rs:214-218`); an entry with just `booktitle`/
  `year` is parsed but **dropped**. Entries that survive carry at least
  title + author and often `url`/`doi`.
- **`@string` macros.** Venue abbreviations are defined as
  `@string{osdi = "OSDI"}`, `@string{atc = "USENIX ATC"}`, etc. These are
  parsed from `strings.bib`/`title.bib` only, via a regex requiring
  double-quoted values (`src/parser.rs:99-114`); the file walker explicitly
  skips any `.bib` whose name contains the substring `title` or `strings`
  (`src/parser.rs:32-97`).
- **Sort order.** Within each file, entries are sorted oldest-to-newest by
  year via `scripts/sort_papers_by_year.py`, which regex-extracts
  `year = {YYYY}` (defaulting to 0 when absent) and preserves the leading
  `%` comment header. Note this script hardcodes a macOS path and must be
  edited before running on this host (see Chapter 10).
- **Field recognized by the parser.** `title`, `author`, `booktitle`,
  `journal`, `year`, `month`, `doi`, `url`, `abstract`, `keywords` are mapped;
  all other tags are ignored (`src/parser.rs:151-219`). Authors are split on
  the literal ` and ` and the token `others` is dropped
  (`src/parser.rs:436-444`). There is **no** field that populates
  `Paper.citations`; parsing initializes that vector empty. The local S2AG
  integration enriches the graph before default ranking (Chapters 5 and 6).
- **Working artifacts.** `bib/` also contains non-database files such as
  `bib/sigcomm.bib.backup` and `bib/sigcomm.removed`, produced by the
  SIGCOMM workshop cleaner; tooling should glob `*.bib` and ignore these.

## Appendix C: Configuration and environment

### C.1 Environment variables

| Variable | Used by | Default / notes |
|---|---|---|
| `S2_API_KEY` | dataset download scripts and legacy API fetchers | Required for dataset downloading only in the snapshot workflow. Local `match_s2ag.py scan/resolve/graph` and Rust builds do not need or use it. |
| `RUST_LOG` | `env_logger` in `main()` | Defaults to `info` (`src/main.rs:25`). |

`.env.example` contains exactly the template `S2_API_KEY=your-api-key-here`.
There is no real `.env` file in the repo (`.env` is gitignored), and no script
loads `.env` (no `python-dotenv`); export the variable yourself, for example
with `set -a; source .env; set +a`. `CLAUDE.md` shows only placeholders
(`"$S2_API_KEY"`, `"..."`). The `RUST_ENV` variable that the old web template
read to inject a live-reload script is no longer used.

### C.2 Base URL and hosting

There is no server, host, or port. `qindex build-site` takes a `--base-url`
(default `/`; `src/cli.rs`, `BuildSite`) that every internal link is built from;
the GitHub Pages project site uses `/q-index/`, which CI obtains from
`actions/configure-pages` (`.github/workflows/pages.yml`). A site must be served
under the same path it was built for. For a local preview, build with the default
and run `python3 -m http.server -d site 8080`. See Chapters 8 and 10.

### C.3 Runtime paths (all relative to the process CWD)

The CLI, including `build-site`, resolves several paths relative to the current
working directory, so QIndex effectively must be run from the repository root.

| Path | Purpose | Reference |
|---|---|---|
| `./bib` | default BibTeX corpus dir | `src/cli.rs:22-23` |
| `./static` | assets copied into the site's `static/` (`--static-dir`) | `src/cli.rs` (`BuildSite`) |
| `./site` | site build output, replaced on every build (`--out-dir`) | `src/cli.rs` (`BuildSite`) |
| `./docs/book` | rendered book copied to `book/` if present (`--book-dir`) | `src/cli.rs` (`BuildSite`) |
| `cache/citations/s2ag_paper_citations.json` | S2AG citation counts keyed by paper id (`--citations-file`) | `s2ag_citations::DEFAULT_PATH`, `src/cli.rs` (`BuildSite`) |
| `./cache/citations/papers.jsonl` | paper list for S2AG matching (`export-papers --output`) | `src/cli.rs` (`ExportPapers`) |
| `cache/citations/s2ag_citation_graph.json` | internal matched-bibliography graph (`--citation-graph-file`) | `s2ag_citations::DEFAULT_GRAPH_PATH`, `src/main.rs` (`load_snapshot`) |

When running from another directory, relocate all required inputs explicitly,
including both snapshot artifacts. Their releases must agree. Missing count
data cannot establish zero external citations; a missing graph does not provide
citation-based venue ranking. See Chapter 10 for missing-input behavior.

### C.4 Build configuration

`Cargo.toml` declares an empty `[workspace]` table (`Cargo.toml:12`) so cargo
treats QIndex as its own workspace root rather than walking up to
`/home/users/shuai/Cargo.toml` (whose `members = ["crates/*"]` would reject
QIndex). The release profile sets `lto = true`, `codegen-units = 1`,
`opt-level = 3` (`Cargo.toml:79-82`), which favors runtime speed at the cost
of slow link/codegen; for iteration use debug builds. See Chapter 10,
"Building, Running, and Deployment."

## Appendix D: Glossary

- **QIndex (project).** This system: a BibTeX-driven ranking tool with a CLI
  and a generated static website.
- **QIndex (metric).** Per-scholar prestige score in `[0, 100]`, normalized
  against the maximum scholar score (`src/algorithm.rs`,
  `calculate_scholar_scores`), using unique eligible authored works.
- **Venue.** Publication forum keyed by `normalize_venue_id` (`src/parser.rs`),
  modeled by `Venue` (`src/models.rs`).
- **Scholar.** Name-based identity keyed by `generate_scholar_id` over the
  normalized name. DBLP numeric disambiguators are preserved; this is not a
  globally verified S2 author-id mapping. Affiliations are not fabricated.
- **CSRankings.** Venue taxonomy and allowlist used by `is_csrankings_venue`
  (`src/algorithm.rs`). QIndex and ranked/search scholar metrics use eligible
  unique works. All-indexed profile metrics also include excluded venues.
- **PageRank.** Directed cross-venue link analysis over the internal matched
  bibliography graph, with dangling mass and nonnegative citation-age decay.
  With no usable cross-venue citations it yields uniform 1/N.
- **Venue Score.** Normalized, unadjusted PageRank, serialized under the retained
  `pagerank` field; no manual prestige, name or venue-eligibility boosts.
- **Count-scaled score.** Venue Score times `ln(unique venue works + 1)`,
  serialized as `impact_factor`; not a journal impact factor.
- **Corpus id.** S2 work identifier used to deduplicate matched BibTeX aliases.
  Unmatched records deduplicate only by local paper id. The representative
  prefers complete venue/DOI/year/authors, then smallest local id.
- **Reference year.** Current UTC year by default; the calculator uses a valid
  snapshot release year when present. Future and unknown years are undecayed.
- **Damping factor.** PageRank's teleportation parameter `d`; default `0.85`
  in `AlgorithmParams` (`src/models.rs:108-125`). The random surfer follows a
  link with probability `d` and jumps to a uniformly random node with
  probability `1-d`.
- **S2AG.** Semantic Scholar Academic Graph. Local release papers provide whole-S2
  counts and local citations provide bibliography-only edges through
  `scripts/match_s2ag.py`. No online API participates in matching or ranking.
- **OpenAlex.** Open scholarly catalog; one of two sources behind
  `combined_cache.json` (DBLP plus OpenAlex). Provides citation counts and
  work IDs.
- **DBLP.** The computer-science bibliography. Source of the BibTeX corpus
  (via `scripts/fetch_main_conference.py`) and the dominant source in
  `combined_cache.json` (597 of 599 entries).

## Appendix E: References and further reading

Citations are intentionally generic; precise URLs and DOIs are omitted where
not verifiable from the source tree.

1. **PageRank.** S. Brin and L. Page, "The Anatomy of a Large-Scale
   Hypertextual Web Search Engine" (1998); and L. Page, S. Brin, R. Motwani,
   T. Winograd, "The PageRank Citation Ranking: Bringing Order to the Web"
   (Stanford technical report, 1999). Defines the link-analysis model and the
   damping factor underlying QIndex's active venue ranking.
2. **h-index.** J. E. Hirsch, "An index to quantify an individual's
   scientific research output," *PNAS* (2005). Background for QIndex's
   h-index computation over unique eligible authored works in ranking output
   and over all-indexed works in the separately labelled profile metrics.
3. **CSRankings.** E. Berger et al., CSRankings: Computer Science Rankings
   (csrankings.org). Source of the subarea taxonomy and venue allowlist that
   QIndex reproduces.
4. **Semantic Scholar / S2AG.** Semantic Scholar Academic Graph and its bulk
   datasets, Allen Institute for AI. Source of both local snapshot artifacts.
   `S2_API_KEY` is required for downloads, not offline matching or ranking.
5. **OpenAlex.** OpenAlex, OurResearch — an open index of scholarly works.
   One of the two backends for `combined_cache.json`.
6. **DBLP.** dblp computer science bibliography, Schloss Dagstuhl. Source of
   the conference corpus and of most citation-cache entries.

For implementation-level cross-references, see Chapter 3 ("The Data Model")
for the structs, Chapter 5 ("The Ranking Algorithm") for the scoring
formulas and the PageRank/fallback split, Chapter 6 ("Citation Data
Integration") for the local snapshot pipeline and legacy paths, and Chapter 8 ("The Static Website
and Its Data Files") for what the website publishes and where its numbers come
from.
