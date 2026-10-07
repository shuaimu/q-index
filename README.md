# QIndex - Academic Quality Index Calculator (Rust Edition)

A high-performance academic quality index calculator built in Rust, using PageRank algorithm to evaluate conferences, journals, and scholars based on citation networks.

## 🚀 Features

- **PageRank-based Venue Scoring**: Calculates prestige scores for academic venues using citation networks
- **Scholar QIndex**: Computes quality-weighted publication scores for researchers
- **H-Index Calculation**: Traditional h-index alongside the novel QIndex metric
- **High Performance**: Leverages Rust's performance and parallelism with Rayon
- **Rich CLI**: Beautiful terminal interface with colored output and progress bars
- **Multiple Export Formats**: JSON and CSV export for further analysis
- **Comprehensive Dataset**: Includes papers from top CS venues with recent additions

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

### Venue PageRank

The algorithm builds a directed graph where:
- **Nodes**: Academic venues (conferences/journals)
- **Edges**: Citation relationships between papers
- **Weights**: Year-decay adjusted citation counts

Key parameters:
- Damping factor: 0.85
- Year decay: 0.95 per year
- Tier bonuses: A* (2.0x), A (1.5x), B (1.2x), C (1.0x)

### Scholar QIndex

```
QIndex = Σ(paper_score) × log(total_papers + 1)

where:
  paper_score = venue_pagerank × year_decay × author_position_weight
```

Author position weights:
- First author: 1.0
- Last author: 0.8
- Middle authors: 0.6/(n-2)

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
- Top 100 scholars by QIndex with H-index and top venues
- A profile page for every scholar, loaded from `data/scholars/*.json`

### Search
- Searches every venue and scholar in the browser (`data/search-index.json`)
- Autocomplete in the navigation bar

### Data Files
- `data/venues.json` - All ranked venues with scores
- `data/scholars.json` - Top scholars with metrics
- `data/stats.json` - Dataset statistics
- `data/search-index.json` - Every venue and scholar with scores

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
- Parallel processing with Rayon for multi-core utilization
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

## 📈 Performance

On a modern multi-core system:
- Parses ~10,000 papers in < 2 seconds
- Calculates PageRank for 100+ venues in < 100ms
- Computes QIndex for 1,000+ scholars in < 500ms

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

## 📊 Sample Output

```
╔═══════════════════════════════════════════════════════╗
║                      QIndex v0.1.0                    ║
╚═══════════════════════════════════════════════════════╝

📚 Loaded 5,234 papers, 89 venues, 3,421 scholars
🔄 Calculating QIndex scores...
✅ Calculation complete!

📊 Top 10 Venues:
┌──────┬─────────┬──────┬────────────┬──────────┬────────┬────────┐
│ Rank │ Venue   │ Tier │ Field      │ PageRank │ Impact │ Papers │
├──────┼─────────┼──────┼────────────┼──────────┼────────┼────────┤
│ 1    │ SOSP    │ A*   │ Systems    │ 0.0523   │ 0.234  │ 142    │
│ 2    │ OSDI    │ A*   │ Systems    │ 0.0498   │ 0.218  │ 128    │
│ 3    │ SIGMOD  │ A*   │ Database   │ 0.0456   │ 0.203  │ 156    │
└──────┴─────────┴──────┴────────────┴──────────┴────────┴────────┘

🎓 Top 10 Scholars:
┌──────┬─────────────────┬────────┬─────────┬────────┬───────────┐
│ Rank │ Scholar         │ QIndex │ H-Index │ Papers │ Citations │
├──────┼─────────────────┼────────┼─────────┼────────┼───────────┤
│ 1    │ Barbara Liskov  │ 98.45  │ 42      │ 67     │ 4,231     │
│ 2    │ Butler Lampson  │ 96.23  │ 38      │ 54     │ 3,892     │
│ 3    │ Michael Stonebraker │ 94.67 │ 45   │ 89     │ 5,123     │
└──────┴─────────────────┴────────┴─────────┴────────┴───────────┘
```

## 🚀 Future Enhancements

- [ ] Web interface with WASM compilation
- [ ] Real-time citation updates
- [ ] Graph visualization with D3.js
- [ ] Integration with DBLP/Google Scholar APIs
- [ ] Temporal analysis and trend detection
- [ ] Multi-field scholar analysis
- [ ] Conference recommendation system