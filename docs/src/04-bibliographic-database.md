# 4. The Bibliographic Database

QIndex names two things at once: the project as a whole, and the per-scholar
prestige metric the project computes. This chapter is about the raw material both
depend on — the `bib/` corpus of BibTeX files that the parser turns into the
in-memory `CitationGraph` described in
[Chapter 3, The Data Model](03-data-model.md). It covers how the corpus is
laid out, how papers are fetched and cleaned, what the corpus actually contains
today, and where its quality is uneven. The ranking that consumes this data is
covered in [Chapter 5, The Ranking Algorithm](05-ranking-algorithm.md), and
the external citation feeds in
[Chapter 6, Citation Data Integration](06-citation-data-integration.md).

## 4.1 Corpus layout: one file per venue

The corpus follows a single rule: one BibTeX file per venue, named in lowercase
after the venue (`sosp.bib`, `osdi.bib`, `sigmod.bib`, `icml.bib`). The `bib/`
directory contains venue files plus macro definitions. Each entry is placed in
its venue file and sorted oldest-to-newest by year.

Conference papers use `@inproceedings`; journal papers use `@article`. Historical
entry counts below describe the earlier corpus snapshot recorded by this book,
not the cleaned 2026-10-10 roster. Consult current parser exports and generated
site statistics for current totals. A typical minimal entry from `bib/sosp.bib`:

```bibtex
@inproceedings{lampson83hints,
author = {Lampson, Butler W.},
title = {Hints for computer system design},
booktitle = "SOSP",
year = {1983},
month =  oct,
}
```

The cite key (`lampson83hints`) becomes the `Paper.id`, and the `booktitle`
string drives venue extraction. Many entries carry no `doi`, `url`, or
`abstract`; missing these fields is allowed. `parse_entry` requires a title and
at least one author, and rejects proceedings volumes. Repeated cite keys are
resolved before indexes are built (Section 4.5).

### Coverage by area

Coverage spans systems, databases, machine learning, networking, security,
programming languages and theory, but is uneven. For historical context, the
earlier book snapshot (before the 2026-10-10 cleanup) recorded:

| Venue | `@inproceedings` | Area |
|-------|-----------------:|------|
| `ccs.bib`     | 2,789 | Security |
| `stoc.bib`    | 1,923 | Theory |
| `sigcomm.bib` | 1,687 | Networks |
| `neurips.bib` | 1,418 | Machine learning |
| `icde.bib`    | 1,402 | Databases |

That historical snapshot also recorded small samples such as `cvpr.bib` (5),
`cidr.bib` (11) and `ndss.bib` (269 bytes). These are dated examples, not current
counts or evidence of the real conference size. The working corpus is not a census.
Further limitations are tracked in
[Chapter 11, Limitations, Known Issues, and Roadmap](11-limitations-and-roadmap.md).

The parser supplies metadata, not references. Default CLI and site consumers
then load the local S2AG count and graph artifacts. Whole-S2 counts supply
reported totals; bibliography-only directed edges supply venue PageRank.
Current roster, match coverage and edge totals belong in generated statistics
and extraction summaries, not frozen chapter figures (Chapter 6).

## 4.2 Special files

Not every file in `bib/` is a venue database.

- **`strings.bib`** — `@string` macros that map lowercase abbreviations to
  display names, e.g. `@string{osdi = "OSDI"}` and
  `@string{atc = "USENIX ATC"}`. The parser loads these first so that a
  `booktitle = osdi` reference can be expanded to `"OSDI"` during parsing
  (`src/parser.rs:99-114`). The macro parser is regex-based and only matches the
  `@string{key="value"}` form with double-quoted values.
- **`title.bib`** — if present, also parsed as `@string` definitions
  (`src/parser.rs:32-97`).
- **Journal files** — venues whose entries are `@article`, e.g. `tods.bib`,
  `tos.bib`, `sigmodrecord.bib`, `jacm.bib`, `cacm.bib`, `toplas.bib`.
- **`tr.bib`** — technical reports (`@techreport`); the parser preserves the
  issuing `institution` as the venue rather than misclassifying the report as
  a conference or displaying an empty venue.
- **`arxiv.bib`** — ArXiv preprints.

A subtle behavior worth flagging: `parse_directory` excludes `strings.bib` and
`title.bib` from the paper-parsing pass using a *substring* match on the file
name (`name.contains("title")` / `name.contains("strings")`,
`src/parser.rs:32-97`), not an exact filename comparison. Any future `.bib` whose
name merely contains those substrings would be excluded from paper parsing as
well. The corpus also contains non-`.bib` working artifacts checked into the tree
— `bib/sigcomm.bib.backup` and `bib/sigcomm.removed` — which are byproducts of
the cleanup process in Section 4.3 and are not parsed (the parser only walks
files whose extension is `bib`).

## 4.3 The fetching workflow

New papers are added with Python scripts that query DBLP, filter out
non-research entries, and append to the appropriate venue file.

### Main-conference-only principle

The governing rule is that QIndex tracks **main-conference papers only** —
not workshops, demos, posters, doctoral-symposium papers, tutorials, or
co-located events. `scripts/fetch_main_conference.py` is the primary fetcher. It
takes a conference and a year and queries the DBLP publication-search API
(`https://dblp.org/search/publ/api`), trying three query patterns in order
(`scripts/fetch_main_conference.py:67-72`):

```text
1. toc:db/conf/{c}/{c}{year}.bht:     # direct proceedings table-of-contents
2. venue:{C} year:{year}              # venue + year filter
3. {C} {year}                         # general search fallback
```

It requests up to 200 results, filters each candidate, requires the venue string
to contain the conference name, deduplicates by lowercased title, and appends new
`@inproceedings` entries (skipping cite keys already present) to `bib/{c}.bib`
with a dated comment header.

### Workshop / demo / poster filtering

`is_workshop_or_poster()` (`scripts/fetch_main_conference.py:32-47`) rejects an
entry if its title or venue contains any of `Workshop`, `Poster`, `Demo`,
`Doctoral`, `Tutorial`, `Panel`, `Keynote`, or `Proceedings of`, or if the venue
contains `@` (as in `eBPF@SIGCOMM`) or `Co-located`.

For SIGCOMM, which historically co-located many workshops, a stricter,
venue-specific cleaner exists. `scripts/deep_clean_sigcomm.py` hardcodes
`bib/sigcomm.bib` and applies an expanded `is_workshop_paper()`
(`scripts/deep_clean_sigcomm.py:40-84`) that additionally rejects `Symposium`,
the `Demonstration of` / `Poster:` / `Demo:` / `Keynote:` prefixes, the `Hot*`
workshop family (HotNets, HotOS, HotSDN, HotMiddlebox, HotCloud, HotPlanet), and
named SIGCOMM workshops (FOCI, NAI, SPIN, WiNTECH, CloudNet, and others). Kept
entries are written back with a comment header; removed entries are saved to
`bib/sigcomm.removed` for review. The presence of `bib/sigcomm.bib.backup` and
`bib/sigcomm.removed` is direct evidence of past cleanup runs.

### Sorting

After fetching and cleaning, `scripts/sort_papers_by_year.py` re-sorts every file
oldest-to-newest. It regex-extracts `year = {YYYY}` (defaulting to 0 when
missing), preserves the leading `%` comment header, and rewrites the file in
place (`scripts/sort_papers_by_year.py:10-101`).

> **Operational caveat.** `sort_papers_by_year.py` hardcodes its directory to a
> macOS path, `/Users/shuai/workspace/qindex/bib`
> (`scripts/sort_papers_by_year.py:72`), which does not exist on the current
> Linux host (`/home/users/shuai/q-index`). The path must be edited before the
> script will run here; as shipped it would glob an empty directory.

A full add-a-conference workflow therefore looks like:

```bash
python3 scripts/fetch_main_conference.py icse 2024
python3 scripts/deep_clean_sigcomm.py      # only for SIGCOMM cleanup
python3 scripts/sort_papers_by_year.py     # fix its hardcoded path first
cargo build --release && cargo run --release -- build-site
python3 scripts/check_site_links.py site /
```

Build and run details are covered in
[Chapter 10, Building, Running, and Deployment](10-building-running-deployment.md).

## 4.4 From files to venues: keying and grouping

File count and runtime venue count differ because venues are keyed by metadata,
not filenames. `expand_string` resolves macros and known abbreviations;
`extract_venue_name` derives the display name, then `normalize_venue_id` folds
it into a key:

```text
trim -> UPPERCASE -> ' ' => '_' -> '-' => '_' -> drop apostrophes
"USENIX ATC" -> "USENIX_ATC"
```

Papers are grouped by this normalized key into the `venues` `IndexMap`. As a
result, two files can collapse into one venue (for example if both resolve to the
same normalized name), and tiny stub files whose entries all fail the
title-and-author requirement contribute no papers and therefore no venue at all.
Either effect can make the venue count smaller than the file count.

The venue-name extraction has a known sharp edge:
`extract_venue_name` strips a trailing four-digit year token and then short-
circuits, returning the string as-is when it is 20 characters or shorter and
contains neither `Proceedings` nor `Conference`. Longer or irregular
`booktitle` strings take a different path and can yield inconsistent venue names,
which in turn produces inconsistent normalized keys and can split what should be
one venue across several keys. This is the parser-side counterpart to the corpus
hygiene the fetch scripts try to maintain.

## 4.5 Data-quality considerations

The corpus is honest working data with several known quality issues that anyone
extending QIndex should keep in mind.

**Parse drop-off.** Macro files are not papers; missing titles/authors,
proceedings-volume exclusions and repeated cite keys reduce the final roster.
The parser rejects `@proceedings`, titles starting with “Proceedings”, and
titles containing an event word, four-digit year and full month name. This
heuristic excludes obvious volumes but does not certify every remaining entry
as a main-conference research paper. Venue collapses reduce venue count, not
paper count.

**Record replacement and work identity.** Paths are sorted and entries read in
source order; the last valid duplicate cite key wins with a warning. Venue and
scholar indexes rebuild from final records in paper-id order, so overwritten
records leave no ghost authors or memberships. Each normalized author and
coauthor is indexed once per local paper. Different cite keys remain separate
metadata records, but shared S2 corpus ids deduplicate scoring, eligible counts,
h-indices, graph endpoints and publication listings. Representatives prefer
venue, DOI, year and author completeness, then smallest local id. Unmatched
different-key aliases remain unresolved. Source cleanup still matters:
deduplication does not repair incorrect author metadata. Two publisher-confirmed
wrong-author aliases were removed from OSDI/SOSP during the 2026-10-10 cleanup;
this is evidence correction, not a change to the authorship policy.

**SIGCOMM workshop residue.** SIGCOMM required a dedicated deep-clean pass
(Section 4.3) precisely because the general filter let workshop papers through.
The `.backup` and `.removed` artifacts document that this cleanup happened, but
they also mean the SIGCOMM file's history is encoded in side files rather than in
version control alone.

**Scholar identity.** `Paper.authors` stores cleaned human-order names:
TeX grouping braces, supported accents and HTML entities are cleaned; BibTeX
comma forms are reordered, while corporate names and DBLP numeric suffixes
are preserved. `normalize_author_name` and `generate_scholar_id` remain lexical
heuristics, not an authority file. Shared names can merge distinct people and
variants can split one person. S2 author metadata can itself be incomplete or
wrong; there is no automatic author-ID merging or inferred affiliation.

**Citation data is not in the corpus.** BibTeX parsing initializes citation
adjacency empty. The local S2AG release `2026-10-06` subsequently supplies two
committed artifacts: `cache/citations/s2ag_paper_citations.json` for whole-S2
snapshot counts, and `cache/citations/s2ag_citation_graph.json` for directed edges
between matched bibliography papers. DOI, exact DBLP ID, or normalized title/year matching joins
the snapshot to the parser's paper ids. Aliases are deduplicated by corpus id for
counts and graph edges. The default CLI and site rank using that internal graph;
coverage and edge totals are recorded in extraction outputs. See Chapter 6.

## 4.6 Summary

The bibliographic database is a per-venue collection of BibTeX files, populated
by DBLP-backed fetch scripts that enforce a main-conference-only policy through
explicit workshop/demo/poster filters, then sorted by year. It parses into the
`CitationGraph` of Chapter 3, but only the paper/venue/scholar nodes; the corpus
carries no citation edges, so prestige and per-scholar QIndex are computed from
structure and external caches rather than from citations in the `bib/` files
themselves. The corpus is real but uneven — a few very large venues, several stub
files, name-based scholar identities, and a measurable parse drop-off from raw
entries to parsed papers — and those properties bound what the downstream
ranking can claim.
