# 7. System Architecture

This chapter describes how QIndex is put together as a program. "QIndex"
names both this project and the per-scholar quality metric it computes; in
this chapter "QIndex" refers to the project unless qualified as the scholar
metric. The goal here is to give an engineer enough of a map to read,
evaluate, and extend the code: the crate layout, the responsibilities of each
module, the end-to-end data flow from BibTeX files to rankings, the third-party
dependencies and why each is present, and the concurrency model. Algorithm
internals are covered in Chapter 5, "The Ranking Algorithm"; the data model in
Chapter 3, "The Data Model"; citation ingestion in Chapter 6, "Citation Data
Integration"; and the HTTP and CLI surfaces in Chapters 8 and 9. This chapter
cross-references those rather than restating them, and it is deliberately
honest about which paths are live, which are fallbacks, and which are stubs.

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
| `s2ag_citations` | `src/s2ag_citations.rs` | Loads `cache/citations/s2ag_citations.json`; serves per-paper citation counts via a global `RwLock`. |
| `export` | `src/export.rs` | JSON / CSV export of rankings (`Exporter`). |
| `cli` | `src/cli.rs` | `clap`-derive command definitions (`Cli`, `Commands`). |
| `utils` | `src/utils.rs` | Shared helper functions. |
| `web` | `src/web/` | HTTP server: `server`, `handlers`, `templates`, `state`. |
| `paper_finder` | `src/paper_finder.rs` | Locate paper PDFs / metadata (ArXiv, CrossRef). Mostly stubbed. |
| `paper_extractor` | `src/paper_extractor.rs` | Download PDFs and extract references. Stubbed; no real extraction. |

The `web` module is itself split (`src/web/mod.rs`): `server` (process setup
and route table), `handlers` (request handlers), `templates` (`maud`-rendered
HTML), and `state` (shared `AppState`, the `APP_STATE` `OnceCell`, and the
ranking cache).

### Declared-but-unused dependencies and stubbed modules

Three heavy dependencies are declared in `Cargo.toml` but have no `use`
statement and no call site anywhere under `src/`: `petgraph` (`Cargo.toml:20`),
`ndarray` (`Cargo.toml:21`), and `rayon` (`Cargo.toml:55`). They suggest an
intended graph library, a matrix-based PageRank, and data-parallel iteration
respectively, but none of that is wired in. PageRank is implemented by hand
over `IndexMap`s in `src/algorithm.rs`, the graph is the project's own
`CitationGraph` rather than a `petgraph` type, and all iteration is sequential.
An extender should treat these as available-but-unused rather than as
load-bearing. (Removing them would shorten the dependency tree; see
Chapter 11, "Limitations, Known Issues, and Roadmap.")

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
`citations.json`, a file that does not exist under `cache/citations/`, and no
web handler or template invokes it at runtime.

## 7.2 End-to-end data flow

The live pipeline is the same for both the CLI and the web server, up to the
point where output is produced. It transforms a directory of `*.bib` files
into ranked venues and scholars.

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
     └── Web path: AppState holds Arc<RwLock<CitationGraph>>
                  get_or_calculate_venues / _scholars
                    → PageRankCalculator::new(&graph).calculate()
                    (load_citation_cache NOT called)
                  → handlers → maud HTML / JSON API
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

The runtime numbers observed this session reflect this pipeline: the `bib/`
corpus parses to roughly 19,954 papers, 44 venues, and about 43,942 scholars
(`GET /api/stats`), and that same endpoint reports `total_citations = 0`
because it returns `graph.edges.len()` and the edge list is empty. These are
observed values, not targets, and they will drift as the corpus changes.

### A separate, parallel citation path

The per-paper citation *counts* shown on detail pages do not come from the
graph at all. They come from `s2ag_citations`, which loads
`cache/citations/s2ag_citations.json` — a committed artifact of roughly 731
papers matched to Semantic Scholar with about 41,468 total citations — into a
global `Lazy<RwLock<S2AGCitationIndex>>` (`src/s2ag_citations.rs:152`). The web
templates call `get_real_citations(title)` and display `"N (S2AG)"` when the
lookup is non-zero, otherwise falling back to `paper.cited_by.len()`, which is
zero. This is a wholly separate ingestion path from the BibTeX graph; the S2AG
*graph* file (`s2ag_citation_graph.json`) is effectively empty (one or two
edges) and feeds nothing. The result is that several "total citations" numbers
in the system are computed over different paper sets and will not agree:
`/api/stats` reports `graph.edges.len()`, `/api/citation-status` sums
`combined_cache.json`, and detail pages show S2AG counts. Chapter 6 lays out
these competing pipelines in full.

## 7.3 Key dependencies and rationale

QIndex's `Cargo.lock` resolves to 363 packages; the release binary is about
10.5 MB. The dependencies that matter architecturally:

| Crate | Role | Why it is here |
| --- | --- | --- |
| `clap` 4.4 (`derive`, `env`) | CLI | Derive-based subcommand parsing; the `Commands` enum maps directly to handlers. |
| `nom-bibtex` 0.5 / `nom` 7.1 | BibTeX parsing | `Bibtex::parse` produces the entries `parse_entry` consumes. |
| `indexmap` 2.1 | Data model | Insertion-ordered maps for papers/venues/scholars so output ordering is stable. |
| `serde` / `serde_json` 1.0 | Serialization | Derived on the model and export structs; JSON for caches and the API. |
| `csv` 1.3 | Export | CSV writer for the two-file CSV export. |
| `actix-web` 4.4, `actix-files` 0.6 | Web server | HTTP server, routing, static files; `maud` integrates via its actix feature. |
| `maud` 0.26 (`actix-web`) | Templating | Compile-time HTML macros; pages are Rust functions returning `Markup`. |
| `tokio` 1.35 (`full`) | Async runtime | Drives actix and the async fetch subcommands. |
| `reqwest` 0.11 (`json`) | HTTP client | Outbound calls in the citation/paper fetchers. |
| `walkdir` 2.4 | I/O | Recursive `*.bib` discovery. |
| `regex` 1.10 | Parsing | `@string` extraction and venue cleanup. |
| `chrono` 0.4 (`serde`) | Time | Timestamps on metrics, cache freshness, export metadata. |
| `indicatif` 0.17 | UX | Per-file parse progress bars. |
| `comfy-table` 7.1, `colored` 2.1 | CLI output | Terminal tables and colored text. |
| `anyhow` / `thiserror` | Errors | Application and typed errors. |
| `once_cell` 1.19 | State | `OnceCell`/`Lazy` for `APP_STATE` and the S2AG index. |
| `petgraph`, `ndarray`, `rayon` | (declared, unused) | Intended graph/matrix/parallel support; no call sites. |

`actix-session` 0.9 is declared with the `cookie-session` feature, but the
server registers no session middleware in `start_server` (`src/web/server.rs`),
so sessions are not used. There is no authentication anywhere; binding to a
non-loopback address exposes the data openly (see 7.4).

## 7.4 Concurrency model

QIndex deliberately avoids a single global async runtime. There is no
`#[tokio::main]`. `main` (`src/main.rs:23-67`) initializes `env_logger`, parses
the CLI, and dispatches on `cli.command`. The five synchronous subcommands
(`Calculate`, `Venues`, `Scholars`, `Search`, `Stats`) run inline on the main
thread. The three asynchronous subcommands each construct their *own* runtime:

```rust
Commands::Web { bib_dir, host, port } => {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async { run_web_server(&bib_dir, &host, port).await })?;
}
```

`FetchCitations` and `ExtractPapers` follow the same pattern
(`src/main.rs:53-66`). Each builds a fresh multi-thread runtime on demand and
blocks on it. This is simple and self-contained — there is no shared executor —
at the cost of per-invocation runtime construction.

### Web server concurrency

Inside the web runtime, `start_server` (`src/web/server.rs:10`) parses the
bibliography *once* on startup (`.expect` panics if parsing fails), builds an
`Arc<AppState>`, and stores it in a process-global `OnceCell`:
`APP_STATE.set(state.clone()).expect(...)` (`src/web/server.rs:30-31`). It then
starts `HttpServer::new(...)` with `Logger` and `Compress` middleware and binds
`(host, port)` (`src/web/server.rs:62`). Note that the server does *not* attach
state via `App::app_data`; handlers reach it through the global `APP_STATE`.
The actix factory closure runs per worker thread; the server uses actix's
default worker count (one per logical CPU) since no `.workers(...)` is called.

State sharing is built on `RwLock`. `AppState` (`src/web/state.rs:8-13`) holds
three `Arc<RwLock<...>>` fields: the parsed `CitationGraph`, a `last_update`
timestamp, and a `Cache` of computed rankings. The graph is read-mostly: every
handler that needs data takes `state.graph.read().unwrap()`. Rankings are
computed lazily and memoized: `get_or_calculate_venues`/`_scholars`
(`src/web/state.rs:55-111`) return cached results when the cache is fresh and
already holds at least the requested count, otherwise they take the graph read
lock, run `PageRankCalculator::new(&*graph).calculate()`, store the top 100,
and stamp the cache. Freshness is a 5-minute window: `is_stale()` is true when
`(now - last_refresh).num_minutes() > 5` (`src/web/state.rs:31-34`). There is
no manual invalidation (`Cache::invalidate()` is never called), so the only way
to force a recompute is to wait out the timer or restart.

This model has two consequences worth flagging for extenders. First, the
`unwrap()` on every lock means a panic while holding a write lock poisons the
lock and turns subsequent requests into cascading panics. Second, the web
ranking path never calls `load_citation_cache`, so its `PageRankCalculator`
runs without the external citation cache that the CLI loads. The web UI and the
CLI `calculate`/`venues`/`scholars` commands can therefore produce different
rankings from the same corpus. Both observations are expanded in Chapter 8,
"The Web Interface and HTTP API."

### Binding and exposure

The server binds `127.0.0.1` by default. A `--host` flag (long-only, default
`"127.0.0.1"`, `src/cli.rs:96-97`) is threaded through
`main.rs` → `run_web_server` → `start_server` → `.bind((host, port))`, so
`qindex web --host 0.0.0.0` exposes it on the network. Because there is no
authentication, doing so publishes the full dataset and API to anyone who can
reach the port. The `--host` flag and the empty `[workspace]` table describe
the current code state and may be uncommitted as of this writing.

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
     │ cli │   │ parser │ │ algorithm│      │     web/      │
     └─────┘   └───┬────┘ └────┬─────┘      │ server        │
                   │           │            │ handlers      │
                   ▼           ▼            │ templates     │
                ┌────────────────┐         │ state         │
                │     models     │◀────────┴───────┬───────┘
                │ (Paper, Venue, │                 │
                │  Scholar,      │                 ▼
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
them, `export` and the `web` handlers consume the ranking structs they emit.
`s2ag_citations` is reached from `web/templates` only, as a parallel source of
per-paper counts that bypasses `models`. The legacy `citations` fetcher and the
`paper_extractor`/`paper_finder` pair sit off to the side: they are reachable
from the `fetch-citations` and `extract-papers` subcommands but contribute
nothing to the live ranking or display paths, and the extractor/finder backends
are stubs.

## 7.6 Summary

QIndex is a single binary built around one shared in-memory data structure, the
`CitationGraph`. A `BibParser` reads `bib/*.bib` into that graph; a
`PageRankCalculator` scores venues and scholars from it; and two front ends —
a `clap` CLI and an actix-web server — present the results. The honest state
of the system is that the citation graph carries no edges (the parser never
populates `Paper.citations`), so the published rankings come from a prestige
fallback rather than from PageRank, and per-paper citation counts arrive
through a separate S2AG cache. Several declared dependencies (`petgraph`,
`ndarray`, `rayon`) and two modules (`paper_finder`, `paper_extractor`) are
present but inert. The concurrency model is straightforward: per-subcommand
tokio runtimes, default actix workers, and `RwLock`-guarded shared state with a
5-minute ranking cache and no authentication. Subsequent chapters drill into
the algorithm (Chapter 5), citation integration (Chapter 6), the web/API
surface (Chapter 8), and the CLI (Chapter 9).
