#!/usr/bin/env python3
"""
Fix conference papers by removing workshop papers and fetching main conference papers
"""

import re
import sys
from pathlib import Path
from collections import defaultdict
import time

def parse_bibtex_file(filepath):
    """Parse a BibTeX file and return entries"""
    entries = []
    current_entry = []
    in_entry = False
    
    with open(filepath, 'r', encoding='utf-8') as f:
        for line in f:
            if line.strip().startswith('@'):
                if current_entry:
                    entries.append(''.join(current_entry))
                current_entry = [line]
                in_entry = True
            elif in_entry:
                current_entry.append(line)
                if line.strip() == '}':
                    in_entry = False
    
    if current_entry:
        entries.append(''.join(current_entry))
    
    return entries

def is_workshop_paper(entry):
    """Check if an entry is a workshop paper based on venue/booktitle"""
    workshop_indicators = [
        '@', 'Workshop', 'workshop', 'WS', 'Co-located', 'co-located',
        'Symposium on', 'Poster', 'Demo', 'Doctoral', 'Tutorial'
    ]
    
    # Extract venue/booktitle
    venue_match = re.search(r'(?:booktitle|venue)\s*=\s*["{]([^"}]+)["}]', entry, re.IGNORECASE)
    if venue_match:
        venue = venue_match.group(1)
        for indicator in workshop_indicators:
            if indicator in venue:
                return True
    
    # Check URL for workshop indicators
    url_match = re.search(r'url\s*=\s*["{]([^"}]+)["}]', entry, re.IGNORECASE)
    if url_match:
        url = url_match.group(1)
        if '/workshops/' in url or '/ws/' in url:
            return True
    
    return False

def extract_year(entry):
    """Extract year from a BibTeX entry"""
    year_match = re.search(r'year\s*=\s*["{]?(\d{4})["}]?', entry, re.IGNORECASE)
    if year_match:
        return int(year_match.group(1))
    return None

def clean_conference_file(conference, filepath):
    """Clean a conference BibTeX file by removing workshop papers"""
    entries = parse_bibtex_file(filepath)
    
    main_conference = []
    workshop_papers = []
    
    for entry in entries:
        if entry.strip().startswith('%') or not entry.strip():
            continue
        
        if is_workshop_paper(entry):
            workshop_papers.append(entry)
        else:
            main_conference.append(entry)
    
    if workshop_papers:
        print(f"\n{conference.upper()}: Found {len(workshop_papers)} workshop papers out of {len(entries)} total")
        
        # Group by year
        workshops_by_year = defaultdict(list)
        for entry in workshop_papers:
            year = extract_year(entry)
            if year:
                workshops_by_year[year].append(entry)
        
        for year in sorted(workshops_by_year.keys()):
            print(f"  {year}: {len(workshops_by_year[year])} workshop papers")
            # Show first workshop paper as example
            venue_match = re.search(r'(?:booktitle|venue)\s*=\s*["{]([^"}]+)["}]', workshops_by_year[year][0], re.IGNORECASE)
            if venue_match:
                print(f"    Example venue: {venue_match.group(1)}")
        
        # Write back only main conference papers
        backup_path = filepath.with_suffix('.bib.backup')
        filepath.rename(backup_path)
        print(f"  Created backup: {backup_path}")
        
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(f"% {conference.upper()} Main Conference Papers\n")
            f.write(f"% Cleaned on {time.strftime('%Y-%m-%d %H:%M')}\n")
            f.write(f"% Removed {len(workshop_papers)} workshop papers\n\n")
            
            for entry in main_conference:
                f.write(entry)
                if not entry.endswith('\n\n'):
                    f.write('\n')
        
        print(f"  Kept {len(main_conference)} main conference papers")
        return len(workshop_papers), len(main_conference)
    
    return 0, len(entries)

def main():
    # Conferences to check
    conferences = [
        'sigcomm', 'sigmod', 'vldb', 'icml', 'neurips', 'cvpr',
        'icde', 'pldi', 'sosp', 'osdi', 'nsdi', 'atc', 'fast',
        'eurosys', 'asplos', 'ccs', 'podc', 'stoc', 'popl'
    ]
    
    bib_dir = Path('bib')
    
    total_removed = 0
    total_kept = 0
    
    for conf in conferences:
        filepath = bib_dir / f"{conf}.bib"
        if filepath.exists():
            removed, kept = clean_conference_file(conf, filepath)
            total_removed += removed
            total_kept += kept
    
    print(f"\n=== SUMMARY ===")
    print(f"Total workshop papers removed: {total_removed}")
    print(f"Total main conference papers kept: {total_kept}")
    
    if total_removed > 0:
        print("\nNext steps:")
        print("1. Review the changes")
        print("2. Re-fetch main conference papers for affected conferences")
        print("3. Run 'python3 scripts/sort_papers_by_year.py' to sort")

if __name__ == "__main__":
    main()