mod models;
mod parser;
mod algorithm;
mod export;
mod cli;
mod utils;
mod web;

use anyhow::Result;
use clap::Parser;
use env_logger::Env;
use log::info;

use crate::cli::{Cli, Commands};
use crate::parser::BibParser;
use crate::algorithm::PageRankCalculator;
use crate::export::Exporter;

fn main() -> Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    
    // Parse CLI arguments
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Calculate { bib_dir, top, output, export } => {
            run_calculate(&bib_dir, top, output.as_deref(), export)?;
        }
        Commands::Venues { bib_dir, top, field, tier } => {
            run_venues(&bib_dir, top, field.as_deref(), tier.as_deref())?;
        }
        Commands::Scholars { bib_dir, top, min_papers } => {
            run_scholars(&bib_dir, top, min_papers)?;
        }
        Commands::Search { bib_dir, query } => {
            run_search(&bib_dir, &query)?;
        }
        Commands::Stats { bib_dir } => {
            run_stats(&bib_dir)?;
        }
        Commands::Web { bib_dir, port } => {
            // Use tokio runtime for web server
            let runtime = tokio::runtime::Runtime::new()?;
            runtime.block_on(async {
                run_web_server(&bib_dir, port).await
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
    
    println!("📚 Loaded {} papers, {} venues, {} scholars",
             graph.papers.len(),
             graph.venues.len(),
             graph.scholars.len());
    
    // Calculate PageRank and QIndex
    println!("🔄 Calculating QIndex scores...");
    let mut calculator = PageRankCalculator::new(&graph);
    let metrics = calculator.calculate()?;
    
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
    calculator.calculate()?;
    
    let venues = calculator.get_top_venues(top, field, tier);
    display_venues(&venues);
    
    Ok(())
}

fn run_scholars(bib_dir: &str, top: usize, min_papers: Option<usize>) -> Result<()> {
    let mut parser = BibParser::new();
    let graph = parser.parse_directory(bib_dir)?;
    
    let mut calculator = PageRankCalculator::new(&graph);
    calculator.calculate()?;
    
    let scholars = calculator.get_top_scholars(top, min_papers);
    display_scholars(&scholars);
    
    Ok(())
}

fn run_search(bib_dir: &str, query: &str) -> Result<()> {
    let mut parser = BibParser::new();
    let graph = parser.parse_directory(bib_dir)?;
    
    let mut calculator = PageRankCalculator::new(&graph);
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
    use comfy_table::{Table, presets::UTF8_FULL};
    
    let venues = calculator.get_top_venues(top, None, None);
    
    let mut table = Table::new();
    table.load_preset(UTF8_FULL)
        .set_header(vec!["Rank", "Venue", "Tier", "Field", "PageRank", "Impact", "Papers"]);
    
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
    use comfy_table::{Table, presets::UTF8_FULL};
    
    let scholars = calculator.get_top_scholars(top, None);
    
    let mut table = Table::new();
    table.load_preset(UTF8_FULL)
        .set_header(vec!["Rank", "Scholar", "QIndex", "H-Index", "Papers", "Citations"]);
    
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
    use comfy_table::{Table, presets::UTF8_FULL};
    
    let mut table = Table::new();
    table.load_preset(UTF8_FULL)
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
    use comfy_table::{Table, presets::UTF8_FULL};
    
    let mut table = Table::new();
    table.load_preset(UTF8_FULL)
        .set_header(vec!["Scholar", "QIndex", "H-Index", "Papers", "Citations"]);
    
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

async fn run_web_server(bib_dir: &str, port: u16) -> Result<()> {
    println!("🌐 Starting QIndex Web Server");
    println!("📚 Loading data from: {}", bib_dir);
    println!("🚀 Server will be available at: http://127.0.0.1:{}", port);
    println!();
    
    crate::web::server::start_server(bib_dir, port).await?;
    Ok(())
}