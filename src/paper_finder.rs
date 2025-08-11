use anyhow::{Result, Context, anyhow};
use log::{info, warn, debug};
use reqwest::Client;
use tokio::time::{sleep, Duration};
use std::collections::HashMap;

/// Advanced paper finder that knows about conference websites
pub struct PaperFinder {
    client: Client,
}

impl PaperFinder {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }
    
    /// Find a paper's PDF URL by searching various sources
    pub async fn find_paper_pdf(
        &self,
        title: &str,
        authors: &[String],
        venue: &str,
        year: Option<u32>,
    ) -> Result<Option<String>> {
        let venue_upper = venue.to_uppercase();
        
        // Try venue-specific strategies
        if venue_upper.contains("OSDI") || venue_upper.contains("ATC") || venue_upper.contains("FAST") {
            if let Some(url) = self.find_usenix_paper(title, venue, year).await? {
                return Ok(Some(url));
            }
        }
        
        if venue_upper.contains("SOSP") {
            if let Some(url) = self.find_acm_paper(title, venue, year).await? {
                return Ok(Some(url));
            }
        }
        
        if venue_upper.contains("NSDI") {
            if let Some(url) = self.find_usenix_paper(title, venue, year).await? {
                return Ok(Some(url));
            }
        }
        
        // Try arXiv as fallback
        if let Some(url) = self.find_arxiv_paper(title, authors).await? {
            return Ok(Some(url));
        }
        
        // Try author homepages (simplified)
        if let Some(url) = self.find_on_author_page(title, authors).await? {
            return Ok(Some(url));
        }
        
        Ok(None)
    }
    
    /// Find paper on USENIX website
    async fn find_usenix_paper(
        &self,
        title: &str,
        venue: &str,
        year: Option<u32>,
    ) -> Result<Option<String>> {
        // USENIX has a predictable URL structure
        // Example: https://www.usenix.org/conference/osdi24/presentation/[lastname]
        
        let conference = if venue.contains("OSDI") {
            "osdi"
        } else if venue.contains("ATC") {
            "atc"
        } else if venue.contains("FAST") {
            "fast"
        } else if venue.contains("NSDI") {
            "nsdi"
        } else {
            return Ok(None);
        };
        
        // Try to construct USENIX search URL
        if let Some(year) = year {
            let year_short = year % 100;
            let search_url = format!(
                "https://www.usenix.org/conference/{}{}/technical-sessions",
                conference, year_short
            );
            
            debug!("Checking USENIX at: {}", search_url);
            
            // Try to fetch the page
            match self.client.get(&search_url).send().await {
                Ok(response) if response.status().is_success() => {
                    let html = response.text().await?;
                    
                    // Search for the paper title in the HTML
                    let title_lower = title.to_lowercase()
                        .replace("{", "")
                        .replace("}", "")
                        .replace(":", "");
                    
                    if html.to_lowercase().contains(&title_lower) {
                        // Try to find PDF link
                        // USENIX usually has links like: /system/files/conference/osdi14/osdi14-paper-lastname.pdf
                        if let Some(pdf_url) = self.extract_usenix_pdf_url(&html, &title_lower) {
                            return Ok(Some(pdf_url));
                        }
                    }
                }
                _ => {}
            }
        }
        
        Ok(None)
    }
    
    /// Extract USENIX PDF URL from HTML
    fn extract_usenix_pdf_url(&self, html: &str, title_search: &str) -> Option<String> {
        // Look for PDF links near the title
        // Pattern: href="/system/files/conference/[conf]/[conf]-paper-[author].pdf"
        
        // Find title position
        if let Some(title_pos) = html.to_lowercase().find(title_search) {
            // Look for PDF link within 2000 chars after title
            let search_window = &html[title_pos..std::cmp::min(title_pos + 2000, html.len())];
            
            // Find PDF link
            if let Some(pdf_start) = search_window.find("/system/files/") {
                let pdf_part = &search_window[pdf_start..];
                if let Some(pdf_end) = pdf_part.find(".pdf") {
                    let pdf_path = &pdf_part[..pdf_end + 4];
                    return Some(format!("https://www.usenix.org{}", pdf_path));
                }
            }
        }
        
        None
    }
    
    /// Find paper on ACM Digital Library
    async fn find_acm_paper(
        &self,
        _title: &str,
        _venue: &str,
        _year: Option<u32>,
    ) -> Result<Option<String>> {
        // ACM DL usually requires subscription
        // We can find the DOI but not always free PDF
        Ok(None)
    }
    
    /// Find paper on arXiv
    async fn find_arxiv_paper(
        &self,
        title: &str,
        _authors: &[String],
    ) -> Result<Option<String>> {
        let clean_title = title
            .replace("{", "")
            .replace("}", "")
            .replace(":", " ")
            .to_lowercase();
        
        let url = format!(
            "http://export.arxiv.org/api/query?search_query=ti:\"{}\"&max_results=1",
            urlencoding::encode(&clean_title)
        );
        
        debug!("Searching ArXiv: {}", url);
        
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            return Ok(None);
        }
        
        let body = response.text().await?;
        
        // Parse ArXiv XML response
        if let Some(arxiv_id) = self.parse_arxiv_id(&body, title) {
            let pdf_url = format!("https://arxiv.org/pdf/{}.pdf", arxiv_id);
            return Ok(Some(pdf_url));
        }
        
        Ok(None)
    }
    
    /// Parse ArXiv ID from XML response
    fn parse_arxiv_id(&self, xml: &str, target_title: &str) -> Option<String> {
        if let Some(entry_start) = xml.find("<entry>") {
            let entry = &xml[entry_start..];
            
            // Check title match
            if let Some(title_start) = entry.find("<title>") {
                if let Some(title_end) = entry.find("</title>") {
                    let title = &entry[title_start + 7..title_end];
                    
                    // Simple title matching
                    if self.titles_similar(title, target_title) {
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
    
    /// Check if two titles are similar
    fn titles_similar(&self, title1: &str, title2: &str) -> bool {
        let clean1 = title1.to_lowercase()
            .replace(&['{', '}', ':', '-', ',', '.'][..], " ");
        let clean2 = title2.to_lowercase()
            .replace(&['{', '}', ':', '-', ',', '.'][..], " ");
        
        // Count matching words
        let words1: Vec<_> = clean1.split_whitespace().filter(|w| w.len() > 3).collect();
        let words2: Vec<_> = clean2.split_whitespace().filter(|w| w.len() > 3).collect();
        
        if words1.is_empty() || words2.is_empty() {
            return false;
        }
        
        let matches = words1.iter().filter(|w| words2.contains(w)).count();
        matches as f64 / words1.len().min(words2.len()) as f64 > 0.7
    }
    
    /// Try to find paper on author's homepage
    async fn find_on_author_page(
        &self,
        _title: &str,
        _authors: &[String],
    ) -> Result<Option<String>> {
        // This would require finding author homepages and searching them
        // Too complex for now
        Ok(None)
    }
    
    /// Get metadata from CrossRef
    pub async fn get_crossref_metadata(
        &self,
        title: &str,
    ) -> Result<Option<CrossRefMetadata>> {
        let clean_title = title.replace("{", "").replace("}", "");
        let url = format!(
            "https://api.crossref.org/works?query.title={}&rows=1",
            urlencoding::encode(&clean_title)
        );
        
        debug!("Searching CrossRef: {}", url);
        
        let response = self.client
            .get(&url)
            .header("User-Agent", "qindex/1.0 (research@example.com)")
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Ok(None);
        }
        
        let json: serde_json::Value = response.json().await?;
        
        if let Some(items) = json["message"]["items"].as_array() {
            if let Some(item) = items.first() {
                // Check title match
                if let Some(item_title) = item["title"][0].as_str() {
                    if self.titles_similar(item_title, title) {
                        return Ok(Some(CrossRefMetadata {
                            doi: item["DOI"].as_str().map(|s| s.to_string()),
                            publisher: item["publisher"].as_str().map(|s| s.to_string()),
                            published_year: item["published-print"]["date-parts"][0][0]
                                .as_u64()
                                .map(|y| y as u32),
                            citation_count: item["is-referenced-by-count"].as_u64()
                                .map(|c| c as usize),
                        }));
                    }
                }
            }
        }
        
        Ok(None)
    }
}

#[derive(Debug)]
pub struct CrossRefMetadata {
    pub doi: Option<String>,
    pub publisher: Option<String>,
    pub published_year: Option<u32>,
    pub citation_count: Option<usize>,
}

/// Extract references from PDF text
pub fn extract_references_from_text(text: &str) -> Vec<String> {
    let mut references = Vec::new();
    
    // Find references section
    let text_lower = text.to_lowercase();
    let ref_start = text_lower.rfind("references")
        .or_else(|| text_lower.rfind("bibliography"));
    
    if let Some(start) = ref_start {
        let ref_section = &text[start..];
        
        // Split by common reference patterns
        // [1], [2], etc. or 1., 2., etc.
        let lines: Vec<&str> = ref_section.lines().collect();
        
        let mut current_ref = String::new();
        let ref_pattern = regex::Regex::new(r"^\s*\[?\d+\]?\.?\s+").unwrap();
        
        for line in lines {
            if ref_pattern.is_match(line) && !current_ref.is_empty() {
                // New reference starts
                references.push(current_ref.trim().to_string());
                current_ref = ref_pattern.replace(line, "").to_string();
            } else if !line.trim().is_empty() {
                // Continue current reference
                if !current_ref.is_empty() {
                    current_ref.push(' ');
                }
                current_ref.push_str(line.trim());
            }
        }
        
        // Add last reference
        if !current_ref.is_empty() {
            references.push(current_ref.trim().to_string());
        }
    }
    
    references
}

/// Parse a reference string to extract components
pub fn parse_reference(reference: &str) -> ParsedReference {
    let mut parsed = ParsedReference::default();
    
    // Try to extract year (4 digits)
    if let Some(caps) = regex::Regex::new(r"\b(19|20)\d{2}\b").unwrap().captures(reference) {
        parsed.year = caps[0].parse().ok();
    }
    
    // Try to extract title (text in quotes or between periods)
    if let Some(title_match) = regex::Regex::new(r#""([^"]+)""#).unwrap().captures(reference) {
        parsed.title = Some(title_match[1].to_string());
    } else if let Some(title_match) = regex::Regex::new(r"\.([^\.]{20,})\.")
        .unwrap().captures(reference) {
        parsed.title = Some(title_match[1].trim().to_string());
    }
    
    // Extract venue (common conference/journal names)
    let venues = ["OSDI", "SOSP", "NSDI", "ATC", "FAST", "EuroSys", "ASPLOS", "PLDI", 
                  "SIGMOD", "VLDB", "ICDE", "CCS", "NDSS", "USENIX", "ACM", "IEEE"];
    for venue in venues {
        if reference.contains(venue) {
            parsed.venue = Some(venue.to_string());
            break;
        }
    }
    
    // Extract authors (text before first period or year)
    if let Some(first_period) = reference.find('.') {
        let author_part = &reference[..first_period];
        let authors: Vec<String> = author_part
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s.len() < 50)
            .collect();
        
        if !authors.is_empty() {
            parsed.authors = Some(authors);
        }
    }
    
    parsed.raw_text = reference.to_string();
    parsed
}

#[derive(Debug, Default)]
pub struct ParsedReference {
    pub raw_text: String,
    pub title: Option<String>,
    pub authors: Option<Vec<String>>,
    pub venue: Option<String>,
    pub year: Option<u32>,
}