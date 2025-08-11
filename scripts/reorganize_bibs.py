#!/usr/bin/env python3
"""
Reorganize BibTeX files according to conference names.
Each conference gets its own file, papers sorted by year.
"""

import os
import re
from collections import defaultdict
from typing import Dict, List, Tuple

def parse_bibtex_entries(content: str) -> List[Dict]:
    """Parse BibTeX content into individual entries."""
    entries = []
    
    # Split by @inproceedings or @article
    pattern = r'(@(?:inproceedings|article)\{[^@]+)'
    matches = re.findall(pattern, content, re.DOTALL | re.IGNORECASE)
    
    for match in matches:
        entry = {'raw': match}
        
        # Extract citation key
        key_match = re.search(r'@\w+\{([^,]+),', match)
        if key_match:
            entry['key'] = key_match.group(1)
        
        # Extract year
        year_match = re.search(r'year\s*=\s*\{?(\d{4})\}?', match, re.IGNORECASE)
        if year_match:
            entry['year'] = int(year_match.group(1))
        else:
            entry['year'] = 9999  # Put entries without year at the end
        
        # Extract booktitle/journal to determine venue
        booktitle_match = re.search(r'booktitle\s*=\s*\{([^}]+)\}', match, re.IGNORECASE)
        journal_match = re.search(r'journal\s*=\s*\{([^}]+)\}', match, re.IGNORECASE)
        
        if booktitle_match:
            entry['venue'] = booktitle_match.group(1)
        elif journal_match:
            entry['venue'] = journal_match.group(1)
        else:
            entry['venue'] = 'unknown'
        
        entries.append(entry)
    
    return entries

def normalize_venue_name(venue: str) -> str:
    """Extract conference name from venue string."""
    venue = venue.upper()
    
    # Common patterns to extract conference name
    patterns = [
        r'^(SOSP)\s*\d{4}',
        r'^(OSDI)\s*\d{4}',
        r'^(SIGMOD)\s*\d{4}',
        r'^(VLDB)\s*\d{4}',
        r'^(NSDI)\s*\d{4}',
        r'^(PLDI)\s*\d{4}',
        r'^(POPL)\s*\d{4}',
        r'^(ICML)\s*\d{4}',
        r'^(NEURIPS)\s*\d{4}',
        r'^(NIPS)\s*\d{4}',  # NeurIPS old name
        r'^(CVPR)\s*\d{4}',
        r'^(ICCV)\s*\d{4}',
        r'^(ECCV)\s*\d{4}',
        r'^(STOC)\s*\d{4}',
        r'^(FOCS)\s*\d{4}',
        r'^(CHI)\s*\d{4}',
        r'^(ACL)\s*\d{4}',
        r'^(EMNLP)\s*\d{4}',
        r'^(NAACL)\s*\d{4}',
        r'^(AAAI)\s*\d{4}',
        r'^(IJCAI)\s*\d{4}',
        r'^(ICLR)\s*\d{4}',
        r'^(ASPLOS)\s*\d{4}',
        r'^(ISCA)\s*\d{4}',
        r'^(MICRO)\s*\d{4}',
        r'^(EUROSYS)\s*\d{4}',
        r'^(FAST)\s*\d{4}',
        r'^(ATC)\s*\d{4}',
        r'^(USENIX ATC)\s*\d{4}',
        r'^(SIGCOMM)\s*\d{4}',
        r'^(OOPSLA)\s*\d{4}',
        r'^(ICFP)\s*\d{4}',
        r'^(PODC)\s*\d{4}',
        r'^(SPAA)\s*\d{4}',
        r'^(CCS)\s*\d{4}',
        r'^(S&P)\s*\d{4}',
        r'^(OAKLAND)\s*\d{4}',
        r'^(NDSS)\s*\d{4}',
        r'^(USENIX SECURITY)\s*\d{4}',
        r'^(DSN)\s*\d{4}',
        r'^(HOTOS)\s*\d{4}',
        r'^(SOCC)\s*\d{4}',
        r'^(CIDR)\s*\d{4}',
        r'^(ICDE)\s*\d{4}',
        r'^(PODS)\s*\d{4}',
        r'^(WWW)\s*\d{4}',
        r'^(KDD)\s*\d{4}',
        r'^(SIGIR)\s*\d{4}',
        r'^(FSE)\s*\d{4}',
        r'^(ICSE)\s*\d{4}',
        r'^(ASE)\s*\d{4}',
        r'^(ISSTA)\s*\d{4}',
        r'^(CAV)\s*\d{4}',
        r'^(LICS)\s*\d{4}',
        r'^(CRYPTO)\s*\d{4}',
        r'^(EUROCRYPT)\s*\d{4}',
        r'^(UIST)\s*\d{4}',
        r'^(UBICOMP)\s*\d{4}',
        r'^(PERVASIVE)\s*\d{4}',
        r'^(MOBISYS)\s*\d{4}',
        r'^(MOBICOM)\s*\d{4}',
        r'^(SENSYS)\s*\d{4}',
        r'^(IMC)\s*\d{4}',
        r'^(SIGMETRICS)\s*\d{4}',
        r'^(SIGGRAPH)\s*\d{4}',
        r'^(VIS)\s*\d{4}',
        r'^(VR)\s*\d{4}',
        r'^(ICRA)\s*\d{4}',
        r'^(IROS)\s*\d{4}',
        r'^(RSS)\s*\d{4}',
        r'^(SC)\s*\d{4}',
        r'^(HPDC)\s*\d{4}',
        r'^(ICS)\s*\d{4}',
    ]
    
    for pattern in patterns:
        match = re.search(pattern, venue)
        if match:
            conf = match.group(1)
            # Handle special cases
            if conf == 'NIPS':
                return 'neurips'
            if conf == 'USENIX ATC' or (conf == 'ATC' and 'USENIX' in venue):
                return 'atc'
            if conf == 'S&P' or conf == 'OAKLAND':
                return 'sp'
            if conf == 'USENIX SECURITY':
                return 'usenixsec'
            return conf.lower()
    
    # Check for journal abbreviations
    journal_patterns = [
        (r'CACM', 'cacm'),
        (r'TOCS', 'tocs'),
        (r'TODS', 'tods'),
        (r'TOPLAS', 'toplas'),
        (r'JACM', 'jacm'),
        (r'TKDE', 'tkde'),
        (r'CSUR', 'csur'),
        (r'JMLR', 'jmlr'),
        (r'TMLR', 'tmlr'),
    ]
    
    for pattern, name in journal_patterns:
        if pattern in venue:
            return name
    
    # If no pattern matches, try to clean up the venue name
    # Remove year, "Proceedings of", etc.
    venue_clean = re.sub(r'\d{4}', '', venue)
    venue_clean = re.sub(r'PROCEEDINGS OF', '', venue_clean)
    venue_clean = re.sub(r'PROC\.?\s*', '', venue_clean)
    venue_clean = re.sub(r'CONFERENCE ON', '', venue_clean)
    venue_clean = re.sub(r'SYMPOSIUM ON', '', venue_clean)
    venue_clean = re.sub(r'WORKSHOP ON', '', venue_clean)
    venue_clean = re.sub(r'INTERNATIONAL', '', venue_clean)
    venue_clean = re.sub(r'[^A-Z]', '', venue_clean)
    
    if venue_clean:
        return venue_clean.lower()[:20]  # Limit length
    
    return 'misc'

def main():
    bib_dir = '../bib'
    
    # Dictionary to store entries by conference
    conference_entries = defaultdict(list)
    
    # Process all .bib files
    for filename in os.listdir(bib_dir):
        if not filename.endswith('.bib'):
            continue
        
        # Skip title files
        if filename.startswith('title'):
            continue
        
        filepath = os.path.join(bib_dir, filename)
        print(f"Processing {filename}...")
        
        with open(filepath, 'r', encoding='utf-8') as f:
            content = f.read()
        
        entries = parse_bibtex_entries(content)
        
        for entry in entries:
            venue_name = normalize_venue_name(entry['venue'])
            conference_entries[venue_name].append(entry)
            print(f"  Found entry for {venue_name} ({entry.get('year', 'unknown')})")
    
    # Remove old files (except title files)
    print("\nRemoving old files...")
    for filename in os.listdir(bib_dir):
        if filename.endswith('.bib') and not filename.startswith('title'):
            os.remove(os.path.join(bib_dir, filename))
            print(f"  Removed {filename}")
    
    # Write new conference-specific files
    print("\nWriting reorganized files...")
    for conf_name, entries in sorted(conference_entries.items()):
        if conf_name == 'misc' or conf_name == 'unknown':
            continue  # Skip misc entries for now
        
        # Sort entries by year
        entries.sort(key=lambda x: (x['year'], x.get('key', '')))
        
        filename = f"{conf_name}.bib"
        filepath = os.path.join(bib_dir, filename)
        
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(f"% {conf_name.upper()} papers\n")
            f.write(f"% Sorted by year\n\n")
            
            current_year = None
            for entry in entries:
                # Add year separator
                if entry['year'] != current_year and entry['year'] != 9999:
                    if current_year is not None:
                        f.write("\n")
                    f.write(f"% Year {entry['year']}\n")
                    current_year = entry['year']
                
                f.write(entry['raw'])
                f.write("\n\n")
        
        print(f"  Wrote {filename} with {len(entries)} entries")
    
    # Handle misc entries
    if conference_entries['misc'] or conference_entries['unknown']:
        misc_entries = conference_entries['misc'] + conference_entries['unknown']
        misc_entries.sort(key=lambda x: (x['year'], x.get('key', '')))
        
        filepath = os.path.join(bib_dir, 'misc.bib')
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write("% Miscellaneous papers\n")
            f.write("% Papers that couldn't be categorized into specific conferences\n\n")
            
            for entry in misc_entries:
                f.write(entry['raw'])
                f.write("\n\n")
        
        print(f"  Wrote misc.bib with {len(misc_entries)} entries")
    
    print("\nReorganization complete!")
    
    # Print summary
    total_entries = sum(len(entries) for entries in conference_entries.values())
    print(f"\nSummary:")
    print(f"  Total papers: {total_entries}")
    print(f"  Conferences: {len(conference_entries)}")
    print(f"  Files created: {len([c for c in conference_entries if c not in ['misc', 'unknown']]) + (1 if conference_entries['misc'] or conference_entries['unknown'] else 0)}")

if __name__ == "__main__":
    main()