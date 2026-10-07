use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tokio::time::{sleep, Duration};

/// Citation data for a single paper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperCitation {
    pub paper_id: String, // Our internal ID
    pub title: String,
    pub semantic_scholar_id: Option<String>,
    pub doi: Option<String>,
    pub citation_count: usize,
    pub reference_count: usize,
    pub references: Vec<String>, // Paper IDs this paper cites
    pub cited_by: Vec<String>,   // Paper IDs that cite this paper
    pub fetched_at: chrono::DateTime<chrono::Utc>,
}

/// Citation cache stored as JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationCache {
    pub papers: HashMap<String, PaperCitation>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub version: String,
}

impl CitationCache {
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

        let contents = fs::read_to_string(path).context("Failed to read citation cache file")?;

        serde_json::from_str(&contents).context("Failed to parse citation cache JSON")
    }

    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let json =
            serde_json::to_string_pretty(self).context("Failed to serialize citation cache")?;

        fs::write(path, json).context("Failed to write citation cache file")?;

        Ok(())
    }

    pub fn get(&self, paper_id: &str) -> Option<&PaperCitation> {
        self.papers.get(paper_id)
    }

    pub fn insert(&mut self, citation: PaperCitation) {
        self.papers.insert(citation.paper_id.clone(), citation);
        self.last_updated = chrono::Utc::now();
    }
}

/// Semantic Scholar API response structures
#[derive(Debug, Deserialize)]
struct SemanticScholarSearchResponse {
    data: Vec<SemanticScholarPaper>,
}

#[derive(Debug, Deserialize)]
struct SemanticScholarPaper {
    #[serde(rename = "paperId")]
    paper_id: String,
    title: String,
    authors: Option<Vec<SemanticScholarAuthor>>,
    venue: Option<String>,
    year: Option<i32>,
    #[serde(rename = "citationCount")]
    citation_count: Option<usize>,
    #[serde(rename = "referenceCount")]
    reference_count: Option<usize>,
    #[serde(rename = "externalIds")]
    external_ids: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
struct SemanticScholarAuthor {
    #[serde(rename = "authorId")]
    author_id: Option<String>,
    name: String,
}

#[derive(Debug, Deserialize)]
struct SemanticScholarCitationsResponse {
    data: Vec<SemanticScholarCitation>,
}

#[derive(Debug, Deserialize)]
struct SemanticScholarCitation {
    #[serde(rename = "citingPaper")]
    citing_paper: Option<SemanticScholarPaper>,
}

#[derive(Debug, Deserialize)]
struct SemanticScholarReferencesResponse {
    data: Vec<SemanticScholarReference>,
}

#[derive(Debug, Deserialize)]
struct SemanticScholarReference {
    #[serde(rename = "citedPaper")]
    cited_paper: Option<SemanticScholarPaper>,
}

/// Citation fetcher using Semantic Scholar API
pub struct CitationFetcher {
    client: Client,
    base_url: String,
    cache: CitationCache,
    cache_path: PathBuf,
    rate_limit_delay: Duration,
}

impl CitationFetcher {
    pub fn new(cache_dir: &Path) -> Result<Self> {
        let cache_path = cache_dir.join("citations.json");
        let cache = CitationCache::load_from_file(&cache_path)?;

        Ok(Self {
            client: Client::new(),
            base_url: "https://api.semanticscholar.org/graph/v1".to_string(),
            cache,
            cache_path,
            rate_limit_delay: Duration::from_millis(1000), // 1 request/second to avoid rate limits
        })
    }

    /// Search for a paper by title and authors
    pub async fn search_paper(
        &self,
        title: &str,
        authors: &[String],
        year: Option<u32>,
    ) -> Result<Option<SemanticScholarPaper>> {
        // Build search query
        let mut query = title.to_string();
        if !authors.is_empty() {
            // Add first author's last name to improve matching
            if let Some(first_author) = authors.first() {
                if let Some(last_name) = first_author.split(',').next() {
                    query.push_str(" ");
                    query.push_str(last_name);
                }
            }
        }

        let url = format!("{}/paper/search", self.base_url);
        let params = [
            ("query", query.as_str()),
            ("limit", "3"),
            (
                "fields",
                "paperId,title,authors,venue,year,citationCount,referenceCount,externalIds",
            ),
        ];

        debug!("Searching for paper: {}", title);

        let response = self
            .client
            .get(&url)
            .query(&params)
            .send()
            .await
            .context("Failed to send request to Semantic Scholar")?;

        // Handle rate limiting
        if response.status() == 429 {
            warn!("Rate limited by Semantic Scholar API. Please wait and try again.");
            return Ok(None);
        }

        if !response.status().is_success() {
            warn!(
                "Semantic Scholar API returned status: {}",
                response.status()
            );
            return Ok(None);
        }

        let data: SemanticScholarSearchResponse = response
            .json()
            .await
            .context("Failed to parse Semantic Scholar response")?;

        // Find best match based on title similarity and year
        for paper in data.data {
            // Simple title matching (could be improved with fuzzy matching)
            let title_lower = title.to_lowercase();
            let paper_title_lower = paper.title.to_lowercase();

            if paper_title_lower.contains(&title_lower) || title_lower.contains(&paper_title_lower)
            {
                // Check year if provided
                if let Some(expected_year) = year {
                    if let Some(paper_year) = paper.year {
                        if (paper_year as i32 - expected_year as i32).abs() <= 1 {
                            return Ok(Some(paper));
                        }
                    }
                } else {
                    return Ok(Some(paper));
                }
            }
        }

        Ok(None)
    }

    /// Fetch citations for a paper
    pub async fn fetch_citations(
        &self,
        semantic_scholar_id: &str,
        limit: usize,
    ) -> Result<Vec<String>> {
        let url = format!("{}/paper/{}/citations", self.base_url, semantic_scholar_id);
        let params = [
            ("fields", "citingPaper.paperId,citingPaper.title"),
            ("limit", &limit.to_string()),
        ];

        let response = self.client.get(&url).query(&params).send().await?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let data: SemanticScholarCitationsResponse = response.json().await?;

        let citations: Vec<String> = data
            .data
            .into_iter()
            .filter_map(|c| c.citing_paper.map(|p| p.paper_id))
            .collect();

        Ok(citations)
    }

    /// Fetch references for a paper
    pub async fn fetch_references(
        &self,
        semantic_scholar_id: &str,
        limit: usize,
    ) -> Result<Vec<String>> {
        let url = format!("{}/paper/{}/references", self.base_url, semantic_scholar_id);
        let params = [
            ("fields", "citedPaper.paperId,citedPaper.title"),
            ("limit", &limit.to_string()),
        ];

        let response = self.client.get(&url).query(&params).send().await?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let data: SemanticScholarReferencesResponse = response.json().await?;

        let references: Vec<String> = data
            .data
            .into_iter()
            .filter_map(|r| r.cited_paper.map(|p| p.paper_id))
            .collect();

        Ok(references)
    }

    /// Fetch citation data for a paper and cache it
    pub async fn fetch_and_cache_paper(
        &mut self,
        paper_id: &str,
        title: &str,
        authors: &[String],
        year: Option<u32>,
    ) -> Result<Option<PaperCitation>> {
        // Check cache first
        if let Some(cached) = self.cache.get(paper_id) {
            let age = chrono::Utc::now() - cached.fetched_at;
            if age.num_days() < 30 {
                // Cache for 30 days
                debug!("Using cached citation data for {}", title);
                return Ok(Some(cached.clone()));
            }
        }

        // Apply rate limiting before API call
        sleep(self.rate_limit_delay).await;

        // Search for paper with retry logic
        let mut retries = 0;
        let max_retries = 3;
        let mut backoff = Duration::from_secs(2);

        let paper = loop {
            match self.search_paper(title, authors, year).await {
                Ok(Some(p)) => break Some(p),
                Ok(None) if retries < max_retries => {
                    // Paper not found could be due to rate limiting, retry
                    retries += 1;
                    warn!(
                        "Paper not found (attempt {}/{}): {}",
                        retries, max_retries, title
                    );
                    sleep(backoff).await;
                    backoff *= 2; // Exponential backoff
                    continue;
                }
                Ok(None) => {
                    warn!("Paper not found after {} attempts: {}", max_retries, title);
                    break None;
                }
                Err(e) => {
                    error!("Error searching for paper {}: {}", title, e);
                    break None;
                }
            }
        };

        let paper = match paper {
            Some(p) => p,
            None => return Ok(None),
        };

        info!(
            "Found paper '{}' with {} citations",
            title,
            paper.citation_count.unwrap_or(0)
        );

        // Apply rate limiting between API calls
        sleep(self.rate_limit_delay).await;

        // Fetch citations and references (limit to reasonable number)
        let citations = self
            .fetch_citations(&paper.paper_id, 50)
            .await
            .unwrap_or_default();

        sleep(self.rate_limit_delay).await;

        let references = self
            .fetch_references(&paper.paper_id, 50)
            .await
            .unwrap_or_default();

        // Create citation record
        let citation_data = PaperCitation {
            paper_id: paper_id.to_string(),
            title: paper.title.clone(),
            semantic_scholar_id: Some(paper.paper_id.clone()),
            doi: paper
                .external_ids
                .as_ref()
                .and_then(|ids| ids.get("DOI"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            citation_count: paper.citation_count.unwrap_or(0),
            reference_count: paper.reference_count.unwrap_or(0),
            references,
            cited_by: citations,
            fetched_at: chrono::Utc::now(),
        };

        // Cache it
        self.cache.insert(citation_data.clone());

        Ok(Some(citation_data))
    }

    /// Save cache to disk
    pub fn save_cache(&self) -> Result<()> {
        self.cache.save_to_file(&self.cache_path)?;
        info!(
            "Saved citation cache with {} papers",
            self.cache.papers.len()
        );
        Ok(())
    }

    /// Get current cache
    pub fn get_cache(&self) -> &CitationCache {
        &self.cache
    }
}

/// Fetch citations for all papers in a BibTeX graph
pub async fn fetch_all_citations(
    graph: &crate::models::CitationGraph,
    cache_dir: &Path,
    max_papers: Option<usize>,
) -> Result<CitationCache> {
    let mut fetcher = CitationFetcher::new(cache_dir)?;

    info!("Starting citation fetch for {} papers", graph.papers.len());

    let mut count = 0;
    let max = max_papers.unwrap_or(graph.papers.len());

    for (paper_id, paper) in graph.papers.iter() {
        if count >= max {
            break;
        }

        // Skip if no venue (likely incomplete data)
        if paper.venue.is_empty() {
            continue;
        }

        // Only fetch for major venues initially
        let venue_upper = paper.venue.to_uppercase();
        let major_venues = ["OSDI", "SOSP", "SIGMOD", "VLDB", "NSDI", "PLDI", "POPL"];
        if !major_venues.iter().any(|v| venue_upper.contains(v)) {
            continue;
        }

        info!(
            "Fetching citations for paper {}/{}: {}",
            count + 1,
            max,
            paper.title
        );

        match fetcher
            .fetch_and_cache_paper(paper_id, &paper.title, &paper.authors, paper.year)
            .await
        {
            Ok(Some(_)) => {
                count += 1;
                // Save cache periodically
                if count % 10 == 0 {
                    fetcher.save_cache()?;
                }
            }
            Ok(None) => {
                debug!("No citation data found for: {}", paper.title);
            }
            Err(e) => {
                error!("Error fetching citations for {}: {}", paper.title, e);
                // Continue with next paper
            }
        }
    }

    // Final save
    fetcher.save_cache()?;

    info!("Citation fetch complete. Fetched data for {} papers", count);

    Ok(fetcher.cache)
}
