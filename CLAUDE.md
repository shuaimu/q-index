# QIndex Project Guidelines

## BibTeX File Organization Rules

All papers in the `bib/` folder have been organized according to the following rules:

### 1. File Naming Convention
Each BibTeX file is named after its conference/venue in lowercase:
- Format: `{conference_name}.bib`
- Examples: `sosp.bib`, `osdi.bib`, `sigmod.bib`, `icml.bib`

### 2. Paper Organization
- Each paper entry is placed in the file corresponding to its conference/venue
- Papers within each file are sorted by year (oldest to newest)
- All files have been automatically sorted using `scripts/sort_papers_by_year.py`

### 3. Conference Coverage
The bib folder now contains papers from major CS conferences (as of 2025-08-13):

**Systems Conferences:**
- `sosp.bib` - 819 papers (main conference only)
- `osdi.bib` - 817 papers (main conference only)
- `nsdi.bib` - 699 papers
- `atc.bib` - 717 papers (USENIX ATC)
- `fast.bib` - 407 papers
- `eurosys.bib` - 1,336 papers
- `asplos.bib` - 1,148 papers

**Database Conferences:**
- `sigmod.bib` - 1,055 papers
- `vldb.bib` - 973 papers
- `icde.bib` - 1,402 papers
- `cidr.bib` - 11 papers

**Machine Learning Conferences:**
- `icml.bib` - 1,211 papers
- `neurips.bib` - 1,418 papers
- `iclr.bib` - 15 papers
- `cvpr.bib` - 5 papers

**Networking Conferences:**
- `sigcomm.bib` - 1,687 papers (main conference only, workshop papers removed)

**Security Conferences:**
- `oakland.bib` - 6 papers
- `ccs.bib` - 2,789 papers
- `ndss.bib` - 1 paper

**Programming Languages:**
- `pldi.bib` - 752 papers
- `popl.bib` - 509 papers

**Theory Conferences:**
- `stoc.bib` - 1,923 papers
- `focs.bib` - 6 papers
- `podc.bib` - 1,131 papers

### 4. Cleanup Process
Mixed files have been properly organized:
- Merged papers from `recent_db_net.bib`, `recent_systems.bib`, `ml_ai.bib` into proper conference files
- Removed incorrectly split files created by previous reorganization attempts
- Papers from `misc.bib` distributed to appropriate conference files or consolidated

### 5. Special Files
- Journal papers in dedicated files: `tods.bib`, `tocs.bib`, `toplas.bib`, `jacm.bib`, etc.
- Technical reports in `tr.bib`
- ArXiv preprints in `arxiv.bib`

### 6. Entry Format Standards
- Conference papers use `@inproceedings`
- Journal papers use `@article`
- All entries include year field for sorting
- Booktitle/journal fields specify venue

## Fetching Conference Papers Workflow

### Important: Main Conference Papers Only
When fetching papers for conferences, it's critical to fetch ONLY main conference papers, not workshop papers, demos, posters, or co-located events.

### Step-by-Step Process for Adding New Conference Papers

1. **Use the Main Conference Fetcher Script**
   ```bash
   # Fetch papers for a specific conference and year
   python3 scripts/fetch_main_conference.py <conference> <year>
   
   # Example: Fetch SIGCOMM 2024 papers
   python3 scripts/fetch_main_conference.py sigcomm 2024
   ```

2. **Batch Fetch Multiple Years**
   ```bash
   # Fetch papers for a range of years
   for year in {2010..2024}; do
     echo "=== $CONF $year ==="
     python3 scripts/fetch_main_conference.py $CONF $year
     sleep 1  # Be nice to DBLP API
   done
   ```

3. **Clean Workshop Papers (if needed)**
   ```bash
   # Deep clean to remove workshop/demo/poster papers
   python3 scripts/deep_clean_sigcomm.py  # Conference-specific cleaner
   ```

4. **Sort Papers by Year**
   ```bash
   python3 scripts/sort_papers_by_year.py
   ```

5. **Rebuild and Test**
   ```bash
   cargo build --release
   cargo run --release -- build-site
   python3 scripts/check_site_links.py site /
   ```

### Common Workshop/Demo Indicators to Filter Out

Papers with these patterns should be excluded:
- Titles containing: "Workshop", "Symposium", "Demo:", "Poster:", "Keynote:", "Tutorial", "Panel"
- Titles starting with "Proceedings of"
- Venue fields with "@" (e.g., "eBPF@SIGCOMM")
- Hot* workshops (HotNets, HotOS, HotCloud, etc.)
- Co-located events
- DOI patterns specific to workshops (check the main conference DOI pattern)

### DBLP API Query Patterns

For fetching main conference papers, try these query patterns in order:
1. `toc:db/conf/{conference}/{conference}{year}.bht:` - Direct proceedings query
2. `venue:{CONFERENCE} year:{year}` - Venue-based query
3. `{CONFERENCE} {year}` - General search

### Verification Steps

After fetching:
1. Check paper count matches expected conference size (typically 40-200 papers/year for major conferences)
2. Verify no workshop papers by checking titles
3. Confirm correct DOI pattern for main conference
4. Sample check a few paper titles to ensure they're research papers

## Available Scripts

### Conference Paper Fetching Scripts
- `scripts/fetch_main_conference.py` - Fetch only main conference papers from DBLP
- `scripts/fetch_conference_year.py` - Generic conference fetcher (use with caution - may include workshops)
- `scripts/deep_clean_sigcomm.py` - Remove workshop/demo/poster papers from SIGCOMM
- `scripts/fix_conference_workshops.py` - Identify and remove workshop papers

### BibTeX Management Scripts
- `scripts/reorganize_bibs_proper.py` - Reorganize mixed BibTeX files by conference
- `scripts/sort_papers_by_year.py` - Sort papers within each file by year

### Usage Examples
```bash
# Add a new conference (e.g., ICSE)
python3 scripts/fetch_main_conference.py icse 2024
python3 scripts/sort_papers_by_year.py

# Batch fetch all years for a conference
for year in {2010..2024}; do
  python3 scripts/fetch_main_conference.py icse $year
  sleep 1
done

# Clean up if workshop papers were accidentally included
python3 scripts/deep_clean_sigcomm.py  # Modify for other conferences

# Sort all BibTeX files by year
python3 scripts/sort_papers_by_year.py
```

## S2AG Citation Data Integration (2025-08-14)

### Downloaded Data Status
- **Papers dataset**: 32 files downloaded (~27GB, release 2025-08-08; one file is truncated)
- **Citations dataset**: 122 files downloaded (~90GB)  
- **Total downloaded**: 138GB from Semantic Scholar Academic Graph (S2AG)
- **Location**: `data/s2ag/`

### Citation Counts (rebuilt 2026-10-08)
Per-paper citation counts on the site come from `cache/citations/s2ag_paper_citations.json`,
keyed by our paper ids (exact lookups in `src/s2ag_citations.rs`, no fuzzy matching).
- **16,462 / 19,954 papers matched (82.5%)**: 97% of papers with a DOI, 48% of papers
  without one (USENIX venues, NeurIPS, ICML have no DOIs and can only be title-matched)
- The no-DOI gap is the 28 S2AG papers files never downloaded; closing it needs the
  API key (title-match API, or downloading the full papers dataset)
- Scholar/venue citation totals and h-indexes on the site are computed from these
  per-paper counts (each S2 corpus id counted once); QIndex/PageRank are unchanged

```bash
# Rebuild the matches (rerun after adding papers to bib/)
./target/release/qindex export-papers                # -> cache/citations/papers.jsonl
python3 -I scripts/match_s2ag.py scan                # local S2AG files -> corpus ids (few min)
python3 -I scripts/match_s2ag.py resolve             # Graph API batch -> current counts
# With a key, also title-match papers missing from the local files (~1 req/s):
S2_API_KEY="$S2_API_KEY" python3 -I scripts/match_s2ag.py resolve --title-match
```
Intermediate results are cached in `data/s2ag/work/` (gitignored), so reruns only fetch
what's new. Commit the updated `cache/citations/s2ag_paper_citations.json` — CI builds
the site from it.

`scripts/parse_s2ag.py` and its outputs (`s2ag_citations.json`, `s2ag_id_mapping.json`,
`s2ag_citation_graph.json`, `s2ag_summary.json`) are superseded: they matched only 731
papers and the site no longer reads them.

### S2AG Scripts
```bash
# Download S2AG datasets (requires API key)
# Set S2_API_KEY in your environment first (see .env.example; e.g. `set -a; source .env; set +a`)
S2_API_KEY="$S2_API_KEY" python3 scripts/download_s2ag.py --dataset papers
S2_API_KEY="$S2_API_KEY" python3 scripts/download_s2ag.py --dataset citations

# Monitor download progress
python3 scripts/monitor_s2ag_download.py
```

### API Key
The Semantic Scholar API key is read from the `S2_API_KEY` environment variable.
Copy `.env.example` to `.env`, set your key there, and load it before running the
S2AG scripts (the `.env` file is gitignored and must never be committed).

### To Continue Downloads
The S2AG dataset has:
- **Papers**: 60 files total (~90GB) - 32 downloaded
- **Citations**: 236 files total (~236GB) - 122 downloaded

Downloads can freeze due to rate limiting. If needed, restart with smaller batches:
```bash
# Download specific number of files
S2_API_KEY="..." python3 scripts/download_s2ag.py --dataset papers --max-files 10
S2_API_KEY="..." python3 scripts/download_s2ag.py --dataset citations --max-files 50
```

## Citation Data Fetching

### Fetching Citation Data from Academic APIs

The project includes scripts to fetch citation data from Semantic Scholar and CrossRef APIs:

1. **Fetch Citations for Papers**
   ```bash
   # Fetch citations for a single conference
   python3 scripts/fetch_citations.py sigcomm --limit 100
   
   # Fetch citations for all major conferences
   python3 scripts/fetch_citations.py --all
   
   # Check statistics
   python3 scripts/fetch_citations.py --stats
   ```

2. **Build Citation Database**
   ```bash
   # Show citation statistics
   python3 scripts/build_citation_database.py --stats
   
   # Export to Rust-compatible format
   python3 scripts/build_citation_database.py --export-rust
   
   # Export to GraphML for visualization
   python3 scripts/build_citation_database.py --export-graphml
   
   # Find highly cited papers (≥100 citations)
   python3 scripts/build_citation_database.py --highly-cited 100
   ```

### Citation Data Storage

Citation data is cached locally in `cache/citations/`:
- `paper_cache.json` - Paper metadata and citation counts
- `citation_graph.json` - Citation relationships (who cites whom)
- `failed_lookups.json` - Papers that couldn't be found (to avoid repeated lookups)
- `citation_data.json` - Rust-compatible export format

### API Rate Limiting

The scripts respect API rate limits:
- 500ms minimum between requests
- Failed lookups are cached for 7 days before retry
- Intermediate results saved every 10 papers

### Citation Scripts
- `scripts/fetch_citations.py` - Fetch citation data from Semantic Scholar/CrossRef
- `scripts/build_citation_database.py` - Build and export citation database

## Testing and Validation

When modifying BibTeX files:
1. Run `cargo build` to ensure parsing still works
2. Run `cargo run --release -- build-site` and `python3 scripts/check_site_links.py site /` to verify the site builds and all links resolve
3. Check that paper counts are as expected
4. Use reorganization scripts to maintain file structure

## Code Quality

- Always run `cargo fmt` before committing Rust code changes
- Run `cargo clippy` to check for common mistakes  
- Run `cargo test` to ensure tests pass

## Static Website

The web interface is a static site generated by `qindex build-site` (no server).
CI (`.github/workflows/pages.yml`) builds it on every push to `main` and deploys
it to GitHub Pages at https://shuaimu.github.io/q-index/ (base URL `/q-index/`).
The built `site/` directory is gitignored — never commit it.

```bash
# Build the site into ./site (base URL "/") and preview it
cargo run --release -- build-site
python3 -m http.server -d site 8080   # http://localhost:8080

# Build for the GitHub Pages sub-path, as CI does
cargo run --release -- build-site --base-url /q-index/

# Include the design book under /book/ (needs mdbook)
mdbook build docs && cargo run --release -- build-site

# Check every internal link (use the same base URL as the build)
python3 scripts/check_site_links.py site /
```

- Pages are pre-rendered from `src/site/templates.rs`; every internal link must go
  through `Ctx::url` / `venue_url` / `scholar_url` so the base URL is applied.
- Search, venue/scholar list filters and scholar profiles run in the browser
  (`static/app.js`) against `site/data/*.json`. Scholar profiles are sharded
  into 256 files by FNV-1a hash of the scholar id — `scholar_shard()` in
  `src/site/mod.rs` and `shardOf()` in `static/app.js` must stay identical.
