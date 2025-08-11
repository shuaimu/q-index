use maud::{html, Markup, DOCTYPE, PreEscaped};
use crate::models::{VenueRanking, ScholarRanking};

pub fn base_template(title: &str, content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " - QIndex" }
                link rel="stylesheet" href="/static/style.css";
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/css/bootstrap.min.css";
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bootstrap-icons@1.11.0/font/bootstrap-icons.css";
                script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.0/dist/chart.umd.min.js" {}
                script src="https://code.jquery.com/jquery-3.7.1.min.js" {}
            }
            body {
                nav class="navbar navbar-expand-lg navbar-dark bg-primary" {
                    div class="container-fluid" {
                        a class="navbar-brand" href="/" {
                            i class="bi bi-graph-up me-2" {}
                            "QIndex"
                        }
                        button class="navbar-toggler" type="button" data-bs-toggle="collapse" data-bs-target="#navbarNav" {
                            span class="navbar-toggler-icon" {}
                        }
                        div class="collapse navbar-collapse" id="navbarNav" {
                            ul class="navbar-nav me-auto" {
                                li class="nav-item" {
                                    a class="nav-link" href="/" { "Dashboard" }
                                }
                                li class="nav-item" {
                                    a class="nav-link" href="/venues" { "Venues" }
                                }
                                li class="nav-item" {
                                    a class="nav-link" href="/scholars" { "Scholars" }
                                }
                                li class="nav-item" {
                                    a class="nav-link" href="/statistics" { "Statistics" }
                                }
                                li class="nav-item" {
                                    a class="nav-link" href="/about" { "About" }
                                }
                            }
                            form class="d-flex" action="/search" method="get" {
                                input class="form-control me-2" type="search" name="q" placeholder="Search..." aria-label="Search";
                                button class="btn btn-outline-light" type="submit" {
                                    i class="bi bi-search" {}
                                }
                            }
                        }
                    }
                }
                
                main class="container my-4" {
                    (content)
                }
                
                footer class="bg-light text-center text-lg-start mt-5" {
                    div class="container p-4" {
                        div class="row" {
                            div class="col-lg-6 col-md-12 mb-4 mb-md-0" {
                                h5 class="text-uppercase" { "QIndex" }
                                p {
                                    "Academic quality index calculator using PageRank algorithm. "
                                    "Evaluating conferences, journals, and scholars based on citation networks."
                                }
                            }
                            div class="col-lg-3 col-md-6 mb-4 mb-md-0" {
                                h5 class="text-uppercase" { "Links" }
                                ul class="list-unstyled mb-0" {
                                    li { a href="/api/docs" class="text-dark" { "API Documentation" } }
                                    li { a href="https://github.com/shuai/qindex" class="text-dark" { "GitHub" } }
                                }
                            }
                            div class="col-lg-3 col-md-6 mb-4 mb-md-0" {
                                h5 class="text-uppercase" { "Info" }
                                p { "Built with Rust, Actix-Web, and Maud" }
                            }
                        }
                    }
                    div class="text-center p-3 bg-dark text-white" {
                        "© 2024 QIndex. MIT License."
                    }
                }
                
                script src="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/js/bootstrap.bundle.min.js" {}
                script src="/static/app.js" {}
                // Include live-reload in development
                @if std::env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()) == "development" {
                    script src="/static/live-reload.js" {}
                }
            }
        }
    }
}

pub fn index_page(top_venues: &[VenueRanking], top_scholars: &[ScholarRanking], stats: &Stats) -> Markup {
    base_template("Dashboard", html! {
        div class="row mb-4" {
            div class="col-12" {
                h1 class="display-4" {
                    i class="bi bi-speedometer2 me-3" {}
                    "QIndex Dashboard"
                }
                p class="lead" {
                    "Real-time academic quality metrics powered by PageRank algorithm"
                }
            }
        }
        
        // Statistics Cards
        div class="row mb-4" {
            div class="col-md-3" {
                div class="card text-white bg-primary mb-3" {
                    div class="card-body" {
                        h5 class="card-title" {
                            i class="bi bi-file-text me-2" {}
                            "Papers"
                        }
                        p class="card-text display-6" { (stats.total_papers) }
                    }
                }
            }
            div class="col-md-3" {
                div class="card text-white bg-success mb-3" {
                    div class="card-body" {
                        h5 class="card-title" {
                            i class="bi bi-building me-2" {}
                            "Venues"
                        }
                        p class="card-text display-6" { (stats.total_venues) }
                    }
                }
            }
            div class="col-md-3" {
                div class="card text-white bg-info mb-3" {
                    div class="card-body" {
                        h5 class="card-title" {
                            i class="bi bi-people me-2" {}
                            "Scholars"
                        }
                        p class="card-text display-6" { (stats.total_scholars) }
                    }
                }
            }
            div class="col-md-3" {
                div class="card text-white bg-warning mb-3" {
                    div class="card-body" {
                        h5 class="card-title" {
                            i class="bi bi-link-45deg me-2" {}
                            "Citations"
                        }
                        p class="card-text display-6" { (stats.total_citations) }
                    }
                }
            }
        }
        
        div class="row" {
            // Top Venues
            div class="col-lg-6 mb-4" {
                div class="card" {
                    div class="card-header bg-primary text-white" {
                        h5 class="mb-0" {
                            i class="bi bi-trophy me-2" {}
                            "Top Venues by PageRank"
                        }
                    }
                    div class="card-body" {
                        div class="table-responsive" {
                            table class="table table-hover" {
                                thead {
                                    tr {
                                        th { "#" }
                                        th { "Venue" }
                                        th { "Tier" }
                                        th { "PageRank" }
                                    }
                                }
                                tbody {
                                    @for (i, venue) in top_venues.iter().enumerate().take(10) {
                                        tr {
                                            td { (i + 1) }
                                            td {
                                                a href=(format!("/venue/{}", venue.id)) {
                                                    (venue.name)
                                                }
                                            }
                                            td {
                                                @if venue.tier == "A*" {
                                                    span class="badge bg-danger" { (venue.tier) }
                                                } @else if venue.tier == "A" {
                                                    span class="badge bg-warning" { (venue.tier) }
                                                } @else {
                                                    span class="badge bg-secondary" { (venue.tier) }
                                                }
                                            }
                                            td { (format!("{:.4}", venue.pagerank)) }
                                        }
                                    }
                                }
                            }
                        }
                        a href="/venues" class="btn btn-primary btn-sm" {
                            "View All Venues"
                            i class="bi bi-arrow-right ms-2" {}
                        }
                    }
                }
            }
            
            // Top Scholars
            div class="col-lg-6 mb-4" {
                div class="card" {
                    div class="card-header bg-success text-white" {
                        h5 class="mb-0" {
                            i class="bi bi-person-badge me-2" {}
                            "Top Scholars by QIndex"
                        }
                    }
                    div class="card-body" {
                        div class="table-responsive" {
                            table class="table table-hover" {
                                thead {
                                    tr {
                                        th { "#" }
                                        th { "Scholar" }
                                        th { "QIndex" }
                                        th { "H-Index" }
                                    }
                                }
                                tbody {
                                    @for (i, scholar) in top_scholars.iter().enumerate().take(10) {
                                        tr {
                                            td { (i + 1) }
                                            td {
                                                a href=(format!("/scholar/{}", scholar.id)) {
                                                    (scholar.name)
                                                }
                                            }
                                            td {
                                                span class="badge bg-primary" {
                                                    (format!("{:.1}", scholar.qindex))
                                                }
                                            }
                                            td { (scholar.h_index) }
                                        }
                                    }
                                }
                            }
                        }
                        a href="/scholars" class="btn btn-success btn-sm" {
                            "View All Scholars"
                            i class="bi bi-arrow-right ms-2" {}
                        }
                    }
                }
            }
        }
        
        // Chart Section
        div class="row mt-4" {
            div class="col-12" {
                div class="card" {
                    div class="card-header bg-info text-white" {
                        h5 class="mb-0" {
                            i class="bi bi-graph-up me-2" {}
                            "Venue Distribution by Field"
                        }
                    }
                    div class="card-body" {
                        canvas id="fieldChart" width="400" height="100" {}
                    }
                }
            }
        }
        
        script {
            (PreEscaped(r#"
            const ctx = document.getElementById('fieldChart').getContext('2d');
            fetch('/api/stats/fields')
                .then(response => response.json())
                .then(data => {
                    new Chart(ctx, {
                        type: 'bar',
                        data: {
                            labels: data.labels,
                            datasets: [{
                                label: 'Number of Papers',
                                data: data.values,
                                backgroundColor: [
                                    'rgba(255, 99, 132, 0.6)',
                                    'rgba(54, 162, 235, 0.6)',
                                    'rgba(255, 206, 86, 0.6)',
                                    'rgba(75, 192, 192, 0.6)',
                                    'rgba(153, 102, 255, 0.6)',
                                    'rgba(255, 159, 64, 0.6)'
                                ],
                                borderColor: [
                                    'rgba(255, 99, 132, 1)',
                                    'rgba(54, 162, 235, 1)',
                                    'rgba(255, 206, 86, 1)',
                                    'rgba(75, 192, 192, 1)',
                                    'rgba(153, 102, 255, 1)',
                                    'rgba(255, 159, 64, 1)'
                                ],
                                borderWidth: 1
                            }]
                        },
                        options: {
                            responsive: true,
                            maintainAspectRatio: true,
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
    })
}

pub fn venues_page(venues: &[VenueRanking], field_filter: Option<&str>, tier_filter: Option<&str>) -> Markup {
    base_template("Venues", html! {
        div class="row mb-4" {
            div class="col-12" {
                h1 {
                    i class="bi bi-building me-3" {}
                    "Academic Venues"
                }
                p class="lead" { "Conferences and journals ranked by PageRank algorithm" }
            }
        }
        
        // Filters
        div class="row mb-4" {
            div class="col-12" {
                div class="card" {
                    div class="card-body" {
                        form method="get" action="/venues" class="row g-3" {
                            div class="col-md-4" {
                                label for="field" class="form-label" { "Field" }
                                select class="form-select" name="field" id="field" {
                                    option value="" { "All Fields" }
                                    // Systems Area
                                    option value="Operating Systems" selected[field_filter == Some("Operating Systems")] { "Operating Systems" }
                                    option value="Computer Networks" selected[field_filter == Some("Computer Networks")] { "Computer Networks" }
                                    option value="Computer Security" selected[field_filter == Some("Computer Security")] { "Computer Security" }
                                    option value="Databases" selected[field_filter == Some("Databases")] { "Databases" }
                                    option value="Computer Architecture" selected[field_filter == Some("Computer Architecture")] { "Computer Architecture" }
                                    option value="Measurement & Perf. Analysis" selected[field_filter == Some("Measurement & Perf. Analysis")] { "Measurement & Perf. Analysis" }
                                    option value="High-Performance Computing" selected[field_filter == Some("High-Performance Computing")] { "High-Performance Computing" }
                                    option value="Mobile Computing" selected[field_filter == Some("Mobile Computing")] { "Mobile Computing" }
                                    option value="Embedded & Real-Time Systems" selected[field_filter == Some("Embedded & Real-Time Systems")] { "Embedded & Real-Time Systems" }
                                    // AI Area
                                    option value="Artificial Intelligence" selected[field_filter == Some("Artificial Intelligence")] { "Artificial Intelligence" }
                                    option value="Computer Vision" selected[field_filter == Some("Computer Vision")] { "Computer Vision" }
                                    option value="Machine Learning & Data Mining" selected[field_filter == Some("Machine Learning & Data Mining")] { "Machine Learning & Data Mining" }
                                    option value="Natural Language Processing" selected[field_filter == Some("Natural Language Processing")] { "Natural Language Processing" }
                                    option value="The Web & Information Retrieval" selected[field_filter == Some("The Web & Information Retrieval")] { "The Web & Information Retrieval" }
                                    // Theory Area
                                    option value="Algorithms & Complexity" selected[field_filter == Some("Algorithms & Complexity")] { "Algorithms & Complexity" }
                                    option value="Cryptography" selected[field_filter == Some("Cryptography")] { "Cryptography" }
                                    option value="Logic & Verification" selected[field_filter == Some("Logic & Verification")] { "Logic & Verification" }
                                    option value="Parallel & Distributed Computing" selected[field_filter == Some("Parallel & Distributed Computing")] { "Parallel & Distributed Computing" }
                                    // Software/Languages
                                    option value="Programming Languages" selected[field_filter == Some("Programming Languages")] { "Programming Languages" }
                                    option value="Software Engineering" selected[field_filter == Some("Software Engineering")] { "Software Engineering" }
                                    // Interdisciplinary
                                    option value="Human-Computer Interaction" selected[field_filter == Some("Human-Computer Interaction")] { "Human-Computer Interaction" }
                                    option value="Computer Graphics" selected[field_filter == Some("Computer Graphics")] { "Computer Graphics" }
                                    option value="Robotics" selected[field_filter == Some("Robotics")] { "Robotics" }
                                    option value="Visualization" selected[field_filter == Some("Visualization")] { "Visualization" }
                                    option value="Computational Biology" selected[field_filter == Some("Computational Biology")] { "Computational Biology" }
                                    // Other
                                    option value="Cloud Computing" selected[field_filter == Some("Cloud Computing")] { "Cloud Computing" }
                                    option value="Distributed Systems" selected[field_filter == Some("Distributed Systems")] { "Distributed Systems" }
                                    option value="General" selected[field_filter == Some("General")] { "General" }
                                }
                            }
                            div class="col-md-4" {
                                label for="tier" class="form-label" { "Tier" }
                                select class="form-select" name="tier" id="tier" {
                                    option value="" { "All Tiers" }
                                    option value="A*" selected[tier_filter == Some("A*")] { "A* (Top Tier)" }
                                    option value="A" selected[tier_filter == Some("A")] { "A (Second Tier)" }
                                    option value="B" selected[tier_filter == Some("B")] { "B" }
                                    option value="C" selected[tier_filter == Some("C")] { "C" }
                                }
                            }
                            div class="col-md-4" {
                                label class="form-label" { "&nbsp;" }
                                div {
                                    button type="submit" class="btn btn-primary me-2" {
                                        i class="bi bi-funnel me-2" {}
                                        "Apply Filters"
                                    }
                                    a href="/venues" class="btn btn-secondary" { "Clear" }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Results Table
        div class="row" {
            div class="col-12" {
                div class="card" {
                    div class="card-body" {
                        div class="table-responsive" {
                            table class="table table-hover" {
                                thead class="table-light" {
                                    tr {
                                        th { "Rank" }
                                        th { "Venue" }
                                        th { "Tier" }
                                        th { "Field" }
                                        th { "PageRank" }
                                        th { "Impact Factor" }
                                        th { "Papers" }
                                    }
                                }
                                tbody {
                                    @for (i, venue) in venues.iter().enumerate() {
                                        tr {
                                            td { (i + 1) }
                                            td {
                                                a href=(format!("/venue/{}", venue.id)) class="text-decoration-none" {
                                                    strong { (venue.name) }
                                                }
                                            }
                                            td {
                                                @if venue.tier == "A*" {
                                                    span class="badge bg-danger" { (venue.tier) }
                                                } @else if venue.tier == "A" {
                                                    span class="badge bg-warning text-dark" { (venue.tier) }
                                                } @else if venue.tier == "B" {
                                                    span class="badge bg-info text-dark" { (venue.tier) }
                                                } @else {
                                                    span class="badge bg-secondary" { (venue.tier) }
                                                }
                                            }
                                            td {
                                                span class="badge bg-light text-dark" { (venue.field) }
                                            }
                                            td { (format!("{:.6}", venue.pagerank)) }
                                            td { (format!("{:.4}", venue.impact_factor)) }
                                            td { (venue.paper_count) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}

pub fn scholars_page(scholars: &[ScholarRanking]) -> Markup {
    base_template("Scholars", html! {
        div class="row mb-4" {
            div class="col-12" {
                h1 {
                    i class="bi bi-people me-3" {}
                    "Academic Scholars"
                }
                p class="lead" { "Researchers ranked by QIndex (quality-weighted publication score)" }
            }
        }
        
        div class="row" {
            div class="col-12" {
                div class="card" {
                    div class="card-body" {
                        div class="table-responsive" {
                            table class="table table-hover" {
                                thead class="table-light" {
                                    tr {
                                        th { "Rank" }
                                        th { "Scholar" }
                                        th { "QIndex" }
                                        th { "H-Index" }
                                        th { "Papers" }
                                        th { "Citations" }
                                        th { "Top Venues" }
                                    }
                                }
                                tbody {
                                    @for (i, scholar) in scholars.iter().enumerate() {
                                        tr {
                                            td { (i + 1) }
                                            td {
                                                a href=(format!("/scholar/{}", scholar.id)) class="text-decoration-none" {
                                                    strong { (scholar.name) }
                                                }
                                            }
                                            td {
                                                span class="badge bg-primary" {
                                                    (format!("{:.2}", scholar.qindex))
                                                }
                                            }
                                            td {
                                                span class="badge bg-success" {
                                                    (scholar.h_index)
                                                }
                                            }
                                            td { (scholar.paper_count) }
                                            td { (scholar.citation_count) }
                                            td {
                                                @for venue in &scholar.top_venues {
                                                    span class="badge bg-light text-dark me-1" { (venue) }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}

pub fn search_results_page(query: &str, venues: &[VenueRanking], scholars: &[ScholarRanking]) -> Markup {
    base_template("Search Results", html! {
        div class="row mb-4" {
            div class="col-12" {
                h1 {
                    i class="bi bi-search me-3" {}
                    "Search Results"
                }
                p class="lead" {
                    "Results for: "
                    strong { (query) }
                }
            }
        }
        
        @if !venues.is_empty() {
            div class="row mb-4" {
                div class="col-12" {
                    h3 {
                        i class="bi bi-building me-2" {}
                        "Venues"
                    }
                    div class="card" {
                        div class="card-body" {
                            div class="table-responsive" {
                                table class="table table-hover" {
                                    thead {
                                        tr {
                                            th { "Venue" }
                                            th { "Tier" }
                                            th { "Field" }
                                            th { "PageRank" }
                                        }
                                    }
                                    tbody {
                                        @for venue in venues {
                                            tr {
                                                td {
                                                    a href=(format!("/venue/{}", venue.id)) {
                                                        (venue.name)
                                                    }
                                                }
                                                td {
                                                    span class="badge bg-secondary" { (venue.tier) }
                                                }
                                                td { (venue.field) }
                                                td { (format!("{:.4}", venue.pagerank)) }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        @if !scholars.is_empty() {
            div class="row mb-4" {
                div class="col-12" {
                    h3 {
                        i class="bi bi-people me-2" {}
                        "Scholars"
                    }
                    div class="card" {
                        div class="card-body" {
                            div class="table-responsive" {
                                table class="table table-hover" {
                                    thead {
                                        tr {
                                            th { "Scholar" }
                                            th { "QIndex" }
                                            th { "H-Index" }
                                            th { "Papers" }
                                        }
                                    }
                                    tbody {
                                        @for scholar in scholars {
                                            tr {
                                                td {
                                                    a href=(format!("/scholar/{}", scholar.id)) {
                                                        (scholar.name)
                                                    }
                                                }
                                                td {
                                                    span class="badge bg-primary" {
                                                        (format!("{:.1}", scholar.qindex))
                                                    }
                                                }
                                                td { (scholar.h_index) }
                                                td { (scholar.paper_count) }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        @if venues.is_empty() && scholars.is_empty() {
            div class="alert alert-info" {
                i class="bi bi-info-circle me-2" {}
                "No results found for your search query."
            }
        }
    })
}

pub struct Stats {
    pub total_papers: usize,
    pub total_venues: usize,
    pub total_scholars: usize,
    pub total_citations: usize,
}