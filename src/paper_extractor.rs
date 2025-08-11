use anyhow::{Result, Context, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tokio::time::{sleep, Duration};
use log::{info, warn, debug, error};
use reqwest::Client;
use std::fs;
use crate::paper_finder::{PaperFinder, parse_reference};

/// Metadata for a paper including citations extracted from PDF
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperMetadata {
    pub paper_id: String,           // Our internal ID from BibTeX
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<u32>,
    pub venue: String,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub pdf_url: Option<String>,
    pub abstract_text: Option<String>,
    pub extracted_references: Vec<ExtractedReference>,
    pub reference_count: usize,
    pub extraction_date: chrono::DateTime<chrono::Utc>,
    pub extraction_method: String,  // "pdf", "html", "manual", etc.
}

/// A reference extracted from a paper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedReference {
    pub raw_text: String,
    pub parsed_title: Option<String>,
    pub parsed_authors: Option<Vec<String>>,
    pub parsed_venue: Option<String>,
    pub parsed_year: Option<u32>,
    pub matched_paper_id: Option<String>,  // If we can match it to our dataset
}

/// Database of extracted paper metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperDatabase {
    pub papers: HashMap<String, PaperMetadata>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub version: String,
}

impl PaperDatabase {
    pub fn new() -> Self {
        Self {
            papers: HashMap::new(),
            last_updated: chrono::Utc::now(),
            version: "1.0".to_string(),
        }
    }
    
    pub fn load_from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        
        let contents = fs::read_to_string(path)
            .context("Failed to read paper database file")?;
        
        serde_json::from_str(&contents)
            .context("Failed to parse paper database JSON")
    }
    
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize paper database")?;
        
        fs::write(path, json)
            .context("Failed to write paper database file")?;
        
        Ok(())
    }
}

/// Paper extractor that finds and extracts metadata from papers
pub struct PaperExtractor {
    client: Client,
    database: PaperDatabase,
    database_path: PathBuf,
    pdf_cache_dir: PathBuf,
}

impl PaperExtractor {
    pub fn new(database_dir: &Path) -> Result<Self> {
        let database_path = database_dir.join("paper_metadata.json");
        let pdf_cache_dir = database_dir.join("pdfs");
        
        // Create PDF cache directory if it doesn't exist
        fs::create_dir_all(&pdf_cache_dir)?;
        
        let database = PaperDatabase::load_from_file(&database_path)?;
        
        Ok(Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
                .timeout(Duration::from_secs(30))
                .build()?,
            database,
            database_path,
            pdf_cache_dir,
        })
    }
    
    /// Search for a paper online and find a publicly available PDF
    pub async fn find_paper_online(
        &self,
        title: &str,
        authors: &[String],
        year: Option<u32>,
    ) -> Result<Option<PaperSearchResult>> {
        // Try multiple strategies to find the paper
        
        // 1. Try ArXiv search
        if let Some(result) = self.search_arxiv(title, authors, year).await? {
            return Ok(Some(result));
        }
        
        // 2. Try finding via DOI (search for DOI first)
        if let Some(doi) = self.find_doi(title, authors, year).await? {
            if let Some(result) = self.find_via_doi(&doi).await? {
                return Ok(Some(result));
            }
        }
        
        // 3. Try direct conference websites (USENIX, ACM, etc.)
        if let Some(result) = self.search_conference_sites(title, authors, year).await? {
            return Ok(Some(result));
        }
        
        // 4. Try Google Scholar (carefully to avoid rate limiting)
        if let Some(result) = self.search_google_scholar(title, authors, year).await? {
            return Ok(Some(result));
        }
        
        Ok(None)
    }
    
    /// Search ArXiv for the paper
    async fn search_arxiv(
        &self,
        title: &str,
        _authors: &[String],
        _year: Option<u32>,
    ) -> Result<Option<PaperSearchResult>> {
        // Clean title for search
        let clean_title = title
            .replace("{", "")
            .replace("}", "")
            .replace(":", " ")
            .to_lowercase();
        
        // ArXiv API search
        let url = format!(
            "http://export.arxiv.org/api/query?search_query=ti:\"{}\"&max_results=3",
            urlencoding::encode(&clean_title)
        );
        
        debug!("Searching ArXiv: {}", url);
        
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            return Ok(None);
        }
        
        let body = response.text().await?;
        
        // Parse ArXiv XML response
        if let Some(arxiv_id) = self.parse_arxiv_response(&body, title) {
            // Construct PDF URL
            let pdf_url = format!("https://arxiv.org/pdf/{}.pdf", arxiv_id);
            
            return Ok(Some(PaperSearchResult {
                pdf_url: Some(pdf_url),
                doi: None,
                arxiv_id: Some(arxiv_id),
                source: "arxiv".to_string(),
            }));
        }
        
        Ok(None)
    }
    
    /// Parse ArXiv XML response to find matching paper
    fn parse_arxiv_response(&self, xml: &str, target_title: &str) -> Option<String> {
        // Simple XML parsing for ArXiv response
        let entries = xml.split("<entry>").skip(1);
        
        for entry in entries {
            // Extract title
            if let Some(title_start) = entry.find("<title>") {
                if let Some(title_end) = entry.find("</title>") {
                    let title = &entry[title_start + 7..title_end];
                    let title_clean = title.trim().to_lowercase();
                    let target_clean = target_title.to_lowercase();
                    
                    // Check if titles match (fuzzy matching)
                    if self.titles_match(&title_clean, &target_clean) {
                        // Extract ArXiv ID
                        if let Some(id_start) = entry.find("<id>http://arxiv.org/abs/") {
                            let id_part = &entry[id_start + 26..];
                            if let Some(id_end) = id_part.find("</id>") {
                                return Some(id_part[..id_end].to_string());
                            }
                        }
                    }
                }
            }
        }
        
        None
    }
    
    /// Check if two titles match (fuzzy matching)
    fn titles_match(&self, title1: &str, title2: &str) -> bool {
        // Remove common words and punctuation for comparison
        let clean1 = title1
            .replace(&['{', '}', ':', '-', ',', '.', '(', ')', '[', ']'][..], " ")
            .to_lowercase();
        let clean2 = title2
            .replace(&['{', '}', ':', '-', ',', '.', '(', ')', '[', ']'][..], " ")
            .to_lowercase();
        
        // Split into words and check overlap
        let words1: HashSet<_> = clean1.split_whitespace()
            .filter(|w| w.len() > 2)
            .collect();
        let words2: HashSet<_> = clean2.split_whitespace()
            .filter(|w| w.len() > 2)
            .collect();
        
        if words1.is_empty() || words2.is_empty() {
            return false;
        }
        
        let intersection = words1.intersection(&words2).count();
        let min_len = words1.len().min(words2.len());
        
        // At least 80% word overlap
        intersection as f64 / min_len as f64 > 0.8
    }
    
    /// Find DOI for a paper
    async fn find_doi(
        &self,
        title: &str,
        _authors: &[String],
        _year: Option<u32>,
    ) -> Result<Option<String>> {
        // Try CrossRef API (free and open)
        let clean_title = title.replace("{", "").replace("}", "");
        let url = format!(
            "https://api.crossref.org/works?query.title={}&rows=3",
            urlencoding::encode(&clean_title)
        );
        
        debug!("Searching CrossRef for DOI: {}", url);
        
        let response = self.client
            .get(&url)
            .header("User-Agent", "qindex/1.0 (mailto:research@example.com)")
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Ok(None);
        }
        
        let json: serde_json::Value = response.json().await?;
        
        // Parse CrossRef response
        if let Some(items) = json["message"]["items"].as_array() {
            for item in items {
                if let Some(item_title) = item["title"][0].as_str() {
                    if self.titles_match(item_title, title) {
                        if let Some(doi) = item["DOI"].as_str() {
                            return Ok(Some(doi.to_string()));
                        }
                    }
                }
            }
        }
        
        Ok(None)
    }
    
    /// Find paper via DOI
    async fn find_via_doi(&self, doi: &str) -> Result<Option<PaperSearchResult>> {
        // Check if DOI resolves to a PDF
        // Many publishers provide PDFs via DOI
        
        // For now, just record the DOI
        // In practice, we'd need to handle publisher-specific access
        Ok(Some(PaperSearchResult {
            pdf_url: None,
            doi: Some(doi.to_string()),
            arxiv_id: None,
            source: "doi".to_string(),
        }))
    }
    
    /// Search conference websites directly
    async fn search_conference_sites(
        &self,
        title: &str,
        _authors: &[String],
        year: Option<u32>,
    ) -> Result<Option<PaperSearchResult>> {
        // USENIX papers are often freely available
        if title.to_lowercase().contains("osdi") || title.to_lowercase().contains("sosp") {
            if let Some(result) = self.search_usenix(title, year).await? {
                return Ok(Some(result));
            }
        }
        
        Ok(None)
    }
    
    /// Search USENIX website
    async fn search_usenix(
        &self,
        title: &str,
        year: Option<u32>,
    ) -> Result<Option<PaperSearchResult>> {
        // USENIX provides free PDFs for many conferences
        // Format: https://www.usenix.org/conference/osdi20/presentation/[author-lastname]
        
        // This would require more sophisticated scraping
        // For now, return None
        Ok(None)
    }
    
    /// Search Google Scholar (carefully)
    async fn search_google_scholar(
        &self,
        title: &str,
        _authors: &[String],
        _year: Option<u32>,
    ) -> Result<Option<PaperSearchResult>> {
        // Google Scholar requires very careful handling to avoid blocking
        // We'll skip this for now to avoid rate limiting issues
        Ok(None)
    }
    
    /// Download PDF from URL
    pub async fn download_pdf(&self, url: &str, paper_id: &str) -> Result<PathBuf> {
        let pdf_path = self.pdf_cache_dir.join(format!("{}.pdf", paper_id));
        
        // Check if already downloaded
        if pdf_path.exists() {
            debug!("PDF already cached: {}", pdf_path.display());
            return Ok(pdf_path);
        }
        
        info!("Downloading PDF from: {}", url);
        
        let response = self.client.get(url).send().await?;
        if !response.status().is_success() {
            return Err(anyhow!("Failed to download PDF: {}", response.status()));
        }
        
        let bytes = response.bytes().await?;
        fs::write(&pdf_path, bytes)?;
        
        info!("Downloaded PDF to: {}", pdf_path.display());
        Ok(pdf_path)
    }
    
    /// Extract references from PDF
    pub fn extract_references_from_pdf(&self, pdf_path: &Path) -> Result<Vec<ExtractedReference>> {
        // We'll use pdf-extract crate or similar
        // For now, implement a simple version
        
        // This would require a PDF parsing library
        // For demonstration, return empty vec
        warn!("PDF extraction not yet implemented");
        Ok(Vec::new())
    }
    
    /// Extract metadata for a paper
    pub async fn extract_paper_metadata(
        &mut self,
        paper_id: &str,
        title: &str,
        authors: &[String],
        year: Option<u32>,
        venue: &str,
    ) -> Result<PaperMetadata> {
        // Check if already in database
        if let Some(existing) = self.database.papers.get(paper_id) {
            let age = chrono::Utc::now() - existing.extraction_date;
            if age.num_days() < 30 {
                debug!("Using cached metadata for {}", title);
                return Ok(existing.clone());
            }
        }
        
        info!("Extracting metadata for: {}", title);
        
        // Use the improved paper finder
        let finder = PaperFinder::new();
        
        // Try to find PDF URL using venue-specific strategies
        let pdf_url = finder.find_paper_pdf(title, authors, venue, year).await?;
        
        // Get CrossRef metadata
        let crossref_meta = finder.get_crossref_metadata(title).await?;
        
        // Search for paper online using existing methods as fallback
        let search_result = if pdf_url.is_none() {
            self.find_paper_online(title, authors, year).await?
        } else {
            None
        };
        
        let mut metadata = PaperMetadata {
            paper_id: paper_id.to_string(),
            title: title.to_string(),
            authors: authors.to_vec(),
            year,
            venue: venue.to_string(),
            doi: crossref_meta.as_ref().and_then(|m| m.doi.clone()),
            arxiv_id: None,
            pdf_url: pdf_url.clone(),
            abstract_text: None,
            extracted_references: Vec::new(),
            reference_count: 0,
            extraction_date: chrono::Utc::now(),
            extraction_method: if pdf_url.is_some() { "venue_search".to_string() } else { "none".to_string() },
        };
        
        // Use search result if we didn't find PDF through venue search
        if pdf_url.is_none() && search_result.is_some() {
            let result = search_result.unwrap();
            metadata.doi = metadata.doi.or(result.doi);
            metadata.arxiv_id = result.arxiv_id;
            metadata.pdf_url = result.pdf_url.clone();
            metadata.extraction_method = result.source;
        }
        
        // If we have a PDF URL, download and extract
        if let Some(pdf_url) = &metadata.pdf_url {
            match self.download_pdf(pdf_url, paper_id).await {
                Ok(pdf_path) => {
                    match self.extract_references_from_pdf(&pdf_path) {
                        Ok(refs) => {
                            metadata.reference_count = refs.len();
                            metadata.extracted_references = refs;
                        }
                        Err(e) => {
                            warn!("Failed to extract references: {}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to download PDF: {}", e);
                }
            }
        }
        
        // Save to database
        self.database.papers.insert(paper_id.to_string(), metadata.clone());
        self.database.last_updated = chrono::Utc::now();
        
        // Save database periodically
        if self.database.papers.len() % 10 == 0 {
            self.save_database()?;
        }
        
        Ok(metadata)
    }
    
    /// Save database to disk
    pub fn save_database(&self) -> Result<()> {
        self.database.save_to_file(&self.database_path)?;
        info!("Saved paper database with {} papers", self.database.papers.len());
        Ok(())
    }
}

/// Result from searching for a paper online
#[derive(Debug)]
struct PaperSearchResult {
    pdf_url: Option<String>,
    doi: Option<String>,
    arxiv_id: Option<String>,
    source: String,
}

/// Extract all papers from a BibTeX graph
pub async fn extract_all_papers(
    graph: &crate::models::CitationGraph,
    database_dir: &Path,
    max_papers: Option<usize>,
) -> Result<PaperDatabase> {
    let mut extractor = PaperExtractor::new(database_dir)?;
    
    info!("Starting metadata extraction for {} papers", graph.papers.len());
    
    let mut count = 0;
    let max = max_papers.unwrap_or(graph.papers.len());
    
    for (paper_id, paper) in graph.papers.iter() {
        if count >= max {
            break;
        }
        
        // Skip if no venue
        if paper.venue.is_empty() {
            continue;
        }
        
        info!("Extracting metadata for paper {}/{}: {}", count + 1, max, paper.title);
        
        match extractor.extract_paper_metadata(
            paper_id,
            &paper.title,
            &paper.authors,
            paper.year,
            &paper.venue,
        ).await {
            Ok(_metadata) => {
                count += 1;
                // Add delay to be respectful
                sleep(Duration::from_secs(2)).await;
            }
            Err(e) => {
                error!("Failed to extract metadata for {}: {}", paper.title, e);
            }
        }
    }
    
    // Final save
    extractor.save_database()?;
    
    info!("Metadata extraction complete. Extracted {} papers", count);
    
    Ok(extractor.database)
}