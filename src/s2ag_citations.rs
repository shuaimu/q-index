//! Citation counts from Semantic Scholar (S2AG).
//!
//! `scripts/match_s2ag.py` matches our papers to Semantic Scholar (by DOI, or
//! by title + year against the S2AG papers dataset) and writes the results
//! keyed by our paper ids, so lookups here are exact.

use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::Path;

use crate::models::{CitationEdge, CitationGraph, Paper};

/// Where `scripts/match_s2ag.py` writes its results.
pub const DEFAULT_PATH: &str = "cache/citations/s2ag_paper_citations.json";
pub const DEFAULT_GRAPH_PATH: &str = "cache/citations/s2ag_citation_graph.json";

/// One matched paper. The file also records the S2 title/year and how the
/// paper was matched (see scripts/match_s2ag.py); only what the site shows
/// is read here.
#[derive(Debug, Clone, Deserialize)]
pub struct S2agPaper {
    pub corpus_id: Option<u64>,
    pub citations: usize,
}

#[derive(Debug, Default, Deserialize)]
pub struct S2agCitations {
    /// Release date of the local dataset, distinct from the extraction date.
    pub release_id: Option<String>,
    pub generated: Option<String>,
    #[serde(default)]
    pub papers: HashMap<String, S2agPaper>,
}

impl S2agCitations {
    /// Loads the matched citation counts; a missing file means no S2AG data.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            log::warn!("S2AG citation file not found: {:?}", path);
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(path)?;
        let citations: Self =
            serde_json::from_str(&contents).with_context(|| format!("parsing {:?}", path))?;
        log::info!(
            "Loaded local S2AG counts for {} papers (snapshot {}, extracted {})",
            citations.papers.len(),
            citations.release_id.as_deref().unwrap_or("unknown"),
            citations.generated.as_deref().unwrap_or("unknown")
        );
        Ok(citations)
    }

    pub fn get(&self, paper_id: &str) -> Option<&S2agPaper> {
        self.papers.get(paper_id)
    }

    pub fn count(&self, paper: &Paper) -> usize {
        self.get(&paper.id)
            .map_or(paper.cited_by.len(), |s| s.citations)
    }

    /// One deterministic bibliography representative per matched S2 work.
    /// Unmatched entries are distinct by local paper id, not guessed titles.
    pub fn unique_papers<'a>(&self, papers: impl IntoIterator<Item = &'a Paper>) -> Vec<&'a Paper> {
        let mut unique = HashMap::new();
        for paper in papers {
            unique
                .entry(self.publication_id(paper))
                .and_modify(|current: &mut &'a Paper| {
                    if preferred_paper(paper, current) {
                        *current = paper;
                    }
                })
                .or_insert(paper);
        }
        let mut papers: Vec<_> = unique.into_values().collect();
        papers.sort_unstable_by(|a, b| a.id.cmp(&b.id));
        papers
    }

    fn publication_id<'a>(&self, paper: &'a Paper) -> PublicationId<'a> {
        self.get(&paper.id)
            .and_then(|matched| matched.corpus_id)
            .map_or(PublicationId::Local(&paper.id), PublicationId::Corpus)
    }

    /// Count each matched corpus once, following the site's existing convention.
    pub fn distinct_counts<'a>(&self, papers: impl IntoIterator<Item = &'a Paper>) -> Vec<usize> {
        let mut seen = HashSet::new();
        papers
            .into_iter()
            .filter(|paper| seen.insert(self.publication_id(paper)))
            .map(|p| self.count(p))
            .collect()
    }

    pub fn total<'a>(&self, papers: impl IntoIterator<Item = &'a Paper>) -> usize {
        let mut seen = HashSet::new();
        papers
            .into_iter()
            .filter(|paper| seen.insert(self.publication_id(paper)))
            .map(|paper| self.count(paper))
            .sum()
    }

    pub fn h_index<'a>(&self, papers: impl IntoIterator<Item = &'a Paper>) -> usize {
        let mut counts = self.distinct_counts(papers);
        counts.sort_unstable_by(|a, b| b.cmp(a));
        counts
            .iter()
            .enumerate()
            .take_while(|(i, c)| **c > *i)
            .count()
    }

    /// Replace adjacency with snapshot edges, preferring complete metadata and
    /// then the smallest local id per corpus. Missing files permit bibliography-only use.
    pub fn load_graph(&self, path: &Path, graph: &mut CitationGraph) -> Result<()> {
        if !path.exists() {
            log::warn!("S2AG graph file not found: {:?}", path);
            return Ok(());
        }
        let artifact: S2agGraph = serde_json::from_str(&fs::read_to_string(path)?)
            .with_context(|| format!("parsing {:?}", path))?;
        ensure!(
            self.release_id.as_deref() == Some(artifact.release_id.as_str()),
            "S2AG count/graph release mismatch: counts {:?}, graph {}",
            self.release_id,
            artifact.release_id
        );
        let mut canonical: HashMap<u64, &str> = HashMap::new();
        for (id, matched) in &self.papers {
            if !graph.papers.contains_key(id) {
                continue;
            }
            if let Some(corpus) = matched.corpus_id {
                canonical
                    .entry(corpus)
                    .and_modify(|current| {
                        if preferred_paper(&graph.papers[id], &graph.papers[*current]) {
                            *current = id;
                        }
                    })
                    .or_insert(id);
            }
        }
        let mut edges = BTreeSet::new();
        for [from, to] in artifact.edges {
            if from == to {
                continue;
            }
            let from_id = canonical.get(&from).with_context(|| {
                format!("S2AG edge source corpus {} is not in bibliography", from)
            })?;
            let to_id = canonical.get(&to).with_context(|| {
                format!("S2AG edge target corpus {} is not in bibliography", to)
            })?;
            edges.insert(((*from_id).to_owned(), (*to_id).to_owned()));
        }
        for paper in graph.papers.values_mut() {
            paper.citations.clear();
            paper.cited_by.clear();
        }
        graph.edges.clear();
        for (from, to) in edges {
            let citing = &graph.papers[&from];
            let cited = &graph.papers[&to];
            graph.edges.push(CitationEdge {
                from: from.clone(),
                to: to.clone(),
                weight: 1.0,
                year: citing.year,
                cross_venue: citing.venue != cited.venue,
            });
            graph
                .papers
                .get_mut(&from)
                .unwrap()
                .citations
                .push(to.clone());
            graph.papers.get_mut(&to).unwrap().cited_by.push(from);
        }
        log::info!(
            "Ingested {} unique internal directed citation edges from local S2AG snapshot {} (extracted {})",
            graph.edges.len(), artifact.release_id, artifact.generated.as_deref().unwrap_or("unknown")
        );
        Ok(())
    }
}

#[derive(Deserialize)]
struct S2agGraph {
    release_id: String,
    generated: Option<String>,
    edges: Vec<[u64; 2]>,
}

#[derive(PartialEq, Eq, Hash)]
enum PublicationId<'a> {
    Corpus(u64),
    Local(&'a str),
}

fn preferred_paper(candidate: &Paper, current: &Paper) -> bool {
    let quality = |paper: &Paper| {
        (
            !paper.venue.is_empty(),
            paper.doi.is_some(),
            paper.year.is_some(),
            paper.authors.len(),
        )
    };
    quality(candidate)
        .cmp(&quality(current))
        .then_with(|| current.id.cmp(&candidate.id))
        .is_gt()
}
