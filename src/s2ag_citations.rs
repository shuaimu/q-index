//! Citation counts from Semantic Scholar (S2AG).
//!
//! `scripts/match_s2ag.py` matches our papers to Semantic Scholar (by DOI, or
//! by title + year against the S2AG papers dataset) and writes the results
//! keyed by our paper ids, so lookups here are exact.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Where `scripts/match_s2ag.py` writes its results.
pub const DEFAULT_PATH: &str = "cache/citations/s2ag_paper_citations.json";

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
    /// Date the counts were fetched.
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
            "Loaded S2AG citation counts for {} papers (fetched {})",
            citations.papers.len(),
            citations.generated.as_deref().unwrap_or("unknown date")
        );
        Ok(citations)
    }

    pub fn get(&self, paper_id: &str) -> Option<&S2agPaper> {
        self.papers.get(paper_id)
    }
}
