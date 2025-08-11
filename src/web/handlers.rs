use actix_web::{web, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use maud::{Markup, html};

use crate::web::state::APP_STATE;
use crate::web::templates::{
    index_page, venues_page, scholars_page, search_results_page, Stats, base_template
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

#[derive(Serialize)]
pub struct ApiResponse<T> {
    success: bool,
    data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

// HTML Handlers

pub async fn index_handler() -> Result<Markup> {
    let state = APP_STATE.get().ok_or_else(|| 
        actix_web::error::ErrorInternalServerError("Application state not initialized")
    )?;
    
    let top_venues = state.get_or_calculate_venues(10);
    let top_scholars = state.get_or_calculate_scholars(10);
    
    let graph = state.graph.read().unwrap();
    let stats = Stats {
        total_papers: graph.papers.len(),
        total_venues: graph.venues.len(),
        total_scholars: graph.scholars.len(),
        total_citations: graph.edges.len(),
    };
    
    Ok(index_page(&top_venues, &top_scholars, &stats))
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

// API Handlers

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

pub async fn about_handler() -> Result<Markup> {
    Ok(base_template("About", html! {
        div class="row" {
            div class="col-lg-8 mx-auto" {
                h1 class="mb-4" {
                    i class="bi bi-info-circle me-3" {}
                    "About QIndex"
                }
                
                div class="card mb-4" {
                    div class="card-body" {
                        h5 class="card-title" { "What is QIndex?" }
                        p class="card-text" {
                            "QIndex is an academic quality index calculator that uses the PageRank algorithm "
                            "to evaluate conferences, journals, and scholars based on citation networks. "
                            "Unlike traditional metrics like h-index, QIndex considers venue prestige and "
                            "citation patterns to provide a more nuanced evaluation of academic impact."
                        }
                    }
                }
                
                div class="card mb-4" {
                    div class="card-body" {
                        h5 class="card-title" { "Algorithm" }
                        h6 { "Venue PageRank" }
                        p {
                            "We build a directed graph where nodes represent venues and edges represent "
                            "citations between papers published in those venues. The PageRank algorithm "
                            "then calculates the relative importance of each venue."
                        }
                        
                        h6 class="mt-3" { "Scholar QIndex" }
                        p {
                            "Scholar scores combine multiple factors:"
                        }
                        pre class="bg-light p-3" {
                            code {
                                "QIndex = Σ(paper_score) × log(total_papers + 1)\n"
                                "\n"
                                "where paper_score = venue_pagerank × year_decay × author_position_weight"
                            }
                        }
                    }
                }
                
                div class="card mb-4" {
                    div class="card-body" {
                        h5 class="card-title" { "Technology Stack" }
                        ul {
                            li { strong { "Language:" } " Rust (for performance and safety)" }
                            li { strong { "Web Framework:" } " Actix-Web" }
                            li { strong { "Templating:" } " Maud (type-safe HTML)" }
                            li { strong { "Algorithm:" } " Custom PageRank implementation" }
                            li { strong { "Data:" } " BibTeX parsing with nom-bibtex" }
                        }
                    }
                }
                
                div class="card" {
                    div class="card-body" {
                        h5 class="card-title" { "Open Source" }
                        p class="card-text" {
                            "QIndex is open source and available on "
                            a href="https://github.com/shuai/qindex" target="_blank" {
                                "GitHub"
                                i class="bi bi-box-arrow-up-right ms-1" {}
                            }
                            ". Contributions, bug reports, and feature requests are welcome!"
                        }
                    }
                }
            }
        }
    }))
}

pub async fn statistics_handler() -> Result<Markup> {
    let state = APP_STATE.get().ok_or_else(|| 
        actix_web::error::ErrorInternalServerError("Application state not initialized")
    )?;
    
    let graph = state.graph.read().unwrap();
    
    Ok(base_template("Statistics", html! {
        div class="row" {
            div class="col-12" {
                h1 class="mb-4" {
                    i class="bi bi-bar-chart me-3" {}
                    "Dataset Statistics"
                }
                
                div class="row mb-4" {
                    div class="col-md-6" {
                        div class="card" {
                            div class="card-header bg-primary text-white" {
                                "Overview"
                            }
                            div class="card-body" {
                                table class="table" {
                                    tbody {
                                        tr {
                                            td { strong { "Total Papers" } }
                                            td { (graph.papers.len()) }
                                        }
                                        tr {
                                            td { strong { "Total Venues" } }
                                            td { (graph.venues.len()) }
                                        }
                                        tr {
                                            td { strong { "Total Scholars" } }
                                            td { (graph.scholars.len()) }
                                        }
                                        tr {
                                            td { strong { "Total Citations" } }
                                            td { (graph.edges.len()) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    div class="col-md-6" {
                        div class="card" {
                            div class="card-header bg-success text-white" {
                                "Venue Distribution"
                            }
                            div class="card-body" {
                                canvas id="tierChart" {}
                            }
                        }
                    }
                }
                
                div class="row" {
                    div class="col-12" {
                        div class="card" {
                            div class="card-header bg-info text-white" {
                                "Papers by Year"
                            }
                            div class="card-body" {
                                canvas id="yearChart" {}
                            }
                        }
                    }
                }
            }
        }
        
        script {
            (maud::PreEscaped(r#"
            // Tier Chart
            fetch('/api/stats')
                .then(response => response.json())
                .then(data => {
                    const tierCtx = document.getElementById('tierChart').getContext('2d');
                    new Chart(tierCtx, {
                        type: 'pie',
                        data: {
                            labels: Object.keys(data.data.papers_by_tier),
                            datasets: [{
                                data: Object.values(data.data.papers_by_tier),
                                backgroundColor: [
                                    'rgba(255, 99, 132, 0.8)',
                                    'rgba(54, 162, 235, 0.8)',
                                    'rgba(255, 206, 86, 0.8)',
                                    'rgba(75, 192, 192, 0.8)'
                                ]
                            }]
                        }
                    });
                    
                    // Year Chart
                    const yearCtx = document.getElementById('yearChart').getContext('2d');
                    const years = Object.keys(data.data.papers_by_year).sort();
                    new Chart(yearCtx, {
                        type: 'line',
                        data: {
                            labels: years,
                            datasets: [{
                                label: 'Papers Published',
                                data: years.map(y => data.data.papers_by_year[y]),
                                borderColor: 'rgb(75, 192, 192)',
                                backgroundColor: 'rgba(75, 192, 192, 0.2)',
                                tension: 0.1
                            }]
                        },
                        options: {
                            responsive: true,
                            scales: {
                                y: {
                                    beginAtZero: true
                                }
                            }
                        }
                    });
                });
            "#))
        }
    }))
}