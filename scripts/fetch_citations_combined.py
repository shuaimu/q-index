#!/usr/bin/env python3
"""
Combined fetcher: DBLP for paper metadata + OpenAlex for citation counts.
No rate limits on either service!
"""

import os
import sys
import time
import json
import hashlib
import requests
from pathlib import Path
from typing import Dict, Optional
import re
import bibtexparser

# Add project root to path
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

class CombinedFetcher:
    def __init__(self):
        self.session = requests.Session()
        self.session.headers.update({
            'User-Agent': 'QIndex/1.0 (mailto:research@example.com)'  # Update with your email
        })
        
        # Cache setup
        self.cache_dir = Path("cache/citations")
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        
        self.combined_cache_file = self.cache_dir / "combined_cache.json"
        self.failed_cache_file = self.cache_dir / "combined_failed.json"
        
        self.load_cache()
        
        # Stats
        self.stats = {
            'total': 0,
            'dblp_found': 0,
            'openalex_found': 0,
            'citations_found': 0,
            'from_cache': 0
        }
    
    def load_cache(self):
        """Load existing cache."""
        if self.combined_cache_file.exists():
            with open(self.combined_cache_file, 'r') as f:
                self.cache = json.load(f)
        else:
            self.cache = {}
        
        if self.failed_cache_file.exists():
            with open(self.failed_cache_file, 'r') as f:
                self.failed = json.load(f)
        else:
            self.failed = {}
    
    def save_cache(self):
        """Save cache to disk."""
        with open(self.combined_cache_file, 'w') as f:
            json.dump(self.cache, f, indent=2)
        
        with open(self.failed_cache_file, 'w') as f:
            json.dump(self.failed, f, indent=2)
    
    def get_paper_key(self, title, authors=None, year=None):
        """Generate unique key for paper."""
        title_normalized = re.sub(r'[^\w\s]', '', title.lower()).strip()
        author_str = authors[0].lower() if authors and len(authors) > 0 else ""
        author_normalized = re.sub(r'[^\w\s]', '', author_str).strip()
        
        key_string = f"{title_normalized}_{author_normalized}_{year}"
        return hashlib.md5(key_string.encode()).hexdigest()
    
    def search_dblp(self, title, authors=None, year=None):
        """Search DBLP for paper metadata."""
        title_clean = re.sub(r'[{}\[\]()]', '', title)
        title_clean = re.sub(r'\s+', ' ', title_clean).strip()
        
        query = f'"{title_clean}"'
        if authors and len(authors) > 0:
            last_name = authors[0].split()[-1] if authors[0] else ""
            if last_name:
                query += f' {last_name}'
        
        url = "https://dblp.org/search/publ/api"
        params = {
            'q': query,
            'format': 'json',
            'h': 5,
            'f': 0
        }
        
        try:
            response = self.session.get(url, params=params)
            response.raise_for_status()
            
            data = response.json()
            result = data.get('result', {})
            
            if result.get('status', {}).get('@code') == '200':
                hits = result.get('hits', {}).get('hit', [])
                
                for hit in hits:
                    info = hit.get('info', {})
                    
                    # Basic year check
                    if year and str(year) != str(info.get('year', '')):
                        continue
                    
                    # Extract DOI if available
                    doi = None
                    ee = info.get('ee', '')
                    if 'doi.org/' in ee:
                        doi_match = re.search(r'doi.org/(.+)', ee)
                        if doi_match:
                            doi = doi_match.group(1)
                    
                    return {
                        'title': info.get('title', ''),
                        'year': info.get('year'),
                        'venue': info.get('venue', ''),
                        'dblp_key': info.get('key', ''),
                        'doi': doi,
                        'source': 'dblp'
                    }
        except Exception as e:
            print(f"  DBLP error: {e}")
        
        return None
    
    def search_openalex(self, title, year=None, doi=None):
        """Search OpenAlex for citation counts."""
        try:
            # Try DOI first if available
            if doi:
                url = f"https://api.openalex.org/works/doi:{doi}"
                response = self.session.get(url)
                
                if response.status_code == 200:
                    data = response.json()
                    return {
                        'citation_count': data.get('cited_by_count', 0),
                        'openalex_id': data.get('id'),
                        'title': data.get('title'),
                        'source': 'openalex'
                    }
            
            # Search by title
            query = f'title.search:"{title}"'
            if year:
                query += f' AND publication_year:{year}'
            
            url = f"https://api.openalex.org/works?filter={query}"
            response = self.session.get(url)
            
            if response.status_code == 200:
                data = response.json()
                results = data.get('results', [])
                
                if results:
                    work = results[0]
                    return {
                        'citation_count': work.get('cited_by_count', 0),
                        'openalex_id': work.get('id'),
                        'title': work.get('title'),
                        'source': 'openalex'
                    }
        except Exception as e:
            print(f"  OpenAlex error: {e}")
        
        return None
    
    def fetch_combined(self, title, authors=None, year=None):
        """Fetch from DBLP + OpenAlex."""
        paper_key = self.get_paper_key(title, authors, year)
        
        # Check cache
        if paper_key in self.cache:
            self.stats['from_cache'] += 1
            return self.cache[paper_key]
        
        # Check failed cache
        if paper_key in self.failed:
            if time.time() - self.failed[paper_key] < 7 * 24 * 3600:
                return None
        
        self.stats['total'] += 1
        
        # Step 1: Get metadata from DBLP
        dblp_result = self.search_dblp(title, authors, year)
        
        if dblp_result:
            self.stats['dblp_found'] += 1
            
            # Step 2: Get citations from OpenAlex
            openalex_result = self.search_openalex(
                title=title,
                year=year,
                doi=dblp_result.get('doi')
            )
            
            if openalex_result:
                self.stats['openalex_found'] += 1
                self.stats['citations_found'] += 1
                
                # Combine results
                combined = {
                    **dblp_result,
                    'citation_count': openalex_result['citation_count'],
                    'openalex_id': openalex_result.get('openalex_id'),
                    'fetched_at': time.time()
                }
            else:
                # DBLP found but no citations
                combined = {
                    **dblp_result,
                    'citation_count': 0,
                    'note': 'DBLP found, OpenAlex not found',
                    'fetched_at': time.time()
                }
        else:
            # Try OpenAlex directly
            openalex_result = self.search_openalex(title, year)
            
            if openalex_result:
                self.stats['openalex_found'] += 1
                self.stats['citations_found'] += 1
                
                combined = {
                    'title': title,
                    'year': year,
                    'citation_count': openalex_result['citation_count'],
                    'openalex_id': openalex_result.get('openalex_id'),
                    'source': 'openalex_only',
                    'fetched_at': time.time()
                }
            else:
                # Not found anywhere
                self.failed[paper_key] = time.time()
                return None
        
        # Cache result
        self.cache[paper_key] = combined
        
        # Save periodically
        if len(self.cache) % 10 == 0:
            self.save_cache()
        
        return combined
    
    def process_bib_file(self, bib_file):
        """Process a BibTeX file."""
        print(f"\nProcessing {bib_file.name}...")
        
        with open(bib_file, 'r', encoding='utf-8') as f:
            bib_content = f.read()
        
        parser = bibtexparser.bparser.BibTexParser(common_strings=True)
        parser.ignore_nonstandard_types = False
        parser.homogenize_fields = True
        
        try:
            bib_database = bibtexparser.loads(bib_content, parser)
        except Exception as e:
            print(f"Error parsing {bib_file}: {e}")
            return []
        
        results = []
        total = len(bib_database.entries)
        
        for i, entry in enumerate(bib_database.entries, 1):
            title = entry.get('title', '').replace('{', '').replace('}', '')
            
            # Parse authors
            authors_str = entry.get('author', '')
            authors = []
            if authors_str:
                author_list = authors_str.split(' and ')
                for author in author_list:
                    author = author.strip()
                    if ',' in author:
                        parts = author.split(',', 1)
                        author = f"{parts[1].strip()} {parts[0].strip()}"
                    authors.append(author)
            
            year = entry.get('year', '')
            
            if not title:
                continue
            
            # Progress
            if i % 50 == 0:
                print(f"  Progress: {i}/{total} papers...")
                self.print_mini_stats()
            
            # Fetch combined data
            result = self.fetch_combined(title, authors, year)
            
            if result:
                results.append(result)
                citations = result.get('citation_count', 'N/A')
                print(f"  ✓ {title[:50]}... ({citations} citations)")
            
            # No delay needed - both APIs have no rate limits!
            # But we'll add a tiny delay to be extra polite
            time.sleep(0.05)
        
        return results
    
    def print_mini_stats(self):
        """Print quick stats during processing."""
        total = self.stats['total']
        if total > 0:
            dblp_rate = (self.stats['dblp_found'] / total) * 100
            citation_rate = (self.stats['citations_found'] / total) * 100
            print(f"    Stats: {total} queries, {dblp_rate:.1f}% in DBLP, {citation_rate:.1f}% with citations")
    
    def print_stats(self):
        """Print detailed statistics."""
        print("\n" + "="*60)
        print("Combined Fetching Statistics:")
        print(f"  Total queries: {self.stats['total']}")
        print(f"  DBLP found: {self.stats['dblp_found']}")
        print(f"  OpenAlex found: {self.stats['openalex_found']}")
        print(f"  With citations: {self.stats['citations_found']}")
        print(f"  From cache: {self.stats['from_cache']}")
        print(f"  Total cached: {len(self.cache)}")
        
        if self.cache:
            # Calculate citation statistics
            citations = [p.get('citation_count', 0) for p in self.cache.values() 
                        if p.get('citation_count') is not None]
            if citations:
                print(f"\nCitation Statistics:")
                print(f"  Total citations: {sum(citations):,}")
                print(f"  Average citations: {sum(citations)/len(citations):.1f}")
                print(f"  Max citations: {max(citations):,}")
                print(f"  Papers with 100+ citations: {len([c for c in citations if c >= 100])}")
        
        print("="*60)

def main():
    """Main function."""
    import argparse
    
    parser = argparse.ArgumentParser(description='Fetch citations from DBLP + OpenAlex')
    parser.add_argument('bib_file', nargs='?', help='BibTeX file to process')
    parser.add_argument('--all', action='store_true', help='Process all BibTeX files')
    parser.add_argument('--conference', help='Process specific conference (e.g., sosp, osdi)')
    
    args = parser.parse_args()
    
    fetcher = CombinedFetcher()
    
    if args.conference:
        # Process specific conference
        bib_file = Path(f"bib/{args.conference}.bib")
        if not bib_file.exists():
            print(f"Error: {bib_file} not found")
            return
        
        results = fetcher.process_bib_file(bib_file)
        print(f"\nCompleted {args.conference}: {len(results)} papers with data")
        
    elif args.all:
        # Process all files
        bib_dir = Path("bib")
        
        # Priority order - systems conferences first
        priority = ['sosp', 'osdi', 'nsdi', 'sigcomm', 'sigmod', 'vldb', 
                   'pldi', 'popl', 'asplos', 'eurosys', 'atc', 'fast']
        
        # Get all bib files
        all_files = list(bib_dir.glob("*.bib"))
        
        # Sort by priority
        def sort_key(f):
            name = f.stem.lower()
            if name in priority:
                return priority.index(name)
            return len(priority) + 1
        
        sorted_files = sorted(all_files, key=sort_key)
        
        print(f"Processing {len(sorted_files)} BibTeX files...")
        print(f"Order: {', '.join(f.stem for f in sorted_files[:10])}...")
        
        for bib_file in sorted_files:
            results = fetcher.process_bib_file(bib_file)
            fetcher.save_cache()
            print(f"Saved cache with {len(fetcher.cache)} total papers")
    
    elif args.bib_file:
        # Process single file
        bib_file = Path(args.bib_file)
        if not bib_file.exists():
            print(f"Error: {bib_file} not found")
            return
        
        results = fetcher.process_bib_file(bib_file)
        print(f"\nCompleted: {len(results)} papers with data")
    
    else:
        print("Usage:")
        print("  python fetch_citations_combined.py bib/sosp.bib")
        print("  python fetch_citations_combined.py --conference sosp")
        print("  python fetch_citations_combined.py --all")
    
    fetcher.save_cache()
    fetcher.print_stats()

if __name__ == "__main__":
    main()