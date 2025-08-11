use std::fs::File;
use std::io::Write;
use std::path::Path;
use anyhow::{Result, Context};
use serde::{Serialize, Deserialize};
use csv::Writer;

use crate::models::{CitationGraph, VenueRanking, ScholarRanking};
use crate::algorithm::PageRankCalculator;

pub struct Exporter<'a> {
    graph: &'a CitationGraph,
    calculator: &'a PageRankCalculator<'a>,
}

#[derive(Serialize, Deserialize)]
struct ExportData {
    metadata: ExportMetadata,
    venues: Vec<VenueExport>,
    scholars: Vec<ScholarExport>,
    statistics: DatasetStatistics,
}

#[derive(Serialize, Deserialize)]
struct ExportMetadata {
    version: String,
    algorithm: String,
    date: String,
    parameters: crate::models::AlgorithmParams,
}

#[derive(Serialize, Deserialize)]
struct VenueExport {
    rank: usize,
    id: String,
    name: String,
    tier: String,
    field: String,
    pagerank: f64,
    impact_factor: f64,
    paper_count: usize,
}

#[derive(Serialize, Deserialize)]
struct ScholarExport {
    rank: usize,
    id: String,
    name: String,
    qindex: f64,
    h_index: usize,
    paper_count: usize,
    citation_count: usize,
    top_venues: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct DatasetStatistics {
    total_papers: usize,
    total_venues: usize,
    total_scholars: usize,
    papers_by_year: std::collections::HashMap<u32, usize>,
    papers_by_tier: std::collections::HashMap<String, usize>,
    papers_by_field: std::collections::HashMap<String, usize>,
}

impl<'a> Exporter<'a> {
    pub fn new(graph: &'a CitationGraph, calculator: &'a PageRankCalculator<'a>) -> Self {
        Self { graph, calculator }
    }
    
    pub fn export(&self, filename: &str, top_n: usize) -> Result<()> {
        let path = Path::new(filename);
        let extension = path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("json");
        
        match extension {
            "json" => self.export_json(filename, top_n),
            "csv" => self.export_csv(filename, top_n),
            _ => self.export_json(filename, top_n),
        }
    }
    
    fn export_json(&self, filename: &str, top_n: usize) -> Result<()> {
        let data = self.prepare_export_data(top_n);
        
        let mut file = File::create(filename)
            .with_context(|| format!("Failed to create file {}", filename))?;
        
        let json = serde_json::to_string_pretty(&data)
            .context("Failed to serialize data to JSON")?;
        
        file.write_all(json.as_bytes())
            .context("Failed to write JSON to file")?;
        
        Ok(())
    }
    
    fn export_csv(&self, filename: &str, top_n: usize) -> Result<()> {
        let base_path = Path::new(filename);
        let parent = base_path.parent().unwrap_or(Path::new("."));
        let stem = base_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("qindex");
        
        // Export venues CSV
        let venues_file = parent.join(format!("{}_venues.csv", stem));
        self.export_venues_csv(&venues_file, top_n)?;
        
        // Export scholars CSV
        let scholars_file = parent.join(format!("{}_scholars.csv", stem));
        self.export_scholars_csv(&scholars_file, top_n)?;
        
        Ok(())
    }
    
    fn export_venues_csv(&self, path: &Path, top_n: usize) -> Result<()> {
        let mut writer = Writer::from_path(path)
            .with_context(|| format!("Failed to create CSV file {:?}", path))?;
        
        // Write header
        writer.write_record(&["Rank", "Venue", "Tier", "Field", "PageRank", "Impact Factor", "Papers"])?;
        
        // Write data
        let venues = self.calculator.get_top_venues(top_n, None, None);
        for (i, venue) in venues.iter().enumerate() {
            writer.write_record(&[
                (i + 1).to_string(),
                venue.name.clone(),
                venue.tier.clone(),
                venue.field.clone(),
                format!("{:.6}", venue.pagerank),
                format!("{:.6}", venue.impact_factor),
                venue.paper_count.to_string(),
            ])?;
        }
        
        writer.flush()?;
        Ok(())
    }
    
    fn export_scholars_csv(&self, path: &Path, top_n: usize) -> Result<()> {
        let mut writer = Writer::from_path(path)
            .with_context(|| format!("Failed to create CSV file {:?}", path))?;
        
        // Write header
        writer.write_record(&["Rank", "Scholar", "QIndex", "H-Index", "Papers", "Citations"])?;
        
        // Write data
        let scholars = self.calculator.get_top_scholars(top_n, None);
        for (i, scholar) in scholars.iter().enumerate() {
            writer.write_record(&[
                (i + 1).to_string(),
                scholar.name.clone(),
                format!("{:.2}", scholar.qindex),
                scholar.h_index.to_string(),
                scholar.paper_count.to_string(),
                scholar.citation_count.to_string(),
            ])?;
        }
        
        writer.flush()?;
        Ok(())
    }
    
    fn prepare_export_data(&self, top_n: usize) -> ExportData {
        let venues = self.calculator.get_top_venues(top_n, None, None);
        let scholars = self.calculator.get_top_scholars(top_n, None);
        
        let venue_exports: Vec<VenueExport> = venues.iter().enumerate()
            .map(|(i, v)| VenueExport {
                rank: i + 1,
                id: v.id.clone(),
                name: v.name.clone(),
                tier: v.tier.clone(),
                field: v.field.clone(),
                pagerank: v.pagerank,
                impact_factor: v.impact_factor,
                paper_count: v.paper_count,
            })
            .collect();
        
        let scholar_exports: Vec<ScholarExport> = scholars.iter().enumerate()
            .map(|(i, s)| ScholarExport {
                rank: i + 1,
                id: s.id.clone(),
                name: s.name.clone(),
                qindex: s.qindex,
                h_index: s.h_index,
                paper_count: s.paper_count,
                citation_count: s.citation_count,
                top_venues: s.top_venues.clone(),
            })
            .collect();
        
        let statistics = self.calculate_statistics();
        
        ExportData {
            metadata: ExportMetadata {
                version: "0.1.0".to_string(),
                algorithm: "PageRank-based QIndex".to_string(),
                date: chrono::Utc::now().to_rfc3339(),
                parameters: crate::models::AlgorithmParams::default(),
            },
            venues: venue_exports,
            scholars: scholar_exports,
            statistics,
        }
    }
    
    fn calculate_statistics(&self) -> DatasetStatistics {
        use std::collections::HashMap;
        
        let mut papers_by_year = HashMap::new();
        for paper in self.graph.papers.values() {
            if let Some(year) = paper.year {
                *papers_by_year.entry(year).or_insert(0) += 1;
            }
        }
        
        let mut papers_by_tier = HashMap::new();
        let mut papers_by_field = HashMap::new();
        
        for venue in self.graph.venues.values() {
            let paper_count = venue.papers.len();
            *papers_by_tier.entry(venue.tier.clone()).or_insert(0) += paper_count;
            *papers_by_field.entry(venue.field.clone()).or_insert(0) += paper_count;
        }
        
        DatasetStatistics {
            total_papers: self.graph.papers.len(),
            total_venues: self.graph.venues.len(),
            total_scholars: self.graph.scholars.len(),
            papers_by_year,
            papers_by_tier,
            papers_by_field,
        }
    }
}