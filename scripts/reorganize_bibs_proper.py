#!/usr/bin/env python3
"""
Properly reorganize BibTeX files by conference name.
This script reads all BibTeX files, extracts conference information,
and organizes papers into conference-specific files.
"""

import os
import re
from pathlib import Path
from collections import defaultdict

def extract_conference_from_booktitle(booktitle):
    """Extract conference abbreviation from booktitle field."""
    if not booktitle:
        return None
    
    # Remove common prefixes and suffixes
    booktitle = booktitle.strip()
    booktitle = re.sub(r'^(Proceedings of|Proc\.?\s+of|In)\s+', '', booktitle, flags=re.IGNORECASE)
    booktitle = re.sub(r'\s+(Conference|Workshop|Symposium|Meeting).*$', '', booktitle, flags=re.IGNORECASE)
    
    # Known conference mappings
    conference_mappings = {
        'SOSP': 'sosp',
        'OSDI': 'osdi', 
        'NSDI': 'nsdi',
        'ATC': 'atc',
        'FAST': 'fast',
        'EuroSys': 'eurosys',
        'ASPLOS': 'asplos',
        'ISCA': 'isca',
        'HotOS': 'hotos',
        'SoCC': 'socc',
        'SIGMOD': 'sigmod',
        'VLDB': 'vldb',
        'ICDE': 'icde',
        'CIDR': 'cidr',
        'SIGCOMM': 'sigcomm',
        'NSDI': 'nsdi',
        'FOCS': 'focs',
        'STOC': 'stoc',
        'PODC': 'podc',
        'SPAA': 'spaa',
        'OOPSLA': 'oopsla',
        'PLDI': 'pldi',
        'POPL': 'popl',
        'ICML': 'icml',
        'NeurIPS': 'neurips',
        'ICLR': 'iclr',
        'CVPR': 'cvpr',
        'ICCV': 'iccv',
        'ECCV': 'eccv',
        'CHI': 'chi',
        'DSN': 'dsn',
        'USENIX Security': 'usenixsec',
        'CCS': 'ccs',
        'NDSS': 'ndss',
        'Oakland': 'oakland',
        'S&P': 'oakland',
    }
    
    # Check for exact matches
    booktitle_upper = booktitle.upper()
    for conf_name, conf_abbrev in conference_mappings.items():
        if conf_name.upper() in booktitle_upper:
            return conf_abbrev
    
    # Try to extract from common patterns
    patterns = [
        r'\b(SOSP|OSDI|NSDI|ATC|FAST|EUROSYS|ASPLOS|ISCA|HOTOS|SOCC)\b',
        r'\b(SIGMOD|VLDB|ICDE|CIDR)\b',
        r'\b(SIGCOMM|NSDI|INFOCOM)\b', 
        r'\b(FOCS|STOC|PODC|SPAA)\b',
        r'\b(OOPSLA|PLDI|POPL|ICFP)\b',
        r'\b(ICML|NEURIPS|ICLR|CVPR|ICCV|ECCV)\b',
        r'\b(CHI|UIST|IUI)\b',
    ]
    
    for pattern in patterns:
        match = re.search(pattern, booktitle_upper)
        if match:
            conf = match.group(1).lower()
            return conf
    
    return None

def extract_conference_from_journal(journal):
    """Extract conference/journal abbreviation from journal field."""
    if not journal:
        return None
    
    journal_mappings = {
        'tods': 'tods',
        'tocs': 'tocs', 
        'toplas': 'toplas',
        'tkde': 'tkde',
        'jacm': 'jacm',
        'cacm': 'cacm',
        'jmlr': 'jmlr',
        'tmlr': 'tmlr',
        'pvldb': 'vldb',  # VLDB journal goes to vldb conference file
        'corr': 'arxiv',
    }
    
    journal_lower = journal.lower().strip()
    for abbrev, target in journal_mappings.items():
        if abbrev in journal_lower:
            return target
    
    return None

def parse_bib_file(file_path):
    """Parse a BibTeX file and extract entries."""
    entries = []
    current_entry = []
    in_entry = False
    brace_count = 0
    
    with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith('%'):
                continue
                
            if line.startswith('@'):
                if in_entry:
                    entries.append('\n'.join(current_entry))
                current_entry = [line]
                in_entry = True
                brace_count = line.count('{') - line.count('}')
            elif in_entry:
                current_entry.append(line)
                brace_count += line.count('{') - line.count('}')
                if brace_count <= 0:
                    entries.append('\n'.join(current_entry))
                    current_entry = []
                    in_entry = False
                    
        if in_entry and current_entry:
            entries.append('\n'.join(current_entry))
    
    return entries

def extract_fields(entry):
    """Extract key fields from a BibTeX entry."""
    fields = {}
    
    # Extract booktitle
    booktitle_match = re.search(r'booktitle\s*=\s*[{"]([^{}",]+)["}]', entry, re.IGNORECASE)
    if booktitle_match:
        fields['booktitle'] = booktitle_match.group(1)
    else:
        # Try without braces/quotes
        booktitle_match = re.search(r'booktitle\s*=\s*([^,\n]+)', entry, re.IGNORECASE)
        if booktitle_match:
            fields['booktitle'] = booktitle_match.group(1).strip()
    
    # Extract journal
    journal_match = re.search(r'journal\s*=\s*[{"]?([^{},"\n]+)["}]?', entry, re.IGNORECASE)
    if journal_match:
        fields['journal'] = journal_match.group(1).strip()
    
    # Extract year
    year_match = re.search(r'year\s*=\s*[{"]?(\d{4})["}]?', entry, re.IGNORECASE)
    if year_match:
        fields['year'] = int(year_match.group(1))
    
    return fields

def organize_papers():
    """Main function to organize papers by conference."""
    bib_dir = Path('/Users/shuai/workspace/qindex/bib')
    
    # Dictionary to hold papers by conference
    conference_papers = defaultdict(list)
    
    # Files to process (including mixed files)
    files_to_process = [
        'recent_db_net.bib',
        'recent_systems.bib', 
        'ml_ai.bib',
        'theory_security.bib',
        'misc.bib',
    ]
    
    for filename in files_to_process:
        file_path = bib_dir / filename
        if not file_path.exists():
            continue
            
        print(f"Processing {filename}...")
        entries = parse_bib_file(file_path)
        
        for entry in entries:
            fields = extract_fields(entry)
            conference = None
            
            # Try to determine conference from booktitle first
            if 'booktitle' in fields:
                conference = extract_conference_from_booktitle(fields['booktitle'])
            
            # Fall back to journal if no conference found
            if not conference and 'journal' in fields:
                conference = extract_conference_from_journal(fields['journal'])
            
            # Add year for sorting
            year = fields.get('year', 0)
            
            if conference:
                conference_papers[conference].append((year, entry))
                print(f"  -> {conference}: {fields.get('booktitle', fields.get('journal', 'unknown'))}")
            else:
                conference_papers['misc'].append((year, entry))
                print(f"  -> misc: {fields.get('booktitle', fields.get('journal', 'unknown'))}")
    
    # Write organized files
    for conference, papers in conference_papers.items():
        if not papers:
            continue
            
        output_file = bib_dir / f"{conference}.bib"
        
        # Sort papers by year
        papers.sort(key=lambda x: x[0])
        
        # Read existing content if file exists
        existing_entries = []
        if output_file.exists():
            existing_entries = parse_bib_file(output_file)
        
        # Combine and write
        with open(output_file, 'w', encoding='utf-8') as f:
            f.write(f"% {conference.upper()} Conference Papers\n\n")
            
            # Write existing entries first
            for entry in existing_entries:
                f.write(entry)
                f.write('\n\n')
            
            # Write new entries
            for year, entry in papers:
                f.write(entry)
                f.write('\n\n')
        
        print(f"Created {output_file} with {len(papers)} new papers + {len(existing_entries)} existing")
    
    # Remove processed files
    for filename in files_to_process:
        file_path = bib_dir / filename
        if file_path.exists():
            os.remove(file_path)
            print(f"Removed {filename}")

if __name__ == "__main__":
    organize_papers()