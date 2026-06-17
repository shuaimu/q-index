#!/usr/bin/env python3
"""
Deep clean SIGCOMM to remove ALL workshop, demo, poster, keynote papers
"""

import re
from pathlib import Path

def parse_bibtex_entries(content):
    """Parse BibTeX content into individual entries and non-entry content"""
    entries = []
    non_entries = []
    current_entry = []
    in_entry = False
    brace_count = 0
    
    for line in content.split('\n'):
        if line.strip().startswith('@inproceedings'):
            if current_entry and brace_count == 0:
                entries.append('\n'.join(current_entry))
            current_entry = [line]
            in_entry = True
            brace_count = line.count('{') - line.count('}')
        elif in_entry:
            current_entry.append(line)
            brace_count += line.count('{') - line.count('}')
            if brace_count == 0 and line.strip().endswith('}'):
                entries.append('\n'.join(current_entry))
                current_entry = []
                in_entry = False
        else:
            # Comments or empty lines
            non_entries.append(line)
    
    if current_entry:
        entries.append('\n'.join(current_entry))
    
    return entries, non_entries

def is_workshop_paper(entry):
    """Check if an entry is a workshop/demo/poster/keynote paper"""
    
    # Extract title
    title_match = re.search(r'title\s*=\s*\{([^}]+)\}', entry, re.IGNORECASE)
    if not title_match:
        return False
    
    title = title_match.group(1)
    
    # Workshop indicators in title
    workshop_keywords = [
        'Workshop', 'workshop',
        'Symposium', 'symposium',
        'Tutorial', 'tutorial',
        'Panel', 'panel',
        'Keynote:', 'keynote:',
        'keynote speaker',
        'Poster:', 'poster:',
        'Demo:', 'demo:',
        'Demonstration of',
        'Doctoral',
        'Proceedings of',
        'HotPlanet', 'hotplanet',
        'HotNets', 'HotOS', 'HotSDN', 'HotMiddlebox', 'HotCloud',
        'co-located', 'Co-located',
        '@',  # Workshop@Conference
        'FOCI', 'NAI', 'SPIN', 'TAURIN', 'VisNEXT',
        'FFSPIN', 'FIRA', 'NET4us', 'FlexNets', 'OptSys',
        '5G-MeMU', 'MCC', 'AllThingsCellular', 'Internet-QoE',
        'WiNTECH', 'LANC', 'CloudNet'
    ]
    
    for keyword in workshop_keywords:
        if keyword in title:
            return True
    
    # Check venue field too
    venue_match = re.search(r'(?:booktitle|venue)\s*=\s*["{]([^"}]+)["}]', entry, re.IGNORECASE)
    if venue_match:
        venue = venue_match.group(1)
        if '@' in venue or 'Workshop' in venue or 'workshop' in venue:
            return True
    
    return False

def clean_sigcomm():
    """Deep clean SIGCOMM file"""
    filepath = Path('bib/sigcomm.bib')
    
    # Read file
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # Parse entries
    entries, non_entries = parse_bibtex_entries(content)
    
    # Filter out workshop papers
    kept_entries = []
    removed_entries = []
    
    for entry in entries:
        if entry.strip():
            if is_workshop_paper(entry):
                removed_entries.append(entry)
                # Extract title and year for logging
                title_match = re.search(r'title\s*=\s*\{([^}]+)\}', entry)
                year_match = re.search(r'year\s*=\s*\{(\d{4})\}', entry)
                if title_match:
                    title = title_match.group(1)[:60]
                    year = year_match.group(1) if year_match else 'Unknown'
                    print(f'Removing [{year}]: {title}...')
            else:
                kept_entries.append(entry)
    
    print(f'\nTotal removed: {len(removed_entries)} workshop/demo/poster papers')
    print(f'Total kept: {len(kept_entries)} main conference papers')
    
    # Write back
    with open(filepath, 'w', encoding='utf-8') as f:
        # Write comment header
        f.write('% SIGCOMM Main Conference Papers\n')
        f.write('% Workshop/Demo/Poster papers removed\n\n')
        
        # Write kept entries
        for entry in kept_entries:
            f.write(entry)
            f.write('\n\n')
    
    # Save removed entries for review
    removed_file = filepath.with_suffix('.removed')
    with open(removed_file, 'w', encoding='utf-8') as f:
        f.write('% Removed workshop/demo/poster papers\n\n')
        for entry in removed_entries:
            f.write(entry)
            f.write('\n\n')
    
    print(f'\nRemoved entries saved to {removed_file} for review')
    
    return len(removed_entries), len(kept_entries)

if __name__ == "__main__":
    removed, kept = clean_sigcomm()
    print(f"\nDone! File cleaned successfully.")