# 9. The Command-Line Interface

QIndex is, first and foremost, a command-line program. The same binary that
generates the static website (Chapter 8, *The Static Website and Its Data Files*)
also exposes a set of subcommands for computing rankings, exporting them, inspecting
the corpus, and driving the (mostly experimental) citation- and
paper-extraction pipelines. This chapter documents every subcommand, its flags,
its defaults, and — importantly — what each one actually does today versus what
it is nominally intended to do.

A note on terminology: "QIndex" names both this project and the per-scholar
score the project computes (`Scholar.qindex`). When the distinction matters,
"the QIndex metric" refers to the score and "QIndex" refers to the tool. The
ranking math behind the metric is described in Chapter 5, *The Ranking
Algorithm*; here we only describe how the CLI surfaces it.

## 9.1 Structure and dispatch

The CLI is built with `clap` 4.4 (derive plus `env` features). The root parser
is a single `Cli` struct holding one `#[command(subcommand)]` field
(`src/cli.rs:3-15`); the program is named `qindex` and reports version
`0.1.0`. There are eight subcommands (`src/cli.rs:18-149`):

| Subcommand        | Purpose                                              | State today                |
|-------------------|-----------------------------------------------------|----------------------------|
| `calculate`       | Compute and print top venues and scholars; optional export | Functional                 |
| `venues`          | Print top venues, with field/tier filters           | Functional                 |
| `scholars`        | Print top scholars, with a minimum-papers filter    | Functional                 |
| `search`          | Substring search over venues and scholars           | Functional                 |
| `stats`           | Print dataset statistics                            | Functional                 |
| `build-site`      | Generate the static website                        | Functional                 |
| `fetch-citations` | Call live Semantic Scholar API for citation counts  | Functional but slow / live |
| `extract-papers`  | Find PDFs and extract references                    | Largely stubbed            |

Every subcommand accepts `-b`/`--bib-dir`, defaulting to `./bib`
(for example `src/cli.rs:22-23`). All file and cache paths are resolved
relative to the process working directory, so QIndex is intended to be run from
the repository root; running it elsewhere will fail to find `./bib` and `./static`
and will silently miss the citation caches under `./cache`.

`main()` initializes `env_logger` with a default filter of `info`
(`src/main.rs:25`) and then matches on `cli.command` (`src/main.rs:30-93`). The
six synchronous subcommands (`calculate`, `venues`, `scholars`, `search`,
`stats`, `build-site`) run directly. The two async ones (`fetch-citations`,
`extract-papers`) each construct their own
`tokio::runtime::Runtime::new()?` and `block_on` the work
(`src/main.rs:76,88`); there is no top-level `#[tokio::main]` and no shared
executor.

### Logging with `RUST_LOG`

Because the logger reads the environment, `RUST_LOG` overrides the default
`info` level. This is the primary way to see what the parser, ranking, and
citation code are doing:

```bash
# Default: info-level messages
qindex stats

# Verbose: see per-file parsing, venue-name logging, cache attempts
RUST_LOG=debug qindex calculate

# Quiet: warnings and errors only (e.g. BibTeX parse failures)
RUST_LOG=warn qindex venues -n 30
```

Parse failures on individual `.bib` files are logged as warnings, not fatal
errors (`src/parser.rs:116-149`), so `RUST_LOG=warn` is useful when auditing a
freshly fetched corpus.

## 9.2 The ranking subcommands

The four read-only ranking subcommands (`calculate`, `venues`, `scholars`,
`search`) share a common prologue: parse `bib_dir` into a `CitationGraph`,
construct a `PageRankCalculator`, attempt to load a citation cache from the
hardcoded path `./cache/citations.json`, then call `calculate()`
(`src/main.rs:116-120,145-149,163-167,181-185`).

A critical caveat applies to all of them. That cache file does not exist in the
repository — only the `cache/citations/` *directory* is present — so
`load_citation_cache` loads nothing (`src/algorithm.rs:84-97`). Combined with
the fact that the parser never populates `Paper.citations`
(`src/parser.rs:166-167`), the venue citation graph is empty and the PageRank
power iteration is dead code. What actually runs is the prestige fallback
described in Chapter 5: venue scores derive from a log of paper counts, a
CSRankings 2x multiplier, a small name-hash perturbation, and the tier bonus.
The numbers printed by these subcommands are this fallback score, not a true
citation-based PageRank. They can also differ from the website, whose build never calls
`load_citation_cache` at all (Section 9.7).

### `calculate`

```
qindex calculate [-b ./bib] [-n 20] [-o <file>] [-e]
```

Flags (`src/cli.rs:20-36`):

- `-n`/`--top` (usize, default `20`): number of top venues and scholars to print.
- `-o`/`--output` (`Option<String>`, no default): export destination.
- `-e`/`--export` (bool flag): enable export.

`calculate` prints a load summary, computes scores, then prints the top venues
and top scholars as `comfy_table` tables (`src/main.rs:79-99`). Export happens
only when **both** `-e` is passed and `-o` is set:
`if export && output.is_some()` (`src/main.rs:102`). Passing `-e` alone is a
silent no-op — nothing is written. The export format is chosen by the output
file's extension (see §9.4).

```bash
# Print top 20, no export
qindex calculate

# Print top 50 and write a JSON export
qindex calculate -n 50 -o out.json -e

# WRONG: -e without -o exports nothing, no error
qindex calculate -e
```

### `venues`

```
qindex venues [-b ./bib] [-n 20] [-f <field>] [-t <tier>]
```

Flags (`src/cli.rs:39-55`): `-n`/`--top` (default `20`), `-f`/`--field`
(`Option<String>`), `-t`/`--tier` (`Option<String>`, e.g. `A*`, `A`, `B`, `C`).
The field and tier values are forwarded to `get_top_venues(top, field, tier)`
(`src/main.rs:123`); the field filter is a case-insensitive substring match on
`Venue.field` and the tier filter is an exact match. The result is printed by
`display_venues`, whose table columns are `Venue, Tier, Field, PageRank, Papers`
with PageRank formatted as `{:.4}` (`src/main.rs:238-256`).

```bash
qindex venues -n 30 -t 'A*'
qindex venues -f Systems
qindex venues -n 40 -f Database -t A
```

Note that `--tier` matches the venue's classified tier string
(`get_venue_tier`, see Chapter 3, *The Data Model*), and `--field` matches the
classified CSRankings subarea string; both are substring/exact comparisons, so
the spelling must match what the classifier produced.

### `scholars`

```
qindex scholars [-b ./bib] [-n 20] [-m <min>]
```

Flags (`src/cli.rs:58-70`): `-n`/`--top` (default `20`), `-m`/`--min-papers`
(`Option<usize>`). The minimum-papers value is passed to
`get_top_scholars(top, min_papers)` (`src/main.rs:141`), which drops scholars
whose CSRankings-venue paper count falls below the threshold. Output columns are
`Scholar, QIndex, H-Index, Papers, Citations`, QIndex formatted as `{:.2}`
(`src/main.rs:258-276`). Because no per-paper `cited_by` data is populated
through the parser path, the `H-Index` and `Citations` columns are `0` for every
scholar in the current build (Chapter 5).

```bash
qindex scholars -n 50 -m 5
```

### `search`

```
qindex search [-b ./bib] <query>
```

`query` is a required positional argument (`src/cli.rs:73-80`). The handler runs
`graph.search_venues(query)` and `graph.search_scholars(query)`, prints up to 10
matches of each (`src/main.rs:189-207`), and falls back to a "No results found"
message when both are empty. Search is a substring match over names; it does not
rank by relevance. Although the handler runs `calculate()` first, the two search
methods copy PageRank, QIndex, and h-index from the graph's own score fields,
which the calculator never fills in (Chapter 3), so the scores printed here are 0.
The website's search (Chapter 8) uses the same matching rules over an index built
from the computed rankings, so it shows the real scores.

```bash
qindex search "distributed systems"
qindex search SOSP
```

### `stats`

```
qindex stats [-b ./bib]
```

`stats` takes only `--bib-dir` (`src/cli.rs:83-87`). It parses the corpus and
calls `CitationGraph::print_statistics` (`src/main.rs:182-188`,
`src/models.rs:253-330`), which prints totals (papers, venues, scholars), a
breakdown of papers by `VenueType`, by tier, and by field (the field breakdown
is a top-10 `comfy_table`), the observed year range, and averages for papers per
scholar and authors per paper. Observed on the current corpus, parsing yields
roughly 19,954 papers, 44 venues, and 43,942 scholars. Unlike the other ranking
subcommands, `stats` does not construct a `PageRankCalculator` and so does no
scoring.

## 9.3 Terminal output

All tabular output uses the `comfy_table` crate with the `UTF8_FULL` preset,
which draws boxed tables with Unicode borders (`src/main.rs:192-275`,
`src/models.rs:254`). The headers are fixed per command:

- `calculate` venues table: `Rank, Venue, Tier, Field, PageRank, Impact, Papers`
  (`src/main.rs:198`); PageRank and Impact formatted `{:.4}`.
- `calculate` scholars table: `Rank, Scholar, QIndex, H-Index, Papers, Citations`
  (`src/main.rs:222`); QIndex formatted `{:.2}`.
- `venues` table: `Venue, Tier, Field, PageRank, Papers` (no rank column).
- `scholars` table: `Scholar, QIndex, H-Index, Papers, Citations`.

Surrounding the tables, the subcommands print decorated status lines using emoji
prefixes (for example `"📚 Loaded N papers, ..."`, `"🔄 Calculating ..."`,
`"✅ Calculation complete!"` at `src/main.rs:79-99`). These are cosmetic; the
machine-readable output is the export file (§9.4), not the terminal table.

## 9.4 Export formats

Export is implemented in `src/export.rs`. The `Exporter::export` entry point
selects the format purely from the output file's **extension**
(`src/export.rs:72-82`):

- `.json` → a single pretty-printed JSON file.
- `.csv` → **two** files written next to the named output: `{stem}_venues.csv`
  and `{stem}_scholars.csv` (`src/export.rs:99-115`).
- any other extension → silently falls back to JSON (`src/export.rs:80`).

The CSV behavior is worth flagging: passing `-o results.csv` does not produce
`results.csv`; it produces `results_venues.csv` and `results_scholars.csv` in
the same directory. The file you named is never created.

### JSON schema

The JSON document has four top-level keys: `metadata`, `venues`, `scholars`,
`statistics` (`src/export.rs:16-22`).

```jsonc
{
  "metadata": {
    "version": "0.1.0",
    "algorithm": "PageRank-based QIndex",
    "date": "<RFC 3339 UTC timestamp>",
    "parameters": {            // AlgorithmParams::default()
      "damping_factor": 0.85,
      "max_iterations": 100,
      "tolerance": 1e-6,
      "venue_weight": 0.7,
      "year_decay": 0.95,
      "tier_bonus": { "A*": 2.0, "A": 1.5, "B": 1.2, "C": 1.0 }
    }
  },
  "venues": [
    { "rank": 1, "id": "...", "name": "...", "tier": "...", "field": "...",
      "pagerank": 0.0, "impact_factor": 0.0, "paper_count": 0 }
  ],
  "scholars": [
    { "rank": 1, "id": "...", "name": "...", "qindex": 0.0, "h_index": 0,
      "paper_count": 0, "citation_count": 0, "top_venues": ["..."] }
  ],
  "statistics": {
    "total_papers": 0, "total_venues": 0, "total_scholars": 0,
    "papers_by_year":  { "2024": 0 },
    "papers_by_tier":  { "A*": 0 },
    "papers_by_field": { "Operating Systems": 0 }
  }
}
```

(Field types come from `VenueExport`, `ScholarExport`, `DatasetStatistics` and
`ExportMetadata` at `src/export.rs:24-64`; the example values are illustrative
placeholders, not observed output.) Note that `metadata.parameters` always
serializes `AlgorithmParams::default()` (`src/export.rs:203`), so it reflects
the default configuration regardless of any run-time tuning, and that the
`damping_factor`/`max_iterations`/`tolerance` it advertises belong to the
PageRank path that does not actually run today (Chapter 5). The
`papers_by_tier` and `papers_by_field` counts are sums of `Venue.papers.len()`
per tier/field (`src/export.rs:224-228`).

### CSV schema

The venues CSV header is `Rank, Venue, Tier, Field, PageRank, Impact Factor,
Papers`, with PageRank and Impact Factor formatted `{:.6}`
(`src/export.rs:122-135`). The scholars CSV header is `Rank, Scholar, QIndex,
H-Index, Papers, Citations`, with QIndex formatted `{:.2}`
(`src/export.rs:147-159`). Both are written with the `csv` crate's `Writer`.

```bash
# JSON
qindex calculate -n 100 -o rankings.json -e

# CSV -> writes rankings_venues.csv and rankings_scholars.csv
qindex calculate -n 100 -o rankings.csv -e

# Unknown extension -> JSON content written to a file named rankings.txt
qindex calculate -n 100 -o rankings.txt -e
```

## 9.5 `fetch-citations`

```
qindex fetch-citations [-b ./bib] [-c ./cache] [-m <max>] [-v <venue>]
```

Flags (`src/cli.rs:113-129`): `-c`/`--cache-dir` (default `./cache`),
`-m`/`--max-papers` (`Option<usize>`), `-v`/`--venue` (`Option<String>`).

This subcommand makes **live network calls** to the Semantic Scholar API. It
parses the corpus, optionally restricts to a venue (a case-insensitive substring
match on `Paper.venue`, applied by `retain` over `graph.papers`,
`src/main.rs:333-351`), creates the cache directory, then calls
`citations::fetch_all_citations(&graph, cache_dir, max_papers)`
(`src/main.rs:361`). It prints progress, then a summary of fetched papers, total
citations, total references, and the top 10 most-cited papers
(`src/main.rs:363-395`). The legacy fetcher rate-limits at roughly one request
per second and caches results, so a full run over the corpus is slow; `-m` is
provided to cap the number of papers for testing, and `-v` to scope to one
venue.

Two caveats. First, the path inconsistency: the read-only ranking subcommands
read the single file `./cache/citations.json`, whereas `fetch-citations` writes
into the `--cache-dir` *directory* via the legacy `citations::CitationFetcher`
path. The relationship between the two is not guaranteed; the data actually
consumed by the website comes from a separate S2AG pipeline writing
`cache/citations/s2ag_citations.json` (Chapter 6, *Citation Data Integration*).
Second, the API key: the Semantic Scholar key is read from the `S2_API_KEY`
environment variable (a template lives in `.env.example`); there is no committed
`.env`. Set it before fetching:

```bash
export S2_API_KEY=...        # or use a .env consumed by your shell
qindex fetch-citations -v sigcomm -m 100
RUST_LOG=info qindex fetch-citations -c ./cache -m 50
```

The bulk S2AG dataset and the Python pipelines that produced the committed
`cache/citations/*.json` artifacts are described in Chapter 6; this subcommand
is the in-binary alternative and is not what populates the website's per-paper
citation counts.

## 9.6 `extract-papers` (experimental, largely stubbed)

```
qindex extract-papers [-b ./bib] [-d ./paper_db] [-m <max>] [-v <venue>]
```

Flags (`src/cli.rs:132-148`): `-d`/`--db-dir` (default `./paper_db`),
`-m`/`--max-papers`, `-v`/`--venue` (same substring-filter semantics as
`fetch-citations`). The handler parses the corpus, optionally filters by venue,
creates the database directory, calls
`paper_extractor::extract_all_papers(...)` (`src/main.rs:477`), and then prints
counts of papers with PDF URLs, DOIs, ArXiv IDs, total references extracted, and
the five papers with the most references (`src/main.rs:479-517`).

This subcommand is the least complete in the tool and should be treated as
experimental. The PDF reference extractor is a stub:
`extract_references_from_pdf` logs `"PDF extraction not yet implemented"` and
returns an empty vector (`src/paper_extractor.rs:374-382`), so
`reference_count` stays `0` even when a PDF is found. Most PDF-finding backends
also return `None`: USENIX scraping (`src/paper_extractor.rs:324-335`), Google
Scholar (`src/paper_extractor.rs:338-347`), ACM
(`src/paper_finder.rs:147-156`), and author pages
(`src/paper_finder.rs:240-248`). Only ArXiv title lookup and CrossRef DOI /
metadata resolution are functional. In practice the "references extracted"
counts this subcommand reports will be near zero.

```bash
# Will run, hit ArXiv/CrossRef, but report ~0 references (extractor stubbed)
qindex extract-papers -d ./paper_db -m 50 -v osdi
```

## 9.7 `build-site`

```
qindex build-site [-b ./bib] [-o ./site] [--base-url /]
                  [--static-dir ./static] [--book-dir ./docs/book]
```

Flags (`src/cli.rs:90-110`): `-o`/`--out-dir` (default `./site`; the directory is
deleted and rewritten on every build), `--base-url` (long-only, default `/`; the
URL path the site will be served under, such as `/q-index/` for the GitHub Pages
project site), `--static-dir` (long-only, default `./static`; copied to `static/`
in the output), and `--book-dir` (long-only, default `./docs/book`; the rendered
mdBook, copied to `book/` when it contains an `index.html` and skipped with a
warning otherwise). The handler `run_build_site` (`src/main.rs:398-433`) calls
`site::build_site` and prints a summary of HTML, JSON, and other files written
with their total size, plus a suggested preview command.

```bash
qindex build-site                              # ./site for a local preview at /
qindex build-site --base-url /q-index/         # what CI publishes to GitHub Pages
python3 -m http.server -d site 8080            # preview http://localhost:8080/
python3 scripts/check_site_links.py site /     # verify every internal link
```

This subcommand replaced the old `web` subcommand, which started an actix-web
server on `--host`/`--port`. There is no server any more: `build-site` writes the
whole website — pre-rendered pages, a search index, scholar profile shards, and
public JSON data files — and any static host serves the result. The generated
site, its URL scheme, and its data files are covered in Chapter 8, *The Static
Website and Its Data Files*; building, previewing, and the GitHub Pages
deployment are in Chapter 10. The relevant CLI fact is that `build-site`, like
the old web path, does not call `load_citation_cache`, so the rankings on the
website can differ from those printed by `calculate`/`venues`/`scholars` once a
`./cache/citations.json` file exists. (Today the file is absent, so both paths run
the same prestige fallback.)

## 9.8 Running from source vs. the release binary

During development, `cargo run -- <subcommand>` builds and runs in debug mode,
which compiles quickly:

```bash
cargo run -- stats
cargo run -- calculate -n 20 -o out.json -e
cargo run --release -- build-site
```

For full site builds, `cargo build --release` produces `target/release/qindex`
(roughly a 6 MB binary). The release profile sets `lto = true`,
`codegen-units = 1`, and `opt-level = 3` (`Cargo.toml:80-83`), which maximizes
runtime performance at the cost of slow link times. `build-site` is the one
subcommand where the optimized build matters in practice: it runs the S2AG fuzzy
title lookup for every paper in the corpus (Chapter 8). Build, packaging, and the workspace-isolation fix (the empty
`[workspace]` table in `Cargo.toml`) are detailed in Chapter 10, *Building,
Running, and Deployment*; for fast iteration, prefer the debug `cargo run` form
above.
