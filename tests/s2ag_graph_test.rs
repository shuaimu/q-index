use qindex::algorithm::PageRankCalculator;
use qindex::models::CitationGraph;
use qindex::parser::BibParser;
use qindex::s2ag_citations::S2agCitations;
use std::fs;
use std::path::Path;

fn fixture(root: &Path) -> (CitationGraph, S2agCitations) {
    let bib = root.join("bib");
    fs::create_dir(&bib).unwrap();
    fs::write(bib.join("fixture.bib"), r#"
@inproceedings{a, title={First}, author={Alice Smith}, booktitle={SOSP}, year={2020}}
@inproceedings{a_alias, title={First duplicate}, author={Alice Smith}, booktitle={SOSP}, year={2020}}
@inproceedings{b, title={Second}, author={Bob Smith}, booktitle={OSDI}, year={2020}}
@inproceedings{c, title={Third}, author={Carol Smith}, booktitle={SIGMOD}, year={2020}}
"#).unwrap();
    let graph = BibParser::new()
        .parse_directory(bib.to_str().unwrap())
        .unwrap();
    let counts = serde_json::from_str(
        r#"{
        "release_id":"2026-10-06", "generated":"2026-10-10", "papers":{
            "a":{"corpus_id":1,"citations":10},
            "a_alias":{"corpus_id":1,"citations":10},
            "b":{"corpus_id":2,"citations":20},
            "c":{"corpus_id":3,"citations":30}
        }
    }"#,
    )
    .unwrap();
    (graph, counts)
}

fn write_edges(root: &Path, release: &str, edges: &str) -> std::path::PathBuf {
    let path = root.join("edges.json");
    fs::write(
        &path,
        format!(
            r#"{{"release_id":"{}","generated":"2026-10-10","edges":{}}}"#,
            release, edges
        ),
    )
    .unwrap();
    path
}

#[test]
fn aliases_and_duplicate_edges_do_not_inflate_adjacency_or_counts() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut graph, counts) = fixture(tmp.path());
    let path = write_edges(tmp.path(), "2026-10-06", "[[1,2],[1,2],[1,1]]");
    counts.load_graph(&path, &mut graph).unwrap();
    // Re-loading must replace, not append to, both adjacency directions.
    counts.load_graph(&path, &mut graph).unwrap();
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].from, "a");
    assert_eq!(graph.edges[0].to, "b");
    assert_eq!(graph.papers["a"].citations, ["b"]);
    assert_eq!(graph.papers["b"].cited_by, ["a"]);
    assert!(graph.papers["a_alias"].citations.is_empty());
    assert!(graph.papers["a_alias"].cited_by.is_empty());
    assert_eq!(counts.total(graph.papers.values()), 60);
    assert_eq!(counts.h_index(graph.papers.values()), 3);
}

#[test]
fn forward_direction_and_dangling_mass_determine_rank() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut graph, counts) = fixture(tmp.path());
    let path = write_edges(tmp.path(), "2026-10-06", "[[1,2]]");
    counts.load_graph(&path, &mut graph).unwrap();
    let metrics = PageRankCalculator::with_citations(&graph, &counts)
        .calculate()
        .unwrap();
    // All three venues have the same tier. For A -> B with two dangling
    // venues, p(B)=1.85/3.85, p(A)=p(C)=1/3.85 at damping 0.85.
    assert!((metrics.venue_scores["OSDI"] - 1.85 / 3.85).abs() < 1e-5);
    assert!((metrics.venue_scores["SOSP"] - 1.0 / 3.85).abs() < 1e-5);
    assert!((metrics.venue_scores["SIGMOD"] - 1.0 / 3.85).abs() < 1e-5);
    assert!((metrics.venue_scores.values().sum::<f64>() - 1.0).abs() < 1e-12);

    let path = write_edges(tmp.path(), "2026-10-06", "[[2,1]]");
    counts.load_graph(&path, &mut graph).unwrap();
    let reversed = PageRankCalculator::with_citations(&graph, &counts)
        .calculate()
        .unwrap();
    assert!((reversed.venue_scores["SOSP"] - 1.85 / 3.85).abs() < 1e-5);
    assert!((reversed.venue_scores["OSDI"] - 1.0 / 3.85).abs() < 1e-5);
}

#[test]
fn mismatched_releases_and_external_endpoints_fail_without_mutation() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut graph, counts) = fixture(tmp.path());
    let valid = write_edges(tmp.path(), "2026-10-06", "[[1,2]]");
    counts.load_graph(&valid, &mut graph).unwrap();
    let mismatch = write_edges(tmp.path(), "2025-08-08", "[[2,1]]");
    let error = counts.load_graph(&mismatch, &mut graph).unwrap_err();
    assert!(error.to_string().contains("release mismatch"));
    assert_eq!(graph.edges[0].from, "a");
    let external = write_edges(tmp.path(), "2026-10-06", "[[1,999]]");
    assert!(counts
        .load_graph(&external, &mut graph)
        .unwrap_err()
        .to_string()
        .contains("not in bibliography"));
    assert_eq!(graph.edges[0].from, "a");
    assert_eq!(graph.papers["b"].cited_by, ["a"]);
}

#[test]
fn a_complete_alias_keeps_citations_attached_to_the_published_venue() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut graph, counts) = fixture(tmp.path());
    graph.papers.get_mut("a").unwrap().venue.clear();
    let path = write_edges(tmp.path(), "2026-10-06", "[[1,2]]");
    counts.load_graph(&path, &mut graph).unwrap();

    assert_eq!(graph.edges[0].from, "a_alias");
    let metrics = PageRankCalculator::with_citations(&graph, &counts)
        .calculate()
        .unwrap();
    assert!((metrics.venue_scores["OSDI"] - 1.85 / 3.85).abs() < 1e-5);
    assert!((metrics.venue_scores["SOSP"] - 1.0 / 3.85).abs() < 1e-5);
}
