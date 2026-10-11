# QIndex - Academic Quality Index Calculator (Rust Edition)

A high-performance academic quality index calculator built in Rust, using PageRank algorithm to evaluate conferences, journals, and scholars based on citation networks.

## 🚀 Features

- **PageRank-based Venue Scoring**: Calculates prestige scores for academic venues using citation networks
- **Scholar QIndex**: Computes quality-weighted publication scores for researchers
- **Scoped H-Index**: Snapshot h-index over the indexed or ranking-eligible works
- **Static Website**: Pre-rendered HTML and browser-side JSON; no application server
- **Rich CLI**: Beautiful terminal interface with colored output and progress bars
- **Multiple Export Formats**: JSON and CSV export for further analysis
- **Curated Dataset**: Selected CS venues; incomplete coverage, not full scholar CVs

## 📦 Installation

### Prerequisites

- Rust 1.70 or higher
- Cargo (comes with Rust)

### Building from Source

```bash
# Clone the repository
git clone https://github.com/shuaimu/q-index.git
cd q-index

# Build in release mode for optimal performance
cargo build --release

# The binary will be at target/release/qindex
```

### Install Locally

```bash
cargo install --path .
```

## 🎯 Usage

### Website

The web interface is a static site generated from the BibTeX data, so it can be
hosted on GitHub Pages (or any static host) without running a server. It is
published at **https://shuaimu.github.io/q-index/** on every push to `main`.

Build and preview it locally:

```bash
qindex build-site                      # writes ./site (base URL "/")
python3 -m http.server -d site 8080    # then open http://localhost:8080
```

`--base-url /q-index/` builds for a sub-path (that's what CI uses for GitHub
Pages); `mdbook build docs` first to include the design book under `/book/`.
The site includes:
- 📊 Dashboard with rankings and a field distribution chart
- 🏛️ Venue rankings (filter by field and tier) and per-venue paper listings
- 👩‍🔬 Scholar rankings and profiles
- 🔍 In-browser search with autocomplete
- 📥 Rankings as JSON files under `/data/`

### Command Line Interface

#### Calculate All Scores

```bash
qindex calculate
```

#### View Top Venues

```bash
qindex venues --top 30
qindex venues --field Systems --tier "A*"
```

#### View Top Scholars

```bash
qindex scholars --top 30
qindex scholars --min-papers 5
```

#### Search

```bash
qindex search "distributed systems"
qindex search "Lampson"
```

#### Show Statistics

```bash
qindex stats
```

#### Export Results

```bash
qindex calculate --output results.json --export
qindex calculate --output results.csv --export
```

### Local S2AG Snapshot Extraction

The complete release `2026-10-06` is downloaded under
`data/s2ag/2026-10-06/` (`papers/`: 60 gzip JSONL shards; `citations/`: 395).
Re-extract after changing the bibliography:

```bash
./target/release/qindex export-papers
python3 -I scripts/match_s2ag.py scan --snapshot-dir data/s2ag/2026-10-06
python3 -I scripts/match_s2ag.py resolve --snapshot-dir data/s2ag/2026-10-06
python3 -I scripts/match_s2ag.py graph --snapshot-dir data/s2ag/2026-10-06
./target/release/qindex build-site
```

Every extraction stage is local-only: no Semantic Scholar search, live count API
or API key is used. `S2_API_KEY` is required only for dataset downloads.
`--snapshot-dir ROOT` is required; `--work-dir` defaults to `ROOT/work` and scan
writes `scan_matches.json` there. `--workers` defaults to 16, capped by available
CPUs. `--papers` defaults to `cache/citations/papers.jsonl`.
Matching prefers exact DOI, then a verified, case-sensitive DBLP record key
(`dblp` in BibTeX, `dblp_id` in the export), then conservative normalized
title/year matching. This can recover shortened or misspelled S2 titles without
fuzzy matching or an online search.

Commit both generated artifacts (`--out` and `--graph-out` override their paths):

- `cache/citations/s2ag_paper_citations.json`: exact paper-id matches and
  **whole-S2 snapshot counts**, including citations outside this bibliography.
- `cache/citations/s2ag_citation_graph.json`: unique directed corpus-id pairs
  with **both endpoints in the matched bibliography**, excluding self edges.

Default ranking CLI commands and the website use these same local inputs.
BibTeX aliases for one corpus id do not multiply graph edges, publication credit,
paper listings, counts or h-indices. Unmatched works remain distinct by local id.
The internal graph drives venue scores and scholar QIndex; whole-S2 counts drive
reported citation totals and h-indices, not internal graph in-degree.
Read current coverage and edge totals from generated extraction summaries.
`release_id` is the snapshot provenance date; `generated` and site build date
are processing dates, not count refresh dates. Loaded graph/count releases must
agree. Raw shards and work files are gitignored; CI uses the committed artifacts.

Custom site inputs:

```bash
qindex build-site \
  --citations-file cache/citations/s2ag_paper_citations.json \
  --citation-graph-file cache/citations/s2ag_citation_graph.json
```


## 🔧 Command-Line Options

```
qindex 0.1.0
Academic quality index calculator using PageRank algorithm

USAGE:
    qindex <SUBCOMMAND>

SUBCOMMANDS:
    calculate    Calculate QIndex scores for all venues and scholars
    venues       Show top venues by PageRank score
    scholars     Show top scholars by QIndex score
    search       Search for venues or scholars
    stats        Show dataset statistics
    help         Print this message or the help of the given subcommand(s)

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information
```

## 📊 Algorithm Details

### Venue Score

The algorithm builds a directed graph where:
- **Nodes**: Academic venues (conferences/journals)
- **Edges**: Directed citations between different venues, extracted from the local bibliography-only graph
- **Weights**: Year-decay adjusted citation-edge weights (each unique edge counted once)

Key parameters:
- Damping factor: 0.85
- Citation-edge age: `max(citing_year - cited_year, 0)`, with weight `0.95^age`
- Tier bonuses: A* (2.0x), A (1.5x), B (1.2x), C (1.0x)

Dangling venues redistribute mass uniformly. Published venue scores are
tier-adjusted and renormalized, not raw PageRank. The JSON field `pagerank`
contains that adjusted score; `impact_factor` is its paper-count scaling
`score × ln(unique venue works + 1)`, not a journal impact factor.

### Scholar QIndex

```
raw_score = Σ(paper_score) × ln(eligible_unique_papers + 1)
QIndex = 100 × raw_score / max_scholar_raw_score

paper_score = adjusted_venue_score × 0.95^(age / 5) × author_position_weight
```

Author position weights:
- First author: 1.0
- Last author: 0.8
- Middle authors: 0.6/(n-2)

Only unique authored works in the CSRankings venue whitelist enter the sum,
productivity multiplier and ranking metrics. `age = max(reference_year - year, 0)`;
the reference year comes from the loaded snapshot release, or the current UTC
year without a valid release. Future and unknown-year works are undecayed.

This is a relative score for a curated bibliography, **not a global scholar
leaderboard or full-career h-index**. The manual tier dictionary is systems/theory-
heavy: ICML, NeurIPS, ICLR and CVPR currently fall back to C. Authorship positions
are heuristics, not verified contributions. Names preserve DBLP disambiguators,
but remaining name collisions/aliases are not resolved by an author-ID service.

## 🌐 Website Features

The site is pre-rendered by `qindex build-site`; the few interactive parts run in
the browser against JSON files, so no server is needed:

### Dashboard
- Statistics cards
- Top venues and scholars tables
- Field distribution chart

### Venues
- Filterable by field and tier
- Color-coded tier badges
- A page per venue with its papers (newest first), top authors and links

### Scholars
- Top 100 by QIndex, with citation totals/h-index/paper counts over eligible unique works
- Profiles separate **all indexed works** from **ranking-eligible works**, with
  deduplicated publication cards and visible snapshot provenance

### Search
- Searches every venue and scholar in the browser (`data/search-index.json`)
- Autocomplete in the navigation bar

### Data Files
- `data/venues.json` - All ranked venues with scores
- `data/scholars.json` - Top scholars with metrics
- `data/stats.json` - Dataset statistics
- `data/search-index.json` - Every venue and scholar with scores

### Publication audit (2026-10-11)

The publication audit compares displayed counts with raw local snapshot records
and adjudicates metadata conflicts against publisher records. It corrected
duplicate-inflated credit, unsafe frozen-2024 age arithmetic, stale indices after
cite-key replacement, editor credits from proceedings volumes, wrong-author
aliases, missing coauthors, SGLang's incorrect OSDI attribution (NeurIPS 2024),
and 16 BPOE workshop papers incorrectly stored as ASPLOS research papers.
Publisher PDF bylines take precedence when web/BibTeX exports omit authors;
S2 author metadata is not blindly copied into the bibliography.
The top-ten audit also reconciled publisher-verified name variants (including
Haibo Chen, Amin Vahdat, M. Frans Kaashoek and Remzi H. Arpaci-Dusseau), removed
duplicate FlexGen/Brainstorm/Diamond/Arrakis/TAOBench records, corrected NeuroCard's
byline, and moved the XStore journal article and MemServe preprint out of the
conference corpus. Unsupported SOSP records and an ICDE workshop paper were
removed. TAPIR's extended version is retained as a University of Washington
technical report, with its institutional publication link.
The ranking-boundary audit removed additional SIGCOMM demos and
HotMiddlebox/BiDEDE/ApPLIED/MobiHeld/EuroMLSys/FFSPIN/NAI workshop records mislabeled
as main-conference papers. It corrected Syrup and Sky Computing attributions,
Faa$T's year and μTree's title, retained the SIGMOD Record article in its journal
file, and excluded an unsafe Octopus/Octopus+ citation association. Title fallback
cannot override a conflicting known published DBLP record; an unresolved version
is shown as `n/a`, not assigned another version's citations.
Redundant CCFS, Protocol-Aware Recovery and malformed Deepview records were
consolidated into the publisher-verified FAST and NSDI originals.

## 🏗️ Architecture

```
src/
├── main.rs          # Entry point and main logic
├── cli.rs           # Command-line interface with clap
├── models.rs        # Data structures (Paper, Venue, Scholar)
├── parser.rs        # BibTeX parsing with nom-bibtex
├── algorithm.rs     # PageRank implementation
├── export.rs        # JSON/CSV export functionality
├── utils.rs         # Helper functions and utilities
└── site/            # Static site generator (`qindex build-site`)
    ├── mod.rs       # Builds pages, JSON data files and assets
    └── templates.rs # Maud HTML templates

static/app.js        # Browser-side search, filters and scholar profiles
scripts/check_site_links.py   # Verifies every internal link in a built site
.github/workflows/pages.yml   # Builds and deploys the site to GitHub Pages
```

## 🧪 Testing

Run the test suite:

```bash
cargo test
```

Run benchmarks:

```bash
cargo bench
```

## 🎨 Features Highlights

### Performance Optimizations
- Efficient graph representation with IndexMap
- Memory-efficient BibTeX parsing
- Optimized PageRank convergence checking

### User Experience
- Colored terminal output for better readability
- Progress bars for long operations
- UTF-8 tables for beautiful result display
- Intuitive search functionality

### Data Quality
- Automatic venue tier classification
- Field categorization (Systems, Database, Theory, etc.)
- Author name normalization
- Citation cycle detection

## 🤝 Contributing

Contributions are welcome! Please feel free to submit pull requests.

### Development Setup

```bash
# Clone the repo
git clone https://github.com/shuaimu/q-index.git
cd q-index

# Run in development mode
cargo run -- calculate

# Run with logging
RUST_LOG=debug cargo run -- calculate

# Format code
cargo fmt

# Run clippy linter
cargo clippy
```

## 📝 License

MIT License - see LICENSE file for details

## 🙏 Acknowledgments

- Built with Rust and amazing crates from the community
- BibTeX parsing powered by nom-bibtex
- CLI interface using clap
- Inspired by PageRank and h-index algorithms

## 🚀 Future Enhancements

- [ ] Web interface with WASM compilation
- [ ] Real-time citation updates
- [ ] Graph visualization with D3.js
- [ ] Integration with DBLP/Google Scholar APIs
- [ ] Temporal analysis and trend detection
- [ ] Multi-field scholar analysis
- [ ] Conference recommendation system