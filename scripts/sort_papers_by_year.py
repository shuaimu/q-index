#!/usr/bin/env python3
"""
Sort papers within each BibTeX file by year.
"""

import os
import re
from pathlib import Path

def extract_year_from_entry(entry):
    """Extract year from a BibTeX entry."""
    year_match = re.search(r'year\s*=\s*[{"]?(\d{4})["}]?', entry, re.IGNORECASE)
    if year_match:
        return int(year_match.group(1))
    return 0  # Default year for entries without year

def parse_and_sort_bib_file(file_path):
    """Parse a BibTeX file and sort entries by year."""
    entries = []
    current_entry = []
    in_entry = False
    brace_count = 0
    header_lines = []
    
    with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
        lines = f.readlines()
    
    # Find header (comments at the top)
    i = 0
    while i < len(lines) and (lines[i].strip().startswith('%') or lines[i].strip() == ''):
        if lines[i].strip().startswith('%'):
            header_lines.append(lines[i].rstrip())
        i += 1
    
    # Parse entries
    for line in lines[i:]:
        line = line.rstrip()
        
        if not line and not in_entry:
            continue
            
        if line.startswith('@'):
            if in_entry:
                entry_text = '\n'.join(current_entry)
                year = extract_year_from_entry(entry_text)
                entries.append((year, entry_text))
            current_entry = [line]
            in_entry = True
            brace_count = line.count('{') - line.count('}')
        elif in_entry:
            current_entry.append(line)
            brace_count += line.count('{') - line.count('}')
            if brace_count <= 0:
                entry_text = '\n'.join(current_entry)
                year = extract_year_from_entry(entry_text)
                entries.append((year, entry_text))
                current_entry = []
                in_entry = False
                
    if in_entry and current_entry:
        entry_text = '\n'.join(current_entry)
        year = extract_year_from_entry(entry_text)
        entries.append((year, entry_text))
    
    # Sort by year
    entries.sort(key=lambda x: x[0])
    
    return header_lines, entries

def sort_all_bib_files():
    """Sort all BibTeX files by year."""
    bib_dir = Path('/Users/shuai/workspace/qindex/bib')
    
    # Get all .bib files
    bib_files = list(bib_dir.glob('*.bib'))
    
    for bib_file in bib_files:
        print(f"Sorting {bib_file.name}...")
        
        header_lines, entries = parse_and_sort_bib_file(bib_file)
        
        if not entries:
            print(f"  No entries found in {bib_file.name}")
            continue
        
        # Write sorted file
        with open(bib_file, 'w', encoding='utf-8') as f:
            # Write header
            for header in header_lines:
                f.write(header)
                f.write('\n')
            
            if header_lines:
                f.write('\n')
            
            # Write sorted entries
            for year, entry in entries:
                f.write(entry)
                f.write('\n\n')
        
        print(f"  Sorted {len(entries)} entries by year ({entries[0][0]}-{entries[-1][0]})")

if __name__ == "__main__":
    sort_all_bib_files()