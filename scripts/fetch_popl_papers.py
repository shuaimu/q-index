#!/usr/bin/env python3
"""
Fetch POPL papers from DBLP by year
"""

import requests
import json
import sys
import time
from pathlib import Path

def clean_title(title):
    """Clean title by removing special characters and normalizing"""
    if isinstance(title, dict):
        title = title.get('text', title.get('@text', str(title)))
    title = str(title).strip()
    # Remove HTML entities
    title = title.replace('&apos;', "'")
    title = title.replace('&quot;', '"')
    title = title.replace('&amp;', '&')
    title = title.replace('&lt;', '<')
    title = title.replace('&gt;', '>')
    return title

def clean_author(author):
    """Clean author name"""
    if isinstance(author, dict):
        author = author.get('text', author.get('@text', str(author)))
    return str(author).strip()

def fetch_popl_year(year):
    """Fetch POPL papers for a specific year"""
    
    print(f"Fetching POPL {year} papers from DBLP...")
    
    all_papers = []
    
    # Try different queries
    queries = [
        f'POPL {year}',
        f'venue:POPL year:{year}',
        f'Principles of Programming Languages {year}'
    ]
    
    for query in queries:
        url = "https://dblp.org/search/publ/api"
        params = {
            'q': query,
            'format': 'json',
            'h': 200,
            'f': 0
        }
        
        try:
            response = requests.get(url, params=params)
            data = response.json()
            
            if 'result' in data and 'hits' in data['result']:
                hits = data['result']['hits']
                total = int(hits.get('@total', 0))
                
                if total > 0:
                    print(f"Found {total} papers with query: {query}")
                    
                    if 'hit' in hits:
                        papers = hits['hit'] if isinstance(hits['hit'], list) else [hits['hit']]
                        
                        for hit in papers:
                            info = hit.get('info', {})
                            
                            # Filter for POPL papers
                            venue = info.get('venue', '')
                            if 'POPL' in venue or 'Principles of Programming' in info.get('title', ''):
                                paper = {
                                    'title': clean_title(info.get('title', 'Unknown Title')),
                                    'authors': [],
                                    'year': year,
                                    'url': info.get('ee', info.get('url', '')),
                                    'doi': ''
                                }
                                
                                # Extract authors
                                authors_data = info.get('authors', {})
                                if authors_data:
                                    author_list = authors_data.get('author', [])
                                    if not isinstance(author_list, list):
                                        author_list = [author_list]
                                    
                                    for author in author_list:
                                        paper['authors'].append(clean_author(author))
                                
                                # Extract DOI
                                if 'doi' in info:
                                    paper['doi'] = info['doi']
                                elif paper['url'] and 'doi.org/' in paper['url']:
                                    paper['doi'] = paper['url'].split('doi.org/')[-1]
                                
                                # Check if paper is actually from POPL
                                if paper['authors'] and paper['title'] != 'Unknown Title':
                                    all_papers.append(paper)
                    
                    if all_papers:
                        break
                        
        except Exception as e:
            print(f"Error fetching data: {e}")
            continue
    
    # Deduplicate by title
    seen_titles = set()
    unique_papers = []
    for paper in all_papers:
        title_lower = paper['title'].lower()
        if title_lower not in seen_titles:
            seen_titles.add(title_lower)
            unique_papers.append(paper)
    
    return unique_papers

def papers_to_bibtex(papers, year):
    """Convert papers to BibTeX format"""
    entries = []
    
    for paper in papers:
        # Generate entry key
        first_author = paper['authors'][0].split()[-1].lower() if paper['authors'] else 'unknown'
        title_words = paper['title'].split()[:2]
        title_part = ''.join(word.lower() for word in title_words if word.isalnum())[:20]
        key = f"{first_author}{str(year)[2:]}{title_part}"
        
        # Format authors
        authors = ' and '.join(paper['authors'])
        
        # Build entry
        entry = f"@inproceedings{{{key},\n"
        entry += f"  title = {{{paper['title']}}},\n"
        if authors:
            entry += f"  author = {{{authors}}},\n"
        entry += f"  booktitle = {{POPL}},\n"
        entry += f"  year = {{{year}}}"
        
        if paper.get('url'):
            entry += f",\n  url = {{{paper['url']}}}"
        if paper.get('doi'):
            entry += f",\n  doi = {{{paper['doi'].upper()}}}"
        
        entry += "\n}"
        entries.append(entry)
    
    return entries

def update_bib_file(entries, year):
    """Update the POPL bib file"""
    bib_path = Path("bib/popl.bib")
    
    # Read existing content
    existing_content = ""
    existing_keys = set()
    
    if bib_path.exists():
        with open(bib_path, 'r', encoding='utf-8') as f:
            existing_content = f.read()
            # Extract existing keys
            import re
            for match in re.finditer(r'@\w+{([^,]+),', existing_content):
                existing_keys.add(match.group(1).strip())
    
    # Filter out duplicates
    new_entries = []
    for entry in entries:
        # Extract key from entry
        import re
        key_match = re.search(r'@\w+{([^,]+),', entry)
        if key_match:
            key = key_match.group(1).strip()
            if key not in existing_keys:
                new_entries.append(entry)
    
    if new_entries:
        # Append new entries
        with open(bib_path, 'a', encoding='utf-8') as f:
            f.write(f"\n\n% POPL {year} Papers (fetched {time.strftime('%Y-%m-%d %H:%M')})\n\n")
            f.write('\n\n'.join(new_entries))
            f.write('\n')
        
        print(f"Added {len(new_entries)} new papers to {bib_path}")
    else:
        print("No new papers to add")
    
    return len(new_entries)

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python3 fetch_popl_papers.py <year>")
        sys.exit(1)
    
    year = int(sys.argv[1])
    
    # Fetch papers
    papers = fetch_popl_year(year)
    
    if papers:
        print(f"\nFound {len(papers)} papers for POPL {year}")
        
        # Convert to BibTeX
        entries = papers_to_bibtex(papers, year)
        
        # Update file
        added = update_bib_file(entries, year)
        print(f"\nSuccess! Added {added} papers.")
        print("Next: Run 'python3 scripts/sort_papers_by_year.py' to sort the file")
    else:
        print(f"No POPL papers found for {year}")