use qindex::site::{build_site, scholar_shard, SiteOptions};
use std::fs;
use std::path::Path;

const BIB: &str = r#"
@inproceedings{lampson83hints,
author = {Lampson, Butler W.},
title = {Hints for computer system design},
booktitle = "SOSP",
year = {1983},
}

@inproceedings{dean04mapreduce,
author = {Dean, Jeffrey and Ghemawat, Sanjay},
title = {MapReduce: Simplified Data Processing on Large Clusters},
booktitle = "OSDI",
year = {2004},
doi = {10.5555/1251254.1251264},
}

@inproceedings{ghemawat03gfs,
author = {Ghemawat, Sanjay and Gobioff, Howard and Leung, Shun-Tak},
title = {The Google File System},
booktitle = "SOSP",
year = {2003},
}
"#;

/// Writes a small bib/ and static/ under `root` and returns build options.
fn options(root: &Path, base_url: &str) -> SiteOptions {
    let bib_dir = root.join("bib");
    let static_dir = root.join("static");
    fs::create_dir_all(&bib_dir).unwrap();
    fs::create_dir_all(&static_dir).unwrap();
    fs::write(bib_dir.join("test.bib"), BIB).unwrap();
    fs::write(static_dir.join("app.js"), "// app").unwrap();
    fs::write(static_dir.join("style.css"), "/* style */").unwrap();

    SiteOptions {
        bib_dir,
        out_dir: root.join("site"),
        base_url: base_url.to_string(),
        static_dir,
        book_dir: None,
        citations_file: None,
        citation_graph_file: None,
    }
}

fn build(root: &Path, base_url: &str) -> std::path::PathBuf {
    let opts = options(root, base_url);
    build_site(&opts).unwrap();
    opts.out_dir
}

/// All href/src/action attribute values in an HTML document.
fn links(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    for attr in ["href=\"", "src=\"", "action=\""] {
        for (i, _) in html.match_indices(attr) {
            let rest = &html[i + attr.len()..];
            out.push(rest[..rest.find('"').unwrap()].to_string());
        }
    }
    out
}

#[test]
fn builds_static_site_under_base_path() {
    let tmp = tempfile::tempdir().unwrap();
    let site = build(tmp.path(), "/q-index/");

    for page in [
        "index.html",
        "venues/index.html",
        "venue/sosp/index.html",
        "venue/osdi/index.html",
        "scholars/index.html",
        "scholar/index.html",
        "search/index.html",
        "statistics/index.html",
        "about/index.html",
        "404.html",
        "static/app.js",
        "data/search-index.json",
        "data/venues.json",
        "data/stats.json",
    ] {
        assert!(site.join(page).is_file(), "missing {}", page);
    }

    // Every internal link carries the base path
    for page in ["index.html", "venue/sosp/index.html", "about/index.html"] {
        let html = fs::read_to_string(site.join(page)).unwrap();
        for link in links(&html) {
            assert!(
                link.starts_with("/q-index/") || link.starts_with("https://") || link == "#",
                "{} links to {}",
                page,
                link
            );
        }
    }

    // Authors are linked to their scholar page, and the scholar's data is in
    // the shard the browser will look in
    let sosp = fs::read_to_string(site.join("venue/sosp/index.html")).unwrap();
    assert!(sosp.contains("/q-index/scholar/?id=sanjay_ghemawat"));
    let shard = format!(
        "data/scholars/{:02x}.json",
        scholar_shard("sanjay_ghemawat")
    );
    let shard: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(site.join(shard)).unwrap()).unwrap();
    let scholar = &shard["sanjay_ghemawat"];
    assert_eq!(scholar["papers"].as_array().unwrap().len(), 2);
    // Newest first
    assert_eq!(scholar["papers"][0]["year"], 2004);
    assert_eq!(scholar["papers"][0]["doi"], "10.5555/1251254.1251264");

    // Search index lists every scholar, with venue URLs relative to the base
    let index: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(site.join("data/search-index.json")).unwrap())
            .unwrap();
    assert_eq!(index["scholars"].as_array().unwrap().len(), 5);
    assert!(index["venues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["url"] == "venue/sosp/"));
}

#[test]
fn rebuild_replaces_previous_output() {
    let tmp = tempfile::tempdir().unwrap();
    let site = build(tmp.path(), "/");
    fs::write(site.join("stale.html"), "old").unwrap();
    build(tmp.path(), "/");
    assert!(!site.join("stale.html").exists());
}

#[test]
fn refuses_to_overwrite_unrelated_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let out_dir = tmp.path().join("site");
    fs::create_dir_all(&out_dir).unwrap();
    fs::write(out_dir.join("precious.txt"), "keep me").unwrap();

    assert!(build_site(&options(tmp.path(), "/")).is_err());
    assert!(out_dir.join("precious.txt").exists());
}

#[test]
fn uses_s2ag_citation_counts() {
    let tmp = tempfile::tempdir().unwrap();
    let citations = tmp.path().join("s2ag.json");
    fs::write(
        &citations,
        r#"{"release_id": "2026-10-06", "generated": "2026-10-08", "papers": {
            "dean04mapreduce": {"corpus_id": 1, "citations": 20000, "s2_title": "MapReduce", "s2_year": 2004, "match": "doi"},
            "ghemawat03gfs": {"corpus_id": 2, "citations": 9000, "s2_title": "The Google File System", "s2_year": 2003, "match": "local-title"},
            "gfsdup": {"corpus_id": 2, "citations": 9000, "s2_title": "The Google File System", "s2_year": 2003, "match": "local-title"}
        }}"#,
    )
    .unwrap();
    let mut opts = options(tmp.path(), "/");
    opts.citations_file = Some(citations);
    // A duplicate entry for the GFS paper, matched to the same corpus id
    fs::write(
        opts.bib_dir.join("dup.bib"),
        "@inproceedings{gfsdup,\nauthor = {Ghemawat, Sanjay},\ntitle = {The Google File System},\nbooktitle = \"SOSP\",\nyear = {2003},\n}\n",
    )
    .unwrap();
    build_site(&opts).unwrap();
    let site = opts.out_dir;

    // Scholar totals and h-index come from the per-paper counts, counting
    // the duplicate GFS entry once
    let shard = format!(
        "data/scholars/{:02x}.json",
        scholar_shard("sanjay_ghemawat")
    );
    let shard: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(site.join(shard)).unwrap()).unwrap();
    let scholar = &shard["sanjay_ghemawat"];
    assert_eq!(scholar["all_indexed"]["paper_count"], 2);
    assert_eq!(scholar["all_indexed"]["citations"], 29000);
    assert_eq!(scholar["all_indexed"]["h_index"], 2);
    assert_eq!(scholar["papers"].as_array().unwrap().len(), 2);
    assert_eq!(scholar["papers"][1]["id"], "ghemawat03gfs");
    assert_eq!(scholar["papers"][1]["corpus_id"], 2);
    assert_eq!(scholar["papers"][1]["first_author"], "Sanjay Ghemawat");
    assert_eq!(scholar["ranking"]["paper_count"], 2);
    assert_eq!(scholar["ranking"]["citations"], 29000);
    assert_eq!(scholar["ranking"]["h_index"], 2);
    assert_eq!(scholar["papers"][0]["citations"], 20000);
    assert_eq!(scholar["papers"][0]["s2ag"], true);

    // Venue pages: matched papers show S2AG counts, unmatched ones "n/a"
    let sosp = fs::read_to_string(site.join("venue/sosp/index.html")).unwrap();
    assert!(sosp.contains("9000"));
    assert!(sosp.contains("Citations: n/a"));
    assert_eq!(sosp.matches("The Google File System").count(), 1);
    let statistics = fs::read_to_string(site.join("statistics/index.html")).unwrap();
    assert_eq!(statistics.matches("The Google File System").count(), 1);
    let stats: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(site.join("data/stats.json")).unwrap()).unwrap();
    assert_eq!(stats["total_citations"], 29000);
    assert_eq!(stats["snapshot_release"], "2026-10-06");
    assert_eq!(stats["total_papers"], 3);
    assert_eq!(stats["papers_by_year"]["2003"], 1);
    let venues: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(site.join("data/venues.json")).unwrap()).unwrap();
    let sosp_rank = venues
        .as_array()
        .unwrap()
        .iter()
        .find(|venue| venue["id"] == "SOSP")
        .unwrap();
    assert_eq!(sosp_rank["paper_count"], 2);
}

#[test]
fn scholar_all_indexed_metrics_are_distinct_from_ranking_scope() {
    let tmp = tempfile::tempdir().unwrap();
    let mut opts = options(tmp.path(), "/q-index/");
    fs::write(
        opts.bib_dir.join("other.bib"),
        r#"
@article{outside,
author = {Ghemawat, Sanjay},
title = {An indexed non-ranking work},
journal = {arXiv},
year = {2004},
}
@misc{unknown,
author = {Ghemawat, Sanjay},
title = {An indexed work without a venue},
year = {2004},
}
"#,
    )
    .unwrap();
    let citations = tmp.path().join("s2ag.json");
    fs::write(
        &citations,
        r#"{"release_id":"2026-10-06","papers":{
        "dean04mapreduce":{"corpus_id":1,"citations":20},
        "ghemawat03gfs":{"corpus_id":2,"citations":10},
        "outside":{"corpus_id":3,"citations":1000},
        "unknown":{"corpus_id":4,"citations":7}
    }}"#,
    )
    .unwrap();
    opts.citations_file = Some(citations);
    build_site(&opts).unwrap();
    let site = &opts.out_dir;
    let shard_path = format!(
        "data/scholars/{:02x}.json",
        scholar_shard("sanjay_ghemawat")
    );
    let shard: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(site.join(shard_path)).unwrap()).unwrap();
    let scholar = &shard["sanjay_ghemawat"];
    let papers = scholar["papers"].as_array().unwrap();
    assert_eq!(papers.len(), 4);
    assert_eq!(scholar["all_indexed"]["paper_count"], papers.len());
    assert_eq!(scholar["all_indexed"]["citations"], 1037);
    assert_eq!(scholar["all_indexed"]["h_index"], 4);
    assert_eq!(scholar["ranking"]["paper_count"], 2);
    assert_eq!(scholar["ranking"]["citations"], 30);
    assert_eq!(scholar["ranking"]["h_index"], 2);
    let ids: Vec<_> = papers
        .iter()
        .map(|paper| paper["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        ["dean04mapreduce", "outside", "unknown", "ghemawat03gfs"]
    );
    assert!(!papers
        .iter()
        .find(|paper| paper["id"] == "unknown")
        .unwrap()["venue"]
        .as_str()
        .unwrap()
        .trim()
        .is_empty());
    let index: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(site.join("data/search-index.json")).unwrap())
            .unwrap();
    let search = index["scholars"]
        .as_array()
        .unwrap()
        .iter()
        .find(|scholar| scholar[0] == "sanjay_ghemawat")
        .unwrap();
    assert_eq!(search[2], scholar["ranking"]["qindex"]);
    assert_eq!(search[3], scholar["ranking"]["h_index"]);
    assert_eq!(search[4], scholar["ranking"]["paper_count"]);
    let rankings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(site.join("data/scholars.json")).unwrap())
            .unwrap();
    let ranking = rankings
        .as_array()
        .unwrap()
        .iter()
        .find(|ranking| ranking["id"] == "sanjay_ghemawat")
        .unwrap();
    assert_eq!(ranking["paper_count"], scholar["ranking"]["paper_count"]);
    assert_eq!(ranking["citation_count"], scholar["ranking"]["citations"]);
    assert_eq!(ranking["h_index"], scholar["ranking"]["h_index"]);
}
