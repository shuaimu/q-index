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
    
    /// Start web server
    Web {
        /// Directory containing BibTeX files
        #[arg(short, long, default_value = "./bib")]
        bib_dir: String,
        
        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,
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