#!/usr/bin/env python3
"""
Fetch only main conference papers from DBLP, excluding workshops
"""

import requests
import json
import sys
import time
from pathlib import Path
import re

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

def is_workshop_or_poster(title, venue):
    """Check if this is a workshop, poster, or demo paper"""
    indicators = [
        'Workshop', 'workshop', 'Poster', 'poster', 'Demo', 'demo',
        'Doctoral', 'Tutorial', 'Panel', 'Keynote', 'Proceedings of'
    ]
    
    for indicator in indicators:
        if indicator in title or indicator in venue:
            return True
    
    # Check for workshop indicators in venue
    if '@' in venue or 'Co-located' in venue or 'co-located' in venue:
        return True
    
    return False

def fetch_main_conference_papers(conference, year):
    """Fetch main conference papers from DBLP"""
    
    print(f"Fetching {conference.upper()} {year} main conference papers...")
    
    # Try to get papers from the conference proceedings page
    proceedings_url = f"https://dblp.org/db/conf/{conference.lower()}/{conference.lower()}{year}.json"
    
    try:
        response = requests.get(proceedings_url)
        if response.status_code == 200:
            print(f"Found proceedings at {proceedings_url}")
    except:
        pass
    
    # Use search API with specific venue filtering
    url = "https://dblp.org/search/publ/api"
    
    # Try different query patterns
    queries = [
        f'toc:db/conf/{conference.lower()}/{conference.lower()}{year}.bht:',
        f'venue:{conference.upper()} year:{year}',
        f'{conference.upper()} {year}'
    ]
    
    all_papers = []
    
    for query in queries:
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
                            
                            title = clean_title(info.get('title', 'Unknown Title'))
                            venue = info.get('venue', '')
                            
                            # Skip workshop/poster papers
                            if is_workshop_or_poster(title, venue):
                                continue
                            
                            # Skip if venue doesn't match conference
                            if venue and conference.upper() not in venue.upper():
                                continue
                            
                            paper = {
                                'title': title,
                                'authors': [],
                                'year': year,
                                'url': info.get('ee', info.get('url', '')),
                                'doi': '',
                                'venue': venue
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
                            
                            all_papers.append(paper)
                    
                    # If we got papers from proceedings, break
                    if 'toc:' in query and all_papers:
                        break
                        
        except Exception as e:
            print(f"Error with query '{query}': {e}")
            continue
    
    # Deduplicate by title
    seen_titles = set()
    unique_papers = []
    for paper in all_papers:
        title_lower = paper['title'].lower()
        if title_lower not in seen_titles and not is_workshop_or_poster(paper['title'], paper['venue']):
            seen_titles.add(title_lower)
            unique_papers.append(paper)
    
    return unique_papers

def papers_to_bibtex(papers, conference, year):
    """Convert papers to BibTeX format"""
    entries = []
    
    for paper in papers:
        # Generate entry key
        first_author = paper['authors'][0].split()[-1].lower() if paper['authors'] else 'unknown'
        title_words = paper['title'].split()[:2]
        title_part = ''.join(word.lower() for word in title_words if word.isalnum())[:20]
        key = f"{first_author}{str(year)[2:]}{title_part}"
        
        # Make key unique
        key_base = key
        counter = 0
        while any(key in entry for entry in entries):
            counter += 1
            key = f"{key_base}{counter}"
        
        # Format authors
        authors = ' and '.join(paper['authors'])
        
        # Build entry
        entry = f"@inproceedings{{{key},\n"
        entry += f"  title = {{{paper['title']}}},\n"
        if authors:
            entry += f"  author = {{{authors}}},\n"
        entry += f"  booktitle = {{{conference.upper()}}},\n"
        entry += f"  year = {{{year}}}"
        
        if paper.get('url'):
            entry += f",\n  url = {{{paper['url']}}}"
        if paper.get('doi'):
            entry += f",\n  doi = {{{paper['doi'].upper()}}}"
        
        entry += "\n}"
        entries.append(entry)
    
    return entries

def update_bib_file(entries, conference, year):
    """Update the conference bib file with new entries"""
    bib_path = Path(f"bib/{conference.lower()}.bib")
    
    # Read existing content
    existing_content = ""
    existing_keys = set()
    
    if bib_path.exists():
        with open(bib_path, 'r', encoding='utf-8') as f:
            existing_content = f.read()
            # Extract existing keys
            for match in re.finditer(r'@\w+{([^,]+),', existing_content):
                existing_keys.add(match.group(1).strip())
    
    # Filter out duplicates
    new_entries = []
    for entry in entries:
        # Extract key from entry
        key_match = re.search(r'@\w+{([^,]+),', entry)
        if key_match:
            key = key_match.group(1).strip()
            if key not in existing_keys:
                new_entries.append(entry)
    
    if new_entries:
        # Append new entries
        with open(bib_path, 'a', encoding='utf-8') as f:
            f.write(f"\n\n% {conference.upper()} {year} Main Conference Papers (fetched {time.strftime('%Y-%m-%d %H:%M')})\n\n")
            f.write('\n\n'.join(new_entries))
            f.write('\n')
        
        print(f"Added {len(new_entries)} new papers to {bib_path}")
    else:
        print("No new papers to add")
    
    return len(new_entries)

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("Usage: python3 fetch_main_conference.py <conference> <year>")
        print("Example: python3 fetch_main_conference.py sigcomm 2024")
        sys.exit(1)
    
    conference = sys.argv[1].lower()
    year = int(sys.argv[2])
    
    # Fetch papers
    papers = fetch_main_conference_papers(conference, year)
    
    if papers:
        print(f"\nFound {len(papers)} main conference papers for {conference.upper()} {year}")
        
        # Show first few titles
        print("\nFirst 5 papers:")
        for i, paper in enumerate(papers[:5], 1):
            print(f"{i}. {paper['title'][:80]}...")
        
        # Convert to BibTeX
        entries = papers_to_bibtex(papers, conference, year)
        
        # Update file
        added = update_bib_file(entries, conference, year)
        print(f"\nSuccess! Added {added} papers.")
        print("Next: Run 'python3 scripts/sort_papers_by_year.py' to sort the file")
    else:
        print(f"No main conference papers found for {conference.upper()} {year}")