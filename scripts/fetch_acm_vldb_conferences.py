#!/usr/bin/env python3
"""
Fetch papers from ACM and VLDB conferences using DBLP API.

This script fetches papers from major database and systems conferences:
- SIGMOD (ACM SIGMOD Conference)
- VLDB (Very Large Data Bases)
- ICDE (IEEE International Conference on Data Engineering)
- SOSP (ACM Symposium on Operating Systems Principles)
- SIGCOMM (ACM SIGCOMM Conference)
- PLDI (ACM SIGPLAN Conference on Programming Language Design and Implementation)
- POPL (ACM SIGPLAN-SIGACT Symposium on Principles of Programming Languages)
- STOC (ACM Symposium on Theory of Computing)
- FOCS (IEEE Symposium on Foundations of Computer Science)
"""

import sys
import os
sys.path.append(os.path.dirname(os.path.abspath(__file__)))

# Import the generic fetcher
from fetch_conference_year import fetch_conference_papers, create_bibtex_entries, update_bib_file
import time

def main():
    """Fetch papers from ACM and VLDB conferences."""
    
    # Configuration for what to fetch
    # Format: (conference, start_year, end_year)
    conferences = [
        # Database conferences
        ('sigmod', 1975, 2024),  # ACM SIGMOD
        ('vldb', 1975, 2024),     # VLDB 
        ('icde', 1984, 2024),     # IEEE ICDE
        ('cidr', 2003, 2025),     # CIDR (every 2 years)
        
        # Systems conferences  
        ('sosp', 1967, 2024),     # ACM SOSP (every 2 years)
        ('eurosys', 2006, 2024),  # EuroSys
        ('asplos', 1982, 2024),   # ASPLOS
        
        # Networking
        ('sigcomm', 1988, 2024),  # ACM SIGCOMM
        ('nsdi', 2004, 2024),     # Already done but let's check for more
        
        # Programming Languages
        ('pldi', 1979, 2024),     # ACM PLDI
        ('popl', 1973, 2024),     # ACM POPL
        ('oopsla', 1986, 2024),   # OOPSLA
        
        # Theory
        ('stoc', 1969, 2024),     # ACM STOC
        ('focs', 1960, 2024),     # IEEE FOCS
        ('podc', 1982, 2024),     # ACM PODC
        
        # Security
        ('ccs', 1993, 2024),      # ACM CCS
        ('oakland', 1980, 2024),  # IEEE S&P (Oakland)
        
        # Machine Learning (for completeness)
        ('icml', 1988, 2024),     # ICML
        ('neurips', 1987, 2024),  # NeurIPS (formerly NIPS)
        ('iclr', 2013, 2024),     # ICLR
    ]
    
    print("=" * 70)
    print("ACM and VLDB Conference Paper Fetcher")
    print("=" * 70)
    
    # Statistics
    total_added = {}
    
    for conf, start_year, end_year in conferences:
        print(f"\n{'='*50}")
        print(f"Processing {conf.upper()} ({start_year}-{end_year})")
        print(f"{'='*50}")
        
        conf_total = 0
        
        # Fetch papers year by year
        for year in range(end_year, start_year - 1, -1):
            # Skip odd years for biennial conferences
            if conf in ['sosp', 'cidr'] and year % 2 == 0 and conf == 'sosp':
                continue  # SOSP is in odd years
            if conf == 'cidr' and year % 2 == 0:
                continue  # CIDR is in odd years
                
            print(f"\nFetching {conf.upper()} {year}...")
            
            try:
                papers = fetch_conference_papers(conf, year)
                
                if papers:
                    print(f"Found {len(papers)} papers for {conf.upper()} {year}")
                    
                    # Convert to BibTeX
                    entries = create_bibtex_entries(papers, conf)
                    
                    # Update bib file
                    added = update_bib_file(conf, year, entries)
                    conf_total += added
                    
                    time.sleep(0.5)  # Be nice to DBLP
                else:
                    # No output if no papers found for that year
                    pass
                    
            except Exception as e:
                print(f"Error fetching {conf.upper()} {year}: {e}")
                
        total_added[conf] = conf_total
        print(f"\nTotal added for {conf.upper()}: {conf_total} papers")
    
    # Print summary
    print("\n" + "=" * 70)
    print("SUMMARY")
    print("=" * 70)
    
    for conf, count in sorted(total_added.items(), key=lambda x: x[1], reverse=True):
        if count > 0:
            print(f"{conf.upper():12} {count:5} new papers")
    
    total = sum(total_added.values())
    print(f"\nTOTAL:       {total:5} new papers across all conferences")
    
    print("\nNext steps:")
    print("1. Run: python3 scripts/sort_papers_by_year.py")
    print("2. Rebuild: cargo build --release")
    print("3. Run server: ./target/release/qindex web")

if __name__ == "__main__":
    main()