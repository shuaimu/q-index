use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use once_cell::sync::Lazy;
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S2AGCitationData {
    pub title: String,
    pub year: Option<String>,
    pub venue: String,
    pub corpus_id: Option<u64>,
    pub s2ag_title: Option<String>,
    pub s2ag_year: Option<u32>,
    pub doi: Option<String>,
    pub citation_count: usize,
    pub reference_count: usize,
    pub authors: Vec<String>,
    pub venue_info: Option<String>,
    pub fields: Option<Vec<serde_json::Value>>,
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct S2AGCitationIndex {
    pub citations: HashMap<String, S2AGCitationData>,
    pub total_citations: usize,
    pub papers_with_citations: usize,
}

impl S2AGCitationIndex {
    pub fn new() -> Self {
        Self {
            citations: HashMap::new(),
            total_citations: 0,
            papers_with_citations: 0,
        }
    }
    
    pub fn load_from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            log::warn!("S2AG citation file not found: {:?}", path);
            return Ok(Self::new());
        }
        
        let contents = fs::read_to_string(path)?;
        let citations: HashMap<String, S2AGCitationData> = serde_json::from_str(&contents)?;
        
        let total_citations: usize = citations.values()
            .map(|c| c.citation_count)
            .sum();
        
        let papers_with_citations = citations.values()
            .filter(|c| c.citation_count > 0)
            .count();
        
        log::info!("Loaded S2AG citations: {} papers with {} total citations", 
                  citations.len(), total_citations);
        
        Ok(Self {
            citations,
            total_citations,
            papers_with_citations,
        })
    }
    
    pub fn get_citation_count_for_paper(&self, title: &str) -> usize {
        // Normalize title for matching
        let normalized = self.normalize_title(title);
        
        // Try exact match first
        if let Some(data) = self.citations.get(&normalized) {
            return data.citation_count;
        }
        
        // Try fuzzy match
        for (key, data) in &self.citations {
            if self.titles_match(&normalized, key) || self.titles_match(title, &data.title) {
                return data.citation_count;
            }
        }
        
        0  // Return 0 if not found
    }
    
    pub fn get_citation_data(&self, title: &str) -> Option<&S2AGCitationData> {
        let normalized = self.normalize_title(title);
        
        // Try exact match
        if let Some(data) = self.citations.get(&normalized) {
            return Some(data);
        }
        
        // Try fuzzy match
        for (key, data) in &self.citations {
            if self.titles_match(&normalized, key) || self.titles_match(title, &data.title) {
                return Some(data);
            }
        }
        
        None
    }
    
    fn normalize_title(&self, title: &str) -> String {
        title.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
    
    fn titles_match(&self, title1: &str, title2: &str) -> bool {
        let t1 = self.normalize_title(title1);
        let t2 = self.normalize_title(title2);
        
        // Check if one title contains the other (handles subtitles)
        if t1.contains(&t2) || t2.contains(&t1) {
            return true;
        }
        
        // Simple word-based similarity check
        let words1: Vec<_> = t1.split_whitespace().collect();
        let words2: Vec<_> = t2.split_whitespace().collect();
        
        if words1.len() < 3 || words2.len() < 3 {
            return false;
        }
        
        // Check if most significant words match
        let matches = words1.iter()
            .filter(|w| w.len() > 3)  // Skip short words
            .filter(|w| words2.contains(w))
            .count();
        
        let significant_words = words1.iter().filter(|w| w.len() > 3).count();
        if significant_words == 0 {
            return false;
        }
        
        let similarity = matches as f32 / significant_words as f32;
        similarity > 0.7
    }
}

// Global S2AG citation index
pub static S2AG_CITATIONS: Lazy<RwLock<S2AGCitationIndex>> = Lazy::new(|| {
    let path = Path::new("cache/citations/s2ag_citations.json");
    let index = S2AGCitationIndex::load_from_file(path)
        .unwrap_or_else(|e| {
            log::error!("Failed to load S2AG citations: {}", e);
            S2AGCitationIndex::new()
        });
    RwLock::new(index)
});

pub fn reload_s2ag_citations() -> Result<()> {
    let path = Path::new("cache/citations/s2ag_citations.json");
    let new_index = S2AGCitationIndex::load_from_file(path)?;
    
    let mut index = S2AG_CITATIONS.write().unwrap();
    *index = new_index;
    
    log::info!("Reloaded S2AG citations");
    Ok(())
}

pub fn get_citation_count(title: &str) -> usize {
    S2AG_CITATIONS.read().unwrap().get_citation_count_for_paper(title)
}