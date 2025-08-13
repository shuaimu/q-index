#!/bin/bash

# Fetch all USENIX conference papers

echo "========================================"
echo "Fetching USENIX Conference Papers"
echo "========================================"

# Make the fetcher executable
chmod +x scripts/fetch_conference_year.py

# OSDI (1994-2023, skipping 2024 as already fetched)
echo -e "\n=== Fetching OSDI Papers ==="
for year in {2023..1994}; do
    echo "Fetching OSDI $year..."
    python3 scripts/fetch_conference_year.py osdi $year
    sleep 1
done

# NSDI (2004-2024)
echo -e "\n=== Fetching NSDI Papers ==="
for year in {2024..2004}; do
    echo "Fetching NSDI $year..."
    python3 scripts/fetch_conference_year.py nsdi $year
    sleep 1
done

# FAST (2002-2024)
echo -e "\n=== Fetching FAST Papers ==="
for year in {2024..2002}; do
    echo "Fetching FAST $year..."
    python3 scripts/fetch_conference_year.py fast $year
    sleep 1
done

# USENIX ATC (2024 down to 1992)
echo -e "\n=== Fetching USENIX ATC Papers ==="
for year in {2024..1992}; do
    echo "Fetching ATC $year..."
    python3 scripts/fetch_conference_year.py atc $year
    sleep 1
done

# HotOS (every 2 years, 2023 down to 1997)
echo -e "\n=== Fetching HotOS Papers ==="
for year in 2023 2021 2019 2017 2015 2013 2011 2009 2007 2005 2003 2001 1999 1997; do
    echo "Fetching HotOS $year..."
    python3 scripts/fetch_conference_year.py hotos $year
    sleep 1
done

echo -e "\n========================================"
echo "Sorting all BibTeX files by year..."
python3 scripts/sort_papers_by_year.py

echo -e "\n========================================"
echo "Done! Next steps:"
echo "1. Review the updated .bib files"
echo "2. Run: cargo build --release"
echo "3. Start server: ./target/release/qindex web"