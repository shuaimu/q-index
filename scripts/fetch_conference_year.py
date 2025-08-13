#!/usr/bin/env python3
"""
Fetch papers from a specific conference and year using DBLP API.
Usage: python3 fetch_conference_year.py <conference> <year>
Example: python3 fetch_conference_year.py osdi 2023
"""

import requests
import json
import re
import sys
import time
from datetime import datetime

def fetch_conference_papers(conference: str, year: int):
    """Fetch papers for a specific conference and year from DBLP."""
    
    # DBLP venue mappings
    venue_map = {
        'osdi': 'OSDI',
        'atc': 'USENIX ATC', 
        'fast': 'FAST',
        'nsdi': 'NSDI',
        'hotos': 'HotOS',
        'eurosys': 'EuroSys',
        'sosp': 'SOSP',
    }
    
    venue_name = venue_map.get(conference.lower(), conference.upper())
    
    print(f"Fetching {venue_name} {year} papers from DBLP...")
    
    # DBLP API
    url = "https://dblp.org/search/publ/api"
    
    # Try different query formats
    queries = [
        f'venue:{venue_name} year:{year}',
        f'{venue_name} {year}',
        f'conf/{conference.lower()}* year:{year}'
    ]
    
    all_papers = []
    
    for query in queries:
        params = {
            'q': query,
            'format': 'json',
            'h': 200,  # Max results per query
            'f': 0
        }
        
        try:
            response = requests.get(url, params=params, timeout=30)
            response.raise_for_status()
            data = response.json()
            
            if 'result' in data and 'hits' in data['result']:
                hits = data['result']['hits'].get('hit', [])
                
                if hits:
                    print(f"Found {len(hits)} papers with query: {query}")
                    
                    for hit in hits:
                        info = hit.get('info', {})
                        
                        # Filter to ensure it's from the right conference
                        venue = info.get('venue', '').lower()
                        key = info.get('key', '').lower()
                        
                        # Check if this is actually from our conference
                        if conference.lower() in key or conference.lower() in venue:
                            paper = {
                                'title': info.get('title', ''),
                                'year': info.get('year', year),
                                'authors': [],
                                'url': info.get('url', ''),
                                'ee': info.get('ee', ''),
                                'doi': info.get('doi', ''),
                                'key': info.get('key', ''),
                            }
                            
                            # Parse authors
                            authors = info.get('authors', {}).get('author', [])
                            if isinstance(authors, str):
                                paper['authors'] = [authors]
                            elif isinstance(authors, list):
                                for author in authors:
                                    if isinstance(author, str):
                                        paper['authors'].append(author)
                                    elif isinstance(author, dict):
                                        paper['authors'].append(author.get('text', str(author)))
                            
                            if paper['title']:
                                all_papers.append(paper)
                    
                    if all_papers:
                        break  # Found papers, don't try other queries
                        
        except Exception as e:
            print(f"Error with query '{query}': {e}")
        
        time.sleep(0.5)  # Be nice to DBLP
    
    # Deduplicate by title
    seen_titles = set()
    unique_papers = []
    for paper in all_papers:
        title_lower = paper['title'].lower().strip()
        if title_lower not in seen_titles:
            seen_titles.add(title_lower)
            unique_papers.append(paper)
    
    return unique_papers

def create_bibtex_entries(papers, conference):
    """Convert papers to BibTeX format."""
    
    entries = []
    
    for i, paper in enumerate(papers):
        # Generate key
        if paper.get('authors'):
            first_author = paper['authors'][0]
            # Remove DBLP number suffixes
            last_name = re.sub(r'\s+\d+$', '', first_author)
            last_name = last_name.split()[-1].lower() if last_name.split() else 'unknown'
            last_name = re.sub(r'[^a-z]', '', last_name)
        else:
            last_name = 'unknown'
        
        title_words = re.findall(r'\w+', paper.get('title', '').lower())[:2]
        title_part = ''.join(title_words)
        year_short = str(paper['year'])[-2:]
        
        key = f"{last_name}{year_short}{title_part}{i}"
        
        # Build entry
        lines = [f"@inproceedings{{{key},"]
        
        # Title with preserved formatting
        if paper.get('title'):
            title = paper['title']
            # Keep acronyms in braces
            title = re.sub(r'\b([A-Z]{2,})\b', r'{\1}', title)
            lines.append(f'  title = {{{title}}},')
        
        # Authors
        if paper.get('authors'):
            authors = ' and '.join(paper['authors'])
            lines.append(f'  author = {{{authors}}},')
        
        # Booktitle
        booktitle_map = {
            'osdi': 'OSDI',
            'atc': 'USENIX ATC',
            'fast': 'FAST',
            'nsdi': 'NSDI',
            'hotos': 'HotOS',
            'eurosys': 'EuroSys',
            'sosp': 'SOSP',
        }
        booktitle = booktitle_map.get(conference.lower(), conference.upper())
        lines.append(f'  booktitle = {{{booktitle}}},')
        
        # Year
        lines.append(f'  year = {{{paper["year"]}}},')
        
        # URL
        if paper.get('ee'):
            lines.append(f'  url = {{{paper["ee"]}}},')
        elif paper.get('url'):
            lines.append(f'  url = {{{paper["url"]}}},')
        
        # DOI
        if paper.get('doi'):
            lines.append(f'  doi = {{{paper["doi"]}}},')
        
        lines.append('}')
        
        entries.append('\n'.join(lines))
    
    return entries

def update_bib_file(conference, year, entries):
    """Update the BibTeX file with new entries."""
    
    bib_file = f'bib/{conference.lower()}.bib'
    
    # Read existing file
    try:
        with open(bib_file, 'r') as f:
            content = f.read()
            # Extract existing titles for duplicate detection
            title_pattern = r'title\s*=\s*\{([^}]+)\}'
            existing_titles = {title.lower().strip() for title in re.findall(title_pattern, content)}
    except FileNotFoundError:
        existing_titles = set()
    
    # Filter out duplicates
    new_entries = []
    for entry in entries:
        # Extract title from entry
        title_match = re.search(r'title\s*=\s*\{([^}]+)\}', entry)
        if title_match:
            title = title_match.group(1).lower().strip()
            # Remove formatting for comparison
            title_normalized = re.sub(r'[{}\\]', '', title)
            if title_normalized not in existing_titles:
                new_entries.append(entry)
                existing_titles.add(title_normalized)
    
    if not new_entries:
        print(f"No new papers to add (all {len(entries)} papers already exist)")
        return 0
    
    # Append to file
    with open(bib_file, 'a') as f:
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M")
        f.write(f'\n% {conference.upper()} {year} Papers (fetched {timestamp})\n\n')
        for entry in new_entries:
            f.write(entry)
            f.write('\n\n')
    
    print(f"Added {len(new_entries)} new papers to {bib_file}")
    return len(new_entries)

def main():
    if len(sys.argv) != 3:
        print("Usage: python3 fetch_conference_year.py <conference> <year>")
        print("Example: python3 fetch_conference_year.py osdi 2023")
        print("Supported conferences: osdi, atc, fast, nsdi, hotos, eurosys, sosp")
        sys.exit(1)
    
    conference = sys.argv[1].lower()
    try:
        year = int(sys.argv[2])
    except ValueError:
        print(f"Error: Year must be a number, got '{sys.argv[2]}'")
        sys.exit(1)
    
    # Fetch papers
    papers = fetch_conference_papers(conference, year)
    
    if not papers:
        print(f"No papers found for {conference.upper()} {year}")
        return
    
    print(f"\nFound {len(papers)} papers for {conference.upper()} {year}")
    
    # Convert to BibTeX
    entries = create_bibtex_entries(papers, conference)
    
    # Update bib file
    added = update_bib_file(conference, year, entries)
    
    if added > 0:
        print(f"\nSuccess! Added {added} papers.")
        print("Next: Run 'python3 scripts/sort_papers_by_year.py' to sort the file")

if __name__ == "__main__":
    main()