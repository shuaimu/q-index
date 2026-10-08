use super::{Citations, Ctx, FieldCounts, StatisticsData, Stats, VenuePage};
use crate::models::{Paper, ScholarRanking, VenueRanking};
use maud::{html, Markup, PreEscaped, DOCTYPE};

// Helper functions for generating paper links
fn get_google_scholar_url(paper: &Paper) -> String {
    let query = format!(
        "{} {}",
        paper.title.replace(" ", "+"),
        paper
            .authors
            .first()
            .map(|a| a.replace(" ", "+"))
            .unwrap_or_default()
    );
    format!("https://scholar.google.com/scholar?q={}", query)
}

pub(super) fn publisher_url(paper: &Paper) -> Option<String> {
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

    match venue_upper.as_str() {
        v if v.contains("SOSP")
            || v.contains("OSDI")
            || v.contains("NSDI")
            || v.contains("ATC")
            || v.contains("FAST") =>
        {
            // USENIX conferences
            paper.year.map(|year| {
                format!(
                    "https://www.usenix.org/conference/{}{}/technical-sessions",
                    venue_upper.to_lowercase(),
                    year
                )
            })
        }
        v if v.contains("SIGMOD")
            || v.contains("VLDB")
            || v.contains("ICDE")
            || v.contains("PODS") =>
        {
            // ACM database conferences
            Some(format!(
                "https://dl.acm.org/action/doSearch?AllField={}",
                paper.title.replace(" ", "+")
            ))
        }
        v if v.contains("PLDI")
            || v.contains("POPL")
            || v.contains("OOPSLA")
            || v.contains("ASPLOS") =>
        {
            // ACM PL conferences
            Some(format!(
                "https://dl.acm.org/action/doSearch?AllField={}",
                paper.title.replace(" ", "+")
            ))
        }
        v if v.contains("ICML") || v.contains("NEURIPS") || v.contains("ICLR") => {
            // ML conferences
            match v {
                _ if v.contains("NEURIPS") => Some("https://papers.nips.cc/".to_string()),
                _ if v.contains("ICML") => Some("https://proceedings.mlr.press/".to_string()),
                _ if v.contains("ICLR") => {
                    Some("https://openreview.net/group?id=ICLR.cc".to_string())
                }
                _ => None,
            }
        }
        v if v.contains("CVPR") || v.contains("ICCV") || v.contains("ECCV") => {
            // Computer Vision conferences (IEEE/CVF)
            Some("https://openaccess.thecvf.com/".to_string())
        }
        _ => {
            // Default to ACM DL search
            Some(format!(
                "https://dl.acm.org/action/doSearch?AllField={}",
                paper.title.replace(" ", "+")
            ))
        }
    }
}

/// Embeds a value as JSON inside a <script> block.
fn json_script<T: serde::Serialize>(value: &T) -> PreEscaped<String> {
    let json = serde_json::to_string(value).expect("serializable");
    PreEscaped(json.replace("</", "<\\/"))
}

fn tier_badge(tier: &str) -> Markup {
    html! {
        @if tier == "A*" {
            span class="badge bg-danger" { (tier) }
        } @else if tier == "A" {
            span class="badge bg-warning text-dark" { (tier) }
        } @else if tier == "B" {
            span class="badge bg-info text-dark" { (tier) }
        } @else {
            span class="badge bg-secondary" { (tier) }
        }
    }
}

/// Author name, linked to the scholar page when we know the scholar.
fn author_link(ctx: &Ctx, author: &str) -> Markup {
    html! {
        @if let Some(url) = ctx.author_url(author) {
            a href=(url) class="text-reset" { (author) }
        } @else {
            (author)
        }
    }
}

/// `page` identifies the page type; `data-page` on <body> tells app.js which
/// browser-side behaviour (filters, search, scholar profile) to run.
pub fn base_template(ctx: &Ctx, title: &str, page: &str, content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                meta name="qindex-base" content=(ctx.base());
                title { (title) " - QIndex" }
                link rel="stylesheet" href=(ctx.url("static/style.css"));
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/css/bootstrap.min.css";
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bootstrap-icons@1.11.0/font/bootstrap-icons.css";
                script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.0/dist/chart.umd.min.js" {}
            }
            body data-page=(page) {
                nav class="navbar navbar-expand-lg navbar-dark bg-primary" {
                    div class="container-fluid" {
                        a class="navbar-brand" href=(ctx.url("")) {
                            i class="bi bi-graph-up me-2" {}
                            "QIndex"
                        }
                        button class="navbar-toggler" type="button" data-bs-toggle="collapse" data-bs-target="#navbarNav" {
                            span class="navbar-toggler-icon" {}
                        }
                        div class="collapse navbar-collapse" id="navbarNav" {
                            ul class="navbar-nav me-auto" {
                                li class="nav-item" {
                                    a class="nav-link" href=(ctx.url("")) { "Dashboard" }
                                }
                                li class="nav-item" {
                                    a class="nav-link" href=(ctx.url("venues/")) { "Venues" }
                                }
                                li class="nav-item" {
                                    a class="nav-link" href=(ctx.url("scholars/")) { "Scholars" }
                                }
                                li class="nav-item" {
                                    a class="nav-link" href=(ctx.url("statistics/")) { "Statistics" }
                                }
                                li class="nav-item" {
                                    a class="nav-link" href=(ctx.url("about/")) { "About" }
                                }
                                li class="nav-item" {
                                    a class="nav-link" href=(ctx.url("book/")) target="_blank" { "Book" }
                                }
                            }
                            form class="d-flex" action=(ctx.url("search/")) method="get" {
                                input class="form-control me-2" type="search" name="q" placeholder="Search..." aria-label="Search" autocomplete="off";
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
                                    li { a href=(ctx.url("about/#open-data")) class="text-dark" { "Open Data (JSON)" } }
                                    li { a href="https://github.com/shuaimu/q-index" class="text-dark" { "GitHub" } }
                                }
                            }
                            div class="col-lg-3 col-md-6 mb-4 mb-md-0" {
                                h5 class="text-uppercase" { "Info" }
                                p {
                                    "Static site generated with Rust and Maud on " (ctx.generated) "."
                                }
                            }
                        }
                    }
                    div class="text-center p-3 bg-dark text-white" {
                        "© 2024 QIndex. MIT License."
                    }
                }

                script src="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/js/bootstrap.bundle.min.js" {}
                script src=(ctx.url("static/app.js")) {}
            }
        }
    }
}

fn loading_spinner(message: &str) -> Markup {
    html! {
        div class="text-center my-5" {
            div class="spinner-border text-primary" role="status" {
                span class="visually-hidden" { "Loading..." }
            }
            p class="mt-3" { (message) }
        }
        noscript {
            div class="alert alert-warning" { "This page needs JavaScript enabled." }
        }
    }
}

pub fn index_page(
    ctx: &Ctx,
    top_venues: &[VenueRanking],
    top_scholars: &[ScholarRanking],
    stats: &Stats,
    fields: &FieldCounts,
) -> Markup {
    base_template(
        ctx,
        "Dashboard",
        "index",
        html! {
            div class="row mb-4" {
                div class="col-12" {
                    h1 class="display-4" {
                        i class="bi bi-speedometer2 me-3" {}
                        "QIndex Dashboard"
                    }
                    p class="lead" {
                        "Academic quality metrics powered by PageRank algorithm"
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
                                                    a href=(ctx.venue_url(&venue.id)) {
                                                        (venue.name)
                                                    }
                                                }
                                                td { (tier_badge(&venue.tier)) }
                                                td { (format!("{:.4}", venue.pagerank)) }
                                            }
                                        }
                                    }
                                }
                            }
                            a href=(ctx.url("venues/")) class="btn btn-primary btn-sm" {
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
                                                    a href=(ctx.scholar_url(&scholar.id)) {
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
                            a href=(ctx.url("scholars/")) class="btn btn-success btn-sm" {
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
                "const FIELD_DATA = " (json_script(&serde_json::json!({
                    "labels": fields.labels,
                    "values": fields.values,
                }))) ";"
                (PreEscaped(r#"
            new Chart(document.getElementById('fieldChart').getContext('2d'), {
                type: 'bar',
                data: {
                    labels: FIELD_DATA.labels,
                    datasets: [{
                        label: 'Number of Papers',
                        data: FIELD_DATA.values,
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
            "#))
            }
        },
    )
}

const FIELD_OPTIONS: &[&str] = &[
    // Systems Area
    "Operating Systems",
    "Computer Networks",
    "Computer Security",
    "Databases",
    "Computer Architecture",
    "Measurement & Perf. Analysis",
    "High-Performance Computing",
    "Mobile Computing",
    "Embedded & Real-Time Systems",
    // AI Area
    "Artificial Intelligence",
    "Computer Vision",
    "Machine Learning & Data Mining",
    "Natural Language Processing",
    "The Web & Information Retrieval",
    // Theory Area
    "Algorithms & Complexity",
    "Cryptography",
    "Logic & Verification",
    "Parallel & Distributed Computing",
    // Software/Languages
    "Programming Languages",
    "Software Engineering",
    // Interdisciplinary
    "Human-Computer Interaction",
    "Computer Graphics",
    "Robotics",
    "Visualization",
    "Computational Biology",
    // Other
    "Cloud Computing",
    "Distributed Systems",
    "General",
];

/// All ranked venues; the field/tier filters run in the browser (app.js).
pub fn venues_page(ctx: &Ctx, venues: &[VenueRanking]) -> Markup {
    base_template(
        ctx,
        "Venues",
        "venues",
        html! {
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
                            form method="get" action=(ctx.url("venues/")) class="row g-3" id="venue-filters" {
                                div class="col-md-4" {
                                    label for="field" class="form-label" { "Field" }
                                    select class="form-select" name="field" id="field" {
                                        option value="" { "All Fields" }
                                        @for field in FIELD_OPTIONS {
                                            option value=(field) { (field) }
                                        }
                                    }
                                }
                                div class="col-md-4" {
                                    label for="tier" class="form-label" { "Tier" }
                                    select class="form-select" name="tier" id="tier" {
                                        option value="" { "All Tiers" }
                                        option value="A*" { "A* (Top Tier)" }
                                        option value="A" { "A (Second Tier)" }
                                        option value="B" { "B" }
                                        option value="C" { "C" }
                                    }
                                }
                                div class="col-md-4" {
                                    label class="form-label" { (PreEscaped("&nbsp;")) }
                                    div {
                                        button type="submit" class="btn btn-primary me-2" {
                                            i class="bi bi-funnel me-2" {}
                                            "Apply Filters"
                                        }
                                        a href=(ctx.url("venues/")) class="btn btn-secondary" { "Clear" }
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
                                table class="table table-hover" id="venues-table" {
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
                                            tr data-field=(venue.field) data-tier=(venue.tier) {
                                                td class="rank" { (i + 1) }
                                                td {
                                                    a href=(ctx.venue_url(&venue.id)) class="text-decoration-none" {
                                                        strong { (venue.name) }
                                                    }
                                                }
                                                td { (tier_badge(&venue.tier)) }
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
                            p class="text-muted d-none" id="venues-empty" { "No venues match these filters." }
                        }
                    }
                }
            }
        },
    )
}

/// Top scholars; the `?min_papers=` filter runs in the browser (app.js).
pub fn scholars_page(ctx: &Ctx, scholars: &[ScholarRanking]) -> Markup {
    base_template(
        ctx,
        "Scholars",
        "scholars",
        html! {
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
                                table class="table table-hover" id="scholars-table" {
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
                                            tr data-papers=(scholar.paper_count) {
                                                td class="rank" { (i + 1) }
                                                td {
                                                    a href=(ctx.scholar_url(&scholar.id)) class="text-decoration-none" {
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
        },
    )
}

/// Search results are computed in the browser from data/search-index.json.
pub fn search_shell_page(ctx: &Ctx) -> Markup {
    base_template(
        ctx,
        "Search Results",
        "search",
        html! {
            div class="row mb-4" {
                div class="col-12" {
                    h1 {
                        i class="bi bi-search me-3" {}
                        "Search Results"
                    }
                    p class="lead" id="search-query" {}
                }
            }
            div id="search-results" {
                (loading_spinner("Loading search index..."))
            }
        },
    )
}

/// Scholar profiles are rendered in the browser from data/scholars/*.json,
/// selected by the `?id=` query parameter.
pub fn scholar_shell_page(ctx: &Ctx) -> Markup {
    base_template(
        ctx,
        "Scholar",
        "scholar",
        html! {
            .container.py-5 {
                nav aria-label="breadcrumb" {
                    ol class="breadcrumb" {
                        li class="breadcrumb-item" {
                            a href=(ctx.url("")) { "Home" }
                        }
                        li class="breadcrumb-item" {
                            a href=(ctx.url("scholars/")) { "Scholars" }
                        }
                        li class="breadcrumb-item active" id="scholar-crumb" {}
                    }
                }
                div id="scholar-root" {
                    (loading_spinner("Loading scholar profile..."))
                }
            }
        },
    )
}

fn pagination(current_page: usize, total_pages: usize, url: impl Fn(usize) -> String) -> Markup {
    html! {
        @if total_pages > 1 {
            nav.mt-3 {
                ul.pagination.justify-content-center {
                    // Previous button
                    li class=(if current_page <= 1 { "page-item disabled" } else { "page-item" }) {
                        a.page-link href=(if current_page > 1 { url(current_page - 1) } else { "#".to_string() }) {
                            "Previous"
                        }
                    }

                    // Page numbers
                    @for page in 1..=total_pages {
                        @if (page == 1) || (page == total_pages) || ((page >= current_page.saturating_sub(2)) && (page <= current_page + 2)) {
                            li class=(if page == current_page { "page-item active" } else { "page-item" }) {
                                a.page-link href=(url(page)) { (page) }
                            }
                        } @else if (page == 2 && current_page > 4) || (page == total_pages.saturating_sub(1) && current_page < total_pages.saturating_sub(3)) {
                            li.page-item.disabled {
                                span.page-link { "..." }
                            }
                        }
                    }

                    // Next button
                    li class=(if current_page >= total_pages { "page-item disabled" } else { "page-item" }) {
                        a.page-link href=(if current_page < total_pages { url(current_page + 1) } else { "#".to_string() }) {
                            "Next"
                        }
                    }
                }
            }
        }
    }
}

pub fn venue_detail_page(ctx: &Ctx, data: &VenuePage, citations: &Citations) -> Markup {
    let venue = data.venue;
    base_template(
        ctx,
        &venue.name,
        "venue",
        html! {
            .container.py-5 {
                // Breadcrumb
                nav aria-label="breadcrumb" {
                    ol class="breadcrumb" {
                        li class="breadcrumb-item" {
                            a href=(ctx.url("")) { "Home" }
                        }
                        li class="breadcrumb-item" {
                            a href=(ctx.url("venues/")) { "Venues" }
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
                                    .display-6 { (data.total_papers) }
                                }
                            }
                            .col-md-3 {
                                .stat-box.text-center {
                                    h5 { "Total Citations" }
                                    .display-6 { (data.total_citations) }
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
                                @if data.top_authors.is_empty() {
                                    p.text-muted { "No authors found" }
                                } @else {
                                    ul.list-group.list-group-flush {
                                        @for (author, count) in data.top_authors {
                                            li.list-group-item.d-flex.justify-content-between {
                                                span { (author_link(ctx, author)) }
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
                                .d-flex.justify-content-between.align-items-center {
                                    h5.mb-0 {
                                        "Papers (Page " (data.current_page) " of " (data.total_pages) ")"
                                    }
                                    small {
                                        "Showing " (data.papers.len()) " of " (data.total_papers) " papers"
                                    }
                                }
                            }
                            .card-body {
                                @if data.papers.is_empty() {
                                    p.text-muted { "No papers found" }
                                } @else {
                                    .list-group {
                                        @for paper in data.papers {
                                            (paper_item(ctx, paper, citations))
                                        }
                                    }
                                }

                                (pagination(data.current_page, data.total_pages, |page| ctx.venue_page_url(&venue.id, page)))
                            }
                        }
                    }
                }
            }
        },
    )
}

fn paper_item(ctx: &Ctx, paper: &Paper, citations: &Citations) -> Markup {
    let cites = citations.of(paper);
    html! {
        .list-group-item {
            h6.mb-2 { (paper.title) }
            p.mb-1.text-muted.small {
                @for (j, author) in paper.authors.iter().enumerate() {
                    @if j > 0 { ", " }
                    (author_link(ctx, author))
                }
            }
            .d-flex.justify-content-between.align-items-center.mb-2 {
                small.text-muted {
                    @if let Some(year) = paper.year {
                        "Year: " (year)
                    }
                }
                small.text-muted {
                    @if cites.s2ag {
                        "Citations: " strong.text-primary { (cites.count) } " (S2AG)"
                    } @else if cites.unknown() {
                        span title="Not found in Semantic Scholar" { "Citations: n/a" }
                    } @else {
                        "Citations: " (cites.count) " (internal)"
                    }
                }
            }
            // Paper links
            .btn-group.btn-group-sm {
                a.btn.btn-outline-primary href=(get_google_scholar_url(paper)) target="_blank" title="Search on Google Scholar" {
                    i.bi.bi-google.me-1 {}
                    "Scholar"
                }
                @if let Some(publisher_url) = publisher_url(paper) {
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
                    button.btn.btn-outline-info type="button" data-copy=(doi) title="Copy DOI to clipboard" {
                        i.bi.bi-clipboard {}
                    }
                }
            }
        }
    }
}

pub fn statistics_page(ctx: &Ctx, data: &StatisticsData) -> Markup {
    let stats = &data.stats;
    base_template(
        ctx,
        "Statistics",
        "statistics",
        html! {
            .container.py-5 {
                h1.mb-4 { "Dataset Statistics" }

                // Overview cards
                .row.mb-4 {
                    .col-md-3 {
                        .card.text-center {
                            .card-body {
                                h5.card-title { "Total Papers" }
                                .display-4 { (stats.total_papers) }
                            }
                        }
                    }
                    .col-md-3 {
                        .card.text-center {
                            .card-body {
                                h5.card-title { "Total Venues" }
                                .display-4 { (stats.total_venues) }
                            }
                        }
                    }
                    .col-md-3 {
                        .card.text-center {
                            .card-body {
                                h5.card-title { "Total Scholars" }
                                .display-4 { (stats.total_scholars) }
                            }
                        }
                    }
                    .col-md-3 {
                        .card.text-center {
                            .card-body {
                                h5.card-title { "Total Citations" }
                                .display-4 { (stats.total_citations) }
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
                                @if data.papers_by_year.is_empty() {
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
                                                @for (year, count) in data.papers_by_year.iter().rev().take(10) {
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
                                            @for (venue, count) in &data.papers_by_venue {
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
                                    @for (title, count) in &data.top_cited {
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
        },
    )
}

pub fn about_page(ctx: &Ctx) -> Markup {
    base_template(
        ctx,
        "About",
        "about",
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

                        .card.mb-4 id="open-data" {
                            .card-body {
                                h5.card-title { "Open Data" }
                                p.card-text {
                                    "The rankings behind this site are published as JSON files, regenerated with every build:"
                                }
                                ul {
                                    li { a href=(ctx.url("data/venues.json")) { "data/venues.json" } " — all ranked venues" }
                                    li { a href=(ctx.url("data/scholars.json")) { "data/scholars.json" } " — top scholars" }
                                    li { a href=(ctx.url("data/stats.json")) { "data/stats.json" } " — dataset statistics" }
                                    li { a href=(ctx.url("data/search-index.json")) { "data/search-index.json" } " — every venue and scholar with scores" }
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
        },
    )
}

pub fn not_found_page(ctx: &Ctx) -> Markup {
    base_template(
        ctx,
        "Page Not Found",
        "404",
        html! {
            .container.py-5.text-center {
                h1.display-4 { "404" }
                p.lead { "That page doesn't exist." }
                a.btn.btn-primary href=(ctx.url("")) { "Back to the dashboard" }
            }
        },
    )
}
