# 3. The Data Model

QIndex is two things at once: the project (a static bibliographic analytics tool) and the *QIndex score*, a per-scholar prestige metric described in Chapter 5, "The Ranking Algorithm." This chapter is about neither the score nor the user interface. It describes the *data model* — the in-memory structures the entire system reads and writes, how a raw BibTeX entry becomes a `Paper`, and the identity and classification rules that decide how papers, venues, and scholars are grouped.

The model is defined in `src/models.rs` (467 lines) and populated by `src/parser.rs` (502 lines). Everything downstream — the ranking pipeline (`src/algorithm.rs`), the static site generator (`src/site/`), and the export module (`src/export.rs`) — operates on the `CitationGraph` produced here. There is no database: the corpus is re-parsed from disk into memory at every process start (see Chapter 7, "System Architecture").

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

Three `IndexMap`s (from the `indexmap` crate) hold the entities; a fourth field holds citation edges. `IndexMap` preserves insertion order, so iteration order is the order in which entries were first seen during parsing — relevant because some downstream code (e.g. the `top_venues` selection in 3.6) relies on iteration order rather than an explicit sort.

`CitationGraph` is the one struct in `models.rs` that does **not** derive `Serialize`/`Deserialize`; it derives only `Clone` and provides a hand-written `Debug` impl (`src/models.rs:69-78`) that prints only the four counts to avoid dumping the whole corpus. The graph is therefore not serializable as a single blob; persistence happens at a coarser granularity through the export module (Chapter 9, "The Command-Line Interface").

A live parse of the corpus in this session produced roughly **19,954 papers, 44 venues, and 43,942 scholars** (observed via `qindex stats` and the site's `data/stats.json`). Chapter 4, "The Bibliographic Database," covers the on-disk corpus behind these counts.

### Keys

The three maps are keyed by three different identity schemes, all computed at parse time:

| Map | Key | Source |
| --- | --- | --- |
| `papers` | `Paper.id` = BibTeX cite key, verbatim | `src/parser.rs:157`, `:394-396` |
| `venues` | `normalize_venue_id(paper.venue)` | `src/parser.rs:332-334`, `:489-495` |
| `scholars` | `generate_scholar_id(normalize_author_name(author))` | `src/parser.rs:353-356`, `:497-503` |

Paper identity is whatever the BibTeX author chose as the cite key; QIndex does no de-duplication across keys, so two entries for the same paper under different keys become two `Paper`s. Venue and scholar identity are *derived* by the normalization functions in 3.4.

### Edges (declared, not populated)

```rust
pub struct CitationEdge {
    pub from: String,
    pub to:   String,
    pub weight: f64,
    pub year: Option<u32>,
    pub cross_venue: bool,
}
```

`CitationGraph.edges` is initialized empty in `CitationGraph::new()` (`src/models.rs:151-156`) and **is never populated** anywhere in `models.rs` or `parser.rs`; the `weight`, `year`, and `cross_venue` fields are consequently dead on the parser path. The dashboard, the statistics page, and the site's `data/stats.json` report `total_citations = graph.edges.len()`, which is why they show `total_citations = 0` against a 20k-paper corpus. Real citation *counts* enter the system through a separate module (`src/s2ag_citations.rs`), covered in Chapter 6, "Citation Data Integration"; they do not become edges in this graph.

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
    pub url: Option<String>,
    pub bib_key: String,        // duplicate of id
    pub source_file: String,    // e.g. "sosp.bib"
}
```

(`src/models.rs:5-22`.) Two fields warrant emphasis. `authors` stores the *raw* display strings split from the BibTeX `author` field; normalization happens only when scholars are built (3.4), so the display name and the scholar key can differ. `citations` and `cited_by` are both initialized empty and stay empty on the parser path (3.5). `bib_key` is a redundant copy of `id`, and `source_file` records the originating filename for provenance.

### VenueType

```rust
pub enum VenueType {
    Conference, Journal, Workshop, Symposium, Unknown,
}
```

(`src/models.rs:24-31`; derives `Copy`, `PartialEq`, `Eq`, `Hash`.) It is set in `parse_entry` from the BibTeX *entry type* string (`src/parser.rs:205-211`):

| BibTeX entry type | `VenueType` |
| --- | --- |
| `inproceedings`, `conference` | `Conference` |
| `article` | `Journal` |
| `inworkshop`, `workshop` | `Workshop` |
| `symposium` | `Symposium` |
| anything else | `Unknown` |

Note this is an *independent* signal from the tier classifier in 3.4, which separately string-matches `WORKSHOP`/`HOT` to assign tier B. The two can disagree (a `@inproceedings` entry at a "HotX" venue is `VenueType::Conference` but tier B).

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
    pub tier: String,          // "A*" | "A" | "B" | "C"
    pub field: String,
}
```

(`src/models.rs:33-44`.) On first creation (`src/parser.rs:334-346`) `name` and `full_name` are both set to `paper.venue` (there is no separate long form); `pagerank` and `impact_factor` are `0.0`; `tier` and `field` come from `get_venue_tier`/`get_venue_field` (3.4). The zero-initialized score fields are filled later by the ranking step; until then, sorting by `pagerank` ranks every venue equally — and would panic on a NaN, since the comparator uses `partial_cmp().unwrap()` (see 3.6).

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

(`src/models.rs:46-58`.) At creation (`src/parser.rs:356-368`) `qindex`, `h_index`, and `citation_count` are zero, and they stay zero: the ranking step (Chapter 5) returns its results in separate `ScholarRanking` structs and never writes them back into the graph. `name` is the *first* raw display string seen for that scholar key, so a scholar who appears as both "J. Smith" and "John Smith" keeps whichever was parsed first. `affiliations` is declared but **never populated** by the parser — it is always an empty `Vec`. `publications_by_venue` groups the scholar's papers by venue ID, and `coauthors` accumulates collaboration counts (3.4).

## 3.3 Parameters and ranking output structs

`AlgorithmParams` (`src/models.rs:98-106`) carries the tunables for the ranking pipeline; its `Default` impl (`src/models.rs:108-125`) is the configuration actually used (`PageRankCalculator::new`, `src/algorithm.rs:75`):

| Field | Default | Meaning |
| --- | --- | --- |
| `damping_factor` | `0.85` | PageRank damping $d$ |
| `max_iterations` | `100` | power-iteration cap |
| `tolerance` | `1e-6` | convergence threshold |
| `venue_weight` | `0.7` | venue-vs-other weight (declared) |
| `year_decay` | `0.95` | recency decay base |
| `tier_bonus` | `{A*:2.0, A:1.5, B:1.2, C:1.0}` | per-tier multiplier |

The semantics and the (partly dead) code paths that consume these values are Chapter 5's subject. `QIndexMetrics` (`src/models.rs:89-96`) is a snapshot container (`venue_scores`, `scholar_scores`, `timestamp`, `algorithm`, `parameters`).

Two flattened "ranking" structs are the CLI and website surface of the model. `VenueRanking` (`src/models.rs:127-136`: `id`, `name`, `tier`, `field`, `pagerank`, `impact_factor`, `paper_count`) and `ScholarRanking` (`src/models.rs:138-147`: `id`, `name`, `qindex`, `h_index`, `paper_count`, `citation_count`, `top_venues`) are produced by `PageRankCalculator::get_top_venues`/`get_top_scholars` (Chapter 5) with computed scores, and by `search_venues`/`search_scholars` (3.6) with the graph's never-filled score fields.

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

### Venue tier

`get_venue_tier` (`src/models.rs:359-389`) uppercases the venue and assigns a tier by **substring** match against two hardcoded lists, with journal and workshop fallbacks:

| Tier | Rule |
| --- | --- |
| **A\*** | venue contains any of: SOSP, OSDI, SIGMOD, VLDB, PLDI, POPL, SIGCOMM, NSDI, ASPLOS, ISCA, MICRO, FAST, EUROSYS, ATC, PODC, SPAA, CCS, SECURITY, OAKLAND, STOC, FOCS, SODA (22 tokens) |
| **A** | else contains any of: SOCC, DSN, ICDCS, IPDPS, CIDR, ICDE, EDBT, VEE, PPOPP, PACT, HPDC, SC, CONEXT, INFOCOM, IMC, OOPSLA, ECOOP, ICSE, DISC, OPODIS (20 tokens); or contains JOURNAL/TRANSACTIONS *and* ACM/IEEE |
| **B** | else contains JOURNAL/TRANSACTIONS (without ACM/IEEE); or contains WORKSHOP/HOT |
| **C** | everything else (default) |

Because matching is `contains()` on the uppercased name, short tokens can false-match longer venue strings (for example `SC` or `EC`). The `C` default absorbs anything unmatched.

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

If no keyword matches but the venue contains JOURNAL/TRANSACTIONS, a secondary sub-keyword pass maps DATABASE→Databases, NETWORK→Computer Networks, PARALLEL/DISTRIBUTED→Parallel & Distributed Computing, SOFTWARE→Software Engineering, COMPUTER→Computer Systems (`src/models.rs:452-465`). The final fallback for anything unmatched is `"General"`. As with the tier classifier, substring matching means short keywords can mis-fire.

## 3.5 From BibTeX entry to `Paper`

The parser is staged. `BibParser` (`src/parser.rs:15-20`) holds plain `HashMap`s for papers, venues, scholars, plus a `strings` map for `@string` macro definitions. `parse_directory` (`src/parser.rs:32-97`) drives the flow:

1. If `strings.bib` or `title.bib` exist, parse their `@string` macros via `parse_strings` (`src/parser.rs:99-114`, a regex over `@string{key="value"}`).
2. Recursively collect every `.bib` file under the directory, then **filter out** any file whose name *contains* the substring `title` or `strings` (a substring test, not an exact-filename test, so an unrelated file like `my_titles.bib` would also be skipped).
3. Parse each surviving file with `parse_file` (`src/parser.rs:116-149`), which uses `nom_bibtex` and calls `parse_entry` per entry. Parse failures are logged as warnings, not treated as fatal.
4. Copy the staging `HashMap`s into the `CitationGraph` `IndexMap`s via `build_graph` (`src/parser.rs:390-409`), then call `build_citation_network` (3.6).

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

A `Paper` is returned only if **both `title` and `authors` are non-empty** (`src/parser.rs:214-218`); otherwise the entry is silently dropped. This is the principal reason the parsed paper count is below the on-disk entry count: minimal entries with only a `booktitle` and `year` (common in this corpus, per Chapter 4) lack an `author` field and are discarded.

Three helpers support the venue resolution chain:

- `expand_string` (`src/parser.rs:221-325`) first looks up the value in the `@string` map, then in a hardcoded table of ~70 lowercase abbreviations (e.g. `pvldb`→`VLDB`, `nips`→`NeurIPS`, `sp`/`s&p`→`Oakland`), defaulting to the trimmed input.
- `extract_venue_name` (`src/parser.rs:446-487`) cleans the string, strips a trailing 4-digit year (`CHI 2019`→`CHI`), and — if the result is ≤20 characters and contains neither "Proceedings" nor "Conference" — returns it as-is. Longer strings fall back to pulling an abbreviation from parentheses or the token after `" of "`. The ≤20-char shortcut means already-abbreviated venues bypass the heavier logic, which keeps common cases clean but can leave longer raw venue strings inconsistent, affecting `normalize_venue_id` keying.
- `clean_bibtex_string` (`src/parser.rs:412-434`) trims, strips one level of outer braces, and applies LaTeX replacements. It replaces `\'` with a bare quote *before* the accent rules (`\'a`→`á`, etc.), so the accent replacements are largely unreachable — a known wart.

After a paper is built, `add_to_venue` (`src/parser.rs:327-349`) and `add_scholars` (`src/parser.rs:351-388`) update the aggregates. `add_to_venue` keys on `normalize_venue_id(paper.venue)`, creating the `Venue` (with tier/field computed) on first sight and appending the paper ID. `add_scholars` normalizes each author, creates the `Scholar` on first sight (storing the raw author string as `name`), appends the paper to `papers` and to `publications_by_venue[venue_id]`, and increments `coauthors[other_author_id]` for every co-author. Neither de-duplicates, so an author listed twice on one entry is double-counted.

## 3.6 The citation network and search methods

`build_citation_network` (`src/models.rs:174-192`) runs at the end of `parse_directory`. For each `cited_id` in each paper's `citations` that resolves to an existing paper, it pushes the citing paper's ID into the cited paper's `cited_by`. Because `citations` is always empty after BibTeX parsing, this loop traverses nothing and `cited_by` stays empty. Citation data must therefore be injected from outside the parser; today that is the S2AG per-paper *count* path of Chapter 6, which does not feed this graph.

`search_venues` (`src/models.rs:194-216`) filters venues by a query substring and emits `VenueRanking`s sorted descending by `pagerank` via `partial_cmp().unwrap()` — safe while all values are real numbers, but a latent panic on NaN. `search_scholars` (`src/models.rs:218-251`) does the analogous thing for scholars, sorted by `qindex`. Its `top_venues` field is built by iterating `scholar.publications_by_venue` — a `HashMap`, hence unordered — and breaking after three entries that resolve in `self.venues`. Despite the name, these are *arbitrary* venues, not the scholar's most-published or highest-ranked ones (`src/models.rs:225-235`). Both methods copy `pagerank`, `qindex`, and `h_index` from the graph, where they are always zero (3.2), so their scores are zero too. Today only the CLI `search` subcommand calls them; the website's search (Chapter 8) runs in the browser over an index built from the computed rankings instead.

## 3.7 On-disk corpus versus runtime graph

The relationship between disk and memory is deliberately simple and one-directional:

```
bib/*.bib  --(BibParser::parse_directory)-->  CitationGraph (in memory)
   ~21k entries                                  ~19,954 papers / 44 venues / 43,942 scholars
```

The `bib/` directory holds roughly 21,000 `@inproceedings`/`@article`/`@techreport` entries across ~44 venue files (Chapter 4 details the layout and per-venue counts). Parsing is performed fresh at every CLI invocation and every site build; the `CitationGraph` is never written back to disk. The gap between on-disk entries and in-memory papers is explained by the `title`-and-`authors`-required filter in `parse_entry` (3.5) plus the `title`/`strings` filename filter in `parse_directory`.

External citation artifacts in `cache/citations/*.json` are a *separate* concern, loaded by other modules at different points (Chapters 5 and 6), and are not part of the `CitationGraph` produced here. The score fields on `Venue` and `Scholar` (`pagerank`, `impact_factor`, `qindex`, `h_index`, `citation_count`) look like the seams where the ranking step (Chapter 5) would write its results back into this model, but nothing writes them: computed scores live only in the `VenueRanking`/`ScholarRanking` values the calculator returns.
