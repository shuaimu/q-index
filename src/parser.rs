use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Result, Context};
use nom_bibtex::{Bibtex, Bibliography};
use walkdir::WalkDir;
use indicatif::{ProgressBar, ProgressStyle};
use log::{info, warn, debug};

use crate::models::{
    Paper, Venue, Scholar, CitationGraph, VenueType,
    normalize_author_name, get_venue_tier, get_venue_field
};

pub struct BibParser {
    papers: HashMap<String, Paper>,
    venues: HashMap<String, Venue>,
    scholars: HashMap<String, Scholar>,
    strings: HashMap<String, String>,
}

impl BibParser {
    pub fn new() -> Self {
        Self {
            papers: HashMap::new(),
            venues: HashMap::new(),
            scholars: HashMap::new(),
            strings: HashMap::new(),
        }
    }
    
    pub fn parse_directory(&mut self, dir: &str) -> Result<CitationGraph> {
        let path = Path::new(dir);
        if !path.exists() {
            anyhow::bail!("Directory {} does not exist", dir);
        }
        
        // First, parse string definition files
        let strings_file = path.join("strings.bib");
        if strings_file.exists() {
            info!("Parsing string definitions from strings.bib");
            self.parse_strings(&strings_file)?;
        }
        
        let title_file = path.join("title.bib");
        if title_file.exists() {
            info!("Parsing venue definitions from title.bib");
            self.parse_strings(&title_file)?;
        }
        
        // Collect all .bib files
        let bib_files: Vec<PathBuf> = WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s == "bib")
                    .unwrap_or(false)
            })
            .map(|e| e.path().to_path_buf())
            .filter(|p| {
                let name = p.file_name().unwrap().to_str().unwrap();
                !name.contains("title") && !name.contains("strings")
            })
            .collect();
        
        info!("Found {} bibliography files", bib_files.len());
        
        // Parse each file with progress bar
        let pb = ProgressBar::new(bib_files.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("##-")
        );
        
        for file in &bib_files {
            let filename = file.file_name().unwrap().to_str().unwrap();
            pb.set_message(format!("Parsing {}", filename));
            
            if let Err(e) = self.parse_file(file) {
                warn!("Error parsing {}: {}", filename, e);
            }
            
            pb.inc(1);
        }
        
        pb.finish_with_message("Parsing complete");
        
        // Build citation graph
        let mut graph = self.build_graph();
        graph.build_citation_network();
        
        Ok(graph)
    }
    
    fn parse_strings(&mut self, file: &Path) -> Result<()> {
        let content = fs::read_to_string(file)
            .context("Failed to read title.bib")?;
        
        // Simple regex-based string extraction
        let re = regex::Regex::new(r#"@string\{(\w+)\s*=\s*"([^"]*)"\}"#).unwrap();
        
        for cap in re.captures_iter(&content) {
            let key = cap[1].to_string();
            let value = cap[2].to_string();
            self.strings.insert(key, value);
            debug!("Found string definition: {} = {}", &cap[1], &cap[2]);
        }
        
        Ok(())
    }
    
    fn parse_file(&mut self, file: &Path) -> Result<()> {
        let content = fs::read_to_string(file)
            .with_context(|| format!("Failed to read {:?}", file))?;
        
        let filename = file.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        // Parse with nom-bibtex
        match Bibtex::parse(&content) {
            Ok(bib) => {
                // The Bibtex struct has multiple fields for different entry types
                // We only care about bibliography entries
                let entries = bib.bibliographies();
                debug!("Parsing {} - found {} entries", filename, entries.len());
                
                for entry in entries {
                    if let Some(paper) = self.parse_entry(&entry, &filename) {
                        let paper_id = paper.id.clone();
                        debug!("  Parsed paper: {} -> venue: {}", paper_id, paper.venue);
                        self.papers.insert(paper_id.clone(), paper.clone());
                        self.add_to_venue(&paper);
                        self.add_scholars(&paper);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to parse {}: {:?}", filename, e);
            }
        }
        
        Ok(())
    }
    
    fn parse_entry(&self, entry: &Bibliography, source_file: &str) -> Option<Paper> {
        let entry_type = entry.entry_type();
        let cite_key = entry.citation_key();
        let fields = entry.tags();
        
        let mut paper = Paper {
            id: cite_key.to_string(),
            bib_key: cite_key.to_string(),
            source_file: source_file.to_string(),
            title: String::new(),
            authors: Vec::new(),
            venue: String::new(),
            venue_type: VenueType::Unknown,
            year: None,
            month: None,
            citations: Vec::new(),
            cited_by: Vec::new(),
            abstract_text: None,
            keywords: Vec::new(),
            doi: None,
            url: None,
        };
        
        // Extract fields from the key-value pairs
        for (key, value) in fields {
            // The value is likely already a string
            let value_str = value.clone();
            match key.as_str() {
                "title" => paper.title = clean_bibtex_string(&value_str),
                "author" => paper.authors = parse_authors(&value_str),
                "booktitle" => {
                    let venue = self.expand_string(&value_str);
                    paper.venue = extract_venue_name(&venue);
                }
                "journal" => {
                    let venue = self.expand_string(&value_str);
                    paper.venue = extract_venue_name(&venue);
                }
                "year" => paper.year = value_str.parse().ok(),
                "month" => paper.month = Some(value_str.clone()),
                "doi" => paper.doi = Some(value_str.clone()),
                "url" => paper.url = Some(value_str.clone()),
                "abstract" => paper.abstract_text = Some(clean_bibtex_string(&value_str)),
                "keywords" => {
                    paper.keywords = value_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect();
                }
                _ => {}
            }
        }
        
        // Determine venue type based on entry type string
        paper.venue_type = match entry_type.to_lowercase().as_str() {
            "inproceedings" | "conference" => VenueType::Conference,
            "article" => VenueType::Journal,
            "inworkshop" | "workshop" => VenueType::Workshop,
            "symposium" => VenueType::Symposium,
            _ => VenueType::Unknown,
        };
        
        // Only return if we have essential information
        if !paper.title.is_empty() && !paper.authors.is_empty() {
            Some(paper)
        } else {
            None
        }
    }
    
    fn expand_string(&self, value: &str) -> String {
        // Check if this is a string reference
        let trimmed = value.trim();
        
        // First check if it's in the strings map
        if let Some(expanded) = self.strings.get(trimmed) {
            return expanded.clone();
        }
        
        // Handle common venue abbreviations used in our BibTeX files
        // These map to the actual conference names
        let venue_map = match trimmed.to_lowercase().as_str() {
            // Systems conferences
            "osdi" => "OSDI",
            "sosp" => "SOSP",
            "eurosys" => "EuroSys",
            "nsdi" => "NSDI",
            "atc" | "usenix atc" => "USENIX ATC",
            "fast" => "FAST",
            "asplos" => "ASPLOS",
            "isca" => "ISCA",
            
            // Database conferences
            "sigmod" => "SIGMOD",
            "vldb" | "pvldb" => "VLDB",
            "icde" => "ICDE",
            "cidr" => "CIDR",
            "pods" => "PODS",
            
            // Security conferences
            "ccs" => "CCS",
            "oakland" | "sp" | "s&p" => "Oakland",
            "usenixsec" | "usenix security" => "USENIX Security",
            "ndss" => "NDSS",
            
            // ML/AI conferences
            "icml" => "ICML",
            "neurips" | "nips" => "NeurIPS",
            "iclr" => "ICLR",
            "cvpr" => "CVPR",
            "iccv" => "ICCV",
            "eccv" => "ECCV",
            "aaai" => "AAAI",
            "ijcai" => "IJCAI",
            
            // PL conferences
            "pldi" => "PLDI",
            "popl" => "POPL",
            "oopsla" => "OOPSLA",
            "icfp" => "ICFP",
            
            // Theory conferences
            "stoc" => "STOC",
            "focs" => "FOCS",
            "soda" => "SODA",
            "crypto" => "CRYPTO",
            "eurocrypt" => "EuroCrypt",
            "podc" => "PODC",
            "spaa" => "SPAA",
            
            // HCI conferences
            "chi" => "CHI",
            "uist" => "UIST",
            "ubicomp" | "pervasive" => "UbiComp",
            "iui" => "IUI",
            
            // Networking conferences
            "sigcomm" => "SIGCOMM",
            "infocom" => "INFOCOM",
            "imc" => "IMC",
            "sigmetrics" => "SIGMETRICS",
            
            // NLP conferences
            "acl" => "ACL",
            "naacl" => "NAACL",
            "emnlp" => "EMNLP",
            
            // Other conferences
            "hotos" => "HotOS",
            "socc" => "SoCC",
            "dsn" => "DSN",
            "sc" => "SC",
            "kdd" => "KDD",
            "www" => "WWW",
            "icse" => "ICSE",
            "fse" => "FSE",
            "ase" => "ASE",
            
            // Journals
            "tods" => "TODS",
            "tocs" => "TOCS",
            "toplas" => "TOPLAS",
            "tkde" => "TKDE",
            "jacm" => "JACM",
            "cacm" => "CACM",
            "jmlr" => "JMLR",
            "tmlr" => "TMLR",
            "csur" => "CSUR",
            
            // Default: return as-is
            _ => trimmed
        };
        
        venue_map.to_string()
    }
    
    fn add_to_venue(&mut self, paper: &Paper) {
        if paper.venue.is_empty() {
            return;
        }
        
        let venue_id = normalize_venue_id(&paper.venue);
        
        let venue = self.venues.entry(venue_id.clone()).or_insert_with(|| {
            Venue {
                id: venue_id.clone(),
                name: paper.venue.clone(),
                full_name: paper.venue.clone(),
                venue_type: paper.venue_type,
                papers: Vec::new(),
                pagerank: 0.0,
                impact_factor: 0.0,
                tier: get_venue_tier(&paper.venue),
                field: get_venue_field(&paper.venue),
            }
        });
        
        venue.papers.push(paper.id.clone());
    }
    
    fn add_scholars(&mut self, paper: &Paper) {
        for author in &paper.authors {
            let normalized_name = normalize_author_name(author);
            let scholar_id = generate_scholar_id(&normalized_name);
            
            let scholar = self.scholars.entry(scholar_id.clone()).or_insert_with(|| {
                Scholar {
                    id: scholar_id.clone(),
                    name: author.clone(),
                    normalized_name: normalized_name.clone(),
                    papers: Vec::new(),
                    affiliations: Vec::new(),
                    qindex: 0.0,
                    h_index: 0,
                    citation_count: 0,
                    publications_by_venue: HashMap::new(),
                    coauthors: HashMap::new(),
                }
            });
            
            scholar.papers.push(paper.id.clone());
            
            // Add to publications by venue
            let venue_id = normalize_venue_id(&paper.venue);
            scholar.publications_by_venue
                .entry(venue_id)
                .or_insert_with(Vec::new)
                .push(paper.id.clone());
            
            // Track coauthors
            for coauthor in &paper.authors {
                if coauthor != author {
                    let coauthor_id = generate_scholar_id(&normalize_author_name(coauthor));
                    *scholar.coauthors.entry(coauthor_id).or_insert(0) += 1;
                }
            }
        }
    }
    
    fn build_graph(&self) -> CitationGraph {
        let mut graph = CitationGraph::new();
        
        // Add all papers
        for (id, paper) in &self.papers {
            graph.papers.insert(id.clone(), paper.clone());
        }
        
        // Add all venues
        for (id, venue) in &self.venues {
            graph.venues.insert(id.clone(), venue.clone());
        }
        
        // Add all scholars
        for (id, scholar) in &self.scholars {
            graph.scholars.insert(id.clone(), scholar.clone());
        }
        
        graph
    }
}

fn clean_bibtex_string(s: &str) -> String {
    let mut result = s.trim().to_string();
    
    // Remove curly braces
    result = result.trim_matches('{').trim_matches('}').to_string();
    
    // Replace LaTeX special characters
    result = result.replace("\\&", "&");
    result = result.replace("\\'", "'");
    result = result.replace("\\\"", "\"");
    result = result.replace("\\-", "-");
    result = result.replace("~", " ");
    result = result.replace("--", "–");
    
    // Handle accented characters
    result = result.replace("\\'a", "á");
    result = result.replace("\\'e", "é");
    result = result.replace("\\'i", "í");
    result = result.replace("\\'o", "ó");
    result = result.replace("\\'u", "ú");
    
    result
}

fn parse_authors(authors_str: &str) -> Vec<String> {
    let cleaned = clean_bibtex_string(authors_str);
    
    cleaned.split(" and ")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != "others")
        .map(|s| s.to_string())
        .collect()
}

fn extract_venue_name(venue: &str) -> String {
    let cleaned = clean_bibtex_string(venue);
    
    // First, strip year if present (e.g., "CHI 2019" -> "CHI")
    let without_year = if let Some(pos) = cleaned.rfind(char::is_whitespace) {
        let potential_year = &cleaned[pos+1..];
        if potential_year.len() == 4 && potential_year.chars().all(|c| c.is_ascii_digit()) {
            cleaned[..pos].trim().to_string()
        } else {
            cleaned.clone()
        }
    } else {
        cleaned.clone()
    };
    
    // If it's already a short conference name (all caps or title case, no spaces), use it as-is
    if without_year.len() <= 20 && !without_year.contains("Proceedings") && !without_year.contains("Conference") {
        return without_year;
    }
    
    // Try to extract abbreviation from parentheses
    if let Some(start) = without_year.find('(') {
        if let Some(end) = without_year[start+1..].find(')') {
            return without_year[start+1..start+1+end].trim().to_string();
        }
    }
    
    // For longer names with "Proceedings" or "Conference", try to extract the key part
    if without_year.contains("Proceedings") || without_year.contains("Conference") {
        // Look for common patterns like "Proceedings of CONF" or "CONF Conference"
        if let Some(idx) = without_year.find(" of ") {
            let after_of = &without_year[idx+4..];
            if let Some(space_idx) = after_of.find(' ') {
                return after_of[..space_idx].trim().to_string();
            }
            return after_of.trim().to_string();
        }
    }
    
    // Otherwise return the venue without year
    without_year
}

pub fn normalize_venue_id(venue: &str) -> String {
    venue.trim()
        .to_uppercase()
        .replace(' ', "_")
        .replace('-', "_")
        .replace('\'', "")
}

pub fn generate_scholar_id(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .replace(' ', "_")
        .replace('.', "")
        .replace('-', "_")
}