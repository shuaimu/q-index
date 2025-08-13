#!/usr/bin/env python3
"""
Fetch all papers from OSDI 2024 using DBLP API and update the local BibTeX database.

DBLP is a comprehensive computer science bibliography database that provides
structured data for conference proceedings.
"""

import requests
import xml.etree.ElementTree as ET
import re
from datetime import datetime
import json
import sys
import time

def fetch_osdi_2024_from_dblp():
    """Fetch OSDI 2024 papers from DBLP."""
    
    # DBLP URL for OSDI 2024
    # The URL pattern for USENIX conferences in DBLP is typically:
    # https://dblp.org/db/conf/osdi/osdi2024.html
    
    print("Fetching OSDI 2024 papers from DBLP...")
    
    # First, try to get the DBLP XML data for OSDI 2024
    dblp_api_url = "https://dblp.org/search/publ/api"
    
    params = {
        'q': 'venue:OSDI year:2024',
        'format': 'json',
        'h': 100,  # Get up to 100 results
        'f': 0     # Start from first result
    }
    
    try:
        response = requests.get(dblp_api_url, params=params)
        response.raise_for_status()
        data = response.json()
    except requests.RequestException as e:
        print(f"Error fetching from DBLP API: {e}")
        return []
    except json.JSONDecodeError as e:
        print(f"Error parsing JSON response: {e}")
        return []
    
    papers = []
    
    if 'result' in data and 'hits' in data['result']:
        hits = data['result']['hits'].get('hit', [])
        
        for hit in hits:
            info = hit.get('info', {})
            
            paper = {
                'title': info.get('title', ''),
                'authors': info.get('authors', {}).get('author', []),
                'year': info.get('year', '2024'),
                'venue': info.get('venue', 'OSDI'),
                'url': info.get('url', ''),
                'doi': info.get('doi', ''),
                'key': info.get('key', ''),
                'type': info.get('type', 'inproceedings')
            }
            
            # Handle authors - could be a string or list
            if isinstance(paper['authors'], str):
                paper['authors'] = [paper['authors']]
            elif isinstance(paper['authors'], dict):
                # Sometimes DBLP returns author as dict with 'text' field
                paper['authors'] = [paper['authors'].get('text', paper['authors'].get('@text', str(paper['authors'])))]
            elif isinstance(paper['authors'], list):
                # Process list of authors (could be strings or dicts)
                processed_authors = []
                for author in paper['authors']:
                    if isinstance(author, str):
                        processed_authors.append(author)
                    elif isinstance(author, dict):
                        # Extract text from dict
                        author_text = author.get('text', author.get('@text', str(author)))
                        processed_authors.append(author_text)
                paper['authors'] = processed_authors
            
            # Clean up the entry
            paper['title'] = paper['title'].replace('\n', ' ').strip()
            paper['authors'] = [str(author).strip() for author in paper['authors'] if author]
            
            if paper['title']:
                papers.append(paper)
    
    print(f"Found {len(papers)} papers from DBLP")
    return papers

def fetch_from_usenix_direct():
    """Directly fetch from USENIX using their JSON API if available."""
    
    print("Attempting to fetch from USENIX directly...")
    
    # USENIX sometimes provides JSON endpoints for their conferences
    # This is the typical pattern, but may need adjustment
    urls_to_try = [
        "https://www.usenix.org/conference/osdi24/technical-sessions/accepted-papers.json",
        "https://www.usenix.org/conference/osdi24/json/accepted_papers",
        "https://www.usenix.org/sites/default/files/osdi24_papers.json"
    ]
    
    for url in urls_to_try:
        try:
            response = requests.get(url, timeout=10)
            if response.status_code == 200:
                data = response.json()
                papers = parse_usenix_json(data)
                if papers:
                    print(f"Successfully fetched {len(papers)} papers from USENIX")
                    return papers
        except:
            continue
    
    return []

def parse_usenix_json(data):
    """Parse USENIX JSON data to extract papers."""
    papers = []
    
    # The structure varies, so we try different approaches
    if isinstance(data, list):
        for item in data:
            if 'title' in item:
                papers.append({
                    'title': item.get('title', ''),
                    'authors': parse_author_list(item.get('authors', '')),
                    'abstract': item.get('abstract', ''),
                    'url': item.get('url', '')
                })
    elif isinstance(data, dict):
        if 'papers' in data:
            return parse_usenix_json(data['papers'])
        elif 'items' in data:
            return parse_usenix_json(data['items'])
    
    return papers

def parse_author_list(authors):
    """Parse various author formats into a list."""
    if isinstance(authors, list):
        return authors
    elif isinstance(authors, str):
        # Try different separators
        if ';' in authors:
            return [a.strip() for a in authors.split(';')]
        elif ',' in authors and '(' in authors:
            # Complex format with affiliations
            authors = re.sub(r'\([^)]*\)', '', authors)
            return [a.strip() for a in re.split(r',(?![^(]*\))', authors) if a.strip()]
        else:
            return [a.strip() for a in authors.split(',')]
    return []

def create_bibtex_entry_from_data(paper, index):
    """Create a BibTeX entry from paper data."""
    
    # Generate a unique key
    if paper.get('key'):
        # Use DBLP key if available
        key_parts = paper['key'].split('/')
        key = key_parts[-1] if key_parts else f"osdi24paper{index}"
    else:
        # Generate key from first author and title
        if paper.get('authors'):
            first_author = paper['authors'][0]
            last_name = first_author.split()[-1].lower()
            last_name = re.sub(r'[^a-z]', '', last_name)
        else:
            last_name = 'unknown'
        
        title_words = re.findall(r'\w+', paper.get('title', '').lower())[:2]
        title_part = ''.join(title_words)
        
        key = f"{last_name}24{title_part}"
    
    # Build the BibTeX entry
    lines = []
    lines.append(f"@inproceedings{{{key},")
    
    if paper.get('title'):
        # Escape special characters in title
        title = paper['title'].replace('{', '{{').replace('}', '}}')
        # Keep certain words in braces for proper capitalization
        title = re.sub(r'\b(OSDI|RDMA|GPU|CPU|API|SQL|NoSQL|TCP|UDP|HTTP|HTTPS|TLS|SSL|DNS|BGP|SDN|NFV|VM|OS|I/O|SSD|NVMe|FPGA|ASIC|ML|AI|DNN|CNN|RNN|LSTM|GAN)\b', r'{\1}', title)
        lines.append(f'  title = {{{title}}},')
    
    if paper.get('authors'):
        authors = ' and '.join(paper['authors'])
        lines.append(f'  author = {{{authors}}},')
    
    lines.append('  booktitle = {OSDI},')
    lines.append(f'  year = {{2024}},')
    lines.append('  month = jul,')
    
    if paper.get('url'):
        lines.append(f'  url = {{{paper["url"]}}},')
    
    if paper.get('doi'):
        lines.append(f'  doi = {{{paper["doi"]}}},')
    
    if paper.get('abstract'):
        # Clean and format abstract
        abstract = paper['abstract'].replace('\n', ' ').replace('"', "'").strip()
        if len(abstract) > 1000:
            abstract = abstract[:997] + '...'
        lines.append(f'  abstract = {{{abstract}}},')
    
    lines.append('}')
    
    return '\n'.join(lines)

def update_osdi_bib(papers):
    """Update the osdi.bib file with new papers."""
    
    bib_file = 'bib/osdi.bib'
    
    # Read existing entries to check for duplicates
    existing_keys = set()
    existing_titles = set()
    
    try:
        with open(bib_file, 'r') as f:
            content = f.read()
            # Extract existing keys
            key_pattern = r'@inproceedings\{([^,]+),'
            existing_keys = set(re.findall(key_pattern, content))
            # Extract existing titles for duplicate detection
            title_pattern = r'title\s*=\s*\{([^}]+)\}'
            existing_titles = {title.lower().strip() for title in re.findall(title_pattern, content)}
    except FileNotFoundError:
        print(f"File {bib_file} not found, will create it")
        content = ""
    
    # Filter out duplicates
    new_papers = []
    for paper in papers:
        title_lower = paper.get('title', '').lower().strip()
        if title_lower and title_lower not in existing_titles:
            new_papers.append(paper)
        else:
            print(f"Skipping duplicate: {paper.get('title', '')[:50]}...")
    
    if not new_papers:
        print("No new papers to add")
        return
    
    print(f"\nAdding {len(new_papers)} new papers to {bib_file}...")
    
    # Generate BibTeX entries
    new_entries = []
    for i, paper in enumerate(new_papers):
        entry = create_bibtex_entry_from_data(paper, i)
        new_entries.append(entry)
        print(f"  - {paper.get('title', 'Unknown')[:60]}...")
    
    # Append to file
    with open(bib_file, 'a') as f:
        f.write(f'\n% OSDI 2024 Papers (fetched {datetime.now().strftime("%Y-%m-%d")})\n\n')
        for entry in new_entries:
            f.write(entry)
            f.write('\n\n')
    
    print(f"\nSuccessfully added {len(new_papers)} papers to {bib_file}")

def fetch_with_crossref(conference="OSDI", year=2024):
    """Try to fetch papers using CrossRef API."""
    
    print(f"Trying CrossRef API for {conference} {year}...")
    
    url = "https://api.crossref.org/works"
    params = {
        'query': f'{conference} {year}',
        'filter': f'from-pub-date:{year},until-pub-date:{year}',
        'rows': 100
    }
    
    try:
        response = requests.get(url, params=params)
        response.raise_for_status()
        data = response.json()
        
        papers = []
        for item in data.get('message', {}).get('items', []):
            # Filter for likely OSDI papers
            container_title = ' '.join(item.get('container-title', []))
            if 'OSDI' in container_title or 'Operating Systems Design' in container_title:
                paper = {
                    'title': item.get('title', [''])[0],
                    'authors': [],
                    'doi': item.get('DOI', ''),
                    'url': item.get('URL', ''),
                    'abstract': item.get('abstract', '')
                }
                
                # Parse authors
                for author in item.get('author', []):
                    name = f"{author.get('given', '')} {author.get('family', '')}".strip()
                    if name:
                        paper['authors'].append(name)
                
                if paper['title']:
                    papers.append(paper)
        
        print(f"Found {len(papers)} papers from CrossRef")
        return papers
    
    except Exception as e:
        print(f"CrossRef API error: {e}")
        return []

def main():
    """Main function to fetch OSDI 2024 papers."""
    
    print("OSDI 2024 Paper Fetcher")
    print("=" * 50)
    
    all_papers = []
    
    # Try different sources
    print("\n1. Trying DBLP...")
    papers = fetch_osdi_2024_from_dblp()
    if papers:
        all_papers.extend(papers)
    
    if not all_papers:
        print("\n2. Trying CrossRef...")
        papers = fetch_with_crossref("OSDI", 2024)
        if papers:
            all_papers.extend(papers)
    
    if not all_papers:
        print("\n3. Trying USENIX direct...")
        papers = fetch_from_usenix_direct()
        if papers:
            all_papers.extend(papers)
    
    if not all_papers:
        print("\nNo papers found from any source.")
        print("You may need to:")
        print("1. Check if OSDI 2024 proceedings are published yet")
        print("2. Manually visit https://www.usenix.org/conference/osdi24/technical-sessions")
        print("3. Update the script with the correct URLs/endpoints")
        return
    
    # Remove duplicates based on title
    unique_papers = []
    seen_titles = set()
    for paper in all_papers:
        title_lower = paper.get('title', '').lower().strip()
        if title_lower and title_lower not in seen_titles:
            seen_titles.add(title_lower)
            unique_papers.append(paper)
    
    print(f"\nTotal unique papers found: {len(unique_papers)}")
    
    # Update the BibTeX file
    update_osdi_bib(unique_papers)
    
    print("\nDone! Next steps:")
    print("1. Review the added entries in bib/osdi.bib")
    print("2. Sort the entries: python3 scripts/sort_papers_by_year.py")
    print("3. Rebuild the index: cargo build && cargo run -- web")

if __name__ == "__main__":
    main()