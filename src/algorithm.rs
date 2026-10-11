use anyhow::Result;
use log::{debug, info};
use ordered_float::OrderedFloat;
use std::collections::{BTreeMap, HashMap};

use crate::models::{
    AlgorithmParams, CitationGraph, Paper, QIndexMetrics, Scholar, ScholarRanking, VenueRanking,
};
use crate::parser::normalize_venue_id;
use crate::s2ag_citations::S2agCitations;

/// Check if a venue name matches a CSRankings conference
fn is_csrankings_venue(venue_name: &str) -> bool {
    // Normalize and remove year from venue name (e.g., "CHI 2019" -> "CHI")
    let normalized = venue_name.to_uppercase();

    // Remove trailing year (4 digits at the end)
    let without_year = if let Some(pos) = normalized.rfind(char::is_whitespace) {
        let potential_year = &normalized[pos + 1..];
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
    venue_graph: BTreeMap<String, BTreeMap<String, f64>>,
    venue_scores: HashMap<String, f64>,
    scholar_scores: HashMap<String, f64>,
    citations: Option<&'a S2agCitations>,
    venue_paper_counts: HashMap<String, usize>,
}

impl<'a> PageRankCalculator<'a> {
    pub fn new(graph: &'a CitationGraph) -> Self {
        Self {
            graph,
            params: AlgorithmParams::default(),
            venue_graph: BTreeMap::new(),
            venue_scores: HashMap::new(),
            scholar_scores: HashMap::new(),
            citations: None,
            venue_paper_counts: HashMap::new(),
        }
    }

    pub fn with_citations(graph: &'a CitationGraph, citations: &'a S2agCitations) -> Self {
        let mut calculator = Self::new(graph);
        calculator.citations = Some(citations);
        if let Some(date) = citations
            .release_id
            .as_deref()
            .and_then(|release| chrono::NaiveDate::parse_from_str(release, "%Y-%m-%d").ok())
        {
            calculator.params.reference_year = chrono::Datelike::year(&date) as u32;
        }
        calculator
    }

    pub fn calculate(&mut self) -> Result<QIndexMetrics> {
        info!("Building venue citation graph...");
        self.build_venue_graph();

        info!("Calculating venue PageRank scores...");
        self.calculate_venue_pagerank()?;

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

    fn unique_papers<'b>(&self, papers: impl IntoIterator<Item = &'b Paper>) -> Vec<&'b Paper> {
        self.citations
            .unwrap_or(&S2agCitations::default())
            .unique_papers(papers)
    }

    fn eligible_scholar_papers(&self, scholar: &Scholar) -> Vec<&'a Paper> {
        self.unique_papers(
            scholar
                .papers
                .iter()
                .filter_map(|id| self.graph.papers.get(id)),
        )
        .into_iter()
        .filter(|paper| {
            paper.authors.iter().any(|author| {
                crate::models::normalize_author_name(author) == scholar.normalized_name
            }) && self
                .graph
                .venues
                .get(&normalize_venue_id(&paper.venue))
                .is_some_and(|venue| is_csrankings_venue(&venue.name))
        })
        .collect()
    }

    fn venue_paper_count(&self, venue_id: &str) -> usize {
        self.venue_paper_counts.get(venue_id).copied().unwrap_or(0)
    }

    fn build_venue_graph(&mut self) {
        self.venue_graph.clear();
        self.venue_paper_counts.clear();
        for paper in self.unique_papers(self.graph.papers.values()) {
            *self
                .venue_paper_counts
                .entry(normalize_venue_id(&paper.venue))
                .or_default() += 1;
            if paper.venue.is_empty() {
                continue;
            }
            let from = normalize_venue_id(&paper.venue);
            if !self.graph.venues.contains_key(&from) {
                continue;
            }
            // Forward references alone: reverse adjacency represents the same edges.
            for cited in self.unique_papers(
                paper
                    .citations
                    .iter()
                    .filter_map(|id| self.graph.papers.get(id)),
            ) {
                let to = normalize_venue_id(&cited.venue);
                if cited.venue.is_empty() || to == from || !self.graph.venues.contains_key(&to) {
                    continue;
                }
                let mut weight = 1.0;
                if let (Some(from_year), Some(to_year)) = (paper.year, cited.year) {
                    weight *= self
                        .params
                        .year_decay
                        .powf(from_year.saturating_sub(to_year) as f64);
                }
                // Underflowed edges carry no outgoing mass and must remain dangling.
                if weight == 0.0 {
                    continue;
                }
                *self
                    .venue_graph
                    .entry(from.clone())
                    .or_default()
                    .entry(to)
                    .or_default() += weight;
            }
        }
        // Normalize each source once, rather than re-summing weights on every iteration.
        for edges in self.venue_graph.values_mut() {
            let total: f64 = edges.values().sum();
            for weight in edges.values_mut() {
                *weight /= total;
            }
        }
        debug!(
            "Built venue graph with {} source nodes",
            self.venue_graph.len()
        );
    }

    fn calculate_venue_pagerank(&mut self) -> Result<()> {
        self.venue_scores.clear();
        let mut venues: Vec<String> = self.graph.venues.keys().cloned().collect();
        venues.sort();
        let n = venues.len();

        if n == 0 {
            return Ok(());
        }

        info!(
            "Running PageRank with citation graph ({} edges)",
            self.venue_graph.values().map(|e| e.len()).sum::<usize>()
        );

        // Initialize uniformly, including graphs with no cross-venue citations.
        let initial_score = 1.0 / n as f64;
        for venue in &venues {
            self.venue_scores.insert(venue.clone(), initial_score);
        }

        // With every venue dangling, uniform initialization is already the fixed point.
        if self.venue_graph.is_empty() {
            return Ok(());
        }

        // Power iteration
        for iteration in 0..self.params.max_iterations {
            let mut new_scores = HashMap::new();

            // Dangling venues have no entry in the sparse forward graph.
            let dangling_mass: f64 = venues
                .iter()
                .filter(|venue| !self.venue_graph.contains_key(*venue))
                .map(|venue| self.venue_scores[venue])
                .sum();
            let base = (1.0 - self.params.damping_factor
                + self.params.damping_factor * dangling_mass)
                / n as f64;
            for venue in &venues {
                new_scores.insert(venue.clone(), base);
            }

            // Add contributions from incoming links
            for (from_venue, edges) in &self.venue_graph {
                let from_score = self.venue_scores[from_venue];
                for (to_venue, weight) in edges {
                    let contribution = self.params.damping_factor * from_score * weight;
                    *new_scores.get_mut(to_venue).unwrap() += contribution;
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
        let sum: f64 = venues.iter().map(|venue| self.venue_scores[venue]).sum();
        if sum > 0.0 {
            for score in self.venue_scores.values_mut() {
                *score /= sum;
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

    fn calculate_scholar_scores(&mut self) {
        self.scholar_scores.clear();
        for (scholar_id, scholar) in &self.graph.scholars {
            let papers = self.eligible_scholar_papers(scholar);
            let mut score = 0.0;
            for paper in &papers {
                let venue_id = normalize_venue_id(&paper.venue);
                let mut paper_score = self.venue_scores.get(&venue_id).copied().unwrap_or(0.0);
                if let Some(year) = paper.year {
                    let age = self.params.reference_year.saturating_sub(year);
                    paper_score *= self.params.year_decay.powf(age as f64 / 5.0);
                }

                // Eligibility guarantees a real author position, even for one-author papers.
                let author_position = paper
                    .authors
                    .iter()
                    .position(|author| {
                        crate::models::normalize_author_name(author) == scholar.normalized_name
                    })
                    .expect("eligible publication must contain the scholar");
                let author_count = paper.authors.len();
                if author_count > 1 {
                    if author_position == 0 {
                        paper_score *= 1.0;
                    } else if author_position == author_count - 1 {
                        paper_score *= 0.8;
                    } else {
                        paper_score *= 0.6 / (author_count - 2) as f64;
                    }
                }
                score += paper_score;
            }
            score *= (papers.len() as f64 + 1.0).ln();
            self.scholar_scores.insert(scholar_id.clone(), score);
        }

        // Normalize to 0-100 scale.
        let max_score = self
            .scholar_scores
            .values()
            .max_by_key(|&&score| OrderedFloat(score))
            .copied()
            .unwrap_or(0.0);
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
        self.citations
            .unwrap_or(&S2agCitations::default())
            .h_index(self.eligible_scholar_papers(scholar))
    }

    pub fn get_top_venues(&self, n: usize, field: Option<&str>) -> Vec<VenueRanking> {
        let mut rankings: Vec<VenueRanking> = self
            .graph
            .venues
            .values()
            .filter(|v| {
                // Only include CSRankings conferences
                is_csrankings_venue(&v.name)
                    && field.map_or(true, |f| v.field.to_lowercase().contains(&f.to_lowercase()))
            })
            .map(|venue| {
                let score = self.venue_scores.get(&venue.id).unwrap_or(&0.0);
                let paper_count = self.venue_paper_count(&venue.id);
                let impact_factor = score * (paper_count as f64 + 1.0).ln();

                VenueRanking {
                    id: venue.id.clone(),
                    name: venue.name.clone(),
                    field: venue.field.clone(),
                    pagerank: *score,
                    impact_factor,
                    paper_count,
                }
            })
            .collect();

        rankings.sort_by(|a, b| {
            b.pagerank
                .total_cmp(&a.pagerank)
                .then_with(|| a.id.cmp(&b.id))
        });
        rankings.truncate(n);
        rankings
    }

    pub fn get_top_scholars(&self, n: usize, min_papers: Option<usize>) -> Vec<ScholarRanking> {
        let min_papers = min_papers.unwrap_or(0);

        let mut rankings: Vec<ScholarRanking> = self
            .graph
            .scholars
            .values()
            .map(|scholar| {
                let counted_papers = self.eligible_scholar_papers(scholar);
                let csrankings_paper_count = counted_papers.len();

                // Only include scholars with minimum CSRankings papers
                if csrankings_paper_count < min_papers {
                    return None;
                }

                let qindex = self.scholar_scores.get(&scholar.id).unwrap_or(&0.0);

                // Get top CSRankings venues
                let mut counts_by_venue = HashMap::<String, usize>::new();
                for paper in &counted_papers {
                    *counts_by_venue
                        .entry(normalize_venue_id(&paper.venue))
                        .or_default() += 1;
                }
                let mut venue_papers: Vec<(String, usize)> = counts_by_venue.into_iter().collect();
                venue_papers.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

                let top_venues: Vec<String> = venue_papers
                    .iter()
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
                    citation_count: self
                        .citations
                        .unwrap_or(&S2agCitations::default())
                        .total(counted_papers),
                    top_venues,
                })
            })
            .flatten()
            .collect();

        rankings.sort_by(|a, b| b.qindex.total_cmp(&a.qindex).then_with(|| a.id.cmp(&b.id)));
        rankings.truncate(n);
        rankings
    }
}

#[cfg(test)]
#[path = "algorithm_tests.rs"]
mod tests;
