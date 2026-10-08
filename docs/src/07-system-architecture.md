# 7. System Architecture

This chapter describes how QIndex is put together as a program. "QIndex"
names both this project and the per-scholar quality metric it computes; in
this chapter "QIndex" refers to the project unless qualified as the scholar
metric. The goal here is to give an engineer enough of a map to read,
evaluate, and extend the code: the crate layout, the responsibilities of each
module, the end-to-end data flow from BibTeX files to a published website, the
third-party dependencies and why each is present, and the concurrency model.
Algorithm internals are covered in Chapter 5, "The Ranking Algorithm"; the data
model in Chapter 3, "The Data Model"; citation ingestion in Chapter 6,
"Citation Data Integration"; and the website and CLI surfaces in Chapters 8 and
9. This chapter cross-references those rather than restating them, and it is
deliberately honest about which paths are live, which are fallbacks, and which
are stubs.

QIndex has no server component. Earlier versions shipped an actix-web HTTP
server (`qindex web`) that parsed the corpus at startup and rendered pages per
request. That server has been removed. The project is now a *static site
generator*: `qindex build-site` computes every ranking once and writes a
directory of plain HTML and JSON files that any static host — in practice
GitHub Pages — serves as-is. The few interactive features (filters, search,
scholar profiles) run in the browser against JSON files produced by the same
build.

## 7.1 Crate layout

QIndex is a single binary crate. There is no Cargo workspace with member
crates; the `[workspace]` table in `Cargo.toml:12` is intentionally *empty*.
Its only purpose is to stop Cargo from walking up the directory tree and
attaching `qindex` to an unrelated parent manifest at `~/Cargo.toml`, which
declares `members = ["crates/*"]` for a different project. Without the empty
table, a build from this directory fails with a workspace-membership error
because `qindex` is not under that parent's `crates/*`. This is described
further in Chapter 10, "Building, Running, and Deployment."

Both `src/lib.rs` and `src/main.rs` declare the same eleven modules
(`src/lib.rs:1-11`, `src/main.rs:1-11`), so the code is reachable both as a
library and as the `qindex` binary. The modules and their responsibilities:

| Module | File | Responsibility |
| --- | --- | --- |
| `models` | `src/models.rs` | Core data types: `Paper`, `Venue`, `Scholar`, `CitationGraph`, ranking structs, `AlgorithmParams`; venue tier/field classification helpers. |
| `parser` | `src/parser.rs` | `BibParser` — reads `*.bib`, parses entries, builds the `CitationGraph`. |
| `algorithm` | `src/algorithm.rs` | `PageRankCalculator` — venue PageRank, scholar QIndex, h-index; CSRankings venue allowlist. |
| `citations` | `src/citations.rs` | Legacy async Semantic Scholar / CrossRef fetcher (`CitationFetcher`). Effectively unused for display today. |
| `s2ag_citations` | `src/s2ag_citations.rs` | `S2agCitations::load` — reads `cache/citations/s2ag_paper_citations.json` (S2AG counts keyed by paper id) for exact lookups. |
| `export` | `src/export.rs` | JSON / CSV export of rankings (`Exporter`). |
| `cli` | `src/cli.rs` | `clap`-derive command definitions (`Cli`, `Commands`). |
| `utils` | `src/utils.rs` | Shared helper functions. |
| `site` | `src/site/` | Static site generator: `mod.rs` (data assembly, URLs, file output) and `templates.rs` (`maud` page templates). |
| `paper_finder` | `src/paper_finder.rs` | Locate paper PDFs / metadata (ArXiv, CrossRef). Mostly stubbed. |
| `paper_extractor` | `src/paper_extractor.rs` | Download PDFs and extract references. Stubbed; no real extraction. |

The `site` module has two halves. `src/site/mod.rs` owns the build: it parses
the corpus, runs the ranking, assembles the per-page data, assigns URLs, and
writes files. `src/site/templates.rs` holds the `maud` templates that turn that
data into HTML. Next to the Rust code, `static/app.js` and `static/style.css`
are copied verbatim into the output and supply the browser-side behaviour
(Chapter 8).

### Declared-but-unused dependencies and stubbed modules

Two heavy dependencies are declared in `Cargo.toml` but have no `use`
statement and no call site anywhere under `src/`: `petgraph` (`Cargo.toml:20`)
and `ndarray` (`Cargo.toml:21`). They suggest an intended graph library and a
matrix-based PageRank, but neither is wired in. PageRank is implemented by hand
over `IndexMap`s in `src/algorithm.rs`, and the graph is the project's own
`CitationGraph` rather than a `petgraph` type. An extender should treat these as
available-but-unused rather than as load-bearing. (Removing them would shorten
the dependency tree; see Chapter 11, "Limitations, Known Issues, and Roadmap.")
`rayon` (`Cargo.toml:55`) is now in the same category. It briefly had one call
site — a parallel per-paper fuzzy S2AG lookup in the site generator — but S2AG
counts are now exact `HashMap` lookups by paper id (Chapter 6), so nothing calls
`par_iter` any more. The ranking itself was always sequential.

Two modules are stubs. `paper_finder` returns `Ok(None)` from `find_acm_paper`
(`src/paper_finder.rs:147-156`) and `find_on_author_page`
(`src/paper_finder.rs:240-248`); only ArXiv title lookup and CrossRef
DOI/metadata are functional. `paper_extractor` returns `Ok(None)` from its
USENIX and Google Scholar searches (`src/paper_extractor.rs:324-347`) and, most
importantly, `extract_references_from_pdf` logs `"PDF extraction not yet
implemented"` and returns an empty vector (`src/paper_extractor.rs:374-382`).
Consequently the `extract-papers` subcommand never recovers any references and
reported reference counts stay zero. The `citations` module
(`CitationFetcher`) is a third near-dead path: it reads and writes
`citations.json`, a file that does not exist under `cache/citations/`, and
neither the site generator nor its templates invoke it.

## 7.2 End-to-end data flow

The parsing and scoring pipeline is shared by the CLI and the site generator up
to the point where output is produced. It transforms a directory of `*.bib`
files into ranked venues and scholars; the site generator then renders those
into files.

```text
  bib/*.bib
     │  (BibParser::parse_directory)
     ▼
  nom_bibtex::Bibtex::parse  per file
     │  parse_entry → Paper (dropped unless title & authors non-empty)
     │  add_to_venue, add_scholars  (staging HashMaps in BibParser)
     ▼
  build_graph → CitationGraph
     │  papers / venues / scholars copied into IndexMaps
     │  build_citation_network  (derives cited_by from citations — empty today)
     ▼
  CitationGraph  (papers, venues, scholars, edges=[])
     │
     ├── CLI path: PageRankCalculator::new(&graph)
     │            .load_citation_cache("./cache/citations.json")  ← file absent
     │            .calculate()  → venue scores, scholar QIndex, h-index
     │            → get_top_venues / get_top_scholars → stdout / export
     │
     └── Site path (qindex build-site → build_site, src/site/mod.rs):
                  PageRankCalculator::new(&graph).calculate()
                    (load_citation_cache NOT called)
                  get_top_venues(all) / get_top_scholars(all)
                  Citations: S2agCitations::load(s2ag_paper_citations.json)
                    → per-paper counts; scholar/venue totals and h-indices
                    │
                    ├── templates.rs → HTML pages   (index, venues, venue/*, …)
                    ├── data/*.json                (search index, rankings, stats)
                    └── data/scholars/00..ff.json  (scholar profiles, 256 shards)
                          │
                          ▼
                  site/  ── static host (GitHub Pages) ── browser + static/app.js
```

Parsing is the substantive step. `BibParser::parse_directory`
(`src/parser.rs:32-97`) optionally reads `strings.bib` / `title.bib` for
`@string` macros, then walks the directory with `walkdir`, keeps files whose
extension is `bib` and whose name does not *contain* the substrings `title` or
`strings`, and parses each with `nom_bibtex`. `parse_entry`
(`src/parser.rs:151-219`) maps BibTeX tags onto a `Paper`, resolving the venue
through `expand_string` and `extract_venue_name`, and returns `None` (dropping
the entry) unless both the title and the author list are non-empty. Papers are
staged in plain `HashMap`s and then copied into the `CitationGraph`'s
insertion-ordered `IndexMap`s by `build_graph` (`src/parser.rs:390-409`).
Venues are keyed by `normalize_venue_id` (uppercased, spaces and hyphens to
underscores); scholars by `generate_scholar_id` applied to a normalized
display name. The exact field mappings, key derivations, and the resulting
struct shapes are the subject of Chapter 3.

Two facts about this flow shape everything downstream. First, `Paper.citations`
is never populated — no BibTeX tag writes it (`src/parser.rs:166-167`), so
`build_citation_network` (`src/models.rs:174-192`) has nothing to traverse and
`Paper.cited_by` stays empty as well. The `CitationGraph.edges` vector is
likewise initialized empty and never filled. The graph that the algorithm
receives therefore has no real citation edges. Second, because of this, the
PageRank power-iteration in `src/algorithm.rs` runs against an empty venue
graph and the code falls through to a *prestige fallback* that scores venues
from paper counts, a CSRankings membership bonus, and a name-hash perturbation.
This fallback is what produces the venue ordering you see today. Chapter 5
documents both the intended PageRank path and the fallback that actually runs.

The runtime numbers observed in the reference session reflect this pipeline:
the `bib/` corpus parses to roughly 19,954 papers, 44 venues, and about 43,942
scholars (`qindex stats`, and `total_*` in the generated `data/stats.json`),
with zero internal citation edges. These are observed values, not targets, and
they will drift as the corpus changes.

### Computed scores are now what the pages show

One consequence of the redesign is a behaviour *fix*. The ranking structs that
`PageRankCalculator` returns (`VenueRanking`, `ScholarRanking`) carry the
computed PageRank, QIndex, and h-index, but the calculator never writes those
values back into the `Venue.pagerank`, `Scholar.qindex`, or `Scholar.h_index`
fields of the graph. The old server's search results and scholar detail pages
read those graph fields, so they always showed 0. The site generator instead
looks every venue and scholar up in the computed rankings
(`venue_rank_by_id`, `scholar_by_id`, `src/site/mod.rs`) when it builds
the search index and the scholar shards. The rankings' own h-index (which
counts `cited_by`) is still 0, because internal citation edges are 0 in the
current corpus; the site replaces it, and each scholar's citation total, with
values derived from the S2AG per-paper counts described next. QIndex and
PageRank values are shown as computed.

### A separate, parallel citation path

The citation numbers shown on the site do not come from the graph at all. An
offline pipeline (`qindex export-papers`, then `scripts/match_s2ag.py`) matches
each parsed paper to Semantic Scholar and commits
`cache/citations/s2ag_paper_citations.json`, keyed by paper id (16,462 of 19,954
papers matched). `build_site` loads it through `S2agCitations::load` (path from
`SiteOptions::citations_file`) and wraps it in `Citations` (`src/site/mod.rs`):
`Citations::of(paper)` is an exact `HashMap` lookup returning
`PaperCitations { count, s2ag }` — the S2AG count when the paper was matched,
otherwise `paper.cited_by.len()`, which is zero. Pages render this as
`"N (S2AG)"`, or `"n/a"` for an unmatched paper. Every aggregate the site shows —
scholar totals and h-indices, venue totals, `data/stats.json`'s
`total_citations`, the dashboard and `/statistics/` — is derived from the same
per-paper counts, counting each Semantic Scholar paper once, so the site's
figures agree with one another. This is a wholly separate ingestion path from the
BibTeX graph: no citation *graph* was extracted from S2AG, so the ranking
algorithm still sees no citations. Chapter 6 describes the pipeline in full.

## 7.3 Key dependencies and rationale

After the server removal, QIndex's `Cargo.lock` resolves to 278 packages (down
from 363); the release binary is about 6 MB (down from about 10.5 MB). The
dependencies that matter architecturally:

| Crate | Role | Why it is here |
| --- | --- | --- |
| `clap` 4.4 (`derive`, `env`) | CLI | Derive-based subcommand parsing; the `Commands` enum maps directly to handlers. |
| `nom-bibtex` 0.5 / `nom` 7.1 | BibTeX parsing | `Bibtex::parse` produces the entries `parse_entry` consumes. |
| `indexmap` 2.1 | Data model | Insertion-ordered maps for papers/venues/scholars so output ordering is stable. |
| `serde` / `serde_json` 1.0 | Serialization | Derived on the model and export structs; JSON for caches and the site's `data/` files. |
| `csv` 1.3 | Export | CSV writer for the two-file CSV export. |
| `maud` 0.26 | Templating | Compile-time HTML macros; pages are Rust functions returning `Markup`, HTML-escaped by default. |
| `tokio` 1.35 (`full`) | Async runtime | Drives the async `fetch-citations` / `extract-papers` subcommands only. |
| `reqwest` 0.11 (`json`) | HTTP client | Outbound calls in the citation/paper fetchers. |
| `urlencoding` 2.1 | URLs | Encodes scholar ids into `scholar/?id=` links; also used by the fetchers. |
| `walkdir` 2.4 | I/O | Recursive `*.bib` discovery; copying `static/` and the book into the output. |
| `regex` 1.10 | Parsing | `@string` extraction and venue cleanup. |
| `chrono` 0.4 (`serde`) | Time | Timestamps on metrics, export metadata, the build date in the site footer. |
| `indicatif` 0.17 | UX | Per-file parse progress bars. |
| `comfy-table` 7.1, `colored` 2.1 | CLI output | Terminal tables and colored text. |
| `anyhow` / `thiserror` | Errors | Application and typed errors. |
| `petgraph`, `ndarray`, `rayon`, `lazy_static` | (declared, unused) | Intended graph/matrix support and parallelism; no call sites. |

`actix-web`, `actix-files`, `actix-session`, `futures`, and `mime` were removed
with the server, and `maud` no longer enables its `actix-web` feature. `once_cell`
went with the global S2AG index it backed. The
browser-side libraries — Bootstrap 5.3, Bootstrap Icons, and Chart.js 4.4 — are
not Rust dependencies at all; the page template links them from the jsDelivr
CDN (`src/site/templates.rs`).

## 7.4 Concurrency model

QIndex deliberately avoids a single global async runtime. There is no
`#[tokio::main]`. `main` (`src/main.rs`) initializes `env_logger`, parses the
CLI, and dispatches on `cli.command`. The six synchronous subcommands
(`Calculate`, `Venues`, `Scholars`, `Search`, `Stats`, `BuildSite`) run inline
on the main thread. The two asynchronous subcommands each construct their *own*
runtime:

```rust
Commands::FetchCitations { bib_dir, cache_dir, max_papers, venue } => {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        run_fetch_citations(&bib_dir, &cache_dir, max_papers, venue.as_deref()).await
    })?;
}
```

`ExtractPapers` follows the same pattern (`src/main.rs`). Each builds a
fresh multi-thread runtime on demand and blocks on it. This is simple and
self-contained — there is no shared executor — at the cost of per-invocation
runtime construction.

### The site build is a batch job

`build_site` (`src/site/mod.rs`) is a straight-line batch computation.
It parses the corpus once, runs `PageRankCalculator::calculate()` once
(propagating any error with `?`, where the old server discarded it with
`.ok()`), and then writes files sequentially, with no parallel section. S2AG
citation counts are loaded once from `s2ag_paper_citations.json` into a plain
`HashMap` owned by the build (`Citations`), and every lookup is an exact hit by
paper id; the old global `Lazy<RwLock<…>>` index and its linear fuzzy-title scan,
which the server repeated for every paper on every detail-page request, are
gone (Chapter 6).

There is no shared mutable state to protect after the build. The old server's
`AppState`, its `APP_STATE` `OnceCell`, the `RwLock`-guarded graph, and the
5-minute ranking cache are gone, and with them the lock-poisoning and
stale-cache failure modes described in earlier versions of this book. On the
reference host a release build of the full site takes about 13 seconds of wall
clock and peaks at roughly 280 MB of resident memory.

### Browser-side concurrency

What used to be server work for search and scholar pages now happens in the
reader's browser. `static/app.js` fetches JSON with `fetch` and memoizes the
search-index download in a single promise (`loadSearchIndex`), so the navbar autocomplete and the search page share one
request. Chapter 8 describes these pages in detail.

### Exposure

With no server there is nothing to bind and no `--host` flag. The published
site is world-readable wherever it is hosted: every ranking, the full search
index (all ~44k scholar names and scores), and every scholar shard are plain
files. This is the same data the old server exposed when bound to `0.0.0.0`,
now published deliberately.

## 7.5 Module-dependency overview

The diagram below shows compile-time dependencies between modules (which module
uses types or functions from which). Arrows point from user to provider.

```text
                    ┌─────────┐
                    │  main   │  (binary entry; dispatch)
                    └────┬────┘
        ┌───────────┬────┼─────────┬───────────────┐
        ▼           ▼    ▼         ▼               ▼
     ┌─────┐   ┌────────┐ ┌──────────┐      ┌───────────────┐
     │ cli │   │ parser │ │ algorithm│      │    site/      │
     └─────┘   └───┬────┘ └────┬─────┘      │ mod (build)   │
                   │           │            │ templates     │
                   ▼           ▼            └───────┬───────┘
                ┌────────────────┐                  │
                │     models     │◀─────────────────┤
                │ (Paper, Venue, │                  │
                │  Scholar,      │                  ▼
                │  CitationGraph)│        ┌──────────────────┐
                └───────┬────────┘        │ s2ag_citations   │
                        ▲                 │ (global RwLock,   │
        ┌───────────────┤                 │  per-paper count)│
        ▼               ▼                 └──────────────────┘
   ┌─────────┐    ┌─────────┐
   │ export  │    │  utils  │
   └─────────┘    └─────────┘

   Largely independent / stubbed:
   ┌───────────┐  ┌─────────────────┐  ┌───────────┐
   │ citations │  │ paper_extractor │──│paper_finder│
   │ (legacy)  │  │  (stub)         │  │  (stub)   │
   └───────────┘  └─────────────────┘  └───────────┘
```

`models` is the hub: `parser` builds its types, `algorithm` reads and scores
them, `export` and `site` consume the ranking structs it emits. `site` also
calls `parser` and `algorithm` directly (it runs the whole pipeline itself) and
reuses `generate_scholar_id` and `normalize_author_name` to link author names to
scholar pages. `s2ag_citations` is reached from `site` only, as a parallel source
of per-paper counts that bypasses `models`. The legacy `citations` fetcher and
the `paper_extractor`/`paper_finder` pair sit off to the side: they are
reachable from the `fetch-citations` and `extract-papers` subcommands but
contribute nothing to the rankings or the website, and the extractor/finder
backends are stubs.

## 7.6 Summary

QIndex is a single binary built around one in-memory data structure, the
`CitationGraph`. A `BibParser` reads `bib/*.bib` into that graph; a
`PageRankCalculator` scores venues and scholars from it; and two front ends —
a `clap` CLI and a static site generator (`qindex build-site`) — present the
results. The generator computes everything once and writes HTML and JSON that a
static host serves, so there is no server process, no shared mutable state,
and no runtime cache. The honest state of the system is unchanged by the
redesign: the citation graph carries no edges (the parser never populates
`Paper.citations`), so the published rankings come from a prestige fallback
rather than from PageRank, and per-paper citation counts arrive through a
separate S2AG cache. Two declared dependencies (`petgraph`, `ndarray`) and two
modules (`paper_finder`, `paper_extractor`) are present but inert. Subsequent
chapters drill into the algorithm (Chapter 5), citation integration
(Chapter 6), the website and its data files (Chapter 8), and the CLI
(Chapter 9).
