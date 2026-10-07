use actix_web::{web, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use maud::{Markup, html};

use crate::web::state::APP_STATE;
use crate::web::templates::{
    index_page_async, venues_page, scholars_page, search_results_page, Stats, base_template,
    venue_detail_page, scholar_detail_page
};

#[derive(Deserialize)]
pub struct VenueQuery {
    field: Option<String>,
    tier: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
pub struct ScholarQuery {
    min_papers: Option<usize>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    q: String,
}

#[derive(Deserialize)]
pub struct PaginationQuery {
    page: Option<usize>,
    per_page: Option<usize>,
}

#[derive(Serialize)]
pub struct ApiResponse<T> {
    success: bool,
    data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

// HTML Handlers

pub async fn index_handler() -> Result<Markup> {
    // Return lightweight HTML that loads data via AJAX
    Ok(index_page_async())
}

pub async fn venues_handler(query: web::Query<VenueQuery>) -> Result<Markup> {
    let state = APP_STATE.get().ok_or_else(|| 
        actix_web::error::ErrorInternalServerError("Application state not initialized")
    )?;
    
    let limit = query.limit.unwrap_or(50);
    let venues = state.get_or_calculate_venues(limit);
    
    // Filter by field and tier if specified
    let filtered_venues: Vec<_> = venues.into_iter()
        .filter(|v| {
            query.field.as_ref().map_or(true, |f| v.field.to_lowercase().contains(&f.to_lowercase())) &&
            query.tier.as_ref().map_or(true, |t| &v.tier == t)
        })
        .collect();
    
    Ok(venues_page(&filtered_venues, query.field.as_deref(), query.tier.as_deref()))
}

pub async fn scholars_handler(query: web::Query<ScholarQuery>) -> Result<Markup> {
    let state = APP_STATE.get().ok_or_else(|| 
        actix_web::error::ErrorInternalServerError("Application state not initialized")
    )?;
    
    let limit = query.limit.unwrap_or(50);
    let scholars = state.get_or_calculate_scholars(limit);
    
    // Filter by minimum papers if specified
    let filtered_scholars: Vec<_> = scholars.into_iter()
        .filter(|s| {
            query.min_papers.map_or(true, |min| s.paper_count >= min)
        })
        .collect();
    
    Ok(scholars_page(&filtered_scholars))
}

pub async fn search_handler(query: web::Query<SearchQuery>) -> Result<Markup> {
    let state = APP_STATE.get().ok_or_else(|| 
        actix_web::error::ErrorInternalServerError("Application state not initialized")
    )?;
    
    let graph = state.graph.read().unwrap();
    
    // Search venues and scholars
    let venues = graph.search_venues(&query.q);
    let scholars = graph.search_scholars(&query.q);
    
    Ok(search_results_page(&query.q, &venues, &scholars))
}

pub async fn venue_detail_handler(
    path: web::Path<String>,
    query: web::Query<PaginationQuery>
) -> Result<Markup> {
    let venue_id = path.into_inner();
    
    let state = APP_STATE.get().ok_or_else(|| 
        actix_web::error::ErrorInternalServerError("Application state not initialized")
    )?;
    
    let graph = state.graph.read().unwrap();
    
    // Find the venue
    let venue = graph.venues.get(&venue_id)
        .ok_or_else(|| actix_web::error::ErrorNotFound("Venue not found"))?;
    
    // Get papers for this venue
    let mut papers: Vec<_> = venue.papers.iter()
        .filter_map(|paper_id| graph.papers.get(paper_id))
        .collect();
    
    // Sort papers by year (latest first)
    papers.sort_by(|a, b| {
        let year_a = a.year.unwrap_or(0);
        let year_b = b.year.unwrap_or(0);
        year_b.cmp(&year_a)
    });
    
    // Calculate venue statistics (before pagination)
    let total_citations: usize = papers.iter()
        .map(|p| p.cited_by.len())
        .sum();
    
    // Get top authors for this venue
    let mut author_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for paper in &papers {
        for author in &paper.authors {
            *author_counts.entry(author.clone()).or_insert(0) += 1;
        }
    }
    let mut top_authors: Vec<_> = author_counts.into_iter().collect();
    top_authors.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    top_authors.truncate(10);
    
    // Pagination
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let total_papers = papers.len();
    let total_pages = (total_papers + per_page - 1) / per_page;
    
    let start = (page - 1) * per_page;
    let end = (start + per_page).min(total_papers);
    let paginated_papers = &papers[start..end];
    
    Ok(venue_detail_page(
        venue, 
        paginated_papers, 
        total_citations, 
        &top_authors,
        page,
        total_pages,
        total_papers
    ))
}

pub async fn scholar_detail_handler(
    path: web::Path<String>,
    query: web::Query<PaginationQuery>
) -> Result<Markup> {
    let scholar_id = path.into_inner();
    let state = APP_STATE.get().ok_or_else(|| 
        actix_web::error::ErrorInternalServerError("Application state not initialized")
    )?;
    
    let graph = state.graph.read().unwrap();
    
    // Find the scholar
    let scholar = graph.scholars.get(&scholar_id)
        .ok_or_else(|| actix_web::error::ErrorNotFound("Scholar not found"))?;
    
    // Get papers for this scholar
    let mut papers: Vec<_> = scholar.papers.iter()
        .filter_map(|paper_id| graph.papers.get(paper_id))
        .collect();
    
    // Sort papers by year (latest first)
    papers.sort_by(|a, b| {
        let year_a = a.year.unwrap_or(0);
        let year_b = b.year.unwrap_or(0);
        year_b.cmp(&year_a)
    });
    
    // Group papers by venue (using all papers, not paginated)
    let mut papers_by_venue: std::collections::HashMap<String, Vec<&crate::models::Paper>> = std::collections::HashMap::new();
    for paper in &papers {
        papers_by_venue.entry(paper.venue.clone()).or_insert_with(Vec::new).push(*paper);
    }
    
    // Calculate scholar statistics (before pagination)
    let total_citations: usize = papers.iter()
        .map(|p| p.cited_by.len())
        .sum();
    
    // Pagination
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let total_papers = papers.len();
    let total_pages = (total_papers + per_page - 1) / per_page;
    
    let start = (page - 1) * per_page;
    let end = (start + per_page).min(total_papers);
    let paginated_papers = &papers[start..end];
    
    Ok(scholar_detail_page(
        scholar, 
        paginated_papers, 
        &papers_by_venue, 
        total_citations,
        page,
        total_pages,
        total_papers
    ))
}

pub async fn about_handler() -> Result<Markup> {
    Ok(base_template(
        "About - QIndex",
        html! {
            .container.py-5 {
                .row.justify-content-center {
                    .col-md-8 {
                        h1.mb-4 { "About QIndex" }
                        
                        .card.mb-4 {
                            .card-body {
                                h5.card-title { "What is QIndex?" }
                                p.card-text {
                                    "QIndex is an academic quality index calculator that uses the PageRank algorithm "
                                    "to evaluate the quality and impact of academic conferences, journals, and scholars. "
                                    "It analyzes citation networks to determine the relative importance of venues and researchers."
                                }
                            }
                        }
                        
                        .card.mb-4 {
                            .card-body {
                                h5.card-title { "How It Works" }
                                ul {
                                    li { "Parses bibliography data from BibTeX files" }
                                    li { "Builds citation networks between papers, venues, and scholars" }
                                    li { "Applies PageRank algorithm to calculate importance scores" }
                                    li { "Computes QIndex scores for scholars based on venue quality" }
                                    li { "Provides rankings and analytics for academic venues and researchers" }
                                }
                            }
                        }
                        
                        .card.mb-4 {
                            .card-body {
                                h5.card-title { "Data Sources" }
                                p.card-text {
                                    "QIndex uses bibliography data from major computer science conferences and journals, "
                                    "including venues tracked by CSRankings.org. Citation data can be enriched using "
                                    "APIs from Semantic Scholar, CrossRef, and other academic databases."
                                }
                            }
                        }
                        
                        .card {
                            .card-body {
                                h5.card-title { "Open Source" }
                                p.card-text {
                                    "QIndex is open source software. Contributions and feedback are welcome!"
                                }
                                a.btn.btn-primary href="https://github.com/shuaimu/q-index" target="_blank" {
                                    "View on GitHub"
                                }
                            }
                        }
                    }
                }
            }
        }
    ))
}

pub async fn statistics_handler() -> Result<Markup> {
    let state = APP_STATE.get().ok_or_else(|| 
        actix_web::error::ErrorInternalServerError("Application state not initialized")
    )?;
    
    let graph = state.graph.read().unwrap();
    
    // Calculate statistics
    let total_papers = graph.papers.len();
    let total_venues = graph.venues.len();
    let total_scholars = graph.scholars.len();
    let total_citations = graph.edges.len();
    
    // Papers by year
    let mut papers_by_year: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    for paper in graph.papers.values() {
        if let Some(year) = paper.year {
            *papers_by_year.entry(year).or_insert(0) += 1;
        }
    }
    
    // Papers by venue
    let mut papers_by_venue: Vec<(String, usize)> = graph.venues.iter()
        .map(|(_, v)| (v.name.clone(), v.papers.len()))
        .collect();
    papers_by_venue.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    papers_by_venue.truncate(10);
    
    // Top cited papers
    let mut top_cited: Vec<_> = graph.papers.values()
        .map(|p| (p.title.clone(), p.cited_by.len()))
        .collect();
    top_cited.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    top_cited.truncate(10);
    
    Ok(base_template(
        "Statistics - QIndex",
        html! {
            .container.py-5 {
                h1.mb-4 { "Dataset Statistics" }
                
                // Overview cards
                .row.mb-4 {
                    .col-md-3 {
                        .card.text-center {
                            .card-body {
                                h5.card-title { "Total Papers" }
                                .display-4 { (total_papers) }
                            }
                        }
                    }
                    .col-md-3 {
                        .card.text-center {
                            .card-body {
                                h5.card-title { "Total Venues" }
                                .display-4 { (total_venues) }
                            }
                        }
                    }
                    .col-md-3 {
                        .card.text-center {
                            .card-body {
                                h5.card-title { "Total Scholars" }
                                .display-4 { (total_scholars) }
                            }
                        }
                    }
                    .col-md-3 {
                        .card.text-center {
                            .card-body {
                                h5.card-title { "Total Citations" }
                                .display-4 { (total_citations) }
                            }
                        }
                    }
                }
                
                .row {
                    // Papers by year
                    .col-md-6.mb-4 {
                        .card.h-100 {
                            .card-header { h5.mb-0 { "Papers by Year" } }
                            .card-body {
                                @if papers_by_year.is_empty() {
                                    p.text-muted { "No year data available" }
                                } @else {
                                    .table-responsive {
                                        table.table.table-sm {
                                            thead {
                                                tr {
                                                    th { "Year" }
                                                    th { "Papers" }
                                                }
                                            }
                                            tbody {
                                                @for (year, count) in papers_by_year.iter().rev().take(10) {
                                                    tr {
                                                        td { (year) }
                                                        td { (count) }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    // Top venues by paper count
                    .col-md-6.mb-4 {
                        .card.h-100 {
                            .card-header { h5.mb-0 { "Top Venues by Paper Count" } }
                            .card-body {
                                .table-responsive {
                                    table.table.table-sm {
                                        thead {
                                            tr {
                                                th { "Venue" }
                                                th { "Papers" }
                                            }
                                        }
                                        tbody {
                                            @for (venue, count) in &papers_by_venue {
                                                tr {
                                                    td { (venue) }
                                                    td { (count) }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                // Most cited papers
                .card {
                    .card-header { h5.mb-0 { "Most Cited Papers" } }
                    .card-body {
                        .table-responsive {
                            table.table.table-sm {
                                thead {
                                    tr {
                                        th { "Paper Title" }
                                        th { "Citations" }
                                    }
                                }
                                tbody {
                                    @for (title, count) in &top_cited {
                                        tr {
                                            td { (title) }
                                            td { (count) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    ))
}

// API Handlers

pub async fn api_homepage() -> Result<HttpResponse> {
    let state = APP_STATE.get().ok_or_else(|| 
        actix_web::error::ErrorInternalServerError("Application state not initialized")
    )?;
    
    let top_venues = state.get_or_calculate_venues(10);
    let top_scholars = state.get_or_calculate_scholars(10);
    
    let graph = state.graph.read().unwrap();
    
    #[derive(Serialize)]
    struct HomepageData {
        stats: Stats,
        top_venues: Vec<crate::models::VenueRanking>,
        top_scholars: Vec<crate::models::ScholarRanking>,
    }
    
    let data = HomepageData {
        stats: Stats {
            total_papers: graph.papers.len(),
            total_venues: graph.venues.len(),
            total_scholars: graph.scholars.len(),
            total_citations: graph.edges.len(),
        },
        top_venues,
        top_scholars,
    };
    
    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data,
        error: None,
    }))
}

pub async fn api_venues() -> Result<HttpResponse> {
    let state = APP_STATE.get().ok_or_else(|| 
        actix_web::error::ErrorInternalServerError("Application state not initialized")
    )?;
    
    let venues = state.get_or_calculate_venues(100);
    
    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: venues,
        error: None,
    }))
}

pub async fn api_scholars() -> Result<HttpResponse> {
    let state = APP_STATE.get().ok_or_else(|| 
        actix_web::error::ErrorInternalServerError("Application state not initialized")
    )?;
    
    let scholars = state.get_or_calculate_scholars(100);
    
    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: scholars,
        error: None,
    }))
}

pub async fn api_search(query: web::Query<SearchQuery>) -> Result<HttpResponse> {
    let state = APP_STATE.get().ok_or_else(|| 
        actix_web::error::ErrorInternalServerError("Application state not initialized")
    )?;
    
    let graph = state.graph.read().unwrap();
    
    #[derive(Serialize)]
    struct SearchResults {
        venues: Vec<crate::models::VenueRanking>,
        scholars: Vec<crate::models::ScholarRanking>,
    }
    
    let results = SearchResults {
        venues: graph.search_venues(&query.q),
        scholars: graph.search_scholars(&query.q),
    };
    
    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: results,
        error: None,
    }))
}

pub async fn api_stats() -> Result<HttpResponse> {
    let state = APP_STATE.get().ok_or_else(|| 
        actix_web::error::ErrorInternalServerError("Application state not initialized")
    )?;
    
    let graph = state.graph.read().unwrap();
    
    #[derive(Serialize)]
    struct Statistics {
        total_papers: usize,
        total_venues: usize,
        total_scholars: usize,
        total_citations: usize,
        papers_by_year: std::collections::HashMap<u32, usize>,
        papers_by_tier: std::collections::HashMap<String, usize>,
        papers_by_field: std::collections::HashMap<String, usize>,
    }
    
    let mut papers_by_year = std::collections::HashMap::new();
    for paper in graph.papers.values() {
        if let Some(year) = paper.year {
            *papers_by_year.entry(year).or_insert(0) += 1;
        }
    }
    
    let mut papers_by_tier = std::collections::HashMap::new();
    let mut papers_by_field = std::collections::HashMap::new();
    
    for venue in graph.venues.values() {
        let count = venue.papers.len();
        *papers_by_tier.entry(venue.tier.clone()).or_insert(0) += count;
        *papers_by_field.entry(venue.field.clone()).or_insert(0) += count;
    }
    
    let stats = Statistics {
        total_papers: graph.papers.len(),
        total_venues: graph.venues.len(),
        total_scholars: graph.scholars.len(),
        total_citations: graph.edges.len(),
        papers_by_year,
        papers_by_tier,
        papers_by_field,
    };
    
    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: stats,
        error: None,
    }))
}

pub async fn api_stats_fields() -> Result<HttpResponse> {
    let state = APP_STATE.get().ok_or_else(|| 
        actix_web::error::ErrorInternalServerError("Application state not initialized")
    )?;
    
    let graph = state.graph.read().unwrap();
    
    let mut field_counts = std::collections::HashMap::new();
    for venue in graph.venues.values() {
        *field_counts.entry(venue.field.clone()).or_insert(0) += venue.papers.len();
    }
    
    let mut field_vec: Vec<_> = field_counts.into_iter().collect();
    field_vec.sort_by(|a, b| b.1.cmp(&a.1));
    
    #[derive(Serialize)]
    struct FieldData {
        labels: Vec<String>,
        values: Vec<usize>,
    }
    
    let data = FieldData {
        labels: field_vec.iter().map(|(k, _)| k.clone()).collect(),
        values: field_vec.iter().map(|(_, v)| *v).collect(),
    };
    
    Ok(HttpResponse::Ok().json(data))
}

pub async fn api_citation_status() -> Result<HttpResponse> {
    use std::collections::HashMap;
    use std::path::Path;
    use std::process::Command;
    
    #[derive(Serialize)]
    struct CitationStatus {
        total_papers: usize,
        total_citations: usize,
        success_rate: f64,
        papers_per_minute: f64,
        active_sources: Vec<String>,
        current_conference: Option<String>,
        successful: usize,
        total_attempts: usize,
        conferences: Vec<ConferenceStatus>,
        citation_distribution: HashMap<String, usize>,
        recent_papers: Vec<RecentPaper>,
        is_running: bool,
        last_update_seconds_ago: u64,
        process_info: ProcessInfo,
    }
    
    #[derive(Serialize)]
    struct ProcessInfo {
        running: bool,
        pid: Option<u32>,
        process_name: Option<String>,
    }
    
    #[derive(Serialize)]
    struct ConferenceStatus {
        name: String,
        status: String,
        papers_fetched: usize,
        total_papers: usize,
        citations: usize,
    }
    
    #[derive(Serialize)]
    struct RecentPaper {
        title: String,
        citations: usize,
    }
    
    // Read cache files
    let cache_path = Path::new("cache/citations/combined_cache.json");
    let dblp_cache_path = Path::new("cache/citations/dblp_cache.json");
    let paper_cache_path = Path::new("cache/citations/paper_cache.json");
    
    let mut total_papers = 0;
    let mut total_citations = 0;
    let mut citation_dist = HashMap::new();
    let mut recent_papers = Vec::new();
    
    // Try combined cache first
    if cache_path.exists() {
        if let Ok(content) = std::fs::read_to_string(cache_path) {
            if let Ok(cache) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&content) {
                total_papers = cache.len();
                
                for (_, paper) in cache.iter() {
                    if let Some(count) = paper.get("citation_count").and_then(|c| c.as_u64()) {
                        total_citations += count as usize;
                        
                        // Update distribution
                        let bucket = match count {
                            0..=10 => "0-10",
                            11..=50 => "11-50",
                            51..=100 => "51-100",
                            101..=500 => "101-500",
                            _ => "500+",
                        };
                        *citation_dist.entry(bucket.to_string()).or_insert(0) += 1;
                    }
                    
                    // Add to recent papers (last 5)
                    if recent_papers.len() < 5 {
                        if let Some(title) = paper.get("title").and_then(|t| t.as_str()) {
                            recent_papers.push(RecentPaper {
                                title: title.to_string(),
                                citations: paper.get("citation_count").and_then(|c| c.as_u64()).unwrap_or(0) as usize,
                            });
                        }
                    }
                }
            }
        }
    } else if paper_cache_path.exists() {
        // Fallback to paper_cache.json
        if let Ok(content) = std::fs::read_to_string(paper_cache_path) {
            if let Ok(cache) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&content) {
                total_papers = cache.len();
                
                for (_, paper) in cache.iter() {
                    if let Some(count) = paper.get("citation_count").and_then(|c| c.as_u64()) {
                        total_citations += count as usize;
                    }
                }
            }
        }
    }
    
    // Calculate conference status (simplified for now)
    let mut conferences = Vec::new();
    let bib_dir = Path::new("bib");
    if bib_dir.exists() {
        for entry in std::fs::read_dir(bib_dir).unwrap() {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("bib") {
                    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
                    
                    // Count papers in this bib file
                    let total_in_conf = if let Ok(content) = std::fs::read_to_string(&path) {
                        content.matches("@inproceedings").count() + content.matches("@article").count()
                    } else {
                        0
                    };
                    
                    conferences.push(ConferenceStatus {
                        name: name.to_uppercase(),
                        status: if total_papers > 0 { "processing" } else { "pending" }.to_string(),
                        papers_fetched: 0, // Would need to track per-conference
                        total_papers: total_in_conf,
                        citations: 0,
                    });
                }
            }
        }
    }
    
    // Sort conferences by name
    conferences.sort_by(|a, b| a.name.cmp(&b.name));
    
    // Check if fetch process is running
    let mut process_info = ProcessInfo {
        running: false,
        pid: None,
        process_name: None,
    };
    
    // Try to check if Python fetch process is running
    if let Ok(output) = Command::new("sh")
        .arg("-c")
        .arg("ps aux | grep -E 'python.*fetch_citations' | grep -v grep | head -1")
        .output()
    {
        let output_str = String::from_utf8_lossy(&output.stdout);
        if !output_str.trim().is_empty() {
            process_info.running = true;
            // Try to extract PID (second field in ps aux output)
            let parts: Vec<&str> = output_str.split_whitespace().collect();
            if parts.len() > 1 {
                if let Ok(pid) = parts[1].parse::<u32>() {
                    process_info.pid = Some(pid);
                }
            }
            // Extract process name
            if parts.len() > 10 {
                process_info.process_name = Some(parts[10..].join(" "));
            }
        }
    }
    
    // Calculate last update time
    let mut last_update_seconds_ago = 999999;
    if let Ok(metadata) = std::fs::metadata(&cache_path) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                last_update_seconds_ago = elapsed.as_secs();
            }
        }
    }
    
    // Determine if really running based on recent updates
    let is_running = process_info.running || last_update_seconds_ago < 60;
    
    let status = CitationStatus {
        total_papers,
        total_citations,
        success_rate: if total_papers > 0 { 100.0 } else { 0.0 },
        papers_per_minute: 2.0, // Estimated rate
        active_sources: vec!["DBLP".to_string(), "OpenAlex".to_string()],
        current_conference: None,
        successful: total_papers,
        total_attempts: total_papers,
        conferences,
        citation_distribution: citation_dist,
        recent_papers,
        is_running,
        last_update_seconds_ago,
        process_info,
    };
    
    Ok(HttpResponse::Ok().json(status))
}

