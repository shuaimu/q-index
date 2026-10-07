# 10. Building, Running, and Deployment

This chapter describes how to build QIndex from source, run its web server and
command-line interface, and deploy it on a single host. Throughout the book,
"QIndex" names both the project as a whole and the per-scholar quality metric it
computes; here we mean the project — the Rust binary and its runtime assets.
This chapter is operations-focused: it covers the toolchain, the release
profile, runtime asset layout, the development workflow, and the failure modes
you are most likely to hit. The data model (Chapter 3), the ranking algorithm
(Chapter 5), the web interface and HTTP API (Chapter 8), and the CLI surface
(Chapter 9) are covered elsewhere and are only referenced here.

## 10.1 Toolchain and Prerequisites

QIndex is a single Rust crate. Building it requires:

- A stable Rust toolchain (`cargo` + `rustc`). The crate uses `edition = "2021"`
  (`Cargo.toml:4`).
- A C linker and the system build essentials, since several transitive
  dependencies build native code.
- Network access on the first build, to fetch dependencies from crates.io.

The dependency footprint is substantial. `Cargo.lock` resolves 363 named
packages. The heavy contributors are the web stack and async runtime —
`actix-web 4.4`, `actix-files 0.6`, `actix-session 0.9`, `tokio 1.35` with
`features = ["full"]`, `reqwest 0.11`, and the `maud 0.26` compile-time HTML
macro — alongside `petgraph`, `ndarray`, `rayon`, and `comfy-table`
(`Cargo.toml:14-75`). A release build of the whole crate plus its dependency
graph is correspondingly large.

The optional Python data-pipeline scripts under `scripts/` (DBLP fetchers,
S2AG download/parse) require Python 3 and `requests`. They are not part of the
Rust build and are not needed to run the server against the committed corpus.
See Chapter 6 (Citation Data Integration) for the pipeline details.

## 10.2 The Workspace Gotcha

Before the first build will succeed, one structural issue must be understood.
A parent manifest at `/home/users/shuai/Cargo.toml` declares a Cargo workspace:

```toml
[workspace]
members = ["crates/*"]
resolver = "2"
```

This belongs to an unrelated project (its `[workspace.dependencies]` references
axum, sqlx, argon2, and similar — none of which QIndex uses). Cargo resolves
workspace membership by walking up the directory tree from the crate being
built. Because the QIndex checkout sits under that parent directory but is not
matched by `crates/*`, an unguarded build walks up to the parent manifest and
fails with a workspace-membership error.

The fix is in `Cargo.toml:10-12`: QIndex declares its own empty `[workspace]`
table.

```toml
# Independent workspace root so cargo does not attach qindex to an unrelated
# workspace manifest in a parent directory (e.g. ~/Cargo.toml).
[workspace]
```

An empty `[workspace]` makes the crate an independent workspace root, so Cargo
stops walking upward and never sees the parent manifest. If you copy this crate
into a directory that is itself inside another Cargo workspace, keep this table;
removing it reintroduces the hijack.

## 10.3 Building

For day-to-day development, use a debug build — it compiles quickly because
`[profile.dev]` sets `opt-level = 0` (`Cargo.toml:92-93`):

```bash
cargo build           # debug binary at target/debug/qindex
cargo run -- stats    # build + run a subcommand
```

For a production server, build with the release profile:

```bash
cargo build --release   # release binary at target/release/qindex (~10.5 MB)
```

### The release profile and its compile-time cost

`[profile.release]` is tuned for runtime performance at the expense of build
time (`Cargo.toml:87-90`):

```toml
[profile.release]
lto = true
codegen-units = 1
opt-level = 3
```

- `lto = true` enables full (whole-program) link-time optimization across the
  entire dependency graph.
- `codegen-units = 1` forces a single codegen unit, removing intra-crate
  parallelism in the final codegen and link stage.
- `opt-level = 3` requests maximum optimization.

The practical consequence is that release builds are slow, and rebuilds are
slow too: full LTO over 363 dependencies (including `actix`, `tokio` with the
`full` feature set, and `reqwest`) with no parallelism in final codegen is the
dominant cost. The book does not report a measured build time — treat "minutes,
not seconds" as the expectation and budget accordingly. For fast iteration,
prefer the debug profile (`cargo run -- ...`); reserve `--release` for actual
deployment or performance measurement. Test builds are also kept fast:
`[profile.test]` sets `opt-level = 0` (`Cargo.toml:95-96`).

## 10.4 Running the Web Server

The web server is the `web` subcommand. Its CLI surface is defined in
`src/cli.rs:90-102`: `--bib-dir/-b` (default `./bib`), `--host` (long-only,
default `127.0.0.1`), and `--port/-p` (default `8080`). The flags are threaded
through `main.rs:46-52` → `run_web_server` (`src/main.rs:341`) →
`web::server::start_server` (`src/web/server.rs:10`), which ultimately calls
`.bind((host, port))` (`src/web/server.rs:62`).

### Local-only (default)

```bash
./target/release/qindex web                 # binds 127.0.0.1:8080
./target/release/qindex web -p 9000         # custom port
```

Binding to `127.0.0.1` means the server is reachable only from the local host.
This is the default and the safe choice for a development laptop.

### Over the network

```bash
./target/release/qindex web --host 0.0.0.0 -p 8080
```

The `--host` flag is a recent addition; passing `0.0.0.0` binds all interfaces
and exposes the server on the LAN. The startup path parses the bib directory
with `BibParser::parse_directory` and `.expect`s on failure
(`src/web/server.rs:14-16`), then stores the application state in a global
`OnceCell` via `APP_STATE.set(...).expect(...)` (`src/web/server.rs:31`); both
panic on failure rather than returning a clean error.

> Security implications. QIndex has no authentication, no authorization, and
> no rate limiting. Every route — the HTML pages and all JSON endpoints under
> `/api/*` — is served to any client that can reach the bound address. Exposing
> the server with `--host 0.0.0.0` therefore publishes the entire corpus and
> all computed rankings to the network with no access control. Do not run it on
> an untrusted network without putting an authenticating reverse proxy in front
> of it. Note also that `GET /api/citation-status` shells out
> (`ps aux | grep ... fetch_citations`, `src/web/handlers.rs:766-768`) and
> calls `std::fs::read_dir(bib_dir).unwrap()` (`src/web/handlers.rs:730`), which
> panics the request worker if `bib/` is unreadable; see Chapter 8 for the full
> endpoint behavior.

## 10.5 Required Runtime Assets

All of the server's runtime paths are relative to the process working
directory, so the server is normally launched from the repository root. The
required assets are:

| Path | Purpose | If missing |
| --- | --- | --- |
| `./bib` | BibTeX corpus parsed at startup | `parse_directory` `.expect` panics at boot (`src/web/server.rs:14-16`) |
| `./static` | `app.js`, `style.css`, `live-reload.js` | served via `Files::new("/static", "./static")` (`src/web/server.rs:60`); no files served if absent |
| `./cache/citations/s2ag_citations.json` | per-paper citation counts | loaded from this hardcoded relative path (`src/s2ag_citations.rs:153`); on error, logs and falls back to an empty index |

The `--bib-dir` flag can relocate the corpus, but `./static` and the S2AG cache
path are hardcoded relative to the CWD. If you launch the binary from anywhere
other than the repo root, static assets silently 404 and the S2AG index loads
empty, so per-paper "Citations: N (S2AG)" badges disappear (the UI then falls
back to the internal `cited_by.len()`, which is effectively zero today — see
Chapter 6). The `static/` directory contains exactly three files: `app.js`,
`style.css`, and `live-reload.js`. The latter is injected only when the
`RUST_ENV` environment variable is not `"production"` (`src/web/templates.rs:167`).

The S2AG cache feeds per-paper citation *counts* only; the citation *graph* is
effectively empty, so it does not feed the PageRank computation. That divergence
and the multiple citation pipelines behind it are the subject of Chapter 6.

## 10.6 Data Layout: `data/` versus `cache/`

QIndex keeps raw inputs and derived artifacts in two separate trees with
different version-control policies:

- `data/` — raw bulk downloads (the S2AG dataset and any S2ORC placeholders).
  This tree is gitignored and not committed. The raw S2AG download described in
  the project notes is large; only the *derived* results are tracked.
- `cache/citations/` — the parsed and derived JSON artifacts. These are
  committed, so a fresh clone has working citation counts without re-running the
  download pipeline.

The single artifact the Rust web path actually consumes is
`cache/citations/s2ag_citations.json` (a dictionary keyed by normalized title;
see Chapter 6 for its schema and the other, partly orphaned, cache files in the
same directory). The `S2_API_KEY` for the download scripts is read from the
environment (`download_s2ag.py`) and templated in `.env.example`:

```bash
# .env.example
S2_API_KEY=your-api-key-here
```

There is no committed `.env`, and no script auto-loads one — export the variable
yourself before running the fetchers. (A real-looking sample key is, regrettably,
checked into `CLAUDE.md`; treat that as a secret-in-VCS issue, not a key to
rely on.)

## 10.7 Development Workflow

The standard Rust quality gates apply. Run them before committing:

```bash
cargo fmt        # format (run before committing per project guidelines)
cargo clippy     # lint for common mistakes
cargo test       # run the test suite (opt-level=0, fast)
cargo build      # confirm the crate still parses and compiles
```

When iterating on the web UI, run the debug server and reload the browser; the
`live-reload.js` asset is injected automatically in non-production mode
(`RUST_ENV != "production"`, `src/web/templates.rs:167`). A typical loop is:

```bash
cargo run -- web              # debug build, 127.0.0.1:8080
# edit templates / static assets, rebuild, refresh
```

After changing the BibTeX corpus, rebuild and re-run, then sanity-check the
counts. As observed against the running server in this book's reference session,
the corpus parses to roughly 19,954 papers, 44 venues, and 43,942 scholars
(`GET /api/stats`). Note that the same endpoint reports `total_citations = 0`,
because it returns `graph.edges.len()` and the in-memory citation graph is
essentially empty; this is expected, not a build error (see Chapters 5 and 6).
The CLI subcommands (`calculate`, `venues`, `scholars`, `search`, `stats`) are
the fastest way to verify parsing without the web stack — see Chapter 9.

One caveat for contributors who touch the data pipeline:
`scripts/sort_papers_by_year.py` hardcodes an absolute macOS path
(`/Users/shuai/workspace/qindex/bib`, `scripts/sort_papers_by_year.py:72`) that
does not exist on the Linux host. Edit that path before running it here.

## 10.8 Troubleshooting

### `Exec format error`

If running `./target/release/qindex` (or the debug binary) fails with
`cannot execute binary file: Exec format error`, the binary on disk was built
for a different platform/architecture than the host. This happens when a
`target/` directory is carried over from another machine (for example a macOS
build copied to this Linux host). The fix is to discard the stale artifacts and
rebuild on the target machine:

```bash
cargo clean
cargo build --release
```

### Rebuild after a toolchain change

The compiled binary embeds platform- and toolchain-specific code. After
switching Rust toolchains, moving the checkout to a different OS or
architecture, or pulling changes that alter dependency versions, rebuild from a
clean state (`cargo clean && cargo build` for debug, or `--release` for
deployment). Because of the release profile's full LTO and single codegen unit
(Section 10.3), a clean release rebuild is the slow case — plan for it.

### Build fails with a workspace error

A workspace-membership error at the very start of the build almost always means
the empty `[workspace]` table is missing or the crate was moved under another
workspace. Confirm `Cargo.toml:12` still contains `[workspace]` (Section 10.2).

### Server starts but pages are blank or citation badges are gone

The server was almost certainly launched from a directory other than the repo
root. `./static` and `cache/citations/s2ag_citations.json` are resolved relative
to the CWD; relaunch from the repository root (Section 10.5).

### Server panics at startup

`parse_directory(...).expect(...)` (`src/web/server.rs:14-16`) and
`APP_STATE.set(...).expect(...)` (`src/web/server.rs:31`) panic rather than
returning errors. A startup panic therefore points to an unparseable/missing
bib directory or a double-initialization of the global state, not to a logic
bug deeper in the request path.

## 10.9 Deployment Summary

A minimal single-host deployment is:

```bash
# from a clean checkout, at the repository root
cargo build --release
./target/release/qindex web --host 0.0.0.0 -p 8080
```

This is sufficient for an internal, trusted-network deployment. For anything
beyond that, place an authenticating, TLS-terminating reverse proxy in front of
the binary, since QIndex itself provides no authentication, no TLS, and no rate
limiting. Keep the server's working directory at the repository root so the
relative asset paths (`./bib`, `./static`, `./cache/citations/...`) resolve, and
remember that the rankings shown over HTTP are computed from the internal
BibTeX-derived graph and the S2AG count cache, not from a live citation graph —
the honest current-state limitations of those inputs are documented in
Chapter 11 (Limitations, Known Issues, and Roadmap).
