//! Static site generator.
//!
//! Renders the whole QIndex website into a directory of plain HTML and JSON
//! files that any static host (e.g. GitHub Pages) can serve. Everything the
//! old actix server computed per request is computed once here:
//!
//! - rankings, venue pages (with pagination), statistics and about pages are
//!   pre-rendered HTML;
//! - search runs in the browser against `data/search-index.json`;
//! - scholar profiles are rendered in the browser from JSON shards in
//!   `data/scholars/`, so the ~44k scholars don't each need an HTML file.

mod templates;

use anyhow::{bail, Context, Result};
use log::info;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use crate::algorithm::PageRankCalculator;
use crate::models::{normalize_author_name, CitationGraph, Paper, ScholarRanking, VenueRanking};
use crate::parser::{generate_scholar_id, BibParser};
use crate::s2ag_citations::S2agCitations;

/// Number of JSON shards scholar profiles are split into.
/// Must match `SCHOLAR_SHARDS` in static/app.js.
pub const SCHOLAR_SHARDS: u32 = 256;

/// Papers per page on venue pages (same default as the old server).
const PAPERS_PER_PAGE: usize = 20;

/// Number of scholars on the rankings page.
const TOP_SCHOLARS: usize = 100;

pub struct SiteOptions {
    pub bib_dir: PathBuf,
    pub out_dir: PathBuf,
    /// URL path prefix the site is served under, e.g. `/q-index/`.
    pub base_url: String,
    /// Directory holding style.css / app.js, copied to `static/`.
    pub static_dir: PathBuf,
    /// Rendered mdBook to copy to `book/` (skipped if it doesn't exist).
    pub book_dir: Option<PathBuf>,
    /// S2AG citation counts from scripts/match_s2ag.py (none if `None`).
    pub citations_file: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub struct BuildReport {
    pub html_files: usize,
    pub json_files: usize,
    pub other_files: usize,
    pub bytes: u64,
    pub book_included: bool,
}

/// URL prefix every internal link is built from. Always starts and ends
/// with `/` (or is a full `scheme://host/path/` URL).
#[derive(Debug, Clone)]
pub struct Base(String);

impl Base {
    pub fn new(raw: &str) -> Self {
        let raw = raw.trim();
        if raw.contains("://") {
            return Base(format!("{}/", raw.trim_end_matches('/')));
        }
        let trimmed = raw.trim_matches('/');
        if trimmed.is_empty() {
            Base("/".to_string())
        } else {
            Base(format!("/{}/", trimmed))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Everything templates need to build links.
pub struct Ctx {
    base: Base,
    /// Build date shown in the footer.
    pub generated: String,
    /// Venue id -> URL slug.
    venue_slugs: HashMap<String, String>,
    /// Scholar ids that exist, so author names are only linked when the
    /// scholar page will actually find data.
    scholar_ids: std::collections::HashSet<String>,
}

impl Ctx {
    pub fn base(&self) -> &str {
        self.base.as_str()
    }

    /// Site-internal URL for a path like `venues/`.
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base.as_str(), path.trim_start_matches('/'))
    }

    pub fn venue_url(&self, venue_id: &str) -> String {
        self.venue_page_url(venue_id, 1)
    }

    pub fn venue_page_url(&self, venue_id: &str, page: usize) -> String {
        self.url(&venue_page_path(self.venue_slug(venue_id), page))
    }

    fn venue_slug<'a>(&'a self, venue_id: &'a str) -> &'a str {
        self.venue_slugs
            .get(venue_id)
            .map(String::as_str)
            .unwrap_or(venue_id)
    }

    pub fn scholar_url(&self, scholar_id: &str) -> String {
        format!(
            "{}?id={}",
            self.url("scholar/"),
            urlencoding::encode(scholar_id)
        )
    }

    /// Scholar page URL for an author name as written on a paper, if that
    /// author is a known scholar.
    pub fn author_url(&self, author: &str) -> Option<String> {
        let id = author_scholar_id(author);
        self.scholar_ids
            .contains(&id)
            .then(|| self.scholar_url(&id))
    }
}

/// Relative path of page `page` (1-based) of a venue's paper listing.
fn venue_page_path(slug: &str, page: usize) -> String {
    if page <= 1 {
        format!("venue/{}/", slug)
    } else {
        format!("venue/{}/page/{}/", slug, page)
    }
}

/// Same id the parser assigns when it creates a scholar from an author name.
fn author_scholar_id(author: &str) -> String {
    generate_scholar_id(&normalize_author_name(author))
}

/// Lowercase ASCII slug, e.g. "USENIX ATC" -> "usenix-atc".
fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("venue");
    }
    out
}

/// Assign every venue a unique, filesystem-safe slug.
fn venue_slugs(graph: &CitationGraph) -> HashMap<String, String> {
    let mut ids: Vec<&String> = graph.venues.keys().collect();
    ids.sort();
    let mut taken = std::collections::HashSet::new();
    let mut slugs = HashMap::new();
    for id in ids {
        let base = slugify(id);
        let mut slug = base.clone();
        let mut n = 2;
        while !taken.insert(slug.clone()) {
            slug = format!("{}-{}", base, n);
            n += 1;
        }
        slugs.insert(id.clone(), slug);
    }
    slugs
}

/// FNV-1a over the UTF-8 bytes of the scholar id.
/// Must match `shardOf` in static/app.js.
pub fn scholar_shard(id: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in id.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h % SCHOLAR_SHARDS
}

/// Citation count shown for a paper: Semantic Scholar's when the paper was
/// matched, otherwise the number of citing papers inside our own dataset.
#[derive(Debug, Clone, Copy)]
pub struct PaperCitations {
    pub count: usize,
    pub s2ag: bool,
}

impl PaperCitations {
    /// Not matched in Semantic Scholar and not cited within our dataset, so
    /// we don't actually know the count.
    pub fn unknown(&self) -> bool {
        !self.s2ag && self.count == 0
    }
}

/// The citation counts every page displays. Scholar and venue totals and
/// h-indexes are derived from these per-paper counts. (Rankings are not:
/// QIndex and PageRank come from `PageRankCalculator` unchanged.)
pub struct Citations {
    s2ag: S2agCitations,
}

impl Citations {
    pub fn of(&self, paper: &Paper) -> PaperCitations {
        match self.s2ag.get(&paper.id) {
            Some(s) => PaperCitations {
                count: s.citations,
                s2ag: true,
            },
            None => PaperCitations {
                count: paper.cited_by.len(),
                s2ag: false,
            },
        }
    }

    pub fn total<'a>(&self, papers: impl IntoIterator<Item = &'a Paper>) -> usize {
        self.distinct_counts(papers).iter().sum()
    }

    pub fn h_index<'a>(&self, papers: impl IntoIterator<Item = &'a Paper>) -> usize {
        let mut counts = self.distinct_counts(papers);
        counts.sort_unstable_by(|a, b| b.cmp(a));
        counts
            .iter()
            .enumerate()
            .take_while(|(i, &c)| c > *i)
            .count()
    }

    /// Per-paper counts, taking each Semantic Scholar paper once: duplicate
    /// BibTeX entries for the same paper (same corpus id) would otherwise be
    /// counted several times.
    fn distinct_counts<'a>(&self, papers: impl IntoIterator<Item = &'a Paper>) -> Vec<usize> {
        let mut seen = std::collections::HashSet::new();
        papers
            .into_iter()
            .filter(|p| match self.s2ag.get(&p.id).and_then(|s| s.corpus_id) {
                Some(corpus_id) => seen.insert(corpus_id),
                None => true,
            })
            .map(|p| self.of(p).count)
            .collect()
    }
}

pub struct Stats {
    pub total_papers: usize,
    pub total_venues: usize,
    pub total_scholars: usize,
    pub total_citations: usize,
}

/// Paper counts per field, largest first (dashboard chart).
pub struct FieldCounts {
    pub labels: Vec<String>,
    pub values: Vec<usize>,
}

/// Data for one page of a venue's paper listing.
pub struct VenuePage<'a> {
    pub venue: &'a crate::models::Venue,
    pub papers: &'a [&'a Paper],
    pub total_citations: usize,
    pub top_authors: &'a [(String, usize)],
    pub current_page: usize,
    pub total_pages: usize,
    pub total_papers: usize,
}

pub struct StatisticsData {
    pub stats: Stats,
    pub papers_by_year: BTreeMap<u32, usize>,
    pub papers_by_venue: Vec<(String, usize)>,
    pub top_cited: Vec<(String, usize)>,
}

/// Builds the site into `opts.out_dir`, replacing anything already there.
pub fn build_site(opts: &SiteOptions) -> Result<BuildReport> {
    let base = Base::new(&opts.base_url);
    info!(
        "Building static site into {:?} (base URL {})",
        opts.out_dir,
        base.as_str()
    );

    let mut parser = BibParser::new();
    let graph = parser
        .parse_directory(
            opts.bib_dir
                .to_str()
                .context("bib dir is not valid UTF-8")?,
        )
        .context("failed to parse bibliography")?;
    info!(
        "Loaded {} papers, {} venues, {} scholars",
        graph.papers.len(),
        graph.venues.len(),
        graph.scholars.len()
    );

    let citations = Citations {
        s2ag: match &opts.citations_file {
            Some(path) => S2agCitations::load(path)?,
            None => S2agCitations::default(),
        },
    };

    let mut calculator = PageRankCalculator::new(&graph);
    calculator.calculate()?;
    let venue_rankings = calculator.get_top_venues(usize::MAX, None, None);
    let mut scholar_rankings = calculator.get_top_scholars(usize::MAX, None);
    // Show citation totals and h-indexes computed from the displayed
    // per-paper counts; the ranking order (QIndex) is left as calculated.
    for ranking in &mut scholar_rankings {
        if let Some(scholar) = graph.scholars.get(&ranking.id) {
            let papers = scholar.papers.iter().filter_map(|id| graph.papers.get(id));
            ranking.citation_count = citations.total(papers.clone());
            ranking.h_index = citations.h_index(papers);
        }
    }
    let scholar_by_id: HashMap<&str, &ScholarRanking> = scholar_rankings
        .iter()
        .map(|s| (s.id.as_str(), s))
        .collect();
    let venue_rank_by_id: HashMap<&str, &VenueRanking> =
        venue_rankings.iter().map(|v| (v.id.as_str(), v)).collect();

    let ctx = Ctx {
        base,
        generated: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        venue_slugs: venue_slugs(&graph),
        scholar_ids: graph.scholars.keys().cloned().collect(),
    };

    prepare_out_dir(&opts.out_dir)?;
    let mut out = Output {
        root: opts.out_dir.clone(),
        report: BuildReport::default(),
    };

    let stats = Stats {
        total_papers: graph.papers.len(),
        total_venues: graph.venues.len(),
        total_scholars: graph.scholars.len(),
        total_citations: citations.total(graph.papers.values()),
    };
    let top_scholars = &scholar_rankings[..scholar_rankings.len().min(TOP_SCHOLARS)];

    // Pre-rendered pages
    out.html(
        "index.html",
        templates::index_page(
            &ctx,
            &venue_rankings,
            top_scholars,
            &stats,
            &field_counts(&graph),
        ),
    )?;
    out.html(
        "venues/index.html",
        templates::venues_page(&ctx, &venue_rankings),
    )?;
    out.html(
        "scholars/index.html",
        templates::scholars_page(&ctx, top_scholars),
    )?;
    out.html("scholar/index.html", templates::scholar_shell_page(&ctx))?;
    out.html("search/index.html", templates::search_shell_page(&ctx))?;
    out.html(
        "statistics/index.html",
        templates::statistics_page(&ctx, &statistics_data(&graph, &citations)),
    )?;
    out.html("about/index.html", templates::about_page(&ctx))?;
    out.html("404.html", templates::not_found_page(&ctx))?;
    write_venue_pages(&mut out, &ctx, &graph, &citations)?;

    // Data for the browser-side pages, plus the old API responses as files
    write_scholar_shards(&mut out, &graph, &scholar_by_id, &citations)?;
    out.json(
        "data/search-index.json",
        &search_index(&ctx, &graph, &venue_rank_by_id, &scholar_by_id),
    )?;
    out.json("data/venues.json", &venue_rankings)?;
    out.json("data/scholars.json", &top_scholars)?;
    out.json("data/stats.json", &stats_json(&graph, &citations))?;

    // Assets
    copy_dir(
        &opts.static_dir,
        &opts.out_dir.join("static"),
        &mut out.report,
    )
    .with_context(|| format!("copying static assets from {:?}", opts.static_dir))?;
    if let Some(book) = opts
        .book_dir
        .as_ref()
        .filter(|b| b.join("index.html").exists())
    {
        copy_dir(book, &opts.out_dir.join("book"), &mut out.report)
            .with_context(|| format!("copying book from {:?}", book))?;
        out.report.book_included = true;
    }
    // Serve files as-is on GitHub Pages (no Jekyll processing)
    out.write(".nojekyll", b"")?;

    Ok(out.report)
}

/// Removes a previous build. Refuses to delete a non-empty directory that
/// doesn't look like one of our builds, so a typo in --out-dir can't wipe
/// something unrelated.
fn prepare_out_dir(dir: &Path) -> Result<()> {
    if dir.exists() {
        let is_empty = fs::read_dir(dir)?.next().is_none();
        if !is_empty && !dir.join(".nojekyll").exists() {
            bail!(
                "{:?} is not empty and doesn't look like a previous site build; \
                 refusing to delete it",
                dir
            );
        }
        fs::remove_dir_all(dir).with_context(|| format!("removing old build {:?}", dir))?;
    }
    fs::create_dir_all(dir)?;
    Ok(())
}

struct Output {
    root: PathBuf,
    report: BuildReport,
}

impl Output {
    fn write(&mut self, rel: &str, contents: &[u8]) -> Result<()> {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, contents).with_context(|| format!("writing {:?}", path))?;
        self.report.bytes += contents.len() as u64;
        if rel.ends_with(".html") {
            self.report.html_files += 1;
        } else if rel.ends_with(".json") {
            self.report.json_files += 1;
        } else {
            self.report.other_files += 1;
        }
        Ok(())
    }

    fn html(&mut self, rel: &str, page: maud::Markup) -> Result<()> {
        self.write(rel, page.into_string().as_bytes())
    }

    fn json<T: Serialize + ?Sized>(&mut self, rel: &str, value: &T) -> Result<()> {
        self.write(rel, &serde_json::to_vec(value)?)
    }
}

fn copy_dir(src: &Path, dst: &Path, report: &mut BuildReport) -> Result<()> {
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src)?;
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            report.bytes += fs::copy(entry.path(), &target)?;
            report.other_files += 1;
        }
    }
    Ok(())
}

fn field_counts(graph: &CitationGraph) -> FieldCounts {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for venue in graph.venues.values() {
        *counts.entry(venue.field.as_str()).or_insert(0) += venue.papers.len();
    }
    let mut counts: Vec<_> = counts.into_iter().collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    FieldCounts {
        labels: counts.iter().map(|(k, _)| k.to_string()).collect(),
        values: counts.iter().map(|(_, v)| *v).collect(),
    }
}

/// A venue's or scholar's papers, newest first.
fn papers_newest_first<'a>(graph: &'a CitationGraph, ids: &[String]) -> Vec<&'a Paper> {
    let mut papers: Vec<&Paper> = ids.iter().filter_map(|id| graph.papers.get(id)).collect();
    papers.sort_by_key(|p| std::cmp::Reverse(p.year.unwrap_or(0)));
    papers
}

fn write_venue_pages(
    out: &mut Output,
    ctx: &Ctx,
    graph: &CitationGraph,
    citations: &Citations,
) -> Result<()> {
    for venue in graph.venues.values() {
        let papers = papers_newest_first(graph, &venue.papers);
        let total_citations = citations.total(papers.iter().copied());

        let mut author_counts: HashMap<&str, usize> = HashMap::new();
        for paper in &papers {
            for author in &paper.authors {
                *author_counts.entry(author.as_str()).or_insert(0) += 1;
            }
        }
        let mut top_authors: Vec<(String, usize)> = author_counts
            .into_iter()
            .map(|(a, n)| (a.to_string(), n))
            .collect();
        top_authors.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        top_authors.truncate(10);

        let total_pages = papers.len().div_ceil(PAPERS_PER_PAGE).max(1);
        for page in 1..=total_pages {
            let start = (page - 1) * PAPERS_PER_PAGE;
            let end = (start + PAPERS_PER_PAGE).min(papers.len());
            let data = VenuePage {
                venue,
                papers: &papers[start..end],
                total_citations,
                top_authors: &top_authors,
                current_page: page,
                total_pages,
                total_papers: papers.len(),
            };
            let rel = format!(
                "{}index.html",
                venue_page_path(ctx.venue_slug(&venue.id), page)
            );
            out.html(&rel, templates::venue_detail_page(ctx, &data, citations))?;
        }
    }
    Ok(())
}

fn statistics_data(graph: &CitationGraph, citations: &Citations) -> StatisticsData {
    let mut papers_by_year = BTreeMap::new();
    for paper in graph.papers.values() {
        if let Some(year) = paper.year {
            *papers_by_year.entry(year).or_insert(0) += 1;
        }
    }

    let mut papers_by_venue: Vec<(String, usize)> = graph
        .venues
        .values()
        .map(|v| (v.name.clone(), v.papers.len()))
        .collect();
    papers_by_venue.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    papers_by_venue.truncate(10);

    let mut top_cited: Vec<(String, usize)> = graph
        .papers
        .values()
        .map(|p| (p.title.clone(), citations.of(p).count))
        .collect();
    top_cited.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    top_cited.truncate(10);

    StatisticsData {
        stats: Stats {
            total_papers: graph.papers.len(),
            total_venues: graph.venues.len(),
            total_scholars: graph.scholars.len(),
            total_citations: citations.total(graph.papers.values()),
        },
        papers_by_year,
        papers_by_venue,
        top_cited,
    }
}

/// Same shape as the old `/api/stats` response body.
fn stats_json(graph: &CitationGraph, citations: &Citations) -> serde_json::Value {
    let mut papers_by_year = BTreeMap::new();
    for paper in graph.papers.values() {
        if let Some(year) = paper.year {
            *papers_by_year.entry(year).or_insert(0usize) += 1;
        }
    }
    let mut papers_by_tier = BTreeMap::new();
    let mut papers_by_field = BTreeMap::new();
    for venue in graph.venues.values() {
        *papers_by_tier.entry(venue.tier.clone()).or_insert(0usize) += venue.papers.len();
        *papers_by_field.entry(venue.field.clone()).or_insert(0usize) += venue.papers.len();
    }
    serde_json::json!({
        "total_papers": graph.papers.len(),
        "total_venues": graph.venues.len(),
        "total_scholars": graph.scholars.len(),
        "total_citations": citations.total(graph.papers.values()),
        "papers_by_year": papers_by_year,
        "papers_by_tier": papers_by_tier,
        "papers_by_field": papers_by_field,
    })
}

#[derive(Serialize)]
struct SearchIndex {
    venues: Vec<SearchVenue>,
    /// `[id, name, qindex, h_index, paper_count]`, kept as arrays because
    /// there are tens of thousands of them.
    scholars: Vec<(String, String, f64, usize, usize)>,
}

#[derive(Serialize)]
struct SearchVenue {
    id: String,
    name: String,
    full_name: String,
    field: String,
    tier: String,
    pagerank: f64,
    url: String,
}

fn search_index(
    ctx: &Ctx,
    graph: &CitationGraph,
    venue_rank_by_id: &HashMap<&str, &VenueRanking>,
    scholar_by_id: &HashMap<&str, &ScholarRanking>,
) -> SearchIndex {
    let mut venues: Vec<SearchVenue> = graph
        .venues
        .values()
        .map(|v| SearchVenue {
            id: v.id.clone(),
            name: v.name.clone(),
            full_name: v.full_name.clone(),
            field: v.field.clone(),
            tier: v.tier.clone(),
            pagerank: venue_rank_by_id
                .get(v.id.as_str())
                .map_or(0.0, |r| r.pagerank),
            // Relative to the base URL; the browser prepends it
            url: venue_page_path(ctx.venue_slug(&v.id), 1),
        })
        .collect();
    venues.sort_by(|a, b| b.pagerank.total_cmp(&a.pagerank));

    let mut scholars: Vec<_> = graph
        .scholars
        .values()
        .map(|s| {
            let (qindex, h_index) = scholar_by_id
                .get(s.id.as_str())
                .map_or((0.0, 0), |r| (r.qindex, r.h_index));
            (
                s.id.clone(),
                s.name.clone(),
                round4(qindex),
                h_index,
                s.papers.len(),
            )
        })
        .collect();
    scholars.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));

    SearchIndex { venues, scholars }
}

fn round4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

#[derive(Serialize)]
struct ScholarRecord<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    affiliations: &'a [String],
    qindex: f64,
    h_index: usize,
    /// Displayed per-paper citation counts, summed.
    citations: usize,
    /// `[venue name, paper count]`, most papers first.
    venues: Vec<(&'a str, usize)>,
    /// Newest first.
    papers: Vec<PaperRecord<'a>>,
}

#[derive(Serialize)]
struct PaperRecord<'a> {
    title: &'a str,
    venue: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    year: Option<u32>,
    citations: usize,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    s2ag: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_author: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    doi: Option<&'a str>,
    /// Publisher link; omitted when it's just the DOI link.
    #[serde(skip_serializing_if = "Option::is_none")]
    publisher_url: Option<String>,
}

fn write_scholar_shards(
    out: &mut Output,
    graph: &CitationGraph,
    scholar_by_id: &HashMap<&str, &ScholarRanking>,
    citations: &Citations,
) -> Result<()> {
    let mut shards: Vec<BTreeMap<&str, ScholarRecord>> =
        (0..SCHOLAR_SHARDS).map(|_| BTreeMap::new()).collect();

    for scholar in graph.scholars.values() {
        let papers = papers_newest_first(graph, &scholar.papers);

        let mut venue_counts: HashMap<&str, usize> = HashMap::new();
        for paper in &papers {
            *venue_counts.entry(paper.venue.as_str()).or_insert(0) += 1;
        }
        let mut venues: Vec<(&str, usize)> = venue_counts.into_iter().collect();
        venues.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));

        let qindex = scholar_by_id
            .get(scholar.id.as_str())
            .map_or(0.0, |r| r.qindex);

        let record = ScholarRecord {
            name: &scholar.name,
            affiliations: &scholar.affiliations,
            qindex: round4(qindex),
            h_index: citations.h_index(papers.iter().copied()),
            citations: citations.total(papers.iter().copied()),
            venues,
            papers: papers
                .iter()
                .map(|p| {
                    let c = citations.of(p);
                    PaperRecord {
                        title: &p.title,
                        venue: &p.venue,
                        year: p.year,
                        citations: c.count,
                        s2ag: c.s2ag,
                        first_author: p.authors.first().map(String::as_str),
                        doi: p.doi.as_deref(),
                        publisher_url: if p.doi.is_some() {
                            None
                        } else {
                            templates::publisher_url(p)
                        },
                    }
                })
                .collect(),
        };
        shards[scholar_shard(&scholar.id) as usize].insert(&scholar.id, record);
    }

    for (i, shard) in shards.iter().enumerate() {
        out.json(&format!("data/scholars/{:02x}.json", i), shard)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_is_normalized() {
        assert_eq!(Base::new("").as_str(), "/");
        assert_eq!(Base::new("/").as_str(), "/");
        assert_eq!(Base::new("q-index").as_str(), "/q-index/");
        assert_eq!(Base::new("/q-index").as_str(), "/q-index/");
        assert_eq!(Base::new("/q-index/").as_str(), "/q-index/");
        assert_eq!(
            Base::new("https://example.org/q").as_str(),
            "https://example.org/q/"
        );
    }

    #[test]
    fn slugs_are_path_safe() {
        assert_eq!(slugify("USENIX_ATC"), "usenix-atc");
        assert_eq!(slugify("S&P"), "s-p");
        assert_eq!(slugify("__"), "venue");
    }

    #[test]
    fn shard_matches_js_implementation() {
        // Reference values computed with shardOf() in static/app.js
        assert_eq!(scholar_shard(""), 0x811c9dc5 % SCHOLAR_SHARDS);
        assert_eq!(scholar_shard("a"), 0xe40c292c % SCHOLAR_SHARDS);
        assert_eq!(scholar_shard("foobar"), 0xbf9cf968 % SCHOLAR_SHARDS);
    }
}
