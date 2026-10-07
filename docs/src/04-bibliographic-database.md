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
directory currently holds 49 `.bib` files. Each entry is placed in the file for
its venue, and entries within a file are sorted oldest-to-newest by year.

Conference papers use `@inproceedings`; journal papers use `@article`. Across the
corpus the entry-type totals are 20,898 `@inproceedings`, 122 `@article`,
8 `@techreport`, and 47 `@string` macros — roughly 21,000 BibTeX entries in
total. Entries are deliberately minimal; a typical record carries only a cite
key, title, author, `booktitle`, and `year`, as in this entry from
`bib/sosp.bib`:

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
`abstract`; the parser tolerates this and only requires a non-empty title and at
least one author (`src/parser.rs:214-218`). Entries that fail that test are
dropped silently, which is the main reason the ~21,000 raw entries parse down to
fewer papers (see Section 4.5).

### Coverage by area

The 49 files span systems, databases, machine learning, networking, security,
programming languages, and theory. The corpus is heavily skewed toward a handful
of large venues:

| Venue | `@inproceedings` | Area |
|-------|-----------------:|------|
| `ccs.bib`     | 2,789 | Security |
| `stoc.bib`    | 1,923 | Theory |
| `sigcomm.bib` | 1,687 | Networks |
| `neurips.bib` | 1,418 | Machine learning |
| `icde.bib`    | 1,402 | Databases |

At the other end, several "venue" files are tiny stubs that do not reflect the
real size of those conferences — for example `cvpr.bib` (5 entries),
`cidr.bib` (11), and `ndss.bib` (a 269-byte file). The dataset is uneven and
incomplete; it is a working corpus, not a comprehensive census. Detailed
limitations are tracked in
[Chapter 11, Limitations, Known Issues, and Roadmap](11-limitations-and-roadmap.md).

When the running server parses this corpus it reports, via `GET /api/stats`,
roughly 19,954 papers, 44 venues, and 43,942 scholars. The gap between 49 files
and 44 venues, and between ~21,000 entries and ~19,954 papers, is explained in
Sections 4.4 and 4.5. The same endpoint reports `total_citations = 0`, because
the in-memory citation graph is essentially empty — `CitationGraph.edges` is
initialized empty and never populated by the parser path
(`src/models.rs:174-192`). Citation *counts* shown per paper come from a separate
S2AG cache, not from this corpus; that path is detailed in Chapter 6.

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
  `jacm.bib`, `cacm.bib`, `toplas.bib`. These begin with `@article` records.
- **`tr.bib`** — technical reports (`@techreport`).
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
cargo build --release && cargo run --release -- web
```

Build and run details are covered in
[Chapter 10, Building, Running, and Deployment](10-building-running-deployment.md).

## 4.4 From files to venues: keying and grouping

The number of files (49) and the number of venues the server reports (44) differ
because venues are not keyed by filename. During parsing, each paper's venue
display name is derived from its `booktitle`/`journal` field through
`expand_string` (which consults `strings.bib` macros and a hardcoded
~70-entry abbreviation map) and `extract_venue_name`
(`src/parser.rs:221-325, 446-487`). The resulting display name is then folded
into a venue key by `normalize_venue_id` (`src/parser.rs:489-495`):

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

**Parse drop-off (entries vs. parsed papers).** The ~21,000 raw BibTeX entries
parse to roughly 19,954 papers. The two leading causes are: (1) the 47 `@string`
macros are not papers and are filtered out of the paper pass; and (2)
`parse_entry` returns `None` for any entry lacking a title *or* lacking authors
(`src/parser.rs:214-218`), and many minimal stub entries carry only
`booktitle` + `year`. Such records are dropped without aborting the parse —
`parse_file` logs a warning and continues (`src/parser.rs:116-149`). The
remaining gap reflects these dropped entries plus venue collapses.

**Possible duplicates.** Neither the fetch scripts' title-based deduplication nor
the parser guarantees a globally unique paper set. The parser keys papers by cite
key, so two entries with the same cite key in different files would collide; and
when building venues and scholars, paper IDs are appended to `Venue.papers` and
`Scholar.papers` with no deduplication (`src/parser.rs:327-388`). If the same
author string appears twice on one paper, that paper can be double-counted in the
scholar's publication list. The fetch scripts dedup only within a single
fetch run, by lowercased title, against existing cite keys — cross-venue or
cross-run duplicates are not detected.

**SIGCOMM workshop residue.** SIGCOMM required a dedicated deep-clean pass
(Section 4.3) precisely because the general filter let workshop papers through.
The `.backup` and `.removed` artifacts document that this cleanup happened, but
they also mean the SIGCOMM file's history is encoded in side files rather than in
version control alone.

**Scholar identity.** Author names are stored raw in `Paper.authors`;
normalization to a scholar key happens only when building the scholar map, via
`normalize_author_name` then `generate_scholar_id` (e.g. `Smith, John A.` ->
`John A Smith` -> `john_a_smith`). This is a string heuristic, not an authority
file: it does not disambiguate distinct researchers who share a name, nor merge
the same researcher across name variants. The ~43,942 scholars the server reports
are therefore name-derived identities, not verified individuals.

**Citation data is not in the corpus.** BibTeX entries here carry no citation
edges. `Paper.citations` is initialized empty and never written from any BibTeX
field, so `Paper.cited_by` also stays empty through the parser path, and the
`CitationGraph.edges` vector is never populated. Any citation information must
come from an external source. The committed S2AG cache
(`cache/citations/s2ag_citations.json`) supplies per-paper *counts* for about
731 matched papers (~41,468 citations total), and these are shown in the web UI;
but the corresponding citation *graph* is effectively empty, so it feeds counts,
not edges. That integration — including why the web rankings fall back to a
prestige score rather than true PageRank — is the subject of Chapter 6.

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
