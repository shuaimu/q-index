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
The bib folder now contains papers from major CS conferences:

**Systems Conferences:**
- `sosp.bib` - 79 papers (1983-2024)
- `osdi.bib` - 82 papers (1999-2024)
- `nsdi.bib` - 60 papers (2005-2024)
- `atc.bib` - 44 papers (1996-2024)
- `fast.bib` - 16 papers (2003-2021)
- `eurosys.bib` - 29 papers (2009-2024)
- `asplos.bib` - 14 papers (1991-2024)

**Database Conferences:**
- `sigmod.bib` - 73 papers (1981-2024)
- `vldb.bib` - 81 papers (1985-2024)
- `icde.bib` - 4 papers (2013-2021)
- `cidr.bib` - 11 papers (2011-2024)

**Machine Learning Conferences:**
- `icml.bib` - 14 papers (2020-2023)
- `neurips.bib` - 19 papers (2017-2023)
- `iclr.bib` - 15 papers (2020-2024)
- `cvpr.bib` - 5 papers (2016-2022)

**Security Conferences:**
- `oakland.bib` - 6 papers (2018-2024)
- `ccs.bib` - 4 papers (2009-2023)
- `ndss.bib` - 1 paper (2024)

**Theory Conferences:**
- `stoc.bib` - 20 papers (1997-2024)
- `focs.bib` - 6 papers (2014-2024)
- `podc.bib` - 18 papers (1988-2024)

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

## Available Scripts

### BibTeX Management Scripts
- `scripts/reorganize_bibs_proper.py` - Reorganize mixed BibTeX files by conference
- `scripts/sort_papers_by_year.py` - Sort papers within each file by year

### Usage
```bash
# Reorganize mixed files into conference-specific files
python3 scripts/reorganize_bibs_proper.py

# Sort all BibTeX files by year
python3 scripts/sort_papers_by_year.py
```

## Testing and Validation

When modifying BibTeX files:
1. Run `cargo build` to ensure parsing still works
2. Run `cargo run -- web` to verify the web interface loads correctly
3. Check that paper counts are as expected
4. Use reorganization scripts to maintain file structure

## Code Quality

- Always run `cargo fmt` before committing Rust code changes
- Run `cargo clippy` to check for common mistakes  
- Run `cargo test` to ensure tests pass

## Web Interface Commands

```bash
# Run web server
cargo run -- web

# Run in background with auto-refresh during development
cargo run -- web &

# Access at http://localhost:8080
```