use std::collections::HashMap;
use anyhow::Result;
use ordered_float::OrderedFloat;
use log::{info, debug};

use crate::models::{
    CitationGraph, QIndexMetrics, AlgorithmParams,
    VenueRanking, ScholarRanking, Scholar
};
// CSRankings venue checking will be implemented inline

/// Check if a venue name matches a CSRankings conference
fn is_csrankings_venue(venue_name: &str) -> bool {
    // Normalize and remove year from venue name (e.g., "CHI 2019" -> "CHI")
    let normalized = venue_name.to_uppercase();
    
    // Remove trailing year (4 digits at the end)
    let without_year = if let Some(pos) = normalized.rfind(char::is_whitespace) {
        let potential_year = &normalized[pos+1..];
        if potential_year.len() == 4 && potential_year.chars().all(|c| c.is_ascii_digit()) {
            normalized[..pos].trim()
        } else {
            normalized.as_str()
        }
    } else {
        normalized.as_str()
    };
    
    // Check exact matches first
    matches!(without_year,
        // AI Area
        "AAAI" | "IJCAI" | "CVPR" | "ECCV" | "ICCV" | "ICLR" | "ICML" | "NEURIPS" | "NIPS" |
        "KDD" | "ACL" | "EMNLP" | "NAACL" | "SIGIR" | "WWW" |
        
        // Systems Area  
        "ASPLOS" | "ISCA" | "MICRO" | "HPCA" | "SIGCOMM" | "NSDI" | "CCS" |
        "OAKLAND" | "SP" | "S&P" | "NDSS" |
        "SIGMOD" | "VLDB" | "ICDE" | "PODS" | "DAC" | "ICCAD" | "EMSOFT" | "RTAS" | "RTSS" |
        "HPDC" | "ICS" | "SC" | "MOBICOM" | "MOBISYS" | "SENSYS" | "IMC" | "SIGMETRICS" |
        "OSDI" | "SOSP" | "EUROSYS" | "FAST" | "USENIX ATC" | "ATC" |
        "PLDI" | "POPL" | "ICFP" | "OOPSLA" | "FSE" | "ICSE" | "ASE" | "ISSTA" |
        "USENIX SECURITY" | "USENIXSEC" |
        
        // Theory Area
        "FOCS" | "SODA" | "STOC" | "CRYPTO" | "EUROCRYPT" | "CAV" | "LICS" | "PODC" | "SPAA" |
        
        // Interdisciplinary Areas
        "ISMB" | "RECOMB" | "SIGGRAPH" | "EUROGRAPHICS" | "SIGCSE" |
        "EC" | "WINE" | "CHI" | "UBICOMP" | "PERVASIVE" | "IMWUT" | "UIST" | "IUI" |
        "ICRA" | "IROS" | "RSS" | "VIS" | "VR" |
        
        // Special cases
        "SOCC"
    ) || 
    // Also check for partial matches
    (without_year.contains("USENIX") && without_year.contains("ATC")) ||
    (without_year.contains("USENIX") && without_year.contains("SECURITY"))
}

pub struct PageRankCalculator<'a> {
    graph: &'a CitationGraph,
    params: AlgorithmParams,
    venue_graph: HashMap<String, HashMap<String, f64>>,
    venue_scores: HashMap<String, f64>,
    scholar_scores: HashMap<String, f64>,
}

impl<'a> PageRankCalculator<'a> {
    pub fn new(graph: &'a CitationGraph) -> Self {
        Self {
            graph,
            params: AlgorithmParams::default(),
            venue_graph: HashMap::new(),
            venue_scores: HashMap::new(),
            scholar_scores: HashMap::new(),
        }
    }
    
    pub fn calculate(&mut self) -> Result<QIndexMetrics> {
        info!("Building venue citation graph...");
        self.build_venue_graph();
        
        info!("Calculating venue PageRank scores...");
        self.calculate_venue_pagerank()?;
        
        info!("Applying tier bonuses...");
        self.apply_tier_bonus();
        
        info!("Calculating scholar QIndex scores...");
        self.calculate_scholar_scores();
        
        info!("Calculating H-indices...");
        self.calculate_h_indices();
        
        Ok(QIndexMetrics {
            venue_scores: self.venue_scores.clone(),
            scholar_scores: self.scholar_scores.clone(),
            timestamp: chrono::Utc::now(),
            algorithm: "PageRank-based QIndex".to_string(),
            parameters: self.params.clone(),
        })
    }
    
    fn build_venue_graph(&mut self) {
        // Build venue-to-venue citation graph
        for paper in self.graph.papers.values() {
            let from_venue = &paper.venue;
            if from_venue.is_empty() {
                continue;
            }
            
            let from_venue_id = normalize_venue_id(from_venue);
            
            // Process citations
            for cited_id in &paper.citations {
                if let Some(cited_paper) = self.graph.papers.get(cited_id) {
                    let to_venue = &cited_paper.venue;
                    if !to_venue.is_empty() && to_venue != from_venue {
                        let to_venue_id = normalize_venue_id(to_venue);
                        
                        // Calculate weight with year decay
                        let mut weight = 1.0;
                        if let (Some(from_year), Some(to_year)) = (paper.year, cited_paper.year) {
                            let year_diff = (from_year as i32 - to_year as i32).abs();
                            if year_diff > 0 {
                                weight *= self.params.year_decay.powi(year_diff);
                            }
                        }
                        
                        *self.venue_graph
                            .entry(from_venue_id.clone())
                            .or_insert_with(HashMap::new)
                            .entry(to_venue_id)
                            .or_insert(0.0) += weight;
                    }
                }
            }
            
            // Process citations to this paper (reverse direction)
            for citing_id in &paper.cited_by {
                if let Some(citing_paper) = self.graph.papers.get(citing_id) {
                    let to_venue = &citing_paper.venue;
                    if !to_venue.is_empty() && to_venue != from_venue {
                        let to_venue_id = normalize_venue_id(to_venue);
                        
                        let mut weight = 1.0;
                        if let (Some(from_year), Some(to_year)) = (citing_paper.year, paper.year) {
                            let year_diff = (from_year as i32 - to_year as i32).abs();
                            if year_diff > 0 {
                                weight *= self.params.year_decay.powi(year_diff);
                            }
                        }
                        
                        *self.venue_graph
                            .entry(to_venue_id.clone())
                            .or_insert_with(HashMap::new)
                            .entry(from_venue_id.clone())
                            .or_insert(0.0) += weight;
                    }
                }
            }
        }
        
        debug!("Built venue graph with {} nodes", self.venue_graph.len());
    }
    
    fn calculate_venue_pagerank(&mut self) -> Result<()> {
        let venues: Vec<String> = self.graph.venues.keys().cloned().collect();
        let n = venues.len();
        
        if n == 0 {
            return Ok(());
        }
        
        // Check if we have any citation data
        let has_citations = !self.venue_graph.is_empty();
        
        if has_citations {
            // Use PageRank if we have citation data
            info!("Running PageRank with citation graph ({} edges)", 
                  self.venue_graph.values().map(|e| e.len()).sum::<usize>());
            
            // Initialize scores
            let initial_score = 1.0 / n as f64;
            for venue in &venues {
                self.venue_scores.insert(venue.clone(), initial_score);
            }
            
            // Power iteration
            for iteration in 0..self.params.max_iterations {
                let mut new_scores = HashMap::new();
                
                // Initialize with damping
                for venue in &venues {
                    new_scores.insert(venue.clone(), (1.0 - self.params.damping_factor) / n as f64);
                }
                
                // Add contributions from incoming links
                for (from_venue, edges) in &self.venue_graph {
                    let total_weight: f64 = edges.values().sum();
                    
                    if total_weight > 0.0 {
                        let from_score = self.venue_scores.get(from_venue).unwrap_or(&initial_score);
                        
                        for (to_venue, weight) in edges {
                            let contribution = self.params.damping_factor * from_score * (weight / total_weight);
                            *new_scores.entry(to_venue.clone()).or_insert(0.0) += contribution;
                    }
                } else {
                    // Distribute evenly if no outgoing links
                    let from_score = self.venue_scores.get(from_venue).unwrap_or(&initial_score);
                    let uniform_contribution = self.params.damping_factor * from_score / n as f64;
                    
                    for venue in &venues {
                        *new_scores.entry(venue.clone()).or_insert(0.0) += uniform_contribution;
                    }
                }
            }
            
            // Check convergence
            let mut max_diff = 0.0;
            for venue in &venues {
                let old_score = self.venue_scores.get(venue).unwrap_or(&0.0);
                let new_score = new_scores.get(venue).unwrap_or(&0.0);
                let diff = (new_score - old_score).abs();
                if diff > max_diff {
                    max_diff = diff;
                }
            }
            
            self.venue_scores = new_scores;
            
            if max_diff < self.params.tolerance {
                debug!("PageRank converged after {} iterations", iteration + 1);
                break;
            }
        }
        
        // Normalize scores
        let sum: f64 = self.venue_scores.values().sum();
        if sum > 0.0 {
            for score in self.venue_scores.values_mut() {
                *score /= sum;
            }
        }
        } else {
            // No citation data - use alternative scoring based on paper count and venue prestige
            info!("No citation data available, using prestige-based scoring");
            
            for (venue_id, venue) in &self.graph.venues {
                let mut score = 0.0;
                
                // Base score from paper count (logarithmic scale)
                let paper_count = venue.papers.len() as f64;
                if paper_count > 0.0 {
                    score = (paper_count + 1.0).ln() / 10.0;
                }
                
                // Adjust based on CSRankings status
                if is_csrankings_venue(&venue.name) {
                    score *= 2.0; // Boost CSRankings venues
                }
                
                // Add some variation based on venue name hash to avoid identical scores
                let name_hash = venue.name.chars().fold(0u32, |acc, c| acc.wrapping_add(c as u32));
                let variation = ((name_hash % 100) as f64) / 10000.0;
                score += variation;
                
                self.venue_scores.insert(venue_id.clone(), score);
            }
            
            // Normalize scores
            let sum: f64 = self.venue_scores.values().sum();
            if sum > 0.0 {
                for score in self.venue_scores.values_mut() {
                    *score /= sum;
                }
            }
        }
        
        // Update venue objects
        for (venue_id, score) in &self.venue_scores {
            if let Some(venue) = self.graph.venues.get(venue_id) {
                // We'll update this in a mutable context later
                debug!("Venue {} score: {:.6}", venue.name, score);
            }
        }
        
        Ok(())
    }
    
    fn apply_tier_bonus(&mut self) {
        for (venue_id, score) in self.venue_scores.iter_mut() {
            if let Some(venue) = self.graph.venues.get(venue_id) {
                if let Some(bonus) = self.params.tier_bonus.get(&venue.tier) {
                    *score *= bonus;
                }
                
                // Calculate impact factor
                let paper_count = venue.papers.len() as f64;
                if paper_count > 0.0 {
                    // Impact factor combines PageRank with paper count
                    let _impact = *score * (paper_count + 1.0).ln();
                }
            }
        }
        
        // Re-normalize after applying bonuses
        let sum: f64 = self.venue_scores.values().sum();
        if sum > 0.0 {
            for score in self.venue_scores.values_mut() {
                *score /= sum;
            }
        }
    }
    
    fn calculate_scholar_scores(&mut self) {
        for (scholar_id, scholar) in &self.graph.scholars {
            let mut score = 0.0;
            let total_papers = scholar.papers.len();
            
            for (venue_id, paper_ids) in &scholar.publications_by_venue {
                // Only consider papers from CSRankings venues
                if let Some(venue) = self.graph.venues.get(venue_id) {
                    if !is_csrankings_venue(&venue.name) {
                        continue;
                    }
                }
                
                let venue_score = self.venue_scores.get(venue_id).unwrap_or(&0.01);
                
                for paper_id in paper_ids {
                    if let Some(paper) = self.graph.papers.get(paper_id) {
                        let mut paper_score = *venue_score;
                        
                        // Apply year decay
                        if let Some(year) = paper.year {
                            let current_year = 2024;
                            let year_diff = (current_year - year) as f64;
                            if year_diff > 0.0 {
                                paper_score *= self.params.year_decay.powf(year_diff / 5.0);
                            }
                        }
                        
                        // Apply author position weight
                        let author_count = paper.authors.len();
                        if author_count > 1 {
                            let author_position = paper.authors.iter()
                                .position(|a| crate::models::normalize_author_name(a) == scholar.normalized_name)
                                .unwrap_or(author_count);
                            
                            if author_position == 0 {
                                // First author
                                paper_score *= 1.0;
                            } else if author_position == author_count - 1 {
                                // Last author
                                paper_score *= 0.8;
                            } else {
                                // Middle author
                                paper_score *= 0.6 / (author_count - 2) as f64;
                            }
                        }
                        
                        score += paper_score;
                    }
                }
            }
            
            // Apply logarithmic scaling based on paper count
            if total_papers > 0 {
                score *= (total_papers as f64 + 1.0).ln();
            }
            
            self.scholar_scores.insert(scholar_id.clone(), score);
        }
        
        // Normalize to 0-100 scale
        let max_score = self.scholar_scores.values()
            .max_by_key(|&&s| OrderedFloat(s))
            .copied()
            .unwrap_or(1.0);
        
        if max_score > 0.0 {
            for score in self.scholar_scores.values_mut() {
                *score = (*score / max_score) * 100.0;
            }
        }
    }
    
    fn calculate_h_indices(&mut self) {
        // Calculate H-index for each scholar (only counting CSRankings papers)
        for scholar in self.graph.scholars.values() {
            let h_index = self.calculate_scholar_h_index(scholar);
            debug!("Scholar {} H-index: {}", scholar.name, h_index);
        }
    }
    
    fn calculate_scholar_h_index(&self, scholar: &Scholar) -> usize {
        let mut citations: Vec<usize> = Vec::new();
        
        for paper_id in &scholar.papers {
            if let Some(paper) = self.graph.papers.get(paper_id) {
                // Only consider papers from CSRankings venues
                if let Some(venue) = self.graph.venues.get(&normalize_venue_id(&paper.venue)) {
                    if is_csrankings_venue(&venue.name) {
                        citations.push(paper.cited_by.len());
                    }
                }
            }
        }
        
        citations.sort_by(|a, b| b.cmp(a));
        
        let mut h_index = 0;
        for (i, &citation_count) in citations.iter().enumerate() {
            if citation_count >= i + 1 {
                h_index = i + 1;
            } else {
                break;
            }
        }
        
        h_index
    }
    
    pub fn get_top_venues(&self, n: usize, field: Option<&str>, tier: Option<&str>) -> Vec<VenueRanking> {
        let mut rankings: Vec<VenueRanking> = self.graph.venues.values()
            .filter(|v| {
                // Only include CSRankings conferences
                is_csrankings_venue(&v.name) &&
                field.map_or(true, |f| v.field.to_lowercase().contains(&f.to_lowercase())) &&
                tier.map_or(true, |t| v.tier == t)
            })
            .map(|venue| {
                let score = self.venue_scores.get(&venue.id).unwrap_or(&0.0);
                let paper_count = venue.papers.len();
                let impact_factor = score * (paper_count as f64 + 1.0).ln();
                
                VenueRanking {
                    id: venue.id.clone(),
                    name: venue.name.clone(),
                    tier: venue.tier.clone(),
                    field: venue.field.clone(),
                    pagerank: *score,
                    impact_factor,
                    paper_count,
                }
            })
            .collect();
        
        rankings.sort_by(|a, b| b.pagerank.partial_cmp(&a.pagerank).unwrap());
        rankings.truncate(n);
        rankings
    }
    
    pub fn get_top_scholars(&self, n: usize, min_papers: Option<usize>) -> Vec<ScholarRanking> {
        let min_papers = min_papers.unwrap_or(0);
        
        let mut rankings: Vec<ScholarRanking> = self.graph.scholars.values()
            .map(|scholar| {
                // Count only CSRankings papers
                let mut csrankings_paper_count = 0;
                let mut citation_count = 0;
                
                for paper_id in &scholar.papers {
                    if let Some(paper) = self.graph.papers.get(paper_id) {
                        if let Some(venue) = self.graph.venues.get(&normalize_venue_id(&paper.venue)) {
                            if is_csrankings_venue(&venue.name) {
                                csrankings_paper_count += 1;
                                citation_count += paper.cited_by.len();
                            }
                        }
                    }
                }
                
                // Only include scholars with minimum CSRankings papers
                if csrankings_paper_count < min_papers {
                    return None;
                }
                
                let qindex = self.scholar_scores.get(&scholar.id).unwrap_or(&0.0);
                
                // Get top CSRankings venues
                let mut venue_papers: Vec<(String, usize)> = scholar.publications_by_venue.iter()
                    .filter_map(|(venue_id, paper_ids)| {
                        if let Some(venue) = self.graph.venues.get(venue_id) {
                            if is_csrankings_venue(&venue.name) {
                                let csrankings_papers_in_venue = paper_ids.iter()
                                    .filter(|paper_id| {
                                        if let Some(paper) = self.graph.papers.get(*paper_id) {
                                            if let Some(v) = self.graph.venues.get(&normalize_venue_id(&paper.venue)) {
                                                return is_csrankings_venue(&v.name);
                                            }
                                        }
                                        false
                                    })
                                    .count();
                                Some((venue_id.clone(), csrankings_papers_in_venue))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect();
                venue_papers.sort_by(|a, b| b.1.cmp(&a.1));
                
                let top_venues: Vec<String> = venue_papers.iter()
                    .take(3)
                    .filter_map(|(venue_id, _)| {
                        self.graph.venues.get(venue_id).map(|v| v.name.clone())
                    })
                    .collect();
                
                Some(ScholarRanking {
                    id: scholar.id.clone(),
                    name: scholar.name.clone(),
                    qindex: *qindex,
                    h_index: self.calculate_scholar_h_index(scholar),
                    paper_count: csrankings_paper_count,
                    citation_count,
                    top_venues,
                })
            })
            .filter_map(|x| x)
            .collect();
        
        rankings.sort_by(|a, b| b.qindex.partial_cmp(&a.qindex).unwrap());
        rankings.truncate(n);
        rankings
    }
}

fn normalize_venue_id(venue: &str) -> String {
    venue.trim()
        .to_uppercase()
        .replace(' ', "_")
        .replace('-', "_")
        .replace('\'', "")
}

fn calculate_h_index(scholar: &Scholar, graph: &CitationGraph) -> usize {
    let mut citations: Vec<usize> = Vec::new();
    
    for paper_id in &scholar.papers {
        if let Some(paper) = graph.papers.get(paper_id) {
            citations.push(paper.cited_by.len());
        }
    }
    
    citations.sort_by(|a, b| b.cmp(a));
    
    let mut h_index = 0;
    for (i, &citation_count) in citations.iter().enumerate() {
        if citation_count >= i + 1 {
            h_index = i + 1;
        } else {
            break;
        }
    }
    
    h_index
}