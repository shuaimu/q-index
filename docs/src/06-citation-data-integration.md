# 6. Citation Data Integration

QIndex uses the downloaded Semantic Scholar Academic Graph (S2AG) release
`2026-10-06` entirely locally. BibTeX supplies bibliographic metadata, not
references. The snapshot pipeline supplies two different citation inputs:

- **Whole-S2 counts:** each matched paper's `citationcount` from the snapshot paper
  record, including citations from papers outside our bibliography. These drive
  reported citation totals and h-indices.
- **Internal graph:** directed citations whose citing and cited corpus ids both
  match our bibliography. These drive venue PageRank and therefore scholar
  QIndex. Internal in-degree is not the whole-S2 citation count.

Neither extraction nor ranking performs Semantic Scholar search or Graph API
calls. An API key is needed only when downloading datasets.

## 6.1 Snapshot and work layout

The complete snapshot lives under `data/s2ag/2026-10-06/`, with 60 gzip JSONL
shards in `papers/` and 395 in `citations/`. Raw downloads and intermediate work
are gitignored. The directory name is the `release_id`; all stages must use the
same root. The default work directory is `ROOT/work`, including
`scan_matches.json`. Do not mix releases or reuse another release's work files.

`release_id` is the citation provenance date (`2026-10-06`). `generated` is the
processing date. A later extraction or website build does not refresh the
snapshot's counts.

## 6.2 Local-only pipeline

Export exactly the paper ids, titles, years, DOIs, optional DBLP keys and venues used by the Rust
parser, then run all three stages:

```bash
./target/release/qindex export-papers
python3 -I scripts/match_s2ag.py scan --snapshot-dir data/s2ag/2026-10-06
python3 -I scripts/match_s2ag.py resolve --snapshot-dir data/s2ag/2026-10-06
python3 -I scripts/match_s2ag.py graph --snapshot-dir data/s2ag/2026-10-06
./target/release/qindex build-site
```

`--snapshot-dir ROOT` is required. `--work-dir` overrides `ROOT/work`;
`--workers` controls bounded parallel scanning (default 16, capped by available
CPUs). `--papers` defaults to `cache/citations/papers.jsonl`, `--out` to
`cache/citations/s2ag_paper_citations.json`, and `--graph-out` to
`cache/citations/s2ag_citation_graph.json`.

### Scan

`scan` streams local paper shards and selects a match per local id in this order:
exact DOI, exact case-sensitive DBLP record key, then normalized title with a
compatible publication year (`cmd_scan`, `pick` in `scripts/match_s2ag.py`).
The optional BibTeX `dblp` field becomes `Paper.dblp_id` and is exported as
`dblp_id`; use the verified record key, such as `journals/pvldb/ChengSKLQCTZBBB22`,
not a URL or a key inferred from author initials. Exact identifiers remain
authoritative when S2's year or title differs. Title-only matches require at
least three words and a year difference of at most one when both years exist.
If both sides identify different published DBLP records, title fallback rejects
the candidate rather than transferring citations between distinct versions.
CoRR identifiers may represent a preprint of the same work and do not by
themselves establish a conflicting published version.
Selected metadata, counts and provenance are saved in `ROOT/work/scan_matches.json`;
no online title-search endpoint is used.

### Resolve

`resolve` validates the scan's release, shard manifest and bibliography digest,
then writes the selected snapshot counts (`cmd_resolve`). It does not fetch
current counts, consult API caches or fall back to online title search. The
count artifact contains `release_id`, `generated`, `source`,
`matched`, `total`, `by_method`, and a `papers` map keyed by our paper ids. Each
entry contains integer `corpus_id`, `citations`, `influential` where available,
`s2_title`, `s2_year`, and `match` (`local-doi`, `local-dblp` or `local-title`).

### Graph

`graph` streams the local citation shards and keeps only records whose two corpus
ids are matched to the bibliography. The graph artifact includes `release_id`,
`generated`, `source`, scan metadata, and `edges`: sorted unique arrays
`[citing_corpus_id, cited_corpus_id]`. Corpus ids are unsigned integers. External
endpoints and same-corpus self edges are excluded. Repeated S2 rows and duplicate
BibTeX aliases must not multiply edges.

Coverage, matching-method totals, scanned-record totals and edge totals should be
read from the generated artifacts and extraction summaries. This chapter does
not carry forward coverage figures from the older partial-download/API run.

## 6.3 Committed artifacts and Rust consumption

Commit both derived files:

- `cache/citations/s2ag_paper_citations.json`
- `cache/citations/s2ag_citation_graph.json`

CI builds from these files without access to the bulk snapshot. The default
ranking CLI and static site consume the same local inputs. Counts are exact
lookups by paper id, not fuzzy title matches during a build. When a graph is
loaded, its release must agree with the counts release.

The graph loader chooses one canonical BibTeX id per corpus id, preferring
complete venue/DOI/year/authors metadata and then smallest local id, and populates
`Paper.citations`, `Paper.cited_by` and `CitationGraph.edges` once. The same
corpus-id deduplication is shared by publication lists, statistics and ranking;
unmatched works deduplicate only by local id. Venue aggregation counts each
directed edge once, not both adjacency representations. Name-based scholar
identity is separate: paper matching and publisher-confirmed alias corrections
do not verify global author identities or supply affiliations.

`qindex build-site --citations-file PATH --citation-graph-file PATH` overrides the
two inputs independently (`SiteOptions.citations_file` and
`SiteOptions.citation_graph_file`). Overrides must still identify the same release.

## 6.4 Interpretation and ranking policy

Whole-S2 counts may greatly exceed internal graph in-degree: the bibliography is
only a subset of the S2 snapshot. Reported scholar and venue totals and h-indices
deduplicate S2 corpus ids so that BibTeX aliases do not inflate counts. Unmatched
papers are not evidence of zero external citations; citation display distinguishes
unknown counts from available internal evidence.

PageRank aggregates directed cross-venue internal citations with year-decayed
weights. Same-venue edges are not part of venue ranking. Dangling venues distribute
mass uniformly. The retained `pagerank` JSON field is normalized, unadjusted
PageRank (**Venue Score**), without manual prestige, name or eligibility boosts;
`impact_factor` is score times `ln(unique venue works + 1)`, not journal impact.
Scholar QIndex retains CSRankings eligibility and author-position weights, but
all contributions and its logarithmic publication count use unique eligible
authored works. Excluded works cannot boost that count. Scholar decay uses
`0.95^(max(reference_year - paper_year, 0)/5)`, with a valid release year replacing
the current-UTC default; unknown years are undecayed. Whole-S2 counts are not a
new QIndex term. A graph without usable cross-venue edges yields uniform 1/N
PageRank. Bibliography boundaries and partial coverage limit interpretation;
see Chapters 5 and 11.

## 6.5 Legacy pipelines

`scripts/parse_s2ag.py` and its old title-keyed outputs are superseded. Older
Semantic Scholar, Crossref and OpenAlex fetching scripts are not the snapshot
extraction path and do not supply the default rankings. In particular, a legacy
live-fetch command is not a substitute for rebuilding the two local artifacts.
Rerun export, scan, resolve and graph after changing the bibliography, or after
selecting a new complete snapshot, and commit both outputs together.
