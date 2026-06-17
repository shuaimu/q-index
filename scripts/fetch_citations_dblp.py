#!/usr/bin/env python3
"""
DBLP Citation Fetcher - No rate limits, perfect for CS papers.
DBLP API Documentation: https://dblp.org/faq/How+to+use+the+dblp+search+API.html
"""

import os
import sys
import time
import json
import hashlib
import requests
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Dict, List, Optional, Tuple
from datetime import datetime
import re
import bibtexparser

# Add project root to path
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

class DBLPFetcher:
    def __init__(self):
        self.session = requests.Session()
        self.session.headers.update({
            'User-Agent': 'QIndex/1.0 (Academic Research Tool)'
        })
        
        # Cache setup
        self.cache_dir = Path("cache/citations")
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        
        self.paper_cache_file = self.cache_dir / "dblp_cache.json"
        self.failed_cache_file = self.cache_dir / "dblp_failed.json"
        self.mapping_file = self.cache_dir / "dblp_mapping.json"
        
        self.load_cache()
        
        # Stats
        self.stats = {
            'total_queries': 0,
            'successful': 0,
            'failed': 0,
            'from_cache': 0
        }
    
    def load_cache(self):
        """Load existing cache files."""
        if self.paper_cache_file.exists():
            with open(self.paper_cache_file, 'r') as f:
                self.paper_cache = json.load(f)
        else:
            self.paper_cache = {}
        
        if self.failed_cache_file.exists():
            with open(self.failed_cache_file, 'r') as f:
                self.failed_cache = json.load(f)
        else:
            self.failed_cache = {}
        
        if self.mapping_file.exists():
            with open(self.mapping_file, 'r') as f:
                self.dblp_mapping = json.load(f)
        else:
            self.dblp_mapping = {}
    
    def save_cache(self):
        """Save cache to disk."""
        with open(self.paper_cache_file, 'w') as f:
            json.dump(self.paper_cache, f, indent=2)
        
        with open(self.failed_cache_file, 'w') as f:
            json.dump(self.failed_cache, f, indent=2)
        
        with open(self.mapping_file, 'w') as f:
            json.dump(self.dblp_mapping, f, indent=2)
    
    def get_paper_key(self, title, authors=None, year=None):
        """Generate unique key for paper."""
        # Normalize title
        title_normalized = re.sub(r'[^\w\s]', '', title.lower()).strip()
        author_str = authors[0].lower() if authors and len(authors) > 0 else ""
        author_normalized = re.sub(r'[^\w\s]', '', author_str).strip()
        
        key_string = f"{title_normalized}_{author_normalized}_{year}"
        return hashlib.md5(key_string.encode()).hexdigest()
    
    def search_dblp(self, title, authors=None, year=None, venue=None):
        """
        Search DBLP for a paper.
        Returns detailed information including DBLP key.
        """
        # Clean title for search
        title_clean = re.sub(r'[{}\[\]()]', '', title)
        title_clean = re.sub(r'\s+', ' ', title_clean).strip()
        
        # Build search query
        query_parts = []
        
        # Title search - use exact phrase
        query_parts.append(f'"{title_clean}"')
        
        # Add author if available
        if authors and len(authors) > 0:
            # Extract last name from first author
            author_parts = authors[0].split()
            if author_parts:
                last_name = author_parts[-1]
                query_parts.append(last_name)
        
        # Combine query
        query = ' '.join(query_parts)
        
        # DBLP search API
        url = "https://dblp.org/search/publ/api"
        params = {
            'q': query,
            'format': 'json',
            'h': 10,  # Get top 10 results to find best match
            'f': 0    # Start from first result
        }
        
        try:
            response = self.session.get(url, params=params)
            response.raise_for_status()
            
            data = response.json()
            result = data.get('result', {})
            
            if result.get('status', {}).get('@code') == '200':
                hits = result.get('hits', {})
                hit_list = hits.get('hit', [])
                
                if not hit_list:
                    return None
                
                # Find best match
                best_match = None
                best_score = 0
                
                for hit in hit_list:
                    info = hit.get('info', {})
                    
                    # Calculate match score
                    score = 0
                    
                    # Check title similarity
                    dblp_title = info.get('title', '').lower()
                    title_lower = title_clean.lower()
                    
                    if title_lower in dblp_title or dblp_title in title_lower:
                        score += 10
                    
                    # Check year match
                    if year and str(year) == str(info.get('year', '')):
                        score += 5
                    
                    # Check author match
                    dblp_authors = info.get('authors', {}).get('author', [])
                    if not isinstance(dblp_authors, list):
                        dblp_authors = [dblp_authors]
                    
                    if authors:
                        for author in authors[:3]:  # Check first 3 authors
                            author_last = author.split()[-1].lower() if author else ""
                            for dblp_author in dblp_authors:
                                if isinstance(dblp_author, dict):
                                    dblp_author_text = dblp_author.get('text', '').lower()
                                else:
                                    dblp_author_text = str(dblp_author).lower()
                                
                                if author_last in dblp_author_text:
                                    score += 2
                                    break
                    
                    # Check venue match if provided
                    if venue:
                        dblp_venue = info.get('venue', '').lower()
                        if venue.lower() in dblp_venue:
                            score += 3
                    
                    if score > best_score:
                        best_score = score
                        best_match = info
                
                if best_match and best_score >= 10:  # Minimum score threshold
                    return self.parse_dblp_info(best_match)
                
            return None
            
        except Exception as e:
            print(f"DBLP search error: {e}")
            return None
    
    def parse_dblp_info(self, info):
        """Parse DBLP info into standard format."""
        # Extract authors
        authors_data = info.get('authors', {}).get('author', [])
        if not isinstance(authors_data, list):
            authors_data = [authors_data]
        
        authors = []
        for author in authors_data:
            if isinstance(author, dict):
                authors.append(author.get('text', ''))
            else:
                authors.append(str(author))
        
        # Build result
        result = {
            'title': info.get('title', ''),
            'authors': authors,
            'year': info.get('year'),
            'venue': info.get('venue', ''),
            'type': info.get('type', ''),
            'key': info.get('key', ''),
            'url': info.get('url', ''),
            'ee': info.get('ee', ''),  # Electronic edition (usually DOI link)
            'source': 'dblp'
        }
        
        # Extract DOI if available
        if result['ee']:
            if 'doi.org/' in result['ee']:
                doi_match = re.search(r'doi.org/(.+)', result['ee'])
                if doi_match:
                    result['doi'] = doi_match.group(1)
        
        return result
    
    def get_citations_from_dblp_key(self, dblp_key):
        """
        Get citation count for a DBLP key.
        Note: DBLP doesn't provide citation counts directly,
        but we can get some metrics.
        """
        # DBLP doesn't provide citation counts in their API
        # We would need to use another service for this
        # For now, we'll mark it as found but without citation count
        return {
            'citation_count': None,  # DBLP doesn't provide this
            'dblp_url': f"https://dblp.org/rec/{dblp_key}",
            'note': 'DBLP found, use CrossRef/OpenAlex for citation count'
        }
    
    def fetch_paper(self, title, authors=None, year=None, venue=None, doi=None):
        """
        Fetch paper information from DBLP.
        """
        paper_key = self.get_paper_key(title, authors, year)
        
        # Check cache
        if paper_key in self.paper_cache:
            self.stats['from_cache'] += 1
            return self.paper_cache[paper_key]
        
        # Check failed cache
        if paper_key in self.failed_cache:
            # Skip if recently failed (within 7 days)
            if time.time() - self.failed_cache[paper_key] < 7 * 24 * 3600:
                return None
        
        # Search DBLP
        self.stats['total_queries'] += 1
        result = self.search_dblp(title, authors, year, venue)
        
        if result:
            # Get additional info if we have the DBLP key
            if result.get('key'):
                citation_info = self.get_citations_from_dblp_key(result['key'])
                result.update(citation_info)
            
            # Cache successful result
            self.paper_cache[paper_key] = result
            
            # Store DBLP key mapping for future use
            self.dblp_mapping[paper_key] = result.get('key', '')
            
            self.stats['successful'] += 1
            
            # Save cache periodically
            if self.stats['successful'] % 10 == 0:
                self.save_cache()
        else:
            # Cache failed lookup
            self.failed_cache[paper_key] = time.time()
            self.stats['failed'] += 1
        
        return result
    
    def fetch_from_bibtex_file(self, bib_file):
        """
        Fetch papers from a BibTeX file.
        """
        print(f"Processing {bib_file}...")
        
        with open(bib_file, 'r', encoding='utf-8') as f:
            bib_content = f.read()
        
        # Parse BibTeX
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
            # Extract paper info
            title = entry.get('title', '').replace('{', '').replace('}', '')
            
            # Parse authors
            authors_str = entry.get('author', '')
            authors = []
            if authors_str:
                # Split by 'and'
                author_list = authors_str.split(' and ')
                for author in author_list:
                    author = author.strip()
                    # Handle "Last, First" format
                    if ',' in author:
                        parts = author.split(',', 1)
                        author = f"{parts[1].strip()} {parts[0].strip()}"
                    authors.append(author)
            
            year = entry.get('year', '')
            venue = entry.get('booktitle', '') or entry.get('journal', '')
            doi = entry.get('doi', '')
            
            if not title:
                continue
            
            # Progress indicator
            if i % 10 == 0:
                print(f"  Progress: {i}/{total} papers...")
            
            # Fetch from DBLP
            result = self.fetch_paper(title, authors, year, venue, doi)
            
            if result:
                results.append(result)
                print(f"  ✓ Found: {title[:60]}...")
            else:
                print(f"  ✗ Not found: {title[:60]}...")
            
            # Small delay to be polite (DBLP doesn't require this, but good practice)
            time.sleep(0.1)
        
        return results
    
    def print_stats(self):
        """Print fetching statistics."""
        print("\n" + "="*60)
        print("DBLP Fetching Statistics:")
        print(f"  Total queries: {self.stats['total_queries']}")
        print(f"  Successful: {self.stats['successful']}")
        print(f"  Failed: {self.stats['failed']}")
        print(f"  From cache: {self.stats['from_cache']}")
        print(f"  Total in cache: {len(self.paper_cache)}")
        print("="*60)

def main():
    """Main function to fetch citations from DBLP."""
    import argparse
    
    parser = argparse.ArgumentParser(description='Fetch paper information from DBLP')
    parser.add_argument('bib_file', nargs='?', help='BibTeX file to process')
    parser.add_argument('--all', action='store_true', help='Process all BibTeX files')
    parser.add_argument('--test', action='store_true', help='Test with a sample paper')
    
    args = parser.parse_args()
    
    fetcher = DBLPFetcher()
    
    if args.test:
        # Test with a known paper
        result = fetcher.fetch_paper(
            title="MapReduce: Simplified Data Processing on Large Clusters",
            authors=["Jeffrey Dean", "Sanjay Ghemawat"],
            year=2004,
            venue="OSDI"
        )
        
        if result:
            print("Test successful!")
            print(json.dumps(result, indent=2))
        else:
            print("Test failed - paper not found")
        
        fetcher.save_cache()
        return
    
    if args.all:
        # Process all BibTeX files
        bib_dir = Path("bib")
        bib_files = sorted(bib_dir.glob("*.bib"))
        
        print(f"Found {len(bib_files)} BibTeX files")
        
        for bib_file in bib_files:
            conf_name = bib_file.stem
            print(f"\n{'='*60}")
            print(f"Processing {conf_name}...")
            print('='*60)
            
            results = fetcher.fetch_from_bibtex_file(bib_file)
            
            print(f"\nCompleted {conf_name}: {len(results)} papers found")
            
            # Save after each conference
            fetcher.save_cache()
    
    elif args.bib_file:
        # Process single file
        bib_file = Path(args.bib_file)
        if not bib_file.exists():
            print(f"Error: File {bib_file} not found")
            return
        
        results = fetcher.fetch_from_bibtex_file(bib_file)
        print(f"\nFound {len(results)} papers")
        fetcher.save_cache()
    
    else:
        print("Usage: python fetch_citations_dblp.py <bib_file>")
        print("       python fetch_citations_dblp.py --all")
        print("       python fetch_citations_dblp.py --test")
    
    fetcher.print_stats()

if __name__ == "__main__":
    main()