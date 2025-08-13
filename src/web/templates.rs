use maud::{html, Markup, DOCTYPE, PreEscaped};
use crate::models::{VenueRanking, ScholarRanking, Paper};

// Helper functions for generating paper links
fn get_google_scholar_url(paper: &Paper) -> String {
    let query = format!("{} {}", 
        paper.title.replace(" ", "+"),
        paper.authors.first().map(|a| a.replace(" ", "+")).unwrap_or_default()
    );
    format!("https://scholar.google.com/scholar?q={}", query)
}

fn get_publisher_url(paper: &Paper) -> Option<String> {
    // If DOI is available, use it
    if let Some(doi) = &paper.doi {
        return Some(format!("https://doi.org/{}", doi));
    }
    
    // If URL is available, use it
    if let Some(url) = &paper.url {
        return Some(url.clone());
    }
    
    // Otherwise, try to generate based on venue
    let venue_upper = paper.venue.to_uppercase();
    let title_slug = paper.title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    
    match venue_upper.as_str() {
        v if v.contains("SOSP") || v.contains("OSDI") || v.contains("NSDI") || v.contains("ATC") || v.contains("FAST") => {
            // USENIX conferences
            paper.year.map(|year| {
                format!("https://www.usenix.org/conference/{}{}/technical-sessions", 
                    venue_upper.to_lowercase(), year)
            })
        },
        v if v.contains("SIGMOD") || v.contains("VLDB") || v.contains("ICDE") || v.contains("PODS") => {
            // ACM database conferences
            Some(format!("https://dl.acm.org/action/doSearch?AllField={}", 
                paper.title.replace(" ", "+")))
        },
        v if v.contains("PLDI") || v.contains("POPL") || v.contains("OOPSLA") || v.contains("ASPLOS") => {
            // ACM PL conferences
            Some(format!("https://dl.acm.org/action/doSearch?AllField={}", 
                paper.title.replace(" ", "+")))
        },
        v if v.contains("ICML") || v.contains("NEURIPS") || v.contains("ICLR") => {
            // ML conferences
            match v {
                _ if v.contains("NEURIPS") => Some("https://papers.nips.cc/".to_string()),
                _ if v.contains("ICML") => Some("https://proceedings.mlr.press/".to_string()),
                _ if v.contains("ICLR") => Some("https://openreview.net/group?id=ICLR.cc".to_string()),
                _ => None
            }
        },
        v if v.contains("CVPR") || v.contains("ICCV") || v.contains("ECCV") => {
            // Computer Vision conferences (IEEE/CVF)
            Some("https://openaccess.thecvf.com/".to_string())
        },
        _ => {
            // Default to ACM DL search
            Some(format!("https://dl.acm.org/action/doSearch?AllField={}", 
                paper.title.replace(" ", "+")))
        }
    }
}

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

pub fn index_page_async() -> Markup {
    base_template(
        "Dashboard - QIndex",
        html! {
            div class="container my-4" {
                // Header
                div class="row mb-4" {
                    div class="col-12" {
                        h1 class="display-4" {
                            i class="bi bi-speedometer2 me-3" {}
                            "QIndex Dashboard"
                        }
                        p class="lead" { "Real-time academic quality metrics powered by PageRank algorithm" }
                    }
                }
                
                // Loading indicator
                div id="loading" class="text-center my-5" {
                    div class="spinner-border text-primary" role="status" {
                        span class="visually-hidden" { "Loading..." }
                    }
                    p class="mt-3" { "Loading dashboard data..." }
                }
                
                // Content containers (initially hidden)
                div id="dashboard-content" style="display: none;" {
                    // Stats cards
                    div class="row mb-4" id="stats-cards" {}
                    
                    // Rankings tables
                    div class="row" {
                        div class="col-lg-6 mb-4" {
                            div class="card" {
                                div class="card-header bg-primary text-white" {
                                    h5 class="mb-0" {
                                        i class="bi bi-trophy me-2" {}
                                        "Top Venues by PageRank"
                                    }
                                }
                                div class="card-body" {
                                    div id="venues-table" {}
                                }
                            }
                        }
                        
                        div class="col-lg-6 mb-4" {
                            div class="card" {
                                div class="card-header bg-success text-white" {
                                    h5 class="mb-0" {
                                        i class="bi bi-person-badge me-2" {}
                                        "Top Scholars by QIndex"
                                    }
                                }
                                div class="card-body" {
                                    div id="scholars-table" {}
                                }
                            }
                        }
                    }
                    
                    // Chart
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
                }
                
                // JavaScript to load data
                script {
                    (PreEscaped(r#"
                    document.addEventListener('DOMContentLoaded', function() {
                        // Fetch homepage data
                        fetch('/api/homepage')
                            .then(response => response.json())
                            .then(result => {
                                if (result.success) {
                                    renderDashboard(result.data);
                                    document.getElementById('loading').style.display = 'none';
                                    document.getElementById('dashboard-content').style.display = 'block';
                                } else {
                                    showError('Failed to load dashboard data');
                                }
                            })
                            .catch(error => {
                                console.error('Error:', error);
                                showError('Failed to load dashboard data');
                            });
                    });
                    
                    function renderDashboard(data) {
                        // Render stats cards
                        const statsHtml = `
                            <div class="col-md-3">
                                <div class="card text-white bg-primary mb-3">
                                    <div class="card-body">
                                        <h5 class="card-title"><i class="bi bi-file-text me-2"></i>Papers</h5>
                                        <p class="card-text display-6">${data.stats.total_papers}</p>
                                    </div>
                                </div>
                            </div>
                            <div class="col-md-3">
                                <div class="card text-white bg-success mb-3">
                                    <div class="card-body">
                                        <h5 class="card-title"><i class="bi bi-building me-2"></i>Venues</h5>
                                        <p class="card-text display-6">${data.stats.total_venues}</p>
                                    </div>
                                </div>
                            </div>
                            <div class="col-md-3">
                                <div class="card text-white bg-info mb-3">
                                    <div class="card-body">
                                        <h5 class="card-title"><i class="bi bi-people me-2"></i>Scholars</h5>
                                        <p class="card-text display-6">${data.stats.total_scholars}</p>
                                    </div>
                                </div>
                            </div>
                            <div class="col-md-3">
                                <div class="card text-white bg-warning mb-3">
                                    <div class="card-body">
                                        <h5 class="card-title"><i class="bi bi-link-45deg me-2"></i>Citations</h5>
                                        <p class="card-text display-6">${data.stats.total_citations}</p>
                                    </div>
                                </div>
                            </div>
                        `;
                        document.getElementById('stats-cards').innerHTML = statsHtml;
                        
                        // Render venues table
                        let venuesHtml = '<div class="table-responsive"><table class="table table-hover">';
                        venuesHtml += '<thead><tr><th>#</th><th>Venue</th><th>Tier</th><th>PageRank</th></tr></thead><tbody>';
                        data.top_venues.forEach((venue, index) => {
                            const tierClass = venue.tier === 'A*' ? 'bg-danger' : 
                                            venue.tier === 'A' ? 'bg-warning' : 'bg-secondary';
                            venuesHtml += `
                                <tr>
                                    <td>${index + 1}</td>
                                    <td><a href="/venue/${venue.id}">${venue.name}</a></td>
                                    <td><span class="badge ${tierClass}">${venue.tier}</span></td>
                                    <td>${venue.pagerank.toFixed(4)}</td>
                                </tr>
                            `;
                        });
                        venuesHtml += '</tbody></table></div>';
                        venuesHtml += '<a href="/venues" class="btn btn-primary btn-sm">View All Venues<i class="bi bi-arrow-right ms-2"></i></a>';
                        document.getElementById('venues-table').innerHTML = venuesHtml;
                        
                        // Render scholars table
                        let scholarsHtml = '<div class="table-responsive"><table class="table table-hover">';
                        scholarsHtml += '<thead><tr><th>#</th><th>Scholar</th><th>QIndex</th><th>H-Index</th></tr></thead><tbody>';
                        data.top_scholars.forEach((scholar, index) => {
                            scholarsHtml += `
                                <tr>
                                    <td>${index + 1}</td>
                                    <td><a href="/scholar/${scholar.id}">${scholar.name}</a></td>
                                    <td><span class="badge bg-primary">${scholar.qindex.toFixed(1)}</span></td>
                                    <td>${scholar.h_index}</td>
                                </tr>
                            `;
                        });
                        scholarsHtml += '</tbody></table></div>';
                        scholarsHtml += '<a href="/scholars" class="btn btn-success btn-sm">View All Scholars<i class="bi bi-arrow-right ms-2"></i></a>';
                        document.getElementById('scholars-table').innerHTML = scholarsHtml;
                        
                        // Load field chart data
                        loadFieldChart();
                    }
                    
                    function loadFieldChart() {
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
                                        maintainAspectRatio: false,
                                        scales: {
                                            y: {
                                                beginAtZero: true
                                            }
                                        }
                                    }
                                });
                            });
                    }
                    
                    function showError(message) {
                        document.getElementById('loading').innerHTML = `
                            <div class="alert alert-danger" role="alert">
                                <i class="bi bi-exclamation-triangle me-2"></i>${message}
                            </div>
                        `;
                    }
                    "#))
                }
            }
        }
    )
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

#[derive(serde::Serialize)]
pub struct Stats {
    pub total_papers: usize,
    pub total_venues: usize,
    pub total_scholars: usize,
    pub total_citations: usize,
}

pub fn venue_detail_page(
    venue: &crate::models::Venue,
    papers: &[&crate::models::Paper],
    total_citations: usize,
    top_authors: &[(String, usize)]
) -> Markup {
    base_template(
        &format!("{} - QIndex", venue.name),
        html! {
            .container.py-5 {
                // Breadcrumb
                nav aria-label="breadcrumb" {
                    ol class="breadcrumb" {
                        li class="breadcrumb-item" {
                            a href="/" { "Home" }
                        }
                        li class="breadcrumb-item" {
                            a href="/venues" { "Venues" }
                        }
                        li class="breadcrumb-item active" {
                            (venue.name)
                        }
                    }
                }
                
                // Venue header
                .card.mb-4.shadow-sm {
                    .card-body {
                        h1.card-title.mb-3 { (venue.name) }
                        
                        .row {
                            .col-md-3 {
                                .stat-box.text-center {
                                    h5 { "Tier" }
                                    .display-6 {
                                        span class=(format!("badge bg-{}", 
                                            if venue.tier == "A*" { "success" } 
                                            else if venue.tier == "A" { "primary" }
                                            else { "secondary" }
                                        )) { (venue.tier) }
                                    }
                                }
                            }
                            .col-md-3 {
                                .stat-box.text-center {
                                    h5 { "Field" }
                                    p.lead { (venue.field) }
                                }
                            }
                            .col-md-3 {
                                .stat-box.text-center {
                                    h5 { "Papers" }
                                    .display-6 { (papers.len()) }
                                }
                            }
                            .col-md-3 {
                                .stat-box.text-center {
                                    h5 { "Total Citations" }
                                    .display-6 { (total_citations) }
                                }
                            }
                        }
                    }
                }
                
                .row {
                    // Top Authors
                    .col-md-4.mb-4 {
                        .card.h-100.shadow-sm {
                            .card-header.bg-primary.text-white {
                                h5.mb-0 { "Top Authors" }
                            }
                            .card-body {
                                @if top_authors.is_empty() {
                                    p.text-muted { "No authors found" }
                                } @else {
                                    ul.list-group.list-group-flush {
                                        @for (author, count) in top_authors {
                                            li.list-group-item.d-flex.justify-content-between {
                                                span { (author) }
                                                span.badge.bg-secondary { (count) " papers" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    // Recent Papers
                    .col-md-8.mb-4 {
                        .card.h-100.shadow-sm {
                            .card-header.bg-primary.text-white {
                                h5.mb-0 { "Papers (" (papers.len()) ")" }
                            }
                            .card-body.overflow-auto style="max-height: 600px;" {
                                @if papers.is_empty() {
                                    p.text-muted { "No papers found" }
                                } @else {
                                    .list-group {
                                        @for (i, paper) in papers.iter().enumerate() {
                                            @if i < 50 {  // Show first 50 papers
                                                .list-group-item {
                                                    h6.mb-2 { (paper.title) }
                                                    p.mb-1.text-muted.small {
                                                        @for (j, author) in paper.authors.iter().enumerate() {
                                                            @if j > 0 { ", " }
                                                            (author)
                                                        }
                                                    }
                                                    .d-flex.justify-content-between.align-items-center.mb-2 {
                                                        small.text-muted {
                                                            @if let Some(year) = paper.year {
                                                                "Year: " (year)
                                                            }
                                                        }
                                                        small.text-muted {
                                                            "Citations: " (paper.cited_by.len())
                                                        }
                                                    }
                                                    // Paper links
                                                    .btn-group.btn-group-sm {
                                                        a.btn.btn-outline-primary href=(get_google_scholar_url(paper)) target="_blank" title="Search on Google Scholar" {
                                                            i.bi.bi-google.me-1 {}
                                                            "Scholar"
                                                        }
                                                        @if let Some(publisher_url) = get_publisher_url(paper) {
                                                            a.btn.btn-outline-secondary href=(publisher_url) target="_blank" title="Publisher Page" {
                                                                i.bi.bi-journal-text.me-1 {}
                                                                @if paper.doi.is_some() {
                                                                    "DOI"
                                                                } @else {
                                                                    "Publisher"
                                                                }
                                                            }
                                                        }
                                                        @if let Some(doi) = &paper.doi {
                                                            button.btn.btn-outline-info type="button" 
                                                                onclick=(format!("navigator.clipboard.writeText('{}')", doi))
                                                                title="Copy DOI to clipboard" {
                                                                i.bi.bi-clipboard {}
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        @if papers.len() > 50 {
                                            .list-group-item.text-center.text-muted {
                                                "... and " (papers.len() - 50) " more papers"
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
    )
}

pub fn scholar_detail_page(
    scholar: &crate::models::Scholar,
    papers: &[&crate::models::Paper],
    papers_by_venue: &std::collections::HashMap<String, Vec<&crate::models::Paper>>,
    total_citations: usize
) -> Markup {
    base_template(
        &format!("{} - QIndex", scholar.name),
        html! {
            .container.py-5 {
                // Breadcrumb
                nav aria-label="breadcrumb" {
                    ol class="breadcrumb" {
                        li class="breadcrumb-item" {
                            a href="/" { "Home" }
                        }
                        li class="breadcrumb-item" {
                            a href="/scholars" { "Scholars" }
                        }
                        li class="breadcrumb-item active" {
                            (scholar.name)
                        }
                    }
                }
                
                // Scholar header
                .card.mb-4.shadow-sm {
                    .card-body {
                        h1.card-title.mb-3 { (scholar.name) }
                        
                        .row {
                            .col-md-3 {
                                .stat-box.text-center {
                                    h5 { "Affiliation" }
                                    p.lead {
                                        @if !scholar.affiliations.is_empty() {
                                            (scholar.affiliations.join(", "))
                                        } @else {
                                            span.text-muted { "Unknown" }
                                        }
                                    }
                                }
                            }
                            .col-md-3 {
                                .stat-box.text-center {
                                    h5 { "Papers" }
                                    .display-6 { (papers.len()) }
                                }
                            }
                            .col-md-3 {
                                .stat-box.text-center {
                                    h5 { "Citations" }
                                    .display-6 { (total_citations) }
                                }
                            }
                            .col-md-3 {
                                .stat-box.text-center {
                                    h5 { "H-Index" }
                                    .display-6 { (scholar.h_index) }
                                }
                            }
                        }
                    }
                }
                
                .row {
                    // Venues Published In
                    .col-md-4.mb-4 {
                        .card.h-100.shadow-sm {
                            .card-header.bg-primary.text-white {
                                h5.mb-0 { "Venues" }
                            }
                            .card-body {
                                @if papers_by_venue.is_empty() {
                                    p.text-muted { "No venues found" }
                                } @else {
                                    ul.list-group.list-group-flush {
                                        @for (venue, venue_papers) in papers_by_venue {
                                            li.list-group-item.d-flex.justify-content-between {
                                                (venue)
                                                span.badge.bg-secondary { (venue_papers.len()) " papers" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    // Publications
                    .col-md-8.mb-4 {
                        .card.h-100.shadow-sm {
                            .card-header.bg-primary.text-white {
                                h5.mb-0 { "Publications" }
                            }
                            .card-body.overflow-auto style="max-height: 600px;" {
                                @if papers.is_empty() {
                                    p.text-muted { "No publications found" }
                                } @else {
                                    .list-group {
                                        @for (i, paper) in papers.iter().enumerate() {
                                            @if i < 50 {  // Show first 50 papers
                                                .list-group-item {
                                                    h6.mb-2 { (paper.title) }
                                                    p.mb-1.text-muted.small {
                                                        "Venue: " (paper.venue)
                                                        @if let Some(year) = paper.year {
                                                            " (" (year) ")"
                                                        }
                                                    }
                                                    .d-flex.justify-content-between.align-items-center.mb-2 {
                                                        small.text-muted {
                                                            "Citations: " (paper.cited_by.len())
                                                        }
                                                    }
                                                    // Paper links
                                                    .btn-group.btn-group-sm {
                                                        a.btn.btn-outline-primary href=(get_google_scholar_url(paper)) target="_blank" title="Search on Google Scholar" {
                                                            i.bi.bi-google.me-1 {}
                                                            "Scholar"
                                                        }
                                                        @if let Some(publisher_url) = get_publisher_url(paper) {
                                                            a.btn.btn-outline-secondary href=(publisher_url) target="_blank" title="Publisher Page" {
                                                                i.bi.bi-journal-text.me-1 {}
                                                                @if paper.doi.is_some() {
                                                                    "DOI"
                                                                } @else {
                                                                    "Publisher"
                                                                }
                                                            }
                                                        }
                                                        @if let Some(doi) = &paper.doi {
                                                            button.btn.btn-outline-info type="button" 
                                                                onclick=(format!("navigator.clipboard.writeText('{}')", doi))
                                                                title="Copy DOI to clipboard" {
                                                                i.bi.bi-clipboard {}
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        @if papers.len() > 50 {
                                            .list-group-item.text-center.text-muted {
                                                "... and " (papers.len() - 50) " more publications"
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
    )
}