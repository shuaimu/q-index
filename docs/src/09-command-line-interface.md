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
`0.1.0`. There are nine subcommands (`src/cli.rs:18-165`):

| Subcommand        | Purpose                                              | State today                |
|-------------------|-----------------------------------------------------|----------------------------|
| `calculate`       | Compute and print top venues and scholars; optional export | Functional                 |
| `venues`          | Print top venues, with a field filter                | Functional                 |
| `scholars`        | Print top scholars, with a minimum-papers filter    | Functional                 |
| `search`          | Substring search over venues and scholars           | Functional                 |
| `stats`           | Print dataset statistics                            | Functional                 |
| `export-papers`   | Write parsed papers as JSON lines for S2AG matching | Functional                 |
| `build-site`      | Generate the static website                        | Functional                 |
| `fetch-citations` | Call live Semantic Scholar API for citation counts  | Functional but slow / live |
| `extract-papers`  | Find PDFs and extract references                    | Largely stubbed            |

Every subcommand accepts `-b`/`--bib-dir`, defaulting to `./bib`
(for example `src/cli.rs:22-23`). All file and cache paths are resolved
relative to the process working directory, so QIndex is intended to be run from
the repository root; running it elsewhere will fail to find `./bib` and `./static`
and will silently miss the citation caches under `./cache`.

`main()` initializes `env_logger` with a default filter of `info`
(`src/main.rs:25`) and then matches on `cli.command` (`src/main.rs:30-104`). The
seven synchronous subcommands (`calculate`, `venues`, `scholars`, `search`,
`stats`, `export-papers`, `build-site`) run directly. The two async ones
(`fetch-citations`, `extract-papers`) each construct their own
`tokio::runtime::Runtime::new()?` and `block_on` the work
(`src/main.rs:87,99`); there is no top-level `#[tokio::main]` and no shared
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

The ranking subcommands parse `bib_dir`, load
`cache/citations/s2ag_paper_citations.json` and
`cache/citations/s2ag_citation_graph.json`, validate release agreement, then score.
These are the same default inputs used by the website. The local graph supplies
unique directed internal edges; snapshot-wide counts supply reported citation
totals and h-indices. No live API call or old aggregate citation cache is involved.

### `calculate`

```
qindex calculate [-b ./bib] [-n 20] [-o <file>] [-e]
```

Flags (`src/cli.rs:20-36`):

- `-n`/`--top` (usize, default `20`): number of top venues and scholars to print.
- `-o`/`--output` (`Option<String>`, no default): export destination.
- `-e`/`--export` (bool flag): enable export.

`calculate` prints a load summary, computes scores, then prints the top venues
and top scholars as `comfy_table` tables (`src/main.rs:133-138`). Export happens
only when **both** `-e` is passed and `-o` is set:
`if export && output.is_some()` (`src/main.rs:141`). Passing `-e` alone is a
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
qindex venues [-b ./bib] [-n 20] [-f <field>]
```

Flags: `-n`/`--top` (default `20`), `-f`/`--field` (`Option<String>`).
These values are forwarded to `get_top_venues(top, field)`, whose API is
`get_top_venues(&self, n: usize, field: Option<&str>) -> Vec<VenueRanking>`.
The field filter is a case-insensitive substring match on `Venue.field`.
`display_venues` prints `Venue, Field, Venue Score, Papers`, with Venue Score
formatted as `{:.4}`.

```bash
qindex venues -n 30
qindex venues -f Systems
qindex venues -n 40 -f Database
```

`--field` matches the automatically classified CSRankings-style subarea string
(Chapter 3), separate from the venue eligibility allowlist.

### `scholars`

```
qindex scholars [-b ./bib] [-n 20] [-m <min>]
```

Flags (`src/cli.rs:58-70`): `-n`/`--top` (default `20`), `-m`/`--min-papers`
(`Option<usize>`). The minimum-papers value is passed to
`get_top_scholars(top, min_papers)` (`src/main.rs:180`), which drops scholars
whose CSRankings-venue paper count falls below the threshold. Output columns are
`Scholar, QIndex, H-Index, Papers, Citations`, QIndex formatted as `{:.2}`
(`src/main.rs:328-351`). Reported h-indices and citation totals use the local
whole-S2 snapshot counts, deduplicated by corpus id, rather than internal in-degree.

```bash
qindex scholars -n 50 -m 5
```

### `search`

```
qindex search [-b ./bib] <query>
```

`query` is a required positional argument. `run_search` (`src/main.rs`) filters
computed venue and scholar rankings by case-insensitive substring matches on
names (also venue full name/field and scholar normalized name), then prints up
to ten of each in score order. It is not a relevance rank. Scores and scholar
metrics use the same eligible unique-work scope as the other ranking commands,
not the graph's unfilled initial score fields. The browser index also includes
unranked indexed identities, with zero ranking metrics.

```bash
qindex search "distributed systems"
qindex search SOSP
```

### `stats`

```
qindex stats [-b ./bib]
```

`stats` takes only `--bib-dir` (`src/cli.rs:83-87`). It parses the corpus and
calls `CitationGraph::print_statistics` (`src/main.rs`, `run_stats`;
`src/models.rs`, `print_statistics`), which prints parsed-record totals,
venue type/field breakdowns, observed years and averages. It does no scoring
or S2 corpus-id deduplication. These raw parsed-record statistics differ from
the website's unique-work statistics; obtain current totals from actual output
rather than carrying forward a previous corpus size.

## 9.3 Terminal output

All tabular output uses the `comfy_table` crate with the `UTF8_FULL` preset,
which draws boxed tables with Unicode borders (`src/main.rs:253-351`,
`src/models.rs:254`). The headers are fixed per command:

- `calculate` venues table: `Rank, Venue, Field, Venue Score, Count-Scaled, Papers`
  (`src/main.rs`, `display_results`); both scores formatted `{:.4}`.
- `calculate` scholars table: `Rank, Scholar, QIndex, H-Index, Papers, Citations`;
  QIndex formatted `{:.2}`.
- `venues` table: `Venue, Field, Venue Score, Papers` (no rank column).
- `scholars` table: `Scholar, QIndex, H-Index, Papers, Citations`.

Surrounding the tables, the subcommands print decorated status lines using emoji
prefixes (for example `"📚 Loaded N papers, ..."`, `"🔄 Calculating ..."`,
`"✅ Calculation complete!"` at `src/main.rs:116-138`). These are cosmetic; the
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
      "reference_year": 2026
    }
  },
  "venues": [
    { "rank": 1, "id": "...", "name": "...", "field": "...",
      "pagerank": 0.0, "impact_factor": 0.0, "paper_count": 0 }
  ],
  "scholars": [
    { "rank": 1, "id": "...", "name": "...", "qindex": 0.0, "h_index": 0,
      "paper_count": 0, "citation_count": 0, "top_venues": ["..."] }
  ],
  "statistics": {
    "total_papers": 0, "total_venues": 0, "total_scholars": 0,
    "papers_by_year":  { "2024": 0 },
    "papers_by_field": { "Operating Systems": 0 }
  }
}
```

(Types: `VenueExport`, `ScholarExport`, `DatasetStatistics`, `ExportMetadata` in
`src/export.rs`; values are illustrative.) The retained `pagerank` is the
normalized, unadjusted PageRank (Venue Score), without manual boosts. `impact_factor` is
`Venue Score * ln(unique venue works + 1)`, not a journal impact factor.
`metadata.parameters` currently serializes `AlgorithmParams::default()` in
`Exporter::prepare_export_data`, including the current UTC `reference_year`;
it does not capture the calculator's snapshot-year override or runtime tuning.
Actual scholar decay uses the valid snapshot release year when available,
`0.95^(max(reference_year - paper_year, 0)/5)`; unknown years are undecayed.
Exporter dataset statistics remain parsed-record counts, unlike the site's
corpus-deduplicated unique-work statistics.

### CSV schema

The venues CSV retains the header `Rank, Venue, Field, PageRank,
Impact Factor, Papers`, with both scores formatted `{:.6}` (`src/export.rs`,
`Exporter::export_csv`). Those names have the Venue Score and count-scaled
semantics above: normalized PageRank, not journal impact. The scholars CSV header is
`Rank, Scholar, QIndex, H-Index, Papers, Citations`, with QIndex formatted `{:.2}`.

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

Flags (`src/cli.rs:128-145`): `-c`/`--cache-dir` (default `./cache`),
`-m`/`--max-papers` (`Option<usize>`), `-v`/`--venue` (`Option<String>`).

This subcommand makes **live network calls** to the Semantic Scholar API. It
parses the corpus, optionally restricts to a venue (a case-insensitive substring
match on `Paper.venue`, applied by `retain` over `graph.papers`,
`src/main.rs:367-385`), creates the cache directory, then calls
`citations::fetch_all_citations(&graph, cache_dir, max_papers)`
(`src/main.rs:395`). It prints progress, then a summary of fetched papers, total
citations, total references, and the top 10 most-cited papers
(`src/main.rs:397-427`). The legacy fetcher rate-limits at roughly one request
per second and caches results, so a full run over the corpus is slow; `-m` is
provided to cap the number of papers for testing, and `-v` to scope to one
venue.

This is a legacy live-fetch command, not the local snapshot extraction workflow.
Its outputs are not consumed by default CLI rankings or the website. Those
consumers read the two artifacts produced by local `scan`, `resolve` and `graph`
using `--snapshot-dir data/s2ag/2026-10-06` (Chapter 6). Do not use this command to
refresh snapshot counts or construct the internal ranking graph. An API key is
not required for the local workflow; only dataset downloading needs one.

Chapter 6 documents the local bulk-snapshot pipeline. This legacy subcommand
does not populate either of its committed artifacts.

## 9.6 `extract-papers` (experimental, largely stubbed)

```
qindex extract-papers [-b ./bib] [-d ./paper_db] [-m <max>] [-v <venue>]
```

Flags (`src/cli.rs:147-164`): `-d`/`--db-dir` (default `./paper_db`),
`-m`/`--max-papers`, `-v`/`--venue` (same substring-filter semantics as
`fetch-citations`). The handler parses the corpus, optionally filters by venue,
creates the database directory, calls
`paper_extractor::extract_all_papers(...)` (`src/main.rs:513`), and then prints
counts of papers with PDF URLs, DOIs, ArXiv IDs, total references extracted, and
the five papers with the most references (`src/main.rs:515-551`).

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

## 9.7 `export-papers`

```
qindex export-papers [-b ./bib] [-o ./cache/citations/papers.jsonl]
```

Flags (`src/cli.rs:89-99`): `-o`/`--output` (default
`./cache/citations/papers.jsonl`). The handler `run_export_papers`
(`src/main.rs`) parses the corpus exactly as every other subcommand does and
streams one JSON object per paper — `{"id", "title", "year", "doi", "dblp_id", "venue"}` —
with `null` for a missing year, DOI or DBLP record key, then prints the observed
record count. External matching works from the same paper
ids and cleaned titles the website uses: `scripts/match_s2ag.py` reads this file
to match papers against the local snapshot and writes the two committed count
and graph artifacts (Chapter 6). The export is gitignored and regenerable.

```bash
qindex export-papers
python3 -I scripts/match_s2ag.py scan --snapshot-dir data/s2ag/2026-10-06
python3 -I scripts/match_s2ag.py resolve --snapshot-dir data/s2ag/2026-10-06
python3 -I scripts/match_s2ag.py graph --snapshot-dir data/s2ag/2026-10-06
```

## 9.8 `build-site`

```
qindex build-site [-b ./bib] [-o ./site] [--base-url /]
                  [--static-dir ./static] [--book-dir ./docs/book]
                  [--citations-file cache/citations/s2ag_paper_citations.json]
                  [--citation-graph-file cache/citations/s2ag_citation_graph.json]
```

Flags (`src/cli.rs:101-126`): `-o`/`--out-dir` (default `./site`; the directory is
deleted and rewritten on every build), `--base-url` (long-only, default `/`; the
URL path the site will be served under, such as `/q-index/` for the GitHub Pages
project site), `--static-dir` (long-only, default `./static`; copied to `static/`
in the output), `--book-dir` (long-only, default `./docs/book`; the rendered
mdBook, copied to `book/` when it contains an `index.html` and skipped with a
warning otherwise), and `--citations-file` (long-only, default
`cache/citations/s2ag_paper_citations.json`, i.e.
`s2ag_citations::DEFAULT_PATH`; the S2AG citation counts every citation figure
on the site is derived from — a missing file logs a warning and the site shows
"n/a" for every paper). The handler `run_build_site` (`src/main.rs:432-469`) calls
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
deployment are in Chapter 10. `--citation-graph-file` selects the local internal
graph (default `cache/citations/s2ag_citation_graph.json`); its release must agree
with `--citations-file`. Default CLI and site use the same artifacts and ranking
policy. `release_id`, not `generated` or build date, identifies count provenance.

## 9.9 Running from source vs. the release binary

During development, `cargo run -- <subcommand>` builds and runs in debug mode,
which compiles quickly:

```bash
cargo run -- stats
cargo run -- calculate -n 20 -o out.json -e
cargo run --release -- build-site
```

For full site builds, `cargo build --release` produces `target/release/qindex`
(roughly a 6 MB binary). The release profile sets `lto = true`,
`codegen-units = 1`, and `opt-level = 3` (`Cargo.toml:79-82`), which maximizes
runtime performance at the cost of slow link times. `build-site` is the one
subcommand where the optimized build matters in practice: it parses and ranks the
whole corpus and writes about a thousand pages (Chapter 8). Build, packaging, and the workspace-isolation fix (the empty
`[workspace]` table in `Cargo.toml`) are detailed in Chapter 10, *Building,
Running, and Deployment*; for fast iteration, prefer the debug `cargo run` form
above.
