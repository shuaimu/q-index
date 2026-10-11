use super::*;
use crate::models::{normalize_author_name, Venue, VenueType};
use crate::s2ag_citations::S2agPaper;

fn paper(id: &str, venue: &str, year: u32, authors: &[&str]) -> Paper {
    Paper {
        id: id.into(),
        title: id.into(),
        authors: authors.iter().map(|a| (*a).into()).collect(),
        venue: venue.into(),
        venue_type: VenueType::Conference,
        year: Some(year),
        month: None,
        citations: Vec::new(),
        cited_by: Vec::new(),
        abstract_text: None,
        keywords: Vec::new(),
        doi: None,
        dblp_id: None,
        url: None,
        bib_key: id.into(),
        source_file: String::new(),
    }
}

fn venue(name: &str) -> Venue {
    Venue {
        id: normalize_venue_id(name),
        name: name.into(),
        full_name: name.into(),
        venue_type: VenueType::Conference,
        papers: Vec::new(),
        pagerank: 0.0,
        impact_factor: 0.0,
        field: "systems".into(),
    }
}

fn scholar(name: &str, ids: &[&str]) -> Scholar {
    Scholar {
        id: name.into(),
        name: name.into(),
        normalized_name: normalize_author_name(name),
        papers: ids.iter().map(|id| (*id).into()).collect(),
        affiliations: Vec::new(),
        qindex: 0.0,
        h_index: 0,
        citation_count: 0,
        publications_by_venue: HashMap::new(),
        coauthors: HashMap::new(),
    }
}

fn add(graph: &mut CitationGraph, paper: Paper) {
    let venue_id = normalize_venue_id(&paper.venue);
    if !graph.venues.contains_key(&venue_id) {
        graph.add_venue(venue(&paper.venue));
    }
    graph
        .venues
        .get_mut(&venue_id)
        .unwrap()
        .papers
        .push(paper.id.clone());
    graph.add_paper(paper);
}

fn counts(entries: &[(&str, u64)]) -> S2agCitations {
    S2agCitations {
        release_id: Some("2026-10-06".into()),
        generated: None,
        papers: entries
            .iter()
            .map(|(id, corpus_id)| {
                (
                    (*id).into(),
                    S2agPaper {
                        corpus_id: Some(*corpus_id),
                        citations: 7,
                    },
                )
            })
            .collect(),
    }
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

#[test]
fn aliases_and_repeated_ids_leave_scores_and_ranked_counts_unchanged() {
    let mut graph = CitationGraph::new();
    add(&mut graph, paper("a", "SOSP", 2026, &["Alice"]));
    add(&mut graph, paper("b", "OSDI", 2026, &["Bob"]));
    graph.add_scholar(scholar("Alice", &["a"]));
    graph.add_scholar(scholar("Bob", &["b"]));
    let citations = counts(&[("a", 1), ("alias", 1), ("b", 2)]);
    let mut baseline = PageRankCalculator::with_citations(&graph, &citations);
    let metrics = baseline.calculate().unwrap();
    let venues = baseline.get_top_venues(10, None);
    let scholars = baseline.get_top_scholars(10, None);

    let mut alias = graph.papers["a"].clone();
    alias.id = "alias".into();
    add(&mut graph, alias);
    graph
        .scholars
        .get_mut("Alice")
        .unwrap()
        .papers
        .extend(["a".into(), "alias".into()]);
    graph
        .venues
        .get_mut("SOSP")
        .unwrap()
        .papers
        .push("a".into());
    let mut duplicated = PageRankCalculator::with_citations(&graph, &citations);
    let duplicate_metrics = duplicated.calculate().unwrap();
    assert_eq!(metrics.scholar_scores, duplicate_metrics.scholar_scores);
    assert_eq!(metrics.venue_scores, duplicate_metrics.venue_scores);
    for (before, after) in scholars.iter().zip(duplicated.get_top_scholars(10, None)) {
        assert_eq!(before.id, after.id);
        assert_eq!(before.paper_count, after.paper_count);
        assert_eq!(before.citation_count, after.citation_count);
        assert_eq!(before.h_index, after.h_index);
        assert_eq!(before.top_venues, after.top_venues);
    }
    for (before, after) in venues.iter().zip(duplicated.get_top_venues(10, None)) {
        assert_eq!(before.id, after.id);
        assert_eq!(before.paper_count, after.paper_count);
        close(before.impact_factor, after.impact_factor);
    }
}

#[test]
fn excluded_and_misattributed_works_cannot_change_scholar_credit() {
    let mut graph = CitationGraph::new();
    add(
        &mut graph,
        paper("eligible", "SOSP", 2026, &["Alice", "Bob"]),
    );
    add(
        &mut graph,
        paper("excluded", "Unlisted Venue", 2026, &["Alice"]),
    );
    add(
        &mut graph,
        paper("absent", "SOSP", 2026, &["Carol", "Dave"]),
    );
    add(&mut graph, paper("empty", "SOSP", 2026, &[]));
    graph.add_scholar(scholar("Alice", &["eligible"]));
    graph.add_scholar(scholar("Bob", &["eligible"]));
    graph.add_scholar(scholar("Nobody", &["absent", "empty"]));
    let mut baseline = PageRankCalculator::new(&graph);
    baseline.params.reference_year = 2026;
    baseline.venue_scores.insert("SOSP".into(), 1.0);
    baseline.calculate_scholar_scores();
    close(baseline.scholar_scores["Alice"], 100.0);
    close(baseline.scholar_scores["Bob"], 80.0);
    close(baseline.scholar_scores["Nobody"], 0.0);
    let scores = baseline.scholar_scores.clone();
    graph.scholars.get_mut("Alice").unwrap().papers.extend([
        "excluded".into(),
        "absent".into(),
        "empty".into(),
        "missing".into(),
    ]);
    let mut changed = PageRankCalculator::new(&graph);
    changed.params.reference_year = 2026;
    changed.venue_scores.insert("SOSP".into(), 1.0);
    changed.calculate_scholar_scores();
    assert_eq!(scores, changed.scholar_scores);
    let rankings = changed.get_top_scholars(10, Some(1));
    assert_eq!(rankings.len(), 2);
    assert!(rankings
        .iter()
        .all(|r| r.paper_count == 1 && r.top_venues == ["SOSP"]));
}

#[test]
fn snapshot_year_and_future_years_have_safe_decay() {
    let mut graph = CitationGraph::new();
    add(&mut graph, paper("old", "SOSP", 2021, &["Old"]));
    add(&mut graph, paper("new", "SOSP", 2026, &["New"]));
    add(&mut graph, paper("future", "SOSP", u32::MAX, &["Future"]));
    for (name, id) in [("Old", "old"), ("New", "new"), ("Future", "future")] {
        graph.add_scholar(scholar(name, &[id]));
    }
    let mut citations = counts(&[]);
    let mut calculator = PageRankCalculator::with_citations(&graph, &citations);
    assert_eq!(calculator.params.reference_year, 2026);
    calculator.venue_scores.insert("SOSP".into(), 1.0);
    calculator.calculate_scholar_scores();
    close(calculator.scholar_scores["Old"], 95.0);
    close(calculator.scholar_scores["New"], 100.0);
    close(calculator.scholar_scores["Future"], 100.0);
    assert!(calculator
        .scholar_scores
        .values()
        .all(|score| score.is_finite()));
    citations.release_id = Some("2026-02-30".into());
    assert_eq!(
        PageRankCalculator::with_citations(&graph, &citations)
            .params
            .reference_year,
        PageRankCalculator::new(&graph).params.reference_year
    );
    citations.release_id = None;
    assert_eq!(
        PageRankCalculator::with_citations(&graph, &citations)
            .params
            .reference_year,
        PageRankCalculator::new(&graph).params.reference_year
    );
}

#[test]
fn author_position_weights_preserve_first_middle_and_last_credit() {
    let mut graph = CitationGraph::new();
    add(
        &mut graph,
        paper(
            "p",
            "SOSP",
            2026,
            &["First", "MiddleOne", "MiddleTwo", "Last"],
        ),
    );
    for name in ["First", "MiddleOne", "MiddleTwo", "Last"] {
        graph.add_scholar(scholar(name, &["p"]));
    }
    let mut calculator = PageRankCalculator::new(&graph);
    calculator.params.reference_year = 2026;
    calculator.venue_scores.insert("SOSP".into(), 1.0);
    calculator.calculate_scholar_scores();
    close(calculator.scholar_scores["First"], 100.0);
    close(calculator.scholar_scores["Last"], 80.0);
    close(calculator.scholar_scores["MiddleOne"], 30.0);
    close(calculator.scholar_scores["MiddleTwo"], 30.0);
}

#[test]
fn dangling_pagerank_conserves_mass_and_matches_two_node_solution() {
    let mut graph = CitationGraph::new();
    let mut source = paper("source", "SOSP", 2026, &["Alice"]);
    source.citations = vec!["target".into(), "alias".into(), "target".into()];
    add(&mut graph, source);
    add(&mut graph, paper("target", "OSDI", 2026, &["Bob"]));
    add(&mut graph, paper("alias", "OSDI", 2026, &["Bob"]));
    let citations = counts(&[("source", 1), ("target", 2), ("alias", 2)]);
    let mut calculator = PageRankCalculator::with_citations(&graph, &citations);
    calculator.params.tolerance = 1e-12;
    calculator.build_venue_graph();
    close(calculator.venue_graph["SOSP"]["OSDI"], 1.0);
    calculator.calculate_venue_pagerank().unwrap();
    close(calculator.venue_scores.values().sum(), 1.0);
    close(calculator.venue_scores["SOSP"], 1.0 / 2.85);
    close(calculator.venue_scores["OSDI"], 1.85 / 2.85);
}

#[test]
fn equally_connected_venues_receive_equal_unadjusted_pagerank() {
    let mut graph = CitationGraph::new();
    let mut source = paper("source", "Unlisted Venue", 2026, &["Alice"]);
    source.citations = vec!["sosp".into(), "socc".into()];
    add(&mut graph, source);
    add(&mut graph, paper("sosp", "SOSP", 2026, &["Bob"]));
    add(&mut graph, paper("socc", "SOCC", 2026, &["Carol"]));

    let mut calculator = PageRankCalculator::new(&graph);
    calculator.params.reference_year = 2026;
    calculator.params.tolerance = 1e-12;
    let metrics = calculator.calculate().unwrap();
    // One source splits its outgoing mass equally between two dangling venues.
    close(
        metrics.venue_scores[&normalize_venue_id("Unlisted Venue")],
        1.0 / 3.85,
    );
    close(metrics.venue_scores["SOSP"], 1.425 / 3.85);
    close(metrics.venue_scores["SOCC"], 1.425 / 3.85);
    close(metrics.venue_scores.values().sum(), 1.0);

    let rankings = calculator.get_top_venues(10, None);
    assert_eq!(
        rankings.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
        ["SOCC", "SOSP"]
    );
    for ranking in rankings {
        close(ranking.pagerank, 1.425 / 3.85);
    }
}

#[test]
fn edgeless_pagerank_is_uniform_despite_names_eligibility_and_paper_counts() {
    let mut graph = CitationGraph::new();
    let mut sosp = paper("sosp", "SOSP", 2026, &["Alice"]);
    sosp.citations.push("sosp".into());
    add(&mut graph, sosp);
    for id in ["socc1", "socc2", "socc3"] {
        let mut socc = paper(id, "SOCC", 2026, &["Bob"]);
        socc.citations.push("socc1".into());
        add(&mut graph, socc);
    }
    graph.add_venue(venue("Unlisted Venue"));

    let mut calculator = PageRankCalculator::new(&graph);
    calculator.params.reference_year = 2026;
    let metrics = calculator.calculate().unwrap();
    assert_eq!(metrics.venue_scores.len(), 3);
    for score in metrics.venue_scores.values() {
        close(*score, 1.0 / 3.0);
    }
    close(metrics.venue_scores.values().sum(), 1.0);

    let rankings = calculator.get_top_venues(10, None);
    assert_eq!(
        rankings.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
        ["SOCC", "SOSP"]
    );
    assert_eq!(rankings[0].paper_count, 3);
    assert_eq!(rankings[1].paper_count, 1);
    close(rankings[0].impact_factor, 4.0_f64.ln() / 3.0);
    close(rankings[1].impact_factor, 2.0_f64.ln() / 3.0);
}

#[test]
fn extreme_and_future_citation_years_do_not_create_nan_edges() {
    let mut graph = CitationGraph::new();
    let mut source = paper("source", "SOSP", u32::MAX, &["Alice"]);
    source.citations.push("old".into());
    let mut old = paper("old", "OSDI", 2026, &["Bob"]);
    old.citations.push("source".into());
    add(&mut graph, source);
    add(&mut graph, old);
    let mut calculator = PageRankCalculator::new(&graph);
    calculator.build_venue_graph();
    assert!(!calculator.venue_graph.contains_key("SOSP"));
    close(calculator.venue_graph["OSDI"]["SOSP"], 1.0);
    calculator.calculate_venue_pagerank().unwrap();
    assert!(calculator
        .venue_scores
        .values()
        .all(|score| score.is_finite()));
    close(calculator.venue_scores.values().sum(), 1.0);
}

#[test]
fn ranking_ties_use_stable_identity_order_and_top_venue_order() {
    let mut graph = CitationGraph::new();
    add(&mut graph, paper("sosp", "SOSP", 2026, &["Zed", "Amy"]));
    add(&mut graph, paper("osdi", "OSDI", 2026, &["Zed", "Amy"]));
    graph.add_scholar(scholar("Zed", &["sosp", "osdi"]));
    graph.add_scholar(scholar("Amy", &["osdi", "sosp"]));
    let mut calculator = PageRankCalculator::new(&graph);
    calculator.build_venue_graph();
    calculator
        .venue_scores
        .extend([("SOSP".into(), 0.5), ("OSDI".into(), 0.5)]);
    calculator
        .scholar_scores
        .extend([("Zed".into(), 100.0), ("Amy".into(), 100.0)]);
    let scholars = calculator.get_top_scholars(10, None);
    assert_eq!(
        scholars.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["Amy", "Zed"]
    );
    assert!(scholars.iter().all(|s| s.top_venues == ["OSDI", "SOSP"]));
    assert_eq!(
        calculator
            .get_top_venues(10, None)
            .iter()
            .map(|v| v.id.as_str())
            .collect::<Vec<_>>(),
        ["OSDI", "SOSP"]
    );
}

#[test]
fn eligible_unique_count_controls_log_scaling_and_top_venue_frequency() {
    let mut graph = CitationGraph::new();
    add(&mut graph, paper("a", "SOSP", 2026, &["Alice"]));
    add(&mut graph, paper("alias", "SOSP", 2026, &["Alice"]));
    add(&mut graph, paper("b1", "OSDI", 2026, &["Bob"]));
    add(&mut graph, paper("b2", "OSDI", 2026, &["Bob"]));
    add(
        &mut graph,
        paper("excluded", "Unlisted Venue", 2026, &["Alice"]),
    );
    graph.add_scholar(scholar("Alice", &["a", "a", "alias", "excluded"]));
    graph.add_scholar(scholar("Bob", &["b1", "b2"]));
    let citations = counts(&[("a", 1), ("alias", 1), ("b1", 2), ("b2", 3)]);
    let mut calculator = PageRankCalculator::with_citations(&graph, &citations);
    calculator
        .venue_scores
        .extend([("SOSP".into(), 0.5), ("OSDI".into(), 0.5)]);
    calculator.calculate_scholar_scores();
    close(
        calculator.scholar_scores["Alice"],
        100.0 * 2.0_f64.ln() / (2.0 * 3.0_f64.ln()),
    );
    close(calculator.scholar_scores["Bob"], 100.0);
    let alice = calculator
        .get_top_scholars(10, None)
        .into_iter()
        .find(|s| s.id == "Alice")
        .unwrap();
    assert_eq!(alice.paper_count, 1);
    assert_eq!(alice.citation_count, 7);
    assert_eq!(alice.h_index, 1);

    // A stale occurrence-heavy venue index cannot win over actual unique publications.
    graph
        .papers
        .get_mut("b1")
        .unwrap()
        .authors
        .push("Alice".into());
    graph
        .papers
        .get_mut("b2")
        .unwrap()
        .authors
        .push("Alice".into());
    let alice = graph.scholars.get_mut("Alice").unwrap();
    alice.papers.extend(["b1".into(), "b2".into()]);
    alice
        .publications_by_venue
        .insert("SOSP".into(), vec!["alias".into(); 20]);
    let calculator = PageRankCalculator::with_citations(&graph, &citations);
    let alice = calculator
        .get_top_scholars(10, None)
        .into_iter()
        .find(|s| s.id == "Alice")
        .unwrap();
    assert_eq!(alice.paper_count, 3);
    assert_eq!(alice.top_venues, ["OSDI", "SOSP"]);
}
