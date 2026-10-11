# 3. The Data Model

QIndex is two things at once: the project (a static bibliographic analytics tool) and the *QIndex score*, a per-scholar prestige metric described in Chapter 5, "The Ranking Algorithm." This chapter is about neither the score nor the user interface. It describes the *data model* — the in-memory structures the entire system reads and writes, how a raw BibTeX entry becomes a `Paper`, and the identity and classification rules that decide how papers, venues, and scholars are grouped.

The model is defined in `src/models.rs` and populated by `BibParser` in `src/parser.rs`. Ranking, site generation and exports consume its `CitationGraph`. There is no database: each invocation reparses the bibliography.

## 3.1 The in-memory graph

The runtime root is `CitationGraph` (`src/models.rs:60-66`):

```rust
#[derive(Clone)]
pub struct CitationGraph {
    pub papers:   IndexMap<String, Paper>,
    pub venues:   IndexMap<String, Venue>,
    pub scholars: IndexMap<String, Scholar>,
    pub edges:    Vec<CitationEdge>,
}
```

Three `IndexMap`s hold entities and a vector holds citation edges. Insertion order is not a semantic ranking order: staging maps are hash maps. Ranking outputs explicitly sort score descending then id ascending; parser indexes are rebuilt in paper-id order.

`CitationGraph` is the one struct in `models.rs` that does **not** derive `Serialize`/`Deserialize`; it derives only `Clone` and provides a hand-written `Debug` impl (`src/models.rs:69-78`) that prints only the four counts to avoid dumping the whole corpus. The graph is therefore not serializable as a single blob; persistence happens at a coarser granularity through the export module (Chapter 9, "The Command-Line Interface").

Use current parser exports and generated `data/stats.json` for roster totals. Older counts describe earlier corpus versions, not the cleaned current bibliography.

### Keys

The three maps are keyed by three different identity schemes, all computed at parse time:

| Map | Key | Source |
| --- | --- | --- |
| `papers` | `Paper.id` = BibTeX cite key, verbatim | `src/parser.rs:157`, `:394-396` |
| `venues` | `normalize_venue_id(paper.venue)` | `src/parser.rs:332-334`, `:489-495` |
| `scholars` | `generate_scholar_id(normalize_author_name(author))` | `src/parser.rs:353-356`, `:497-503` |

Local `Paper.id` remains the cite key. Different keys can describe the same work; downstream `S2agCitations::unique_papers` groups matched aliases by corpus id and unmatched papers by local id. It prefers a nonempty venue, DOI presence, year presence, then author count, finally smallest id. The same identity policy supports graph endpoints, scoring, counts and publication cards. Without a corpus match, different local keys cannot be recognized as aliases.

### Edges populated by snapshot integration

```rust
pub struct CitationEdge {
    pub from: String,
    pub to:   String,
    pub weight: f64,
    pub year: Option<u32>,
    pub cross_venue: bool,
}
```

The parser initializes `CitationGraph.edges` empty because BibTeX supplies no references. Before default CLI or site ranking, the local S2AG graph loader populates edges and both paper adjacency lists. It deterministically canonicalizes BibTeX aliases for each corpus id and inserts unique directed edges once. Whole-S2 citation totals come from the separate count artifact, not `graph.edges.len()`; see Chapter 6.

## 3.2 The entity structs

### Paper

```rust
pub struct Paper {
    pub id: String,            // = BibTeX cite key
    pub title: String,
    pub authors: Vec<String>,  // display names, NOT normalized
    pub venue: String,         // extracted venue name
    pub venue_type: VenueType,
    pub year: Option<u32>,
    pub month: Option<String>,
    pub citations: Vec<String>, // paper IDs this paper cites
    pub cited_by: Vec<String>,  // paper IDs citing this
    pub abstract_text: Option<String>,
    pub keywords: Vec<String>,
    pub doi: Option<String>,
    pub dblp_id: Option<String>, // exact DBLP record key from BibTeX dblp
    pub url: Option<String>,
    pub bib_key: String,        // duplicate of id
    pub source_file: String,    // e.g. "sosp.bib"
}
```

`authors` contains cleaned human-order display names, not raw BibTeX syntax. `parse_authors` handles `Last, First` and `Last, Suffix, First`, preserves grouped corporate names and numeric DBLP disambiguators, and splits `and` only outside brace groups. Scholar-key normalization remains a separate lexical step. Citation lists start empty and are enriched by snapshot loading; `bib_key` duplicates `id`, while `source_file` records provenance.

### VenueType

```rust
pub enum VenueType {
    Conference, Journal, Workshop, Symposium, TechnicalReport, Unknown,
}
```

(`src/models.rs`; derives `Copy`, `PartialEq`, `Eq`, `Hash`.) `parse_entry` sets it from the BibTeX *entry type* string:

| BibTeX entry type | `VenueType` |
| --- | --- |
| `inproceedings`, `conference` | `Conference` |
| `article` | `Journal` |
| `inworkshop`, `workshop` | `Workshop` |
| `symposium` | `Symposium` |
| `techreport` | `TechnicalReport` |
| anything else | `Unknown` |


Technical reports use their cleaned `institution` as the venue when no explicit
venue is supplied. The institution is retained in full rather than shortened
as if it were a conference name.

### Venue

```rust
pub struct Venue {
    pub id: String,
    pub name: String,
    pub full_name: String,
    pub venue_type: VenueType,
    pub papers: Vec<String>,   // paper IDs
    pub pagerank: f64,
    pub impact_factor: f64,
    pub field: String,
}
```

The parser creates `name` and `full_name` from the venue string and initializes score fields to zero. `get_venue_field` classifies its field. The calculator returns computed metrics and ranking structs rather than mutating these model score fields. In published rankings `pagerank` is normalized, unadjusted PageRank (Venue Score), and `impact_factor` is Venue Score times `ln(unique venue works + 1)` (Count-Scaled Score), not a journal impact factor.

### Scholar

```rust
pub struct Scholar {
    pub id: String,
    pub name: String,            // first-seen display name
    pub normalized_name: String,
    pub papers: Vec<String>,     // paper IDs
    pub affiliations: Vec<String>,
    pub qindex: f64,
    pub h_index: usize,
    pub citation_count: usize,
    pub publications_by_venue: HashMap<String, Vec<String>>, // venue ID -> paper IDs
    pub coauthors: HashMap<String, usize>,                   // scholar ID -> count
}
```

Model score fields start at zero and are not overwritten by the calculator, which returns separate ranking structs. Display names are chosen while rebuilding indexes from final records in paper-id order. Affiliations remain unpopulated. Publication and coauthor indexes include each normalized author once per local paper; overwritten cite-key records cannot leave ghost scholars or stale venue memberships.

## 3.3 Parameters and ranking output structs

`AlgorithmParams` and its `Default` implementation in `src/models.rs` configure `PageRankCalculator`:

| Field | Default | Meaning |
| --- | --- | --- |
| `damping_factor` | `0.85` | PageRank damping $d$ |
| `max_iterations` | `100` | power-iteration cap |
| `tolerance` | `1e-6` | convergence threshold |
| `venue_weight` | `0.7` | venue-vs-other weight (declared) |
| `year_decay` | `0.95` | recency decay base |
| `reference_year` | current UTC year; valid loaded snapshot release year overrides it | scholar recency reference |

`venue_weight` is retained but unused by the current algorithm. `year_decay` discounts citation age directly and scholar age divided by five. `QIndexMetrics` carries scores, processing timestamp, algorithm name and effective parameters. Snapshot release date is separate provenance.

`VenueRanking` carries id, name, field, normalized `pagerank`, count-scaled `impact_factor` and unique paper count. `ScholarRanking` carries id, name, QIndex, eligible publication count, snapshot citation total, eligible h-index and top venues. The eligible set is deduplicated before checking representative author membership and CSRankings venue eligibility. Profiles distinguish broader `all_indexed` metrics from this `ranking` scope; ranking/search consumers use computed rankings, not the metadata graph's initial score fields.

## 3.4 Identity, normalization, and classification

### Author-name normalization

`normalize_author_name` (`src/models.rs:334-357`) maps a raw author string to a canonical display form, and `generate_scholar_id` (`src/parser.rs:497-503`) then maps that to the map key. The pipeline:

1. Trim and lowercase the input.
2. If a comma is present, treat it as `Last, First` and reorder to `First Last` (splitting at the comma and swapping the still-lowercase halves).
3. Remove `.` and replace `-` with a space.
4. Title-case each whitespace-separated word (uppercase the first character).

`generate_scholar_id` then lowercases, replaces spaces with `_`, drops `.`, and replaces `-` with `_`. Worked example (illustrative, traced through the code):

```
"Smith, John A."
  -> normalize_author_name -> "John A Smith"
  -> generate_scholar_id   -> "john_a_smith"
```

This is purely lexical. It does not disambiguate distinct people who share a normalized name, nor unify a person who publishes under variants that normalize differently (e.g. with and without a middle initial). The comma branch is correct only because the later title-case pass fixes the casing of the reordered halves.

### Venue-ID normalization

`normalize_venue_id` (`src/parser.rs:489-495`) produces the venue map key: trim, uppercase, replace ` ` and `-` with `_`, and strip apostrophes. Example: `USENIX ATC` → `USENIX_ATC`. Before this runs, the raw `booktitle`/`journal` value passes through `expand_string` and `extract_venue_name` (3.5), so the quality of venue grouping depends on those producing a consistent surface name.

### Venue field

`get_venue_field` (`src/models.rs:391-468`) classifies a venue into a CSRankings-style subarea by returning the first field whose keyword list has a substring match. There are 35 keyword→field rows covering roughly 90 venue tokens; a representative subset:

| Field | Example venue tokens |
| --- | --- |
| Operating Systems | SOSP, OSDI, EUROSYS, ATC, FAST, VEE, HOTOS |
| Computer Networks | SIGCOMM, NSDI, CONEXT, IMC |
| Computer Security | CCS, SECURITY, OAKLAND, NDSS, USENIXSEC |
| Databases | SIGMOD, VLDB, ICDE, PODS, EDBT, CIDR |
| Computer Architecture | ASPLOS, ISCA, MICRO, HPCA |
| Programming Languages | PLDI, POPL, ICFP, OOPSLA |
| Machine Learning & Data Mining | ICML, NEURIPS, NIPS, ICLR |
| Algorithms & Complexity | STOC, FOCS |
| Parallel & Distributed Computing | PODC, SPAA, DISC |
| Software Engineering | FSE, ICSE, ASE, ISSTA |

If no keyword matches but the venue contains JOURNAL/TRANSACTIONS, a secondary sub-keyword pass maps DATABASE→Databases, NETWORK→Computer Networks, PARALLEL/DISTRIBUTED→Parallel & Distributed Computing, SOFTWARE→Software Engineering, COMPUTER→Computer Systems. The final fallback for anything unmatched is `"General"`. Substring matching means short keywords can mis-fire. This automatic field taxonomy is separate from the CSRankings scholar-eligibility allowlist; neither supplies venue-score boosts.

## 3.5 From BibTeX entry to `Paper`

`BibParser::parse_directory` stages records and then builds indexes:

1. Clear parser state so reuse starts a fresh bibliography; load `strings.bib` and `title.bib` macros if present.
2. Collect `.bib` paths recursively, excluding filenames containing `title` or `strings`, then sort paths.
3. Parse records in source order. The last valid record for a repeated cite key wins, with a warning. Rejected later entries do not replace valid records.
4. `rebuild_indexes` clears venue/scholar indexes and rebuilds from final papers sorted by id, deduplicating normalized authors and coauthors per paper. Thus replaced records leave no stale membership.
5. `build_graph` copies staging maps into the runtime graph; parser citation adjacency is initially empty.

`parse_entry` (`src/parser.rs:151-219`) sets `id` = `bib_key` = the cite key, then maps tags:

| BibTeX tag | `Paper` field | Transform |
| --- | --- | --- |
| `title` | `title` | `clean_bibtex_string` |
| `author` | `authors` | `parse_authors` (split on ` and `) |
| `booktitle` | `venue` | `expand_string` → `extract_venue_name` |
| `journal` | `venue` | `expand_string` → `extract_venue_name` |
| `year` | `year` | `parse::<u32>()` (`Option`) |
| `month` | `month` | `Some(value)` |
| `doi` | `doi` | `Some(value)` |
| `url` | `url` | `Some(value)` |
| `abstract` | `abstract_text` | `clean_bibtex_string` |
| `keywords` | `keywords` | split on `,`, trim each |

All other tags are ignored. Crucially, **no tag maps to `Paper.citations`** — there is no BibTeX field for "papers this cites" in the corpus, so `citations` stays the empty vec it was initialized to (`src/parser.rs:166`).

`parse_entry` requires nonempty title and authors. It also rejects `@proceedings` and obvious volume titles: a first word “Proceedings”, or an event word (conference/symposium/workshop) together with a four-digit year and full month name. This targeted heuristic prevents proceedings editors from becoming research-paper authors; it is not a comprehensive research-paper classifier.

Three helpers support the venue resolution chain:

- `expand_string` (`src/parser.rs:221-325`) first looks up the value in the `@string` map, then in a hardcoded table of ~70 lowercase abbreviations (e.g. `pvldb`→`VLDB`, `nips`→`NeurIPS`, `sp`/`s&p`→`Oakland`), defaulting to the trimmed input.
- `extract_venue_name` (`src/parser.rs:446-487`) cleans the string, strips a trailing 4-digit year (`CHI 2019`→`CHI`), and — if the result is ≤20 characters and contains neither "Proceedings" nor "Conference" — returns it as-is. Longer strings fall back to pulling an abbreviation from parentheses or the token after `" of "`. The ≤20-char shortcut means already-abbreviated venues bypass the heavier logic, which keeps common cases clean but can leave longer raw venue strings inconsistent, affecting `normalize_venue_id` keying.
- `clean_bibtex_string` interprets supported TeX accents before removing grouping braces, unescapes common punctuation, handles selected ligatures/dashes, and decodes HTML entities. It is not a general TeX interpreter.

Venue and scholar indexes are created only after all valid final records are selected, via `rebuild_indexes`, `add_to_venue` and `add_scholars`.

## 3.6 The citation network and search methods

`build_citation_network` derives reverse adjacency from paper references. Immediately after BibTeX parsing those references are empty. The local S2AG integration subsequently supplies `Paper.citations`, `Paper.cited_by` and `CitationGraph.edges`, using only matched bibliography endpoints. Both artifacts must refer to the same snapshot release.

The low-level metadata helpers `CitationGraph::search_venues` and `search_scholars` still read graph score fields and use `partial_cmp().unwrap()`; the scholar helper picks up to three venues from a hash map, not ranked top venues. They are not the computed ranking/search surface: consumers use calculator ranking structs, with deterministic score/id ordering and eligible metrics. This distinction matters if adding callers to these helpers.

## 3.7 On-disk corpus versus runtime graph

The relationship between disk and memory is deliberately simple and one-directional:

```
bib/*.bib  --(BibParser::parse_directory)-->  CitationGraph (in memory)
   metadata records                         final-record paper/venue/scholar indexes
```

Disk counts need not equal runtime paper counts: macro files, missing essential fields, proceedings exclusions and repeated cite keys affect the roster. Venue grouping depends on normalized metadata, not the number of files. Consult generated statistics for current totals.

The parser-produced graph contains metadata only. Default ranking consumers load `cache/citations/s2ag_paper_citations.json` and `cache/citations/s2ag_citation_graph.json` before scoring. The latter enriches the runtime `CitationGraph`; the former supplies whole-S2 counts. Computed rankings remain distinct from the model's initial score fields. Chapters 5 and 6 describe the ranking and integration boundaries.
