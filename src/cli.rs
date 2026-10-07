use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "qindex",
    version = "0.1.0",
    author = "Your Name",
    about = "Academic quality index calculator using PageRank algorithm",
    long_about = "QIndex calculates quality scores for academic conferences, journals, and scholars\n\
                  using a PageRank-based algorithm on citation networks."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Calculate QIndex scores for all venues and scholars
    Calculate {
        /// Directory containing BibTeX files
        #[arg(short, long, default_value = "./bib")]
        bib_dir: String,

        /// Number of top results to show
        #[arg(short = 'n', long, default_value = "20")]
        top: usize,

        /// Output file for results
        #[arg(short, long)]
        output: Option<String>,

        /// Export results to file (JSON or CSV based on extension)
        #[arg(short, long)]
        export: bool,
    },

    /// Show top venues by PageRank score
    Venues {
        /// Directory containing BibTeX files
        #[arg(short, long, default_value = "./bib")]
        bib_dir: String,

        /// Number of top venues to show
        #[arg(short = 'n', long, default_value = "20")]
        top: usize,

        /// Filter by field (e.g., Systems, Database, Theory)
        #[arg(short, long)]
        field: Option<String>,

        /// Filter by tier (A*, A, B, C)
        #[arg(short = 't', long)]
        tier: Option<String>,
    },

    /// Show top scholars by QIndex score
    Scholars {
        /// Directory containing BibTeX files
        #[arg(short, long, default_value = "./bib")]
        bib_dir: String,

        /// Number of top scholars to show
        #[arg(short = 'n', long, default_value = "20")]
        top: usize,

        /// Minimum number of papers
        #[arg(short, long)]
        min_papers: Option<usize>,
    },

    /// Search for venues or scholars
    Search {
        /// Directory containing BibTeX files
        #[arg(short, long, default_value = "./bib")]
        bib_dir: String,

        /// Search query
        query: String,
    },

    /// Show dataset statistics
    Stats {
        /// Directory containing BibTeX files
        #[arg(short, long, default_value = "./bib")]
        bib_dir: String,
    },

    /// Generate the static website (for GitHub Pages or any static host)
    BuildSite {
        /// Directory containing BibTeX files
        #[arg(short, long, default_value = "./bib")]
        bib_dir: String,

        /// Output directory (replaced on every build)
        #[arg(short, long, default_value = "./site")]
        out_dir: String,

        /// URL path the site is served under, e.g. /q-index/ for a GitHub Pages project site
        #[arg(long, default_value = "/")]
        base_url: String,

        /// Directory with static assets (style.css, app.js)
        #[arg(long, default_value = "./static")]
        static_dir: String,

        /// Rendered mdBook to publish under book/ (skipped if missing; build it with `mdbook build docs`)
        #[arg(long, default_value = "./docs/book")]
        book_dir: String,
    },

    /// Fetch citation data from Semantic Scholar
    FetchCitations {
        /// Directory containing BibTeX files
        #[arg(short, long, default_value = "./bib")]
        bib_dir: String,

        /// Cache directory for citation data
        #[arg(short, long, default_value = "./cache")]
        cache_dir: String,

        /// Maximum number of papers to fetch (for testing)
        #[arg(short, long)]
        max_papers: Option<usize>,

        /// Only fetch for specific venue
        #[arg(short, long)]
        venue: Option<String>,
    },

    /// Extract paper metadata and references from online sources
    ExtractPapers {
        /// Directory containing BibTeX files
        #[arg(short, long, default_value = "./bib")]
        bib_dir: String,

        /// Database directory for extracted metadata
        #[arg(short, long, default_value = "./paper_db")]
        db_dir: String,

        /// Maximum number of papers to extract (for testing)
        #[arg(short, long)]
        max_papers: Option<usize>,

        /// Only extract for specific venue
        #[arg(short, long)]
        venue: Option<String>,
    },
}
