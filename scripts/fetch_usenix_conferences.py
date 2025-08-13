#!/usr/bin/env python3
"""
Fetch papers from USENIX conferences using DBLP API.

This script can fetch papers from various USENIX conferences including:
- OSDI (Operating Systems Design and Implementation)
- USENIX ATC (Annual Technical Conference)
- FAST (File and Storage Technologies)
- NSDI (Networked Systems Design and Implementation)
- HotOS (Hot Topics in Operating Systems)
- And more...
"""

import requests
import json
import re
import time
from datetime import datetime
from typing import List, Dict, Optional, Set
import sys
import os

class USENIXFetcher:
    """Fetcher for USENIX conference papers from DBLP."""
    
    # DBLP venue keys for USENIX conferences
    VENUE_MAPPINGS = {
        'osdi': 'conf/osdi',
        'atc': 'conf/usenix',
        'fast': 'conf/fast',
        'nsdi': 'conf/nsdi',
        'hotos': 'conf/hotos',
        'eurosys': 'conf/eurosys',
        'sosp': 'conf/sosp',
        'lisa': 'conf/lisa',
        'sec': 'conf/uss',  # USENIX Security
        'osdi/sosp': 'conf/osdi|conf/sosp',  # Combined query
    }
    
    def __init__(self):
        self.session = requests.Session()
        self.session.headers.update({
            'User-Agent': 'QIndex Academic Fetcher/1.0'
        })
        
    def fetch_from_dblp(self, venue: str, year: Optional[int] = None, start_year: Optional[int] = None, end_year: Optional[int] = None) -> List[Dict]:
        """Fetch papers from DBLP for a specific venue and year range."""
        
        # Get DBLP venue key
        venue_key = self.VENUE_MAPPINGS.get(venue.lower())
        if not venue_key:
            print(f"Warning: Unknown venue {venue}, using direct search")
            venue_key = venue
            
        papers = []
        
        # Build query
        if year:
            years = [year]
        elif start_year and end_year:
            years = list(range(start_year, end_year + 1))
        else:
            # Fetch all years
            years = list(range(1990, 2025))
            
        for y in years:
            print(f"Fetching {venue.upper()} {y}...")
            
            # DBLP API endpoint
            url = "https://dblp.org/search/publ/api"
            
            # Build query - try different query formats
            queries = [
                f'venue:{venue_key}* year:{y}',
                f'toc:{venue_key}/{venue.lower()}{y}:',
                f'key:{venue_key}.*{y}.*'
            ]
            
            for query in queries:
                params = {
                    'q': query,
                    'format': 'json',
                    'h': 1000,  # Max results
                    'f': 0
                }
                
                try:
                    response = self.session.get(url, params=params)
                    response.raise_for_status()
                    data = response.json()
                    
                    if 'result' in data and 'hits' in data['result']:
                        hits = data['result']['hits'].get('hit', [])
                        
                        if hits:
                            print(f"  Found {len(hits)} papers with query: {query}")
                            
                            for hit in hits:
                                paper = self.parse_dblp_entry(hit, venue.upper(), y)
                                if paper:
                                    papers.append(paper)
                            break  # Success, don't try other queries
                            
                except Exception as e:
                    print(f"  Error with query '{query}': {e}")
                    
                time.sleep(0.5)  # Be nice to DBLP
                
        return papers
    
    def parse_dblp_entry(self, hit: Dict, venue: str, year: int) -> Optional[Dict]:
        """Parse a DBLP entry into our paper format."""
        
        info = hit.get('info', {})
        
        paper = {
            'title': info.get('title', ''),
            'year': info.get('year', year),
            'venue': venue,
            'url': info.get('url', ''),
            'ee': info.get('ee', ''),  # Electronic edition (often DOI URL)
            'doi': info.get('doi', ''),
            'key': info.get('key', ''),
            'type': info.get('type', 'inproceedings')
        }
        
        # Parse authors
        authors = info.get('authors', {}).get('author', [])
        if isinstance(authors, str):
            paper['authors'] = [authors]
        elif isinstance(authors, dict):
            paper['authors'] = [authors.get('text', str(authors))]
        elif isinstance(authors, list):
            processed_authors = []
            for author in authors:
                if isinstance(author, str):
                    processed_authors.append(author)
                elif isinstance(author, dict):
                    processed_authors.append(author.get('text', str(author)))
            paper['authors'] = processed_authors
        else:
            paper['authors'] = []
            
        # Clean up
        paper['title'] = paper['title'].strip()
        paper['authors'] = [str(a).strip() for a in paper['authors'] if a]
        
        # Skip if no title
        if not paper['title']:
            return None
            
        return paper
    
    def create_bibtex_entry(self, paper: Dict, index: int) -> str:
        """Create a BibTeX entry from paper data."""
        
        # Generate a unique key
        if paper.get('key'):
            key_parts = paper['key'].split('/')
            key = key_parts[-1] if key_parts else f"{paper['venue'].lower()}{paper['year']}paper{index}"
        else:
            # Generate key from first author and title
            if paper.get('authors'):
                first_author = paper['authors'][0]
                # Handle DBLP author format with numbers (e.g., "John Doe 0001")
                last_name = re.sub(r'\s+\d+$', '', first_author)  # Remove trailing numbers
                last_name = last_name.split()[-1].lower() if last_name.split() else 'unknown'
                last_name = re.sub(r'[^a-z]', '', last_name)
            else:
                last_name = 'unknown'
            
            title_words = re.findall(r'\w+', paper.get('title', '').lower())[:2]
            title_part = ''.join(title_words)
            
            year_short = str(paper['year'])[-2:] if paper.get('year') else '00'
            key = f"{last_name}{year_short}{title_part}"
        
        # Build the BibTeX entry
        lines = []
        lines.append(f"@inproceedings{{{key},")
        
        if paper.get('title'):
            # Preserve special formatting
            title = paper['title']
            # Keep acronyms and special terms in braces
            title = re.sub(r'\b(OSDI|ATC|FAST|NSDI|USENIX|RDMA|GPU|CPU|API|SQL|NoSQL|TCP|UDP|HTTP|HTTPS|TLS|SSL|DNS|BGP|SDN|NFV|VM|OS|I/O|SSD|NVMe|FPGA|ASIC|ML|AI|DNN|CNN|RNN|LSTM|GAN|LLM|KV)\b', r'{\1}', title)
            lines.append(f'  title = {{{title}}},')
        
        if paper.get('authors'):
            authors = ' and '.join(paper['authors'])
            lines.append(f'  author = {{{authors}}},')
        
        # Venue-specific booktitle
        booktitle_map = {
            'OSDI': 'OSDI',
            'ATC': 'USENIX ATC',
            'FAST': 'FAST',
            'NSDI': 'NSDI',
            'HOTOS': 'HotOS',
            'EUROSYS': 'EuroSys',
            'SOSP': 'SOSP',
            'SEC': 'USENIX Security',
        }
        booktitle = booktitle_map.get(paper['venue'].upper(), paper['venue'].upper())
        lines.append(f'  booktitle = {{{booktitle}}},')
        
        if paper.get('year'):
            lines.append(f'  year = {{{paper["year"]}}},')
        
        # Add URL (prefer electronic edition over DBLP URL)
        if paper.get('ee'):
            lines.append(f'  url = {{{paper["ee"]}}},')
        elif paper.get('url'):
            lines.append(f'  url = {{{paper["url"]}}},')
        
        if paper.get('doi'):
            lines.append(f'  doi = {{{paper["doi"]}}},')
        
        lines.append('}')
        
        return '\n'.join(lines)
    
    def update_bib_file(self, venue: str, papers: List[Dict]):
        """Update the appropriate .bib file with new papers."""
        
        # Map venue to bib file
        bib_file_map = {
            'osdi': 'bib/osdi.bib',
            'atc': 'bib/atc.bib',
            'fast': 'bib/fast.bib',
            'nsdi': 'bib/nsdi.bib',
            'hotos': 'bib/hotos.bib',
            'eurosys': 'bib/eurosys.bib',
            'sosp': 'bib/sosp.bib',
        }
        
        bib_file = bib_file_map.get(venue.lower(), f'bib/{venue.lower()}.bib')
        
        # Read existing entries
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
            # Remove special characters for comparison
            title_normalized = re.sub(r'[{}\\]', '', title_lower)
            
            if title_normalized and title_normalized not in existing_titles:
                new_papers.append(paper)
                existing_titles.add(title_normalized)
        
        if not new_papers:
            print(f"No new papers to add to {bib_file}")
            return
        
        print(f"\nAdding {len(new_papers)} new papers to {bib_file}...")
        
        # Generate BibTeX entries
        new_entries = []
        for i, paper in enumerate(new_papers):
            entry = self.create_bibtex_entry(paper, i)
            new_entries.append(entry)
            print(f"  - {paper.get('title', 'Unknown')[:60]}... ({paper.get('year', 'Unknown')})")
        
        # Append to file
        with open(bib_file, 'a') as f:
            timestamp = datetime.now().strftime("%Y-%m-%d %H:%M")
            f.write(f'\n% {venue.upper()} Papers (fetched {timestamp})\n\n')
            for entry in new_entries:
                f.write(entry)
                f.write('\n\n')
        
        print(f"Successfully added {len(new_papers)} papers to {bib_file}")

def main():
    """Main function to fetch USENIX conference papers."""
    
    fetcher = USENIXFetcher()
    
    # Configuration for what to fetch
    fetch_configs = [
        # OSDI - all years except 2024 (already fetched)
        {'venue': 'osdi', 'start_year': 1994, 'end_year': 2023},
        
        # USENIX ATC - all years
        {'venue': 'atc', 'start_year': 1992, 'end_year': 2024},
        
        # FAST - all years
        {'venue': 'fast', 'start_year': 2002, 'end_year': 2024},
        
        # NSDI - all years  
        {'venue': 'nsdi', 'start_year': 2004, 'end_year': 2024},
        
        # HotOS - all years
        {'venue': 'hotos', 'start_year': 1997, 'end_year': 2023},
    ]
    
    print("=" * 70)
    print("USENIX Conference Paper Fetcher")
    print("=" * 70)
    
    all_stats = {}
    
    for config in fetch_configs:
        venue = config['venue']
        start_year = config.get('start_year')
        end_year = config.get('end_year')
        year = config.get('year')
        
        print(f"\n{'='*50}")
        if year:
            print(f"Fetching {venue.upper()} {year}")
        else:
            print(f"Fetching {venue.upper()} ({start_year}-{end_year})")
        print(f"{'='*50}")
        
        papers = fetcher.fetch_from_dblp(venue, year, start_year, end_year)
        
        if papers:
            # Group by year for statistics
            papers_by_year = {}
            for p in papers:
                y = p.get('year', 'Unknown')
                if y not in papers_by_year:
                    papers_by_year[y] = []
                papers_by_year[y].append(p)
            
            print(f"\nFound {len(papers)} total papers:")
            for y in sorted(papers_by_year.keys()):
                if isinstance(y, int) or (isinstance(y, str) and y.isdigit()):
                    print(f"  {y}: {len(papers_by_year[y])} papers")
            
            # Update bib file
            fetcher.update_bib_file(venue, papers)
            
            all_stats[venue] = len(papers)
        else:
            print(f"No papers found for {venue.upper()}")
            all_stats[venue] = 0
    
    # Print summary
    print("\n" + "=" * 70)
    print("SUMMARY")
    print("=" * 70)
    for venue, count in all_stats.items():
        print(f"{venue.upper()}: {count} new papers added")
    
    print("\nNext steps:")
    print("1. Run: python3 scripts/sort_papers_by_year.py")
    print("2. Rebuild: cargo build --release")
    print("3. Run server: ./target/release/qindex web")

if __name__ == "__main__":
    main()