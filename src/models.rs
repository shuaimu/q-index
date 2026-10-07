use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paper {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub venue: String,
    pub venue_type: VenueType,
    pub year: Option<u32>,
    pub month: Option<String>,
    pub citations: Vec<String>,
    pub cited_by: Vec<String>,
    pub abstract_text: Option<String>,
    pub keywords: Vec<String>,
    pub doi: Option<String>,
    pub url: Option<String>,
    pub bib_key: String,
    pub source_file: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VenueType {
    Conference,
    Journal,
    Workshop,
    Symposium,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Venue {
    pub id: String,
    pub name: String,
    pub full_name: String,
    pub venue_type: VenueType,
    pub papers: Vec<String>, // Paper IDs
    pub pagerank: f64,
    pub impact_factor: f64,
    pub tier: String,
    pub field: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scholar {
    pub id: String,
    pub name: String,
    pub normalized_name: String,
    pub papers: Vec<String>, // Paper IDs
    pub affiliations: Vec<String>,
    pub qindex: f64,
    pub h_index: usize,
    pub citation_count: usize,
    pub publications_by_venue: HashMap<String, Vec<String>>, // Venue ID -> Paper IDs
    pub coauthors: HashMap<String, usize>,                   // Scholar ID -> collaboration count
}

#[derive(Clone)]
pub struct CitationGraph {
    pub papers: IndexMap<String, Paper>,
    pub venues: IndexMap<String, Venue>,
    pub scholars: IndexMap<String, Scholar>,
    pub edges: Vec<CitationEdge>,
}

// Manual Debug implementation to avoid printing large data
impl std::fmt::Debug for CitationGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CitationGraph")
            .field("papers_count", &self.papers.len())
            .field("venues_count", &self.venues.len())
            .field("scholars_count", &self.scholars.len())
            .field("edges_count", &self.edges.len())
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationEdge {
    pub from: String,
    pub to: String,
    pub weight: f64,
    pub year: Option<u32>,
    pub cross_venue: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QIndexMetrics {
    pub venue_scores: HashMap<String, f64>,
    pub scholar_scores: HashMap<String, f64>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub algorithm: String,
    pub parameters: AlgorithmParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlgorithmParams {
    pub damping_factor: f64,
    pub max_iterations: usize,
    pub tolerance: f64,
    pub venue_weight: f64,
    pub year_decay: f64,
    pub tier_bonus: HashMap<String, f64>,
}

impl Default for AlgorithmParams {
    fn default() -> Self {
        let mut tier_bonus = HashMap::new();
        tier_bonus.insert("A*".to_string(), 2.0);
        tier_bonus.insert("A".to_string(), 1.5);
        tier_bonus.insert("B".to_string(), 1.2);
        tier_bonus.insert("C".to_string(), 1.0);

        Self {
            damping_factor: 0.85,
            max_iterations: 100,
            tolerance: 1e-6,
            venue_weight: 0.7,
            year_decay: 0.95,
            tier_bonus,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VenueRanking {
    pub id: String,
    pub name: String,
    pub tier: String,
    pub field: String,
    pub pagerank: f64,
    pub impact_factor: f64,
    pub paper_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScholarRanking {
    pub id: String,
    pub name: String,
    pub qindex: f64,
    pub h_index: usize,
    pub paper_count: usize,
    pub citation_count: usize,
    pub top_venues: Vec<String>,
}

impl CitationGraph {
    pub fn new() -> Self {
        Self {
            papers: IndexMap::new(),
            venues: IndexMap::new(),
            scholars: IndexMap::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_paper(&mut self, paper: Paper) {
        let paper_id = paper.id.clone();
        self.papers.insert(paper_id.clone(), paper);
    }

    pub fn add_venue(&mut self, venue: Venue) {
        let venue_id = venue.id.clone();
        self.venues.insert(venue_id, venue);
    }

    pub fn add_scholar(&mut self, scholar: Scholar) {
        let scholar_id = scholar.id.clone();
        self.scholars.insert(scholar_id, scholar);
    }

    pub fn build_citation_network(&mut self) {
        // Build citation relationships
        let mut citations_to_add = Vec::new();

        for (paper_id, paper) in &self.papers {
            for cited_id in &paper.citations {
                if self.papers.contains_key(cited_id) {
                    citations_to_add.push((cited_id.clone(), paper_id.clone()));
                }
            }
        }

        // Add reverse citations
        for (cited_id, citing_id) in citations_to_add {
            if let Some(cited_paper) = self.papers.get_mut(&cited_id) {
                cited_paper.cited_by.push(citing_id);
            }
        }
    }

    pub fn search_venues(&self, query: &str) -> Vec<VenueRanking> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for venue in self.venues.values() {
            if venue.name.to_lowercase().contains(&query_lower)
                || venue.full_name.to_lowercase().contains(&query_lower)
                || venue.field.to_lowercase().contains(&query_lower)
            {
                results.push(VenueRanking {
                    id: venue.id.clone(),
                    name: venue.name.clone(),
                    tier: venue.tier.clone(),
                    field: venue.field.clone(),
                    pagerank: venue.pagerank,
                    impact_factor: venue.impact_factor,
                    paper_count: venue.papers.len(),
                });
            }
        }

        results.sort_by(|a, b| b.pagerank.partial_cmp(&a.pagerank).unwrap());
        results
    }

    pub fn search_scholars(&self, query: &str) -> Vec<ScholarRanking> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for scholar in self.scholars.values() {
            if scholar.name.to_lowercase().contains(&query_lower)
                || scholar
                    .normalized_name
                    .to_lowercase()
                    .contains(&query_lower)
            {
                let mut top_venues = Vec::new();
                for (venue_id, papers) in &scholar.publications_by_venue {
                    if papers.len() > 0 {
                        if let Some(venue) = self.venues.get(venue_id) {
                            top_venues.push(venue.name.clone());
                        }
                    }
                    if top_venues.len() >= 3 {
                        break;
                    }
                }

                results.push(ScholarRanking {
                    id: scholar.id.clone(),
                    name: scholar.name.clone(),
                    qindex: scholar.qindex,
                    h_index: scholar.h_index,
                    paper_count: scholar.papers.len(),
                    citation_count: scholar.citation_count,
                    top_venues,
                });
            }
        }

        results.sort_by(|a, b| b.qindex.partial_cmp(&a.qindex).unwrap());
        results
    }

    pub fn print_statistics(&self) {
        use comfy_table::{presets::UTF8_FULL, Table};

        println!("\n📊 Dataset Statistics");
        println!("{}", "=".repeat(50));

        println!("Total Papers: {}", self.papers.len());
        println!("Total Venues: {}", self.venues.len());
        println!("Total Scholars: {}", self.scholars.len());

        // Papers by venue type
        let mut venue_type_counts = HashMap::new();
        for paper in self.papers.values() {
            *venue_type_counts.entry(paper.venue_type).or_insert(0) += 1;
        }

        println!("\n📚 Papers by Venue Type:");
        for (vtype, count) in venue_type_counts {
            println!("  {:?}: {}", vtype, count);
        }

        // Papers by tier
        let mut tier_counts = HashMap::new();
        for venue in self.venues.values() {
            *tier_counts.entry(venue.tier.clone()).or_insert(0) += venue.papers.len();
        }

        println!("\n🏆 Papers by Venue Tier:");
        for (tier, count) in tier_counts.iter() {
            println!("  {}: {}", tier, count);
        }

        // Papers by field
        let mut field_counts = HashMap::new();
        for venue in self.venues.values() {
            *field_counts.entry(venue.field.clone()).or_insert(0) += venue.papers.len();
        }

        println!("\n🔬 Papers by Field:");
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .set_header(vec!["Field", "Papers"]);

        let mut field_vec: Vec<_> = field_counts.iter().collect();
        field_vec.sort_by(|a, b| b.1.cmp(a.1));

        for (field, count) in field_vec.iter().take(10) {
            table.add_row(vec![field.to_string(), count.to_string()]);
        }
        println!("{}", table);

        // Year distribution
        let mut year_counts = HashMap::new();
        let mut min_year = u32::MAX;
        let mut max_year = 0;

        for paper in self.papers.values() {
            if let Some(year) = paper.year {
                *year_counts.entry(year).or_insert(0) += 1;
                min_year = min_year.min(year);
                max_year = max_year.max(year);
            }
        }

        if min_year != u32::MAX {
            println!("\n📅 Year Range: {} - {}", min_year, max_year);
        }

        // Average papers per scholar
        let total_papers: usize = self.scholars.values().map(|s| s.papers.len()).sum();
        let avg_papers = total_papers as f64 / self.scholars.len() as f64;
        println!("\n📝 Average Papers per Scholar: {:.2}", avg_papers);

        // Average authors per paper
        let total_authors: usize = self.papers.values().map(|p| p.authors.len()).sum();
        let avg_authors = total_authors as f64 / self.papers.len() as f64;
        println!("👥 Average Authors per Paper: {:.2}", avg_authors);
    }
}

// Helper functions
pub fn normalize_author_name(name: &str) -> String {
    let mut normalized = name.trim().to_lowercase();

    // Handle "Last, First" format
    if let Some(comma_pos) = normalized.find(',') {
        let (last, first) = normalized.split_at(comma_pos);
        normalized = format!("{} {}", first[1..].trim(), last.trim());
    }

    // Remove dots and hyphens
    normalized = normalized.replace('.', "").replace('-', " ");

    // Capitalize words
    normalized
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn get_venue_tier(venue: &str) -> String {
    let venue_upper = venue.to_uppercase();

    // Top tier venues
    let top_tier = [
        "SOSP", "OSDI", "SIGMOD", "VLDB", "PLDI", "POPL", "SIGCOMM", "NSDI", "ASPLOS", "ISCA",
        "MICRO", "FAST", "EUROSYS", "ATC", "PODC", "SPAA", "CCS", "SECURITY", "OAKLAND", "STOC",
        "FOCS", "SODA",
    ];

    // Second tier venues
    let second_tier = [
        "SOCC", "DSN", "ICDCS", "IPDPS", "CIDR", "ICDE", "EDBT", "VEE", "PPOPP", "PACT", "HPDC",
        "SC", "CONEXT", "INFOCOM", "IMC", "OOPSLA", "ECOOP", "ICSE", "DISC", "OPODIS",
    ];

    if top_tier.iter().any(|&v| venue_upper.contains(v)) {
        "A*".to_string()
    } else if second_tier.iter().any(|&v| venue_upper.contains(v)) {
        "A".to_string()
    } else if venue_upper.contains("JOURNAL") || venue_upper.contains("TRANSACTIONS") {
        if venue_upper.contains("ACM") || venue_upper.contains("IEEE") {
            "A".to_string()
        } else {
            "B".to_string()
        }
    } else if venue_upper.contains("WORKSHOP") || venue_upper.contains("HOT") {
        "B".to_string()
    } else {
        "C".to_string()
    }
}

pub fn get_venue_field(venue: &str) -> String {
    let venue_upper = venue.to_uppercase();

    // CSRankings subcategories mapping
    let fields = [
        // Systems Area
        (
            vec!["SOSP", "OSDI", "EUROSYS", "ATC", "FAST", "VEE", "HOTOS"],
            "Operating Systems",
        ),
        (
            vec!["SIGCOMM", "NSDI", "CONEXT", "IMC"],
            "Computer Networks",
        ),
        (
            vec!["CCS", "SECURITY", "OAKLAND", "NDSS", "USENIXSEC"],
            "Computer Security",
        ),
        (
            vec!["SIGMOD", "VLDB", "ICDE", "PODS", "EDBT", "CIDR"],
            "Databases",
        ),
        (
            vec!["SIGMETRICS", "SIGMETRICS", "IMC"],
            "Measurement & Perf. Analysis",
        ),
        (vec!["DAC", "ICCAD"], "Design Automation"),
        (
            vec!["EMSOFT", "RTAS", "RTSS"],
            "Embedded & Real-Time Systems",
        ),
        (
            vec!["HPDC", "ICS", "SC", "PPoPP"],
            "High-Performance Computing",
        ),
        (
            vec!["MOBICOM", "MOBISYS", "SENSYS", "UBICOMP", "IMWUT"],
            "Mobile Computing",
        ),
        // AI Area
        (vec!["AAAI", "IJCAI"], "Artificial Intelligence"),
        (vec!["CVPR", "ECCV", "ICCV"], "Computer Vision"),
        (
            vec!["ICML", "NEURIPS", "NIPS", "ICLR"],
            "Machine Learning & Data Mining",
        ),
        (vec!["ACL", "EMNLP", "NAACL"], "Natural Language Processing"),
        (vec!["SIGIR", "WWW"], "The Web & Information Retrieval"),
        // Theory Area
        (vec!["STOC", "FOCS"], "Algorithms & Complexity"),
        (vec!["CRYPTO", "EUROCRYPT"], "Cryptography"),
        (vec!["CAV", "LICS"], "Logic & Verification"),
        (
            vec!["PODC", "SPAA", "DISC"],
            "Parallel & Distributed Computing",
        ),
        // Systems/Architecture
        (
            vec!["ASPLOS", "ISCA", "MICRO", "HPCA"],
            "Computer Architecture",
        ),
        (
            vec!["PLDI", "POPL", "ICFP", "OOPSLA"],
            "Programming Languages",
        ),
        (vec!["FSE", "ICSE", "ASE", "ISSTA"], "Software Engineering"),
        // Interdisciplinary Areas
        (
            vec!["SIGGRAPH", "SIGGRAPH ASIA", "EUROGRAPHICS"],
            "Computer Graphics",
        ),
        (vec!["EC", "WINE"], "Economics & Computation"),
        (
            vec!["CHI", "UIST", "IUI", "CSCW"],
            "Human-Computer Interaction",
        ),
        (vec!["ICRA", "IROS", "RSS"], "Robotics"),
        (vec!["VIS", "VR", "ISMAR"], "Visualization"),
        (vec!["ISMB", "RECOMB"], "Computational Biology"),
        (vec!["SIGCSE"], "Computer Science Education"),
        // Cloud/Distributed
        (vec!["SOCC"], "Cloud Computing"),
        (vec!["ICDCS", "MIDDLEWARE"], "Distributed Systems"),
        // Other important venues
        (vec!["KDD"], "Data Mining"),
        (vec!["INFOCOM"], "Networking"),
        (vec!["DSN"], "Dependable Systems"),
        (vec!["SODA"], "Algorithms"),
        (vec!["IPDPS"], "Parallel Processing"),
    ];

    for (keywords, field) in fields {
        if keywords.iter().any(|k| venue_upper.contains(k)) {
            return field.to_string();
        }
    }

    // Check for journals
    if venue_upper.contains("JOURNAL") || venue_upper.contains("TRANSACTIONS") {
        if venue_upper.contains("DATABASE") {
            return "Databases".to_string();
        } else if venue_upper.contains("NETWORK") {
            return "Computer Networks".to_string();
        } else if venue_upper.contains("PARALLEL") || venue_upper.contains("DISTRIBUTED") {
            return "Parallel & Distributed Computing".to_string();
        } else if venue_upper.contains("SOFTWARE") {
            return "Software Engineering".to_string();
        } else if venue_upper.contains("COMPUTER") {
            return "Computer Systems".to_string();
        }
    }

    "General".to_string()
}
