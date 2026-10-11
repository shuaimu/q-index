use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, info, warn};
use nom_bibtex::{Bibliography, Bibtex};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::models::{
    get_venue_field, normalize_author_name, CitationGraph, Paper, Scholar, Venue, VenueType,
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

        // A parser can be reused; each directory is a fresh bibliography.
        self.papers.clear();
        self.venues.clear();
        self.scholars.clear();
        self.strings.clear();

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
        let mut bib_files: Vec<PathBuf> = WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
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
        // Duplicate keys use the last record in sorted path order, and source
        // order within a file. Never let filesystem iteration choose a winner.
        bib_files.sort();

        info!("Found {} bibliography files", bib_files.len());

        // Parse each file with progress bar
        let pb = ProgressBar::new(bib_files.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("##-"),
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
        self.rebuild_indexes();

        // Build citation graph
        let mut graph = self.build_graph();
        graph.build_citation_network();

        Ok(graph)
    }

    fn parse_strings(&mut self, file: &Path) -> Result<()> {
        let content = fs::read_to_string(file).context("Failed to read title.bib")?;

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
        let content =
            fs::read_to_string(file).with_context(|| format!("Failed to read {:?}", file))?;

        let filename = file
            .file_name()
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
                        if let Some(previous) = self.papers.insert(paper_id.clone(), paper) {
                            warn!(
                                "Repeated BibTeX key {}: replacing {} with the later record in {}",
                                paper_id, previous.source_file, filename
                            );
                        }
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
        if entry_type.eq_ignore_ascii_case("proceedings") {
            return None;
        }
        let cite_key = entry.citation_key();
        let mut fields: Vec<_> = entry.tags().iter().collect();
        fields.sort_by(|(a, _), (b, _)| a.cmp(b));

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
            dblp_id: None,
            url: None,
        };

        // Extract fields from the key-value pairs
        for (key, value) in fields {
            let value_str = value;
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
                "institution"
                    if entry_type.eq_ignore_ascii_case("techreport") && paper.venue.is_empty() =>
                {
                    paper.venue = clean_bibtex_string(&value_str);
                }
                "year" => paper.year = value_str.parse().ok(),
                "month" => paper.month = Some(value_str.clone()),
                "doi" => paper.doi = Some(value_str.clone()),
                "dblp" => paper.dblp_id = Some(value_str.clone()),
                "url" => paper.url = Some(value_str.clone()),
                "abstract" => paper.abstract_text = Some(clean_bibtex_string(&value_str)),
                "keywords" => {
                    paper.keywords = value_str.split(',').map(|s| s.trim().to_string()).collect();
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
            "techreport" => VenueType::TechnicalReport,
            _ => VenueType::Unknown,
        };

        // Conference volumes list editors, not authors of a research paper.
        if is_proceedings_title(&paper.title) {
            return None;
        }

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
            _ => trimmed,
        };

        venue_map.to_string()
    }

    fn rebuild_indexes(&mut self) {
        self.venues.clear();
        self.scholars.clear();
        let papers = std::mem::take(&mut self.papers);
        let mut ordered: Vec<_> = papers.values().collect();
        ordered.sort_by(|a, b| a.id.cmp(&b.id));
        for paper in ordered {
            self.add_to_venue(paper);
            self.add_scholars(paper);
        }
        self.papers = papers;
    }

    fn add_to_venue(&mut self, paper: &Paper) {
        if paper.venue.is_empty() {
            return;
        }

        let venue_id = normalize_venue_id(&paper.venue);

        let venue = self
            .venues
            .entry(venue_id.clone())
            .or_insert_with(|| Venue {
                id: venue_id.clone(),
                name: paper.venue.clone(),
                full_name: paper.venue.clone(),
                venue_type: paper.venue_type,
                papers: Vec::new(),
                pagerank: 0.0,
                impact_factor: 0.0,
                field: get_venue_field(&paper.venue),
            });

        venue.papers.push(paper.id.clone());
    }

    fn add_scholars(&mut self, paper: &Paper) {
        let mut seen = HashSet::new();
        for author in &paper.authors {
            let normalized_name = normalize_author_name(author);
            let scholar_id = generate_scholar_id(&normalized_name);
            if !seen.insert(scholar_id.clone()) {
                continue;
            }

            let scholar = self
                .scholars
                .entry(scholar_id.clone())
                .or_insert_with(|| Scholar {
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
                });

            scholar.papers.push(paper.id.clone());

            // Add to publications by venue
            let venue_id = normalize_venue_id(&paper.venue);
            scholar
                .publications_by_venue
                .entry(venue_id)
                .or_insert_with(Vec::new)
                .push(paper.id.clone());

            // Track coauthors
            let mut coauthors = HashSet::new();
            for coauthor in &paper.authors {
                let coauthor_id = generate_scholar_id(&normalize_author_name(coauthor));
                if coauthor_id != scholar_id && coauthors.insert(coauthor_id.clone()) {
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

fn is_proceedings_title(title: &str) -> bool {
    let words = || {
        title
            .split_whitespace()
            .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()))
    };
    if words()
        .next()
        .is_some_and(|word| word.eq_ignore_ascii_case("proceedings"))
    {
        return true;
    }
    if !words().any(|word| {
        ["conference", "symposium", "workshop"]
            .iter()
            .any(|event| word.eq_ignore_ascii_case(event))
    }) {
        return false;
    }
    let has_year = words().any(|word| word.len() == 4 && word.chars().all(|c| c.is_ascii_digit()));
    let has_month = words().any(|word| {
        [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ]
        .iter()
        .any(|month| word.eq_ignore_ascii_case(month))
    });
    has_year && has_month
}

fn clean_bibtex_string(s: &str) -> String {
    // Interpret accents before stripping grouping braces or command prefixes.
    lazy_static::lazy_static! {
        static ref ACCENT: regex::Regex = regex::Regex::new(
            r#"\\(['"`^~=.uvHck])\s*\{?([A-Za-z])\}?"#
        ).unwrap();
    }
    let accented = ACCENT.replace_all(s.trim(), |cap: &regex::Captures<'_>| {
        let letters = match &cap[1] {
            "'" => ("AaCcEeIiLlNnOoRrSsUuYyZz", "ÁáĆćÉéÍíĹĺŃńÓóŔŕŚśÚúÝýŹź"),
            "\"" => ("AaEeIiOoUuYy", "ÄäËëÏïÖöÜüŸÿ"),
            "`" => ("AaEeIiOoUu", "ÀàÈèÌìÒòÙù"),
            "^" => ("AaCcEeGgHhIiJjOoSsUuWwYy", "ÂâĈĉÊêĜĝĤĥÎîĴĵÔôŜŝÛûŴŵŶŷ"),
            "~" => ("AaIiNnOoUu", "ÃãĨĩÑñÕõŨũ"),
            "=" => ("AaEeIiOoUuYy", "ĀāĒēĪīŌōŪūȲȳ"),
            "." => ("CcEeGgIiZz", "ĊċĖėĠġİiŻż"),
            "u" => ("AaEeGgIiOoUu", "ĂăĔĕĞğĬĭŎŏŬŭ"),
            "v" => ("CcDdEeLlNnRrSsTtZz", "ČčĎďĚěĽľŇňŘřŠšŤťŽž"),
            "H" => ("OoUu", "ŐőŰű"),
            "c" => ("CcGgKkLlNnRrSsTt", "ÇçĢģĶķĻļŅņŖŗŞşŢţ"),
            "k" => ("AaEeIiUu", "ĄąĘęĮįŲų"),
            _ => unreachable!(),
        };
        let letter = cap[2].chars().next().unwrap();
        letters
            .0
            .chars()
            .zip(letters.1.chars())
            .find_map(|(plain, accented)| (plain == letter).then_some(accented.to_string()))
            .unwrap_or_else(|| cap[0].to_string())
    });
    let mut result = String::with_capacity(accented.len());
    let mut chars = accented.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' | '}' => {}
            '\\' if chars
                .peek()
                .is_some_and(|next| matches!(next, '{' | '}' | '&' | '%' | '_' | '#' | '$')) =>
            {
                result.push(chars.next().unwrap());
            }
            '~' => result.push(' '),
            _ => result.push(ch),
        }
    }
    let result = result
        .replace("---", "—")
        .replace("--", "–")
        .replace("\\-", "")
        .replace("\\ss", "ß")
        .replace("\\ae", "æ")
        .replace("\\AE", "Æ")
        .replace("\\oe", "œ")
        .replace("\\OE", "Œ")
        .replace("\\aa", "å")
        .replace("\\AA", "Å")
        .replace("\\o", "ø")
        .replace("\\O", "Ø")
        .replace("\\l", "ł")
        .replace("\\L", "Ł");
    html_escape::decode_html_entities(&result)
        .trim()
        .to_string()
}

fn parse_authors(authors_str: &str) -> Vec<String> {
    // Grouped corporate names can contain "and" and commas.
    let mut authors = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut escaped = false;
    for (index, ch) in authors_str.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            'a' if depth == 0
                && authors_str[index..].starts_with("and")
                && (index == 0 || authors_str[..index].ends_with(char::is_whitespace))
                && authors_str[index + 3..].starts_with(char::is_whitespace) =>
            {
                authors.push(&authors_str[start..index]);
                start = index + 3;
            }
            _ => {}
        }
    }
    authors.push(&authors_str[start..]);
    authors
        .into_iter()
        .filter_map(|raw| {
            let cleaned = clean_bibtex_string(raw);
            if cleaned.is_empty() || cleaned == "others" {
                return None;
            }
            if is_grouped_name(raw.trim()) {
                return Some(cleaned);
            }
            let parts: Vec<_> = cleaned.split(',').map(str::trim).collect();
            let name = match parts.as_slice() {
                [last, first] => format!("{} {}", first, last),
                [last, suffix, first] => format!("{} {} {}", first, last, suffix),
                _ => cleaned,
            };
            Some(name.split_whitespace().collect::<Vec<_>>().join(" "))
        })
        .collect()
}

fn is_grouped_name(name: &str) -> bool {
    if !name.starts_with('{') || !name.ends_with('}') {
        return false;
    }
    let mut depth = 0usize;
    let mut escaped = false;
    for (index, ch) in name.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return index + 1 == name.len();
                }
            }
            _ => {}
        }
    }
    false
}

fn extract_venue_name(venue: &str) -> String {
    let cleaned = clean_bibtex_string(venue);

    // First, strip year if present (e.g., "CHI 2019" -> "CHI")
    let without_year = if let Some(pos) = cleaned.rfind(char::is_whitespace) {
        let potential_year = &cleaned[pos + 1..];
        if potential_year.len() == 4 && potential_year.chars().all(|c| c.is_ascii_digit()) {
            cleaned[..pos].trim().to_string()
        } else {
            cleaned.clone()
        }
    } else {
        cleaned.clone()
    };

    // If it's already a short conference name (all caps or title case, no spaces), use it as-is
    if without_year.len() <= 20
        && !without_year.contains("Proceedings")
        && !without_year.contains("Conference")
    {
        return without_year;
    }

    // Try to extract abbreviation from parentheses
    if let Some(start) = without_year.find('(') {
        if let Some(end) = without_year[start + 1..].find(')') {
            return without_year[start + 1..start + 1 + end].trim().to_string();
        }
    }

    // For longer names with "Proceedings" or "Conference", try to extract the key part
    if without_year.contains("Proceedings") || without_year.contains("Conference") {
        // Look for common patterns like "Proceedings of CONF" or "CONF Conference"
        if let Some(idx) = without_year.find(" of ") {
            let after_of = &without_year[idx + 4..];
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
    let mut normalized = String::with_capacity(venue.len());
    for character in venue.trim().chars().flat_map(char::to_uppercase) {
        match character {
            ' ' | '-' => normalized.push('_'),
            '\'' => {}
            _ => normalized.push(character),
        }
    }
    normalized
}

pub fn generate_scholar_id(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .replace(' ', "_")
        .replace('.', "")
        .replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_records(records: &str) -> CitationGraph {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("papers.bib"), records).unwrap();
        BibParser::new()
            .parse_directory(dir.path().to_str().unwrap())
            .unwrap()
    }

    #[test]
    fn metadata_is_readable_and_author_identity_and_order_are_preserved() {
        let graph = parse_records(
            r#"
@inproceedings{one,
 title = {Can&apos;t {ZKML}: {&#x3B1;} &amp; {Systems}},
 author = {Krishnamurthy, Arvind and Arpaci-Dusseau, Remzi H. and Chen 0001, Haibo and Garc{\'i}a, Jos{\'e} and {de la Cruz}, Ana},
 booktitle = {OSDI}, year = {2025}
}
@inproceedings{two,
 title = {Another paper},
 author = {Arvind Krishnamurthy and Remzi H. Arpaci-Dusseau and Haibo Chen 0001},
 booktitle = {OSDI}, year = {2026}
}
@inproceedings{three,
 title = {Distinct identity},
 author = {Haibo Chen 0002},
 booktitle = {OSDI}, year = {2026}
}
"#,
        );
        let paper = &graph.papers["one"];
        assert_eq!(paper.title, "Can't ZKML: α & Systems");
        assert_eq!(
            paper.authors,
            [
                "Arvind Krishnamurthy",
                "Remzi H. Arpaci-Dusseau",
                "Haibo Chen 0001",
                "José García",
                "Ana de la Cruz",
            ]
        );
        for id in [
            "arvind_krishnamurthy",
            "remzi_h_arpaci_dusseau",
            "haibo_chen_0001",
        ] {
            assert_eq!(graph.scholars[id].papers, ["one", "two"]);
        }
        assert_eq!(graph.scholars["haibo_chen_0002"].papers, ["three"]);
        assert_eq!(
            graph.scholars["remzi_h_arpaci_dusseau"].name,
            "Remzi H. Arpaci-Dusseau"
        );
    }

    #[test]
    fn technical_reports_preserve_the_issuing_institution_without_changing_journal_venues() {
        let graph = parse_records(
            r#"
@techreport{report, title={Extended research report}, author={Alice Researcher}, institution={University of {Washington} CSE}, year={2015}}
@article{article, title={Journal research}, author={Alice Researcher}, institution={An author affiliation}, journal={PVLDB}, year={2015}}
"#,
        );
        assert_eq!(graph.papers["report"].venue, "University of Washington CSE");
        assert_eq!(
            graph.papers["report"].venue_type,
            VenueType::TechnicalReport
        );
        assert_eq!(graph.papers["article"].venue, "VLDB");
        assert_eq!(graph.papers["article"].venue_type, VenueType::Journal);
    }

    #[test]
    fn final_records_alone_define_indexes_with_deterministic_duplicate_policy() {
        let dir = tempfile::tempdir().unwrap();
        // Write paths in reverse order to ensure creation order is irrelevant.
        fs::write(dir.path().join("z.bib"), r#"
@inproceedings{shared, title={Final}, author={Final, Alice and Alice Final and Bob Peer}, booktitle={OSDI}, year={2026}}
"#).unwrap();
        fs::write(
            dir.path().join("a.bib"),
            r#"
@inproceedings{shared, title={Old}, author={Ghost Person}, booktitle={SOSP}, year={2024}}
@inproceedings{local, title={Overwritten}, author={Another Ghost}, booktitle={NSDI}, year={2024}}
@inproceedings{local, title={Local final}, author={Bob Peer}, booktitle={OSDI}, year={2025}}
"#,
        )
        .unwrap();
        let mut parser = BibParser::new();
        let graph = parser
            .parse_directory(dir.path().to_str().unwrap())
            .unwrap();
        assert_eq!(graph.papers["shared"].title, "Final");
        assert_eq!(graph.papers["local"].title, "Local final");
        assert_eq!(graph.scholars.len(), 2);
        assert_eq!(graph.scholars["alice_final"].papers, ["shared"]);
        assert_eq!(graph.scholars["bob_peer"].papers, ["local", "shared"]);
        assert_eq!(
            graph.scholars["alice_final"].publications_by_venue["OSDI"],
            ["shared"]
        );
        assert_eq!(graph.scholars["bob_peer"].coauthors["alice_final"], 1);
        assert!(!graph.scholars["alice_final"]
            .coauthors
            .contains_key("alice_final"));
        assert_eq!(graph.venues.len(), 1);
        assert_eq!(graph.venues["OSDI"].papers, ["local", "shared"]);

        let fresh = tempfile::tempdir().unwrap();
        fs::write(
            fresh.path().join("fresh.bib"),
            r#"
@inproceedings{fresh, title={Fresh}, author={New Person}, booktitle={SOSP}, year={2026}}
"#,
        )
        .unwrap();
        let graph = parser
            .parse_directory(fresh.path().to_str().unwrap())
            .unwrap();
        assert_eq!(graph.papers.len(), 1);
        assert_eq!(graph.scholars.len(), 1);
        assert!(graph.scholars.contains_key("new_person"));
        assert!(!graph.venues.contains_key("OSDI"));
    }

    #[test]
    fn grouped_corporate_authors_do_not_split_or_reorder() {
        let graph = parse_records(
            r#"
@inproceedings{group, title={{Research} and Development}, author={{Research and Development, Inc.} and M{\"u}ller, J{\"o}rg}, booktitle={OSDI}, year={2026}}
"#,
        );
        assert_eq!(
            graph.papers["group"].authors,
            ["Research and Development, Inc.", "Jörg Müller",]
        );
    }
    #[test]
    fn conference_volumes_do_not_credit_editors_as_paper_authors() {
        let graph = parse_records(
            r#"
@proceedings{typed, title={A conference volume}, author={Editor One}, booktitle={SOSP}, year={2024}}
@inproceedings{prefixed, title={Proceedings of the OSDI Conference}, author={Editor Two}, booktitle={OSDI}, year={2024}}
@inproceedings{dated, title={Eighth Eurosys Conference 2013, Prague, April 14-17, 2013}, author={Editor Three}, booktitle={EuroSys}, year={2013}}
@inproceedings{paper, title={Efficient Conference Scheduling}, author={Real Author}, booktitle={OSDI}, year={2024}}
"#,
        );
        assert_eq!(
            graph.papers.keys().map(String::as_str).collect::<Vec<_>>(),
            ["paper"]
        );
        assert_eq!(
            graph
                .scholars
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["real_author"]
        );
        assert_eq!(graph.venues["OSDI"].papers, ["paper"]);
    }
}
