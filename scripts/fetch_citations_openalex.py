#!/usr/bin/env python3
"""
Fetch citation data from OpenAlex API
OpenAlex is free, no API key required, and has good rate limits
"""

import json
import time
import hashlib
import os
import sys
import urllib.request
import urllib.parse
from pathlib import Path
from typing import Dict, List, Optional, Tuple
from collections import defaultdict
import re

class OpenAlexFetcher:
    def __init__(self, cache_dir: str = "cache/citations"):
        self.cache_dir = Path(cache_dir)
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        
        # OpenAlex API endpoint
        self.base_url = "https://api.openalex.org"
        
        # Cache files
        self.cache_file = self.cache_dir / "openalex_cache.json"
        self.failed_cache_file = self.cache_dir / "openalex_failed.json"
        
        # Load existing cache
        self.paper_cache = self.load_cache(self.cache_file)
        self.failed_cache = self.load_cache(self.failed_cache_file)
        
        # Stats
        self.stats = {
            'total_queries': 0,
            'cache_hits': 0,
            'api_calls': 0,
            'papers_found': 0,
            'papers_not_found': 0,
            'total_citations': 0
        }
        
        # Rate limiting: OpenAlex allows 10 requests/sec without key
        # We'll be polite and do 5/sec
        self.rate_limit_delay = 0.2  # 200ms between requests
        self.last_request_time = 0
        
    def load_cache(self, filepath: Path) -> Dict:
        """Load existing cache from file"""
        if filepath.exists():
            try:
                with open(filepath, 'r') as f:
                    return json.load(f)
            except:
                return {}
        return {}
    
    def save_cache(self):
        """Save cache to file"""
        with open(self.cache_file, 'w') as f:
            json.dump(self.paper_cache, f, indent=2)
        
        with open(self.failed_cache_file, 'w') as f:
            json.dump(self.failed_cache, f, indent=2)
    
    def get_paper_key(self, title: str, authors: str = None, year: str = None) -> str:
        """Generate unique key for paper"""
        key_str = f"{title}_{authors or ''}_{year or ''}"
        return hashlib.md5(key_str.encode()).hexdigest()
    
    def normalize_title(self, title: str) -> str:
        """Normalize title for better matching"""
        # Remove latex commands
        title = re.sub(r'\\[a-zA-Z]+\{([^}]*)\}', r'\1', title)
        title = re.sub(r'[{}]', '', title)
        # Remove extra whitespace
        title = re.sub(r'\s+', ' ', title).strip()
        return title
    
    def search_openalex(self, title: str, year: Optional[int] = None, doi: Optional[str] = None) -> Optional[Dict]:
        """Search for a paper in OpenAlex"""
        
        # Rate limiting
        current_time = time.time()
        time_since_last = current_time - self.last_request_time
        if time_since_last < self.rate_limit_delay:
            time.sleep(self.rate_limit_delay - time_since_last)
        
        try:
            # Normalize title
            title_clean = self.normalize_title(title)
            
            # Build query
            params = {}
            
            # If we have DOI, use it directly
            if doi:
                params['filter'] = f'doi:{doi}'
            else:
                # Use title search
                params['search'] = title_clean
                if year:
                    params['filter'] = f'publication_year:{year}'
            
            # Construct URL
            query_string = urllib.parse.urlencode(params)
            url = f"{self.base_url}/works?{query_string}&per_page=1"
            
            # Make request
            req = urllib.request.Request(url)
            req.add_header('User-Agent', 'QIndex Citation Fetcher (academic research)')
            
            with urllib.request.urlopen(req) as response:
                data = json.loads(response.read())
            
            self.last_request_time = time.time()
            self.stats['api_calls'] += 1
            
            # Check if we found results
            if data.get('results') and len(data['results']) > 0:
                work = data['results'][0]
                
                # Verify title similarity (fuzzy match)
                openalex_title = work.get('title', '').lower()
                search_title = title_clean.lower()
                
                # Simple similarity check
                if self.is_similar_title(search_title, openalex_title):
                    return {
                        'openalex_id': work.get('id'),
                        'doi': work.get('doi'),
                        'title': work.get('title'),
                        'year': work.get('publication_year'),
                        'cited_by_count': work.get('cited_by_count', 0),
                        'references_count': work.get('referenced_works_count', 0),
                        'venue': work.get('primary_location', {}).get('source', {}).get('display_name'),
                        'authors': [a.get('author', {}).get('display_name', '') for a in work.get('authorships', [])],
                        'abstract': work.get('abstract'),
                        'citations_url': work.get('cited_by_api_url'),
                        'references': work.get('referenced_works', [])
                    }
            
            return None
            
        except Exception as e:
            print(f"Error searching OpenAlex: {e}")
            return None
    
    def is_similar_title(self, title1: str, title2: str) -> bool:
        """Check if two titles are similar enough"""
        # Remove common words and punctuation for comparison
        def clean_for_comparison(s):
            s = re.sub(r'[^\w\s]', '', s.lower())
            common_words = {'the', 'a', 'an', 'and', 'or', 'of', 'in', 'on', 'at', 'to', 'for'}
            words = [w for w in s.split() if w not in common_words]
            return set(words)
        
        words1 = clean_for_comparison(title1)
        words2 = clean_for_comparison(title2)
        
        if not words1 or not words2:
            return False
        
        # Calculate Jaccard similarity
        intersection = words1.intersection(words2)
        union = words1.union(words2)
        
        if not union:
            return False
        
        similarity = len(intersection) / len(union)
        return similarity > 0.7  # 70% similarity threshold
    
    def fetch_paper_citations(self, title: str, authors: str = None, year: str = None, doi: str = None) -> Optional[Dict]:
        """Fetch citation data for a paper"""
        self.stats['total_queries'] += 1
        
        # Check cache first
        paper_key = self.get_paper_key(title, authors, year)
        
        if paper_key in self.paper_cache:
            self.stats['cache_hits'] += 1
            return self.paper_cache[paper_key]
        
        # Check failed cache (papers we couldn't find before)
        if paper_key in self.failed_cache:
            # Check if it's been more than 7 days since last attempt
            last_attempt = self.failed_cache[paper_key].get('timestamp', 0)
            if time.time() - last_attempt < 7 * 24 * 3600:
                return None
        
        # Search OpenAlex
        print(f"Searching OpenAlex for: {title[:60]}...")
        
        result = self.search_openalex(title, int(year) if year else None, doi)
        
        if result:
            self.stats['papers_found'] += 1
            self.stats['total_citations'] += result['cited_by_count']
            self.paper_cache[paper_key] = result
            print(f"  ✓ Found: {result['cited_by_count']} citations")
        else:
            self.stats['papers_not_found'] += 1
            self.failed_cache[paper_key] = {
                'title': title,
                'timestamp': time.time()
            }
            print(f"  ✗ Not found")
        
        # Save cache periodically
        if self.stats['total_queries'] % 10 == 0:
            self.save_cache()
        
        return result
    
    def parse_bibtex_file(self, bib_file: Path) -> List[Dict]:
        """Simple BibTeX parser"""
        entries = []
        
        with open(bib_file, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # Split by @ to find entries
        raw_entries = content.split('@')[1:]  # Skip the first empty split
        
        for raw_entry in raw_entries:
            if not raw_entry.strip():
                continue
            
            # Get entry type and content
            if '{' not in raw_entry:
                continue
            
            entry_type = raw_entry.split('{')[0].strip().lower()
            if entry_type not in ['article', 'inproceedings', 'book', 'incollection', 
                                  'phdthesis', 'mastersthesis', 'techreport', 'misc']:
                continue
            
            entry = {}
            
            # Extract title
            title_patterns = [
                r'title\s*=\s*\{([^}]+)\}',
                r'title\s*=\s*"([^"]+)"',
                r'title\s*=\s*\{([^}]+)'
            ]
            for pattern in title_patterns:
                title_match = re.search(pattern, raw_entry, re.IGNORECASE)
                if title_match:
                    entry['title'] = title_match.group(1).strip('{}')
                    break
            
            # Extract year
            year_patterns = [
                r'year\s*=\s*\{(\d{4})\}',
                r'year\s*=\s*"(\d{4})"',
                r'year\s*=\s*(\d{4})'
            ]
            for pattern in year_patterns:
                year_match = re.search(pattern, raw_entry, re.IGNORECASE)
                if year_match:
                    entry['year'] = year_match.group(1)
                    break
            
            # Extract authors
            author_patterns = [
                r'author\s*=\s*\{([^}]+)\}',
                r'author\s*=\s*"([^"]+)"'
            ]
            for pattern in author_patterns:
                author_match = re.search(pattern, raw_entry, re.IGNORECASE)
                if author_match:
                    entry['author'] = author_match.group(1)
                    break
            
            # Extract DOI
            doi_patterns = [
                r'doi\s*=\s*\{([^}]+)\}',
                r'doi\s*=\s*"([^"]+)"'
            ]
            for pattern in doi_patterns:
                doi_match = re.search(pattern, raw_entry, re.IGNORECASE)
                if doi_match:
                    entry['doi'] = doi_match.group(1)
                    break
            
            if 'title' in entry:
                entries.append(entry)
        
        return entries
    
    def process_bibtex_file(self, bib_file: Path) -> Dict:
        """Process a single BibTeX file"""
        print(f"\nProcessing {bib_file.name}...")
        
        results = {
            'venue': bib_file.stem,
            'papers_processed': 0,
            'papers_found': 0,
            'total_citations': 0,
            'papers': []
        }
        
        try:
            entries = self.parse_bibtex_file(bib_file)
            
            for entry in entries:
                title = entry.get('title', '').strip('{}')
                year = entry.get('year', '')
                authors = entry.get('author', '')
                doi = entry.get('doi', '')
                
                if not title:
                    continue
                
                results['papers_processed'] += 1
                
                # Fetch citations
                citation_data = self.fetch_paper_citations(title, authors, year, doi)
                
                if citation_data:
                    results['papers_found'] += 1
                    results['total_citations'] += citation_data['cited_by_count']
                    results['papers'].append({
                        'title': title,
                        'year': year,
                        'citations': citation_data['cited_by_count'],
                        'openalex_id': citation_data.get('openalex_id')
                    })
        
        except Exception as e:
            print(f"Error processing {bib_file}: {e}")
        
        return results
    
    def process_all_bibtex(self, bib_dir: str = "bib"):
        """Process all BibTeX files"""
        bib_path = Path(bib_dir)
        bib_files = sorted(bib_path.glob("*.bib"))
        
        print(f"Found {len(bib_files)} BibTeX files")
        print("=" * 60)
        
        all_results = []
        
        for bib_file in bib_files:
            result = self.process_bibtex_file(bib_file)
            all_results.append(result)
            
            # Print progress
            print(f"  {result['venue']}: {result['papers_found']}/{result['papers_processed']} papers, "
                  f"{result['total_citations']} citations")
        
        # Save final cache
        self.save_cache()
        
        # Print summary
        print("\n" + "=" * 60)
        print("SUMMARY")
        print("=" * 60)
        print(f"Total queries: {self.stats['total_queries']}")
        print(f"Cache hits: {self.stats['cache_hits']}")
        print(f"API calls: {self.stats['api_calls']}")
        print(f"Papers found: {self.stats['papers_found']}")
        print(f"Papers not found: {self.stats['papers_not_found']}")
        print(f"Total citations: {self.stats['total_citations']:,}")
        print(f"Success rate: {100 * self.stats['papers_found'] / max(1, self.stats['total_queries']):.1f}%")
        
        # Save summary
        summary_file = self.cache_dir / "openalex_summary.json"
        with open(summary_file, 'w') as f:
            json.dump({
                'stats': self.stats,
                'venues': all_results,
                'timestamp': time.time()
            }, f, indent=2)
        
        print(f"\nResults saved to {summary_file}")
        
        return all_results

def main():
    import argparse
    
    parser = argparse.ArgumentParser(description='Fetch citations from OpenAlex')
    parser.add_argument('--bib-dir', default='bib', help='Directory containing BibTeX files')
    parser.add_argument('--venue', help='Process only specific venue (e.g., sosp)')
    parser.add_argument('--limit', type=int, help='Limit number of papers to process')
    parser.add_argument('--test', action='store_true', help='Test with one paper')
    
    args = parser.parse_args()
    
    fetcher = OpenAlexFetcher()
    
    if args.test:
        # Test with a known paper
        result = fetcher.fetch_paper_citations(
            "The Google File System",
            year="2003"
        )
        if result:
            print(f"Test successful!")
            print(f"Citations: {result['cited_by_count']}")
            print(f"OpenAlex ID: {result['openalex_id']}")
        else:
            print("Test failed - paper not found")
    
    elif args.venue:
        # Process single venue
        bib_file = Path(args.bib_dir) / f"{args.venue}.bib"
        if bib_file.exists():
            result = fetcher.process_bibtex_file(bib_file)
            print(f"\nProcessed {result['papers_processed']} papers from {args.venue}")
            print(f"Found {result['papers_found']} papers with {result['total_citations']} total citations")
        else:
            print(f"File not found: {bib_file}")
    
    else:
        # Process all venues
        fetcher.process_all_bibtex(args.bib_dir)

if __name__ == "__main__":
    main()