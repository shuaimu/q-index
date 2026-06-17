#!/usr/bin/env python3
"""
Fetch citation data for papers and store citation relationships locally
"""

import json
import time
import requests
from pathlib import Path
import re
import sys
import hashlib
from datetime import datetime, timedelta
import pickle

class CitationFetcher:
    def __init__(self, cache_dir="cache/citations"):
        self.cache_dir = Path(cache_dir)
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        
        # Semantic Scholar API endpoint
        self.ss_base_url = "https://api.semanticscholar.org/graph/v1"
        
        # DBLP API endpoint
        self.dblp_base_url = "https://dblp.org/search/publ/api"
        
        # CrossRef API endpoint
        self.crossref_base_url = "https://api.crossref.org/works"
        
        # Rate limiting
        self.last_request_time = 0
        self.min_request_interval = 0.5  # 500ms between requests
        
        # Cache files
        self.paper_cache_file = self.cache_dir / "paper_cache.json"
        self.citation_cache_file = self.cache_dir / "citation_graph.json"
        self.failed_lookups_file = self.cache_dir / "failed_lookups.json"
        
        # Load existing caches
        self.paper_cache = self.load_json_cache(self.paper_cache_file)
        self.citation_graph = self.load_json_cache(self.citation_cache_file)
        self.failed_lookups = self.load_json_cache(self.failed_lookups_file)
    
    def load_json_cache(self, filepath):
        """Load JSON cache file"""
        if filepath.exists():
            try:
                with open(filepath, 'r', encoding='utf-8') as f:
                    return json.load(f)
            except:
                return {}
        return {}
    
    def save_json_cache(self, data, filepath):
        """Save JSON cache file"""
        with open(filepath, 'w', encoding='utf-8') as f:
            json.dump(data, f, indent=2, ensure_ascii=False)
    
    def rate_limit(self):
        """Enforce rate limiting"""
        elapsed = time.time() - self.last_request_time
        if elapsed < self.min_request_interval:
            time.sleep(self.min_request_interval - elapsed)
        self.last_request_time = time.time()
    
    def get_paper_key(self, title, authors=None, year=None):
        """Generate a unique key for a paper"""
        # Normalize title
        title_norm = re.sub(r'[^\w\s]', '', title.lower()).strip()
        
        # Include first author if available
        if authors and len(authors) > 0:
            first_author = re.sub(r'[^\w\s]', '', str(authors[0]).lower()).strip()
            key = f"{title_norm}_{first_author}"
        else:
            key = title_norm
        
        # Add year if available
        if year:
            key += f"_{year}"
        
        return hashlib.md5(key.encode()).hexdigest()
    
    def search_semantic_scholar(self, title, authors=None, year=None):
        """Search for a paper in Semantic Scholar"""
        self.rate_limit()
        
        # Build query
        query = title
        if year:
            query += f" {year}"
        
        url = f"{self.ss_base_url}/paper/search"
        params = {
            'query': query,
            'limit': 10,
            'fields': 'paperId,title,authors,year,citationCount,references,citations,doi,venue'
        }
        
        try:
            response = requests.get(url, params=params, timeout=10)
            if response.status_code == 200:
                data = response.json()
                papers = data.get('data', [])
                
                # Find best match
                for paper in papers:
                    # Check title similarity
                    paper_title = paper.get('title', '').lower()
                    search_title = title.lower()
                    
                    # Fuzzy match - at least 80% of words match
                    search_words = set(search_title.split())
                    paper_words = set(paper_title.split())
                    
                    if len(search_words) > 0:
                        overlap = len(search_words & paper_words)
                        similarity = overlap / len(search_words)
                        
                        if similarity > 0.7:
                            # Check year if provided
                            if year and paper.get('year'):
                                if abs(paper['year'] - year) > 1:
                                    continue
                            
                            return paper
                
                # Return first result if no good match
                if papers:
                    return papers[0]
        except Exception as e:
            print(f"  Error searching Semantic Scholar: {e}")
        
        return None
    
    def search_crossref(self, title, authors=None, year=None):
        """Search for a paper in CrossRef by DOI or title"""
        self.rate_limit()
        
        url = self.crossref_base_url
        params = {
            'query.title': title,
            'rows': 5
        }
        
        if authors and len(authors) > 0:
            params['query.author'] = authors[0]
        
        try:
            response = requests.get(url, params=params, timeout=10)
            if response.status_code == 200:
                data = response.json()
                items = data.get('message', {}).get('items', [])
                
                for item in items:
                    # Check title similarity
                    item_title = ' '.join(item.get('title', [])).lower()
                    search_title = title.lower()
                    
                    if self.similar_titles(search_title, item_title):
                        # Extract citation info
                        return {
                            'doi': item.get('DOI'),
                            'title': ' '.join(item.get('title', [])),
                            'citationCount': item.get('is-referenced-by-count', 0),
                            'year': item.get('published-print', {}).get('date-parts', [[None]])[0][0]
                        }
        except Exception as e:
            print(f"  Error searching CrossRef: {e}")
        
        return None
    
    def similar_titles(self, title1, title2):
        """Check if two titles are similar enough"""
        # Normalize titles
        t1 = re.sub(r'[^\w\s]', '', title1.lower()).strip()
        t2 = re.sub(r'[^\w\s]', '', title2.lower()).strip()
        
        # Word-based similarity
        words1 = set(t1.split())
        words2 = set(t2.split())
        
        if len(words1) == 0 or len(words2) == 0:
            return False
        
        intersection = len(words1 & words2)
        union = len(words1 | words2)
        
        # Jaccard similarity
        if union > 0:
            similarity = intersection / union
            return similarity > 0.6
        
        return False
    
    def fetch_paper_citations(self, title, authors=None, year=None, doi=None):
        """Fetch citation data for a single paper"""
        paper_key = self.get_paper_key(title, authors, year)
        
        # Check if already processed
        if paper_key in self.paper_cache:
            return self.paper_cache[paper_key]
        
        # Check if previously failed
        if paper_key in self.failed_lookups:
            failed_time = self.failed_lookups[paper_key]
            # Retry after 7 days
            if time.time() - failed_time < 7 * 24 * 3600:
                return None
        
        print(f"  Fetching: {title[:60]}... ({year})")
        
        # Try Semantic Scholar first
        paper_data = self.search_semantic_scholar(title, authors, year)
        
        # Try CrossRef if SS fails
        if not paper_data and doi:
            paper_data = self.search_crossref(title, authors, year)
        
        if paper_data:
            # Store in cache
            self.paper_cache[paper_key] = {
                'title': title,
                'authors': authors,
                'year': year,
                'doi': doi,
                'ss_id': paper_data.get('paperId'),
                'citation_count': paper_data.get('citationCount', 0),
                'venue': paper_data.get('venue'),
                'fetched_at': time.time()
            }
            
            # Store citation relationships
            if paper_data.get('references'):
                self.citation_graph[paper_key] = {
                    'cites': [ref.get('paperId') for ref in paper_data.get('references', []) if ref.get('paperId')],
                    'cited_by': []
                }
            
            if paper_data.get('citations'):
                if paper_key not in self.citation_graph:
                    self.citation_graph[paper_key] = {'cites': [], 'cited_by': []}
                self.citation_graph[paper_key]['cited_by'] = [
                    cit.get('paperId') for cit in paper_data.get('citations', []) if cit.get('paperId')
                ]
            
            return self.paper_cache[paper_key]
        else:
            # Mark as failed
            self.failed_lookups[paper_key] = time.time()
            return None
    
    def parse_bibtex_file(self, filepath):
        """Parse a BibTeX file and extract paper information"""
        papers = []
        
        with open(filepath, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # Find all entries
        entries = re.findall(r'@\w+\{[^@]+\}', content, re.DOTALL)
        
        for entry in entries:
            # Skip comments
            if entry.strip().startswith('%'):
                continue
            
            # Extract title
            title_match = re.search(r'title\s*=\s*\{([^}]+)\}', entry, re.IGNORECASE)
            if not title_match:
                continue
            title = title_match.group(1).strip()
            
            # Skip proceedings
            if 'Proceedings of' in title:
                continue
            
            # Extract authors
            authors = []
            author_match = re.search(r'author\s*=\s*\{([^}]+)\}', entry, re.IGNORECASE)
            if author_match:
                author_str = author_match.group(1)
                # Split by 'and'
                authors = [a.strip() for a in re.split(r'\s+and\s+', author_str)]
            
            # Extract year
            year = None
            year_match = re.search(r'year\s*=\s*\{?(\d{4})\}?', entry, re.IGNORECASE)
            if year_match:
                year = int(year_match.group(1))
            
            # Extract DOI
            doi = None
            doi_match = re.search(r'doi\s*=\s*\{([^}]+)\}', entry, re.IGNORECASE)
            if doi_match:
                doi = doi_match.group(1).strip()
            
            papers.append({
                'title': title,
                'authors': authors,
                'year': year,
                'doi': doi
            })
        
        return papers
    
    def process_conference(self, conf_name, limit=None):
        """Process all papers from a conference file"""
        filepath = Path(f"bib/{conf_name}.bib")
        
        if not filepath.exists():
            print(f"File not found: {filepath}")
            return
        
        print(f"\nProcessing {conf_name.upper()}...")
        papers = self.parse_bibtex_file(filepath)
        
        if limit:
            papers = papers[:limit]
        
        print(f"Found {len(papers)} papers to process")
        
        successful = 0
        failed = 0
        
        for i, paper in enumerate(papers, 1):
            if i % 10 == 0:
                print(f"  Progress: {i}/{len(papers)} papers...")
                # Save intermediate results
                self.save_caches()
            
            result = self.fetch_paper_citations(
                paper['title'],
                paper['authors'],
                paper['year'],
                paper['doi']
            )
            
            if result:
                successful += 1
            else:
                failed += 1
        
        print(f"  Completed: {successful} successful, {failed} failed")
        
        # Save final results
        self.save_caches()
    
    def save_caches(self):
        """Save all caches to disk"""
        self.save_json_cache(self.paper_cache, self.paper_cache_file)
        self.save_json_cache(self.citation_graph, self.citation_cache_file)
        self.save_json_cache(self.failed_lookups, self.failed_lookups_file)
    
    def get_statistics(self):
        """Get statistics about fetched citations"""
        stats = {
            'total_papers': len(self.paper_cache),
            'papers_with_citations': 0,
            'total_citations': 0,
            'total_references': 0,
            'failed_lookups': len(self.failed_lookups)
        }
        
        for paper_key, paper_data in self.paper_cache.items():
            if paper_data.get('citation_count', 0) > 0:
                stats['papers_with_citations'] += 1
                stats['total_citations'] += paper_data['citation_count']
        
        for paper_key, relations in self.citation_graph.items():
            stats['total_references'] += len(relations.get('cites', []))
        
        return stats
    
    def export_citation_graph(self, output_file="cache/citations/citation_graph.csv"):
        """Export citation graph as CSV for analysis"""
        output_path = Path(output_file)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        
        with open(output_path, 'w', encoding='utf-8') as f:
            f.write("source_paper,target_paper,relationship\n")
            
            for paper_key, relations in self.citation_graph.items():
                # Get paper info
                paper_info = self.paper_cache.get(paper_key, {})
                source_title = paper_info.get('title', paper_key)[:100]
                
                # Export citations (this paper cites these)
                for cited_id in relations.get('cites', []):
                    f.write(f'"{source_title}","{cited_id}","cites"\n')
                
                # Export cited_by (these papers cite this)
                for citing_id in relations.get('cited_by', []):
                    f.write(f'"{citing_id}","{source_title}","cited_by"\n')
        
        print(f"Citation graph exported to {output_path}")

def main():
    """Main function"""
    import argparse
    
    parser = argparse.ArgumentParser(description='Fetch citation data for papers')
    parser.add_argument('conference', nargs='?', help='Conference name (e.g., sigcomm, sosp)')
    parser.add_argument('--limit', type=int, help='Limit number of papers to process')
    parser.add_argument('--all', action='store_true', help='Process all conferences')
    parser.add_argument('--stats', action='store_true', help='Show statistics only')
    parser.add_argument('--export', action='store_true', help='Export citation graph')
    
    args = parser.parse_args()
    
    fetcher = CitationFetcher()
    
    if args.stats:
        stats = fetcher.get_statistics()
        print("\n=== Citation Data Statistics ===")
        for key, value in stats.items():
            print(f"{key}: {value:,}")
        return
    
    if args.export:
        fetcher.export_citation_graph()
        return
    
    if args.all:
        # Process all major conferences
        conferences = [
            'sigcomm', 'sosp', 'osdi', 'nsdi', 'sigmod', 'vldb',
            'icml', 'neurips', 'pldi', 'popl', 'stoc', 'podc'
        ]
        
        for conf in conferences:
            fetcher.process_conference(conf, limit=args.limit)
            # Save after each conference
            fetcher.save_caches()
            time.sleep(2)  # Be nice to APIs
    elif args.conference:
        fetcher.process_conference(args.conference, limit=args.limit)
    else:
        parser.print_help()
        print("\nExamples:")
        print("  python3 scripts/fetch_citations.py sigcomm --limit 10")
        print("  python3 scripts/fetch_citations.py sosp")
        print("  python3 scripts/fetch_citations.py --all --limit 5")
        print("  python3 scripts/fetch_citations.py --stats")
        print("  python3 scripts/fetch_citations.py --export")

if __name__ == "__main__":
    main()