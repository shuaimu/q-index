# 12. Appendices

Throughout this book, "QIndex" names both the project and the per-scholar
prestige metric it computes; where ambiguity is possible below, the metric is
written as the *QIndex metric* and the project simply as *QIndex*. These
appendices collect reference material that the preceding chapters point to:
the on-disk JSON cache formats (Appendix A), BibTeX corpus conventions
(Appendix B), configuration and runtime environment (Appendix C), a glossary
(Appendix D), and external references (Appendix E). All claims here are
grounded in the source tree; file paths are absolute-from-repo-root and code
citations use `path:line`. Where a feature is a fallback, stub, or
aspirational, it is labelled as such rather than presented as working.

## Appendix A: cache/citations JSON file formats

Every file discussed here lives under `cache/citations/`. The directory is
committed to the repository; the multi-gigabyte raw S2AG bulk download under
`data/` is gitignored and is *not* the subject of this appendix. Several of
these files are produced by competing Python pipelines (see Chapter 6,
"Citation Data Integration") and only two of them are read by Rust at runtime:
`s2ag_citations.json` (per-paper display, `src/s2ag_citations.rs:152`) and
`combined_cache.json` (homepage/citation-status stats,
`src/web/handlers.rs:669`).

### A.1 `s2ag_citations.json` (consumed by Rust)

A single JSON object mapping a *normalized title* to one S2AG record. The key
is the title lowercased, reduced to alphanumerics and single spaces (no
punctuation), matching `normalize_title` at `src/s2ag_citations.rs:108-116`.
Each value deserializes into `S2AGCitationData` (`src/s2ag_citations.rs:9-26`);
note the `abstract` JSON field is renamed to `abstract_text` because
`abstract` is a Rust keyword (`src/s2ag_citations.rs:23-24`). The file holds
731 entries.

```json
{
  "hang doctor runtime detection and diagnosis of soft hangs for smartphone apps": {
    "title": "Hang Doctor: ...",
    "year": "2016",
    "venue": "eurosys",
    "corpus_id": 4936200,
    "s2ag_title": "Hang Doctor: ...",
    "s2ag_year": 2016,
    "doi": null,
    "citation_count": 12,
    "reference_count": 0,
    "authors": ["..."],
    "venue_info": null,
    "fields": null,
    "abstract": null,
    "url": null
  }
}
```

The value above is illustrative in its numeric fields; the key form and field
set are taken from the loader struct and observed data. `abstract` is
frequently `null` in the actual file.

### A.2 `s2ag_summary.json` (human-readable summary)

Aggregate statistics emitted by `scripts/parse_s2ag.py`. Top-level keys
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

The values shown are the actual observed counts: 731 matched papers, 41,468
total citations, top paper "Wait-free synchronization" (1,966). No Rust code
reads this file; it exists for inspection.

### A.3 `combined_cache.json` (DBLP + OpenAlex; consumed by Rust)

Output of the DBLP/OpenAlex pipeline (`scripts/fetch_citations_combined.py`),
read by `/api/citation-status` and the homepage stats. The key is an MD5 hash
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
| `s2ag_id_mapping.json` | `{normalized_title: corpus_id}` | 731 | read by no Rust |
| `s2ag_citation_graph.json` | `{corpus_id_str: {cites:[], cited_by:[]}}` | 190 nodes | orphan; only 1 cites + 1 cited_by edge total; written by no script |
| `paper_cache.json` | `{md5: {title,authors,year,doi,ss_id,citation_count,venue,fetched_at}}` | 103 | fallback for `/api/citation-status` (`src/web/handlers.rs:711`) |
| `citation_data.json` | `{papers:[...], citations:[], metadata:{...}}` | 103 papers, 0 links | read by no Rust |
| `citation_graph.json` | `{}` | empty | read by no Rust |
| `dblp_cache.json` | `{md5: {...}}` | 1 | path declared but unused (`src/web/handlers.rs:670`) |
| `combined_failed.json` | failure cache | 3170 | scripts only |
| `failed_lookups.json` | failure cache | 266 | legacy fetcher |
| `openalex_cache.json` / `openalex_failed.json` | caches | 152 / 178 | scripts only |

A `paper_cache.json` value carries author lists and may have `ss_id: null`
and `venue: null` (verified from the file). The legacy fetcher
`src/citations.rs` reads and writes `cache_dir.join("citations.json")`, a
filename that does **not** exist under `cache/citations/`; that path is
effectively dead for the web UI (`src/citations.rs:124-145`).

Honesty note: the citation *graph* artifacts are non-functional.
`s2ag_citation_graph.json` has 190 nodes but 2 total edges,
`citation_graph.json` is `{}`, and `citation_data.json` reports
`total_citation_links: 0`. The earlier "285 citation relationships" claim in
`CLAUDE.md` is not reflected in any current artifact. Consequently S2AG feeds
per-paper citation *counts* only, never the PageRank graph; see Chapter 5,
"The Ranking Algorithm."

## Appendix B: BibTeX entry conventions

The corpus lives in `bib/`, one file per venue named `{venue}.bib` lowercase
(Chapter 4, "The Bibliographic Database"). The directory holds 49 `.bib`
files. Entry-type totals across all files: 20,898 `@inproceedings`, 122
`@article`, 47 `@string`, 8 `@techreport`.

- **Entry types.** Conference papers use `@inproceedings`; journal papers use
  `@article` (e.g. `bib/tods.bib`, `bib/jacm.bib`). `parse_entry` maps the
  entry type to `VenueType` (`src/parser.rs:205-211`): `inproceedings`/
  `conference` -> Conference, `article` -> Journal, `inworkshop`/`workshop` ->
  Workshop, `symposium` -> Symposium, else Unknown.
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
  `Paper.citations`; that vector is always initialized empty
  (`src/parser.rs:166-167`), which is why the BibTeX-derived citation graph is
  empty (Chapter 5).
- **Working artifacts.** `bib/` also contains non-database files such as
  `bib/sigcomm.bib.backup` and `bib/sigcomm.removed`, produced by the
  SIGCOMM workshop cleaner; tooling should glob `*.bib` and ignore these.

## Appendix C: Configuration and environment

### C.1 Environment variables

| Variable | Used by | Default / notes |
|---|---|---|
| `S2_API_KEY` | `scripts/download_s2ag.py:258`, `scripts/fetch_citations_multi_api.py` | Semantic Scholar key; read via `os.environ.get('S2_API_KEY')`. No Rust code reads it. |
| `RUST_LOG` | `env_logger` in `main()` | Defaults to `info` (`src/main.rs:23`). |
| `RUST_ENV` | `base_template` (`src/web/templates.rs:167`) | Defaults to `development`; when not `production`, the page injects `/static/live-reload.js`. |

`.env.example` contains exactly the template `S2_API_KEY=your-api-key-here`.
There is no real `.env` file in the repo, and no script loads `.env` (no
`python-dotenv`). Honesty note: a sample API key is hardcoded into
`CLAUDE.md`, which is a secret-in-version-control concern; treat it as
compromised and supply your own via the environment.

### C.2 Ports and host binding

The web server binds `(host, port)` (`src/web/server.rs:62`). Defaults are
`--host 127.0.0.1` (`src/cli.rs:96-97`) and `--port 8080` (`src/cli.rs:100`).
To expose on the network use `qindex web --host 0.0.0.0 -p 8080`. There is
**no authentication** of any kind. The `--host` flag is threaded
`main.rs:46-52` -> `run_web_server` (`src/main.rs:341`) ->
`start_server` (`src/web/server.rs:10`).

### C.3 Runtime paths (all relative to the process CWD)

The server and CLI resolve several paths relative to the current working
directory, so QIndex effectively must be launched from the repository root.

| Path | Purpose | Reference |
|---|---|---|
| `./bib` | default BibTeX corpus dir | `src/cli.rs:22-23` |
| `./static` | static assets (`Files::new("/static", "./static")`) | `src/web/server.rs` |
| `cache/citations/s2ag_citations.json` | per-paper S2AG counts | `src/s2ag_citations.rs:152-163` (hardcoded relative) |
| `cache/citations/combined_cache.json` | homepage / citation-status stats | `src/web/handlers.rs:669` |
| `./cache/citations.json` | citation cache for CLI `calculate`/`venues`/`scholars`/`search` | `src/main.rs:89` etc. (file does not exist; load is a silent no-op) |

If the server is started from any other directory, static files are not
served and citation reads return empty silently.

### C.4 Build configuration

`Cargo.toml` declares an empty `[workspace]` table (`Cargo.toml:12`) so cargo
treats QIndex as its own workspace root rather than walking up to
`/home/users/shuai/Cargo.toml` (whose `members = ["crates/*"]` would reject
QIndex). The release profile sets `lto = true`, `codegen-units = 1`,
`opt-level = 3` (`Cargo.toml:87-91`), which favors runtime speed at the cost
of slow link/codegen; for iteration use debug builds. See Chapter 10,
"Building, Running, and Deployment."

## Appendix D: Glossary

- **QIndex (project).** This system: a BibTeX-driven ranking tool with a CLI
  and an actix-web server.
- **QIndex (metric).** The per-scholar prestige score in `[0, 100]` computed
  in `calculate_scholar_scores` (`src/algorithm.rs:405-476`), normalized
  against the maximum scholar score. Distinct from the h-index.
- **Venue.** A publication forum (conference or journal), keyed by
  `normalize_venue_id` (uppercase, spaces/hyphens to `_`, apostrophes
  stripped; `src/parser.rs:489-495`). Modeled by `Venue`
  (`src/models.rs:33-44`).
- **Scholar.** An author identity, keyed by `generate_scholar_id` over the
  normalized display name (`src/parser.rs:497-503`). Modeled by `Scholar`
  (`src/models.rs:46-58`). Affiliations are declared but never populated.
- **Tier.** A coarse venue rank (`A*`, `A`, `B`, `C`) assigned by substring
  matching in `get_venue_tier` (`src/models.rs:359-389`); `A*` has 22 venues,
  `A` has 20. Tiers feed `tier_bonus` (`A*=2.0, A=1.5, B=1.2, C=1.0`).
- **CSRankings.** A community venue taxonomy (csrankings.org) whose subarea
  list QIndex mirrors in `get_venue_field` (`src/models.rs:391-468`) and whose
  venue allowlist gates scoring in `is_csrankings_venue`
  (`src/algorithm.rs:15-60`). Only CSRankings venues contribute to QIndex and
  h-index.
- **PageRank.** Brin and Page's link-analysis algorithm. QIndex has a venue
  PageRank implementation (`src/algorithm.rs:255-378`), but it is **dead code
  at runtime today** because the venue graph is always empty; a prestige
  fallback runs instead (`src/algorithm.rs:334-367`). See Chapter 5.
- **Damping factor.** PageRank's teleportation parameter `d`; default `0.85`
  in `AlgorithmParams` (`src/models.rs:108-125`). The random surfer follows a
  link with probability `d` and jumps to a uniformly random node with
  probability `1-d`.
- **S2AG.** Semantic Scholar Academic Graph; bulk-download corpus used to
  fetch per-paper citation counts. Loaded by `src/s2ag_citations.rs`.
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
   damping factor that QIndex's (currently inactive) venue ranking is modeled
   on.
2. **h-index.** J. E. Hirsch, "An index to quantify an individual's
   scientific research output," *PNAS* (2005). Background for QIndex's
   h-index computation (`src/algorithm.rs:486-512`), which today returns 0
   for all scholars because no `cited_by` data is populated.
3. **CSRankings.** E. Berger et al., CSRankings: Computer Science Rankings
   (csrankings.org). Source of the subarea taxonomy and venue allowlist that
   QIndex reproduces.
4. **Semantic Scholar / S2AG.** Semantic Scholar Academic Graph and its bulk
   datasets and API, Allen Institute for AI. Source of `s2ag_citations.json`.
   API access is keyed by `S2_API_KEY`.
5. **OpenAlex.** OpenAlex, OurResearch — an open index of scholarly works.
   One of the two backends for `combined_cache.json`.
6. **DBLP.** dblp computer science bibliography, Schloss Dagstuhl. Source of
   the conference corpus and of most citation-cache entries.

For implementation-level cross-references, see Chapter 3 ("The Data Model")
for the structs, Chapter 5 ("The Ranking Algorithm") for the scoring
formulas and the PageRank/fallback split, Chapter 6 ("Citation Data
Integration") for the competing pipelines, and Chapter 8 ("The Web Interface
and HTTP API") for which endpoints read which cache files.
