mod algorithm;
mod citations;
mod cli;
mod export;
mod models;
mod paper_extractor;
mod paper_finder;
mod parser;
mod s2ag_citations;
mod site;
mod utils;

use anyhow::Result;
use clap::Parser;
use env_logger::Env;
use log::info;

use crate::algorithm::PageRankCalculator;
use crate::cli::{Cli, Commands};
use crate::export::Exporter;
use crate::parser::BibParser;

fn main() -> Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    // Parse CLI arguments
    let cli = Cli::parse();

    match cli.command {
        Commands::Calculate {
            bib_dir,
            top,
            output,
            export,
        } => {
            run_calculate(&bib_dir, top, output.as_deref(), export)?;
        }
        Commands::Venues {
            bib_dir,
            top,
            field,
            tier,
        } => {
            run_venues(&bib_dir, top, field.as_deref(), tier.as_deref())?;
        }
        Commands::Scholars {
            bib_dir,
            top,
            min_papers,
        } => {
            run_scholars(&bib_dir, top, min_papers)?;
        }
        Commands::Search { bib_dir, query } => {
            run_search(&bib_dir, &query)?;
        }
        Commands::Stats { bib_dir } => {
            run_stats(&bib_dir)?;
        }
        Commands::BuildSite {
            bib_dir,
            out_dir,
            base_url,
            static_dir,
            book_dir,
        } => {
            run_build_site(&bib_dir, &out_dir, &base_url, &static_dir, &book_dir)?;
        }
        Commands::FetchCitations {
            bib_dir,
            cache_dir,
            max_papers,
            venue,
        } => {
            // Use tokio runtime for async API calls
            let runtime = tokio::runtime::Runtime::new()?;
            runtime.block_on(async {
                run_fetch_citations(&bib_dir, &cache_dir, max_papers, venue.as_deref()).await
            })?;
        }
        Commands::ExtractPapers {
            bib_dir,
            db_dir,
            max_papers,
            venue,
        } => {
            // Use tokio runtime for async operations
            let runtime = tokio::runtime::Runtime::new()?;
            runtime.block_on(async {
                run_extract_papers(&bib_dir, &db_dir, max_papers, venue.as_deref()).await
            })?;
        }
    }

    Ok(())
}

fn run_calculate(bib_dir: &str, top: usize, output: Option<&str>, export: bool) -> Result<()> {
    info!("Loading bibliography from {}", bib_dir);

    // Parse bibliography files
    let mut parser = BibParser::new();
    let graph = parser.parse_directory(bib_dir)?;

    println!(
        "📚 Loaded {} papers, {} venues, {} scholars",
        graph.papers.len(),
        graph.venues.len(),
        graph.scholars.len()
    );

    // Calculate PageRank and QIndex
    println!("🔄 Calculating QIndex scores...");
    let mut calculator = PageRankCalculator::new(&graph);

    // Try to load citation cache
    let cache_path = std::path::Path::new("./cache/citations.json");
    calculator.load_citation_cache(cache_path)?;

    let _metrics = calculator.calculate()?;

    println!("✅ Calculation complete!");
    println!("\n📊 Top {} Venues:", top);
    display_top_venues(&calculator, top);

    println!("\n🎓 Top {} Scholars:", top);
    display_top_scholars(&calculator, top);

    // Export results if requested
    if export && output.is_some() {
        let exporter = Exporter::new(&graph, &calculator);
        exporter.export(output.unwrap(), top)?;
        println!("\n💾 Results exported to {}", output.unwrap());
    }

    Ok(())
}

fn run_venues(bib_dir: &str, top: usize, field: Option<&str>, tier: Option<&str>) -> Result<()> {
    let mut parser = BibParser::new();
    let graph = parser.parse_directory(bib_dir)?;

    let mut calculator = PageRankCalculator::new(&graph);

    // Try to load citation cache
    let cache_path = std::path::Path::new("./cache/citations.json");
    calculator.load_citation_cache(cache_path)?;

    calculator.calculate()?;

    let venues = calculator.get_top_venues(top, field, tier);
    display_venues(&venues);

    Ok(())
}

fn run_scholars(bib_dir: &str, top: usize, min_papers: Option<usize>) -> Result<()> {
    let mut parser = BibParser::new();
    let graph = parser.parse_directory(bib_dir)?;

    let mut calculator = PageRankCalculator::new(&graph);

    // Try to load citation cache
    let cache_path = std::path::Path::new("./cache/citations.json");
    calculator.load_citation_cache(cache_path)?;

    calculator.calculate()?;

    let scholars = calculator.get_top_scholars(top, min_papers);
    display_scholars(&scholars);

    Ok(())
}

fn run_search(bib_dir: &str, query: &str) -> Result<()> {
    let mut parser = BibParser::new();
    let graph = parser.parse_directory(bib_dir)?;

    let mut calculator = PageRankCalculator::new(&graph);

    // Try to load citation cache
    let cache_path = std::path::Path::new("./cache/citations.json");
    calculator.load_citation_cache(cache_path)?;

    calculator.calculate()?;

    println!("🔍 Searching for '{}'...\n", query);

    // Search venues
    let matching_venues = graph.search_venues(query);
    if !matching_venues.is_empty() {
        println!("📍 Venues:");
        display_venues(&matching_venues[..matching_venues.len().min(10)]);
    }

    // Search scholars
    let matching_scholars = graph.search_scholars(query);
    if !matching_scholars.is_empty() {
        println!("\n👤 Scholars:");
        display_scholars(&matching_scholars[..matching_scholars.len().min(10)]);
    }

    if matching_venues.is_empty() && matching_scholars.is_empty() {
        println!("No results found for '{}'", query);
    }

    Ok(())
}

fn run_stats(bib_dir: &str) -> Result<()> {
    let mut parser = BibParser::new();
    let graph = parser.parse_directory(bib_dir)?;

    graph.print_statistics();

    Ok(())
}

fn display_top_venues(calculator: &PageRankCalculator, top: usize) {
    use comfy_table::{presets::UTF8_FULL, Table};

    let venues = calculator.get_top_venues(top, None, None);

    let mut table = Table::new();
    table.load_preset(UTF8_FULL).set_header(vec![
        "Rank", "Venue", "Tier", "Field", "PageRank", "Impact", "Papers",
    ]);

    for (i, venue) in venues.iter().enumerate() {
        table.add_row(vec![
            (i + 1).to_string(),
            venue.name.clone(),
            venue.tier.clone(),
            venue.field.clone(),
            format!("{:.4}", venue.pagerank),
            format!("{:.4}", venue.impact_factor),
            venue.paper_count.to_string(),
        ]);
    }

    println!("{}", table);
}

fn display_top_scholars(calculator: &PageRankCalculator, top: usize) {
    use comfy_table::{presets::UTF8_FULL, Table};

    let scholars = calculator.get_top_scholars(top, None);

    let mut table = Table::new();
    table.load_preset(UTF8_FULL).set_header(vec![
        "Rank",
        "Scholar",
        "QIndex",
        "H-Index",
        "Papers",
        "Citations",
    ]);

    for (i, scholar) in scholars.iter().enumerate() {
        table.add_row(vec![
            (i + 1).to_string(),
            scholar.name.clone(),
            format!("{:.2}", scholar.qindex),
            scholar.h_index.to_string(),
            scholar.paper_count.to_string(),
            scholar.citation_count.to_string(),
        ]);
    }

    println!("{}", table);
}

fn display_venues(venues: &[models::VenueRanking]) {
    use comfy_table::{presets::UTF8_FULL, Table};

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_header(vec!["Venue", "Tier", "Field", "PageRank", "Papers"]);

    for venue in venues {
        table.add_row(vec![
            venue.name.clone(),
            venue.tier.clone(),
            venue.field.clone(),
            format!("{:.4}", venue.pagerank),
            venue.paper_count.to_string(),
        ]);
    }

    println!("{}", table);
}

fn display_scholars(scholars: &[models::ScholarRanking]) {
    use comfy_table::{presets::UTF8_FULL, Table};

    let mut table = Table::new();
    table.load_preset(UTF8_FULL).set_header(vec![
        "Scholar",
        "QIndex",
        "H-Index",
        "Papers",
        "Citations",
    ]);

    for scholar in scholars {
        table.add_row(vec![
            scholar.name.clone(),
            format!("{:.2}", scholar.qindex),
            scholar.h_index.to_string(),
            scholar.paper_count.to_string(),
            scholar.citation_count.to_string(),
        ]);
    }

    println!("{}", table);
}

async fn run_fetch_citations(
    bib_dir: &str,
    cache_dir: &str,
    max_papers: Option<usize>,
    venue: Option<&str>,
) -> Result<()> {
    println!("📚 Loading bibliography from {}", bib_dir);

    // Parse bibliography files
    let mut parser = BibParser::new();
    let mut graph = parser.parse_directory(bib_dir)?;

    println!("🔍 Found {} papers", graph.papers.len());

    // Filter by venue if specified
    if let Some(venue_filter) = venue {
        let venue_upper = venue_filter.to_uppercase();
        let filtered: Vec<_> = graph
            .papers
            .iter()
            .filter(|(_, p)| p.venue.to_uppercase().contains(&venue_upper))
            .map(|(id, _)| id.clone())
            .collect();

        println!(
            "📍 Filtering to {} papers from {}",
            filtered.len(),
            venue_filter
        );

        // Keep only filtered papers
        graph.papers.retain(|id, _| filtered.contains(id));
    }

    // Create cache directory if it doesn't exist
    std::fs::create_dir_all(cache_dir)?;

    println!("🌐 Fetching citation data from Semantic Scholar API...");
    println!("   (This may take a while due to rate limits)");

    // Fetch citations
    let cache_path = std::path::Path::new(cache_dir);
    let citation_cache = citations::fetch_all_citations(&graph, cache_path, max_papers).await?;

    println!("\n✅ Citation fetch complete!");
    println!("📊 Fetched data for {} papers", citation_cache.papers.len());

    // Show some statistics
    let total_citations: usize = citation_cache
        .papers
        .values()
        .map(|p| p.citation_count)
        .sum();
    let total_references: usize = citation_cache
        .papers
        .values()
        .map(|p| p.reference_count)
        .sum();

    println!("📈 Total citations: {}", total_citations);
    println!("📚 Total references: {}", total_references);

    // Show top cited papers
    let mut papers_by_citations: Vec<_> = citation_cache.papers.values().collect();
    papers_by_citations.sort_by_key(|p| std::cmp::Reverse(p.citation_count));

    println!("\n🏆 Top 10 Most Cited Papers:");
    for (i, paper) in papers_by_citations.iter().take(10).enumerate() {
        println!(
            "{}. {} ({} citations)",
            i + 1,
            paper.title,
            paper.citation_count
        );
    }

    Ok(())
}

fn run_build_site(
    bib_dir: &str,
    out_dir: &str,
    base_url: &str,
    static_dir: &str,
    book_dir: &str,
) -> Result<()> {
    println!("🏗️  Building static site");
    let report = crate::site::build_site(&crate::site::SiteOptions {
        bib_dir: bib_dir.into(),
        out_dir: out_dir.into(),
        base_url: base_url.to_string(),
        static_dir: static_dir.into(),
        book_dir: Some(book_dir.into()),
    })?;

    println!(
        "✅ Wrote {} HTML pages, {} JSON files and {} other files ({:.1} MB) to {}",
        report.html_files,
        report.json_files,
        report.other_files,
        report.bytes as f64 / 1_048_576.0,
        out_dir
    );
    if !report.book_included {
        println!(
            "⚠️  No rendered book found at {} — run `mdbook build docs` to include it",
            book_dir
        );
    }
    println!(
        "👀 Preview locally: python3 -m http.server -d {} 8080",
        out_dir
    );
    Ok(())
}

async fn run_extract_papers(
    bib_dir: &str,
    db_dir: &str,
    max_papers: Option<usize>,
    venue: Option<&str>,
) -> Result<()> {
    println!("📚 Loading bibliography from {}", bib_dir);

    // Parse bibliography files
    let mut parser = BibParser::new();
    let mut graph = parser.parse_directory(bib_dir)?;

    println!("🔍 Found {} papers", graph.papers.len());

    // Filter by venue if specified
    if let Some(venue_filter) = venue {
        let venue_upper = venue_filter.to_uppercase();
        let filtered: Vec<_> = graph
            .papers
            .iter()
            .filter(|(_, p)| p.venue.to_uppercase().contains(&venue_upper))
            .map(|(id, _)| id.clone())
            .collect();

        println!(
            "📍 Filtering to {} papers from {}",
            filtered.len(),
            venue_filter
        );

        // Keep only filtered papers
        graph.papers.retain(|id, _| filtered.contains(id));
    }

    // Create database directory if it doesn't exist
    std::fs::create_dir_all(db_dir)?;

    println!("🌐 Extracting paper metadata from online sources...");
    println!("   (This may take a while to be respectful to servers)");

    // Extract metadata
    let db_path = std::path::Path::new(db_dir);
    let paper_db = paper_extractor::extract_all_papers(&graph, db_path, max_papers).await?;

    println!("\n✅ Paper extraction complete!");
    println!("📊 Extracted metadata for {} papers", paper_db.papers.len());

    // Show statistics
    let with_pdf: usize = paper_db
        .papers
        .values()
        .filter(|p| p.pdf_url.is_some())
        .count();
    let with_doi: usize = paper_db.papers.values().filter(|p| p.doi.is_some()).count();
    let with_arxiv: usize = paper_db
        .papers
        .values()
        .filter(|p| p.arxiv_id.is_some())
        .count();
    let total_refs: usize = paper_db.papers.values().map(|p| p.reference_count).sum();

    println!("📄 Papers with PDF URLs: {}", with_pdf);
    println!("🔗 Papers with DOIs: {}", with_doi);
    println!("📝 Papers from ArXiv: {}", with_arxiv);
    println!("📚 Total references extracted: {}", total_refs);

    // Show papers with most references extracted
    let mut papers_by_refs: Vec<_> = paper_db.papers.values().collect();
    papers_by_refs.sort_by_key(|p| std::cmp::Reverse(p.reference_count));

    if !papers_by_refs.is_empty() {
        println!("\n🏆 Papers with Most References Extracted:");
        for (i, paper) in papers_by_refs.iter().take(5).enumerate() {
            println!(
                "{}. {} ({} references)",
                i + 1,
                paper.title,
                paper.reference_count
            );
        }
    }

    Ok(())
}
