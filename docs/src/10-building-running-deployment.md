# 10. Building, Running, and Deployment

This chapter describes how to build QIndex from source, generate its static
website, preview it locally, and publish it to GitHub Pages. Throughout the book,
"QIndex" names both the project as a whole and the per-scholar quality metric it
computes; here we mean the project — the Rust binary, the generated site, and the
CI workflow that publishes it. This chapter is operations-focused: it covers the
toolchain, the release profile, the inputs the build reads, the deployment
pipeline, the development workflow, and the failure modes you are most likely to
hit. The data model (Chapter 3), the ranking algorithm (Chapter 5), the website
and its data files (Chapter 8), and the CLI surface (Chapter 9) are covered
elsewhere and are only referenced here.

There is no server to deploy. QIndex used to ship an actix-web server
(`qindex web`) that had to be kept running, bound to a host and port, and
secured. It has been replaced by `qindex build-site`, which writes a directory of
static files; publishing means uploading that directory to a static host.

## 10.1 Toolchain and Prerequisites

QIndex is a single Rust crate. Building it requires:

- A stable Rust toolchain (`cargo` + `rustc`). The crate uses `edition = "2021"`
  (`Cargo.toml:4`).
- A C linker and the system build essentials, since several transitive
  dependencies build native code.
- Network access on the first build, to fetch dependencies from crates.io.

Optional tools:

- **mdBook**, to render this book (`mdbook build docs` → `docs/book/`) so the
  site build can publish it under `book/`. CI downloads a pinned release; locally
  it is only needed if you want the book in your preview.
- **Python 3** for `scripts/check_site_links.py` (standard library only) and for
  a quick local preview server (`python3 -m http.server`).

With the web stack gone, `Cargo.lock` resolves 278 named packages (363 before).
The remaining heavy contributors are `tokio 1.35` with `features = ["full"]` and
`reqwest 0.11`, both used only by the citation/paper fetch subcommands, plus the
compile-time `maud 0.26` HTML macro and the unused `petgraph` and `ndarray`
(Chapter 7).

The other Python data-pipeline scripts under `scripts/` (DBLP fetchers, S2AG
download/parse) require Python 3 and `requests`. They are not part of the Rust
build and are not needed to build the site from the committed corpus. See
Chapter 6 (Citation Data Integration) for the pipeline details.

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
removing it reintroduces the hijack. (CI checks out the repository on its own, so
the table is harmless there.)

## 10.3 Building

For day-to-day development, use a debug build — it compiles quickly because
`[profile.dev]` sets `opt-level = 0` (`Cargo.toml:85-86`):

```bash
cargo build           # debug binary at target/debug/qindex
cargo run -- stats    # build + run a subcommand
```

For generating the full site, build with the release profile:

```bash
cargo build --release   # release binary at target/release/qindex (~6 MB)
```

### The release profile and its compile-time cost

`[profile.release]` is tuned for runtime performance at the expense of build
time (`Cargo.toml:80-83`):

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

The practical consequence is that release builds are slow, and rebuilds are slow
too: full LTO with no parallelism in final codegen is the dominant cost. Dropping
the actix stack shrank the dependency graph, but `tokio` (with `full`) and
`reqwest` remain. The book does not report a measured clean-build time — treat
"minutes, not seconds" as the expectation. A debug build is fine for checking
that the site renders, but the S2AG fuzzy title matching runs once per paper
during the build (Chapter 8) and is much faster optimized, so use `--release` for
full-corpus site builds. Test builds are kept fast: `[profile.test]` sets
`opt-level = 0` (`Cargo.toml:88-89`).

## 10.4 Generating the Site

The site generator is the `build-site` subcommand (`src/cli.rs`, `BuildSite`;
documented in Chapter 9):

```text
qindex build-site [-b ./bib] [-o ./site] [--base-url /]
                  [--static-dir ./static] [--book-dir ./docs/book]
```

| Flag | Default | Meaning |
| --- | --- | --- |
| `-b`, `--bib-dir` | `./bib` | BibTeX corpus to parse |
| `-o`, `--out-dir` | `./site` | output directory; replaced on every build |
| `--base-url` | `/` | URL path the site is served under (e.g. `/q-index/`) |
| `--static-dir` | `./static` | assets copied to `static/` (`app.js`, `style.css`) |
| `--book-dir` | `./docs/book` | rendered mdBook copied to `book/`; skipped with a warning if missing |

A typical local build and preview:

```bash
mdbook build docs                       # optional: include the book under /book/
cargo run --release -- build-site       # writes ./site with base URL "/"
python3 -m http.server -d site 8080     # browse http://localhost:8080/
```

The build prints a summary line (HTML/JSON/other file counts and total size); on
the reference corpus it was 1,030 HTML pages, 260 JSON files, and 59 other files,
about 62 MB, in about 13 seconds. It also prints a reminder when no rendered book
was found.

### The base URL must match where the site is served

Every internal link includes the base URL (Chapter 8), so a site built for one
path does not work at another. Build with the default `/` for a local preview at
the server root. To preview exactly what GitHub Pages will serve, build with the
project path and serve the output *under* that path, for example:

```bash
cargo run --release -- build-site --base-url /q-index/
mkdir -p /tmp/preview && ln -sfn "$PWD/site" /tmp/preview/q-index
python3 -m http.server -d /tmp/preview 8080   # http://localhost:8080/q-index/
```

Opening `site/index.html` directly from disk (`file://`) does not work: links are
absolute paths under the base URL, and browsers refuse the `fetch` calls that the
search and scholar pages make from `file://` pages. Always preview through an
HTTP server.

### Output directory safety

`build-site` deletes the output directory before writing. To prevent a mistyped
`--out-dir` from deleting something unrelated, it refuses to delete a non-empty
directory that lacks the `.nojekyll` marker that every build writes, and fails
with a "refusing to delete it" error instead (`prepare_out_dir`,
`src/site/mod.rs`).

## 10.5 Required Build Inputs

All build inputs are paths relative to the process working directory, so the
build is normally run from the repository root:

| Path | Purpose | If missing |
| --- | --- | --- |
| `./bib` | BibTeX corpus | the build fails with "Directory … does not exist" (overridable with `--bib-dir`) |
| `./static` | `app.js`, `style.css` | the build fails while copying assets (overridable with `--static-dir`) |
| `./cache/citations/s2ag_citations.json` | per-paper citation counts | loaded from this hardcoded relative path (`src/s2ag_citations.rs:153`); on error it logs and falls back to an empty index |
| `./docs/book` | rendered book for `book/` | skipped with a warning; the navbar's "Book" link then points nowhere and the link checker fails |

`--bib-dir`, `--static-dir`, and `--book-dir` relocate their inputs, but the S2AG
cache path is hardcoded relative to the CWD. If you run the build from anywhere
other than the repo root, the S2AG index loads empty without failing the build,
and every paper's "Citations: N (S2AG)" label becomes the internal
`cited_by.len()`, which is zero today (Chapter 6).

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
  committed, so a fresh clone (and CI) has working citation counts without
  re-running the download pipeline.

Do not confuse the repository's `data/` with the site's `site/data/`: the latter
is the generated JSON published with the website (Chapter 8), and like the rest
of `site/` it is gitignored.

The single artifact the site build consumes from the cache is
`cache/citations/s2ag_citations.json` (a dictionary keyed by normalized title;
see Chapter 6 for its schema and the other, partly orphaned, cache files in the
same directory). The `S2_API_KEY` for the download scripts is read from the
environment (`download_s2ag.py`) and templated in `.env.example`:

```bash
# .env.example
S2_API_KEY=your-api-key-here
```

There is no committed `.env` (it is gitignored), and no script auto-loads one —
export the variable yourself before running the fetchers, for example with
`set -a; source .env; set +a`.

## 10.7 Deployment to GitHub Pages

The site is published to <https://shuaimu.github.io/q-index/> by the GitHub
Actions workflow in `.github/workflows/pages.yml`. It runs on every push to `main`
and can be started by hand (`workflow_dispatch`). The `build` job:

1. checks out the repository and installs a stable Rust toolchain, with
   `Swatinem/rust-cache` caching compiled dependencies between runs;
2. downloads a pinned mdBook release (`MDBOOK_VERSION`);
3. runs `actions/configure-pages`, whose `base_path` output is the URL path of the
   Pages site (`/q-index` for this project site, empty for a custom domain);
4. `cargo build --release`, then the library unit tests and `tests/site_test.rs`
   (`cargo test --release --lib --test site_test`);
5. `mdbook build docs`;
6. `qindex build-site --out-dir site --base-url "<base_path>/"`;
7. `python3 scripts/check_site_links.py site "<base_path>/"` — any broken internal
   link fails the job, so a broken site is never deployed;
8. uploads `site/` with `actions/upload-pages-artifact`.

The `deploy` job then publishes the artifact with `actions/deploy-pages`. A
`concurrency` group ensures only one deployment runs at a time, and an in-progress
deployment is not cancelled by a newer push.

Taking the base path from `configure-pages` rather than hardcoding `/q-index/`
means the same workflow keeps working if the repository is renamed or given a
custom domain. The repository's Pages settings must use **GitHub Actions** as the
build source (rather than "Deploy from a branch"). Nothing is pushed to a
`gh-pages` branch, and `site/` is gitignored: the published site is always
rebuilt from the committed `bib/` and `cache/citations/`.

Everything on the published site is public: rankings, the full search index (all
scholar names and scores), and every scholar profile shard.

## 10.8 Development Workflow

The standard Rust quality gates apply. Run them before committing:

```bash
cargo fmt        # format (run before committing per project guidelines)
cargo clippy     # lint for common mistakes
cargo test       # run the test suite (opt-level=0, fast)
cargo build      # confirm the crate still parses and compiles
```

`tests/site_test.rs` builds a site from a three-paper fixture into a temporary
directory, under a non-root base path, and checks the generated files and links;
the unit tests in `src/site/mod.rs` pin base-path normalization, slugging, and the
scholar shard hash. Note that `tests/integration_test.rs` contains a long-standing
failing assertion (`test_get_venue_field` expects the field name "Systems",
which the code renamed to "Operating Systems"); CI runs only the library and site
tests for that reason.

When iterating on templates, `static/app.js`, or `static/style.css`, rebuild and
reload:

```bash
cargo run --release -- build-site && python3 -m http.server -d site 8080
# edit, re-run build-site, refresh the browser
python3 scripts/check_site_links.py site /     # same base URL as the build
```

`static/` is copied into the output on every build, so asset edits also need a
rebuild (or a manual copy into `site/static/` while experimenting).

After changing the BibTeX corpus, rebuild the site and sanity-check the counts. In
this book's reference session the corpus parsed to roughly 19,954 papers, 44
venues, and 43,942 scholars (`qindex stats`; also the `total_*` fields of
`site/data/stats.json`). The same file reports `total_citations = 0`, because it
is `graph.edges.len()` and the in-memory citation graph is essentially empty;
this is expected, not a build error (see Chapters 5 and 6). The CLI subcommands
(`calculate`, `venues`, `scholars`, `search`, `stats`) are the fastest way to
verify parsing without building the site — see Chapter 9.

One caveat for contributors who touch the data pipeline:
`scripts/sort_papers_by_year.py` hardcodes an absolute macOS path
(`/Users/shuai/workspace/qindex/bib`, `scripts/sort_papers_by_year.py:72`) that
does not exist on the Linux host. Edit that path before running it here.

## 10.9 Troubleshooting

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
site builds). Because of the release profile's full LTO and single codegen unit
(Section 10.3), a clean release rebuild is the slow case — plan for it.

### Build fails with a workspace error

A workspace-membership error at the very start of the build almost always means
the empty `[workspace]` table is missing or the crate was moved under another
workspace. Confirm `Cargo.toml:12` still contains `[workspace]` (Section 10.2).

### `build-site` says it is "refusing to delete" the output directory

The `--out-dir` exists, is not empty, and has no `.nojekyll` marker, so it does
not look like a previous site build. Check the path; if it really is an old build
from before the marker existed, delete it yourself and rerun (Section 10.4).

### Pages are unstyled, or every link 404s

The site was built for a different base URL than the one it is served under — for
example built with `--base-url /q-index/` and served at the server root, or the
reverse. Rebuild with the matching `--base-url`, or serve the output under the
path it was built for (Section 10.4). The link checker catches this when given the
same base URL the server uses.

### Search or scholar pages show "Failed to load …"

The browser could not fetch `data/search-index.json` or a `data/scholars/*.json`
shard. The usual causes are opening the site from `file://` instead of over HTTP,
or the base-URL mismatch above. The browser's developer console shows the failing
URL.

### Citation labels all say "(internal)"

The build ran from a directory other than the repository root, so
`cache/citations/s2ag_citations.json` was not found and the S2AG index loaded
empty (Section 10.5). Rebuild from the repository root.

### The "Book" link is broken, or the link checker fails on `book/`

No rendered book was found at `--book-dir` when the site was built. Run
`mdbook build docs` first, then rebuild the site.

### The deployment workflow fails at "Check links"

`scripts/check_site_links.py` found an internal link that leaves the base path,
points to a missing file, or names an unknown scholar; the log lists up to 50
broken links with the page that contains each. Reproduce locally by building with
`--base-url /q-index/` and running the checker with the same base.

## 10.10 Deployment Summary

Publishing is a `git push` to `main`; the workflow in Section 10.7 rebuilds,
tests, link-checks, and deploys the site. To produce the same output by hand:

```bash
# from a clean checkout, at the repository root
cargo build --release
mdbook build docs
./target/release/qindex build-site --base-url /q-index/
python3 scripts/check_site_links.py site /q-index/
# upload site/ to any static host that serves it under /q-index/
```

The generated directory is self-contained apart from the Bootstrap, Bootstrap
Icons, and Chart.js files it loads from the jsDelivr CDN, so any static host
works. There is no process to keep running, no port to bind, and no
authentication to configure — and correspondingly, everything in the build is
public once published. Remember that the rankings on the site are computed from
the internal BibTeX-derived graph and the S2AG count cache, not from a live
citation graph; the honest current-state limitations of those inputs are
documented in Chapter 11 (Limitations, Known Issues, and Roadmap).
