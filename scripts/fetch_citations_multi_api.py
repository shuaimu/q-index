#!/usr/bin/env python3
"""
Enhanced citation fetcher with multiple API strategies to bypass rate limits.
"""

import os
import sys
import time
import json
import random
import hashlib
import requests
from typing import Dict, List, Optional, Tuple
from pathlib import Path
from datetime import datetime
import concurrent.futures
from itertools import cycle

# Rotating user agents to appear as different clients
USER_AGENTS = [
    'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
    'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36',
    'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36',
    'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:91.0) Gecko/20100101',
]

# Multiple Semantic Scholar API keys (if available)
# You can get API keys from: https://www.semanticscholar.org/product/api
SEMANTIC_SCHOLAR_API_KEYS = [
    # Add your API keys here
    # os.getenv('SS_API_KEY_1'),
    # os.getenv('SS_API_KEY_2'),
]

# Proxy servers for IP rotation
PROXY_SERVERS = [
    # Add proxy servers here if available
    # 'http://proxy1.com:8080',
    # 'http://proxy2.com:8080',
]

class MultiSourceCitationFetcher:
    def __init__(self):
        self.session = requests.Session()
        self.api_key_cycle = cycle(SEMANTIC_SCHOLAR_API_KEYS) if SEMANTIC_SCHOLAR_API_KEYS else None
        self.user_agent_cycle = cycle(USER_AGENTS)
        self.proxy_cycle = cycle(PROXY_SERVERS) if PROXY_SERVERS else None
        
        # Cache directories
        self.cache_dir = Path("cache/citations")
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        
        # Load existing cache
        self.load_cache()
        
        # Alternative APIs and data sources
        self.alternative_sources = {
            'openalex': self.fetch_from_openalex,
            'crossref': self.fetch_from_crossref,
            'unpaywall': self.fetch_from_unpaywall,
            'arxiv': self.fetch_from_arxiv,
            'dblp': self.fetch_from_dblp,
            'microsoft_academic': self.fetch_from_microsoft_academic,
            'google_scholar': self.fetch_from_google_scholar_cache,
        }
    
    def load_cache(self):
        """Load existing cache files."""
        cache_file = self.cache_dir / "paper_cache.json"
        if cache_file.exists():
            with open(cache_file, 'r') as f:
                self.paper_cache = json.load(f)
        else:
            self.paper_cache = {}
    
    def get_headers(self):
        """Rotate headers for each request."""
        headers = {
            'User-Agent': next(self.user_agent_cycle),
            'Accept': 'application/json',
        }
        
        # Add API key if available
        if self.api_key_cycle:
            headers['x-api-key'] = next(self.api_key_cycle)
        
        return headers
    
    def get_proxies(self):
        """Get rotating proxy if available."""
        if self.proxy_cycle:
            proxy = next(self.proxy_cycle)
            return {'http': proxy, 'https': proxy}
        return None
    
    def fetch_from_openalex(self, title, authors=None, year=None):
        """
        Fetch from OpenAlex API (free, no rate limits for polite use).
        https://docs.openalex.org/
        """
        try:
            # OpenAlex requires email in User-Agent for polite use
            email = "your-email@example.com"  # Replace with your email
            
            query = f'title.search:"{title}"'
            if year:
                query += f' AND publication_year:{year}'
            
            url = f"https://api.openalex.org/works?filter={query}&mailto={email}"
            
            response = requests.get(url, headers={'User-Agent': f'mailto:{email}'})
            if response.status_code == 200:
                data = response.json()
                if data.get('results'):
                    work = data['results'][0]
                    return {
                        'title': work.get('title'),
                        'citation_count': work.get('cited_by_count', 0),
                        'doi': work.get('doi'),
                        'source': 'openalex',
                        'id': work.get('id'),
                    }
        except Exception as e:
            print(f"OpenAlex error: {e}")
        return None
    
    def fetch_from_crossref(self, title, authors=None, year=None, doi=None):
        """
        Fetch from CrossRef API (free, polite use encouraged).
        """
        try:
            if doi:
                url = f"https://api.crossref.org/works/{doi}"
            else:
                query = title.replace(' ', '+')
                url = f"https://api.crossref.org/works?query={query}&rows=1"
            
            headers = self.get_headers()
            headers['User-Agent'] += ' (mailto:your-email@example.com)'  # Add your email
            
            response = requests.get(url, headers=headers)
            if response.status_code == 200:
                data = response.json()
                if doi:
                    work = data.get('message', {})
                else:
                    work = data.get('message', {}).get('items', [{}])[0]
                
                if work:
                    return {
                        'title': work.get('title', [''])[0],
                        'citation_count': work.get('is-referenced-by-count', 0),
                        'doi': work.get('DOI'),
                        'source': 'crossref',
                    }
        except Exception as e:
            print(f"CrossRef error: {e}")
        return None
    
    def fetch_from_unpaywall(self, doi):
        """
        Fetch from Unpaywall API (free, requires email).
        """
        if not doi:
            return None
        
        try:
            email = "your-email@example.com"  # Replace with your email
            url = f"https://api.unpaywall.org/v2/{doi}?email={email}"
            
            response = requests.get(url, headers=self.get_headers())
            if response.status_code == 200:
                data = response.json()
                return {
                    'title': data.get('title'),
                    'doi': data.get('doi'),
                    'source': 'unpaywall',
                    'oa_status': data.get('oa_status'),
                }
        except Exception as e:
            print(f"Unpaywall error: {e}")
        return None
    
    def fetch_from_arxiv(self, title, authors=None):
        """
        Fetch from arXiv API (free, no strict rate limits).
        """
        try:
            import urllib.parse
            query = f'ti:"{title}"'
            if authors and len(authors) > 0:
                query += f' AND au:"{authors[0]}"'
            
            url = f"http://export.arxiv.org/api/query?search_query={urllib.parse.quote(query)}&max_results=1"
            
            response = requests.get(url, headers=self.get_headers())
            if response.status_code == 200:
                # Parse XML response
                import xml.etree.ElementTree as ET
                root = ET.fromstring(response.text)
                
                # Find first entry
                ns = {'atom': 'http://www.w3.org/2005/Atom'}
                entry = root.find('atom:entry', ns)
                if entry is not None:
                    title_elem = entry.find('atom:title', ns)
                    id_elem = entry.find('atom:id', ns)
                    
                    if title_elem is not None:
                        return {
                            'title': title_elem.text.strip(),
                            'arxiv_id': id_elem.text if id_elem is not None else None,
                            'source': 'arxiv',
                        }
        except Exception as e:
            print(f"arXiv error: {e}")
        return None
    
    def fetch_from_dblp(self, title, authors=None, year=None):
        """
        Fetch from DBLP API (free, no rate limits).
        """
        try:
            query = title.replace(' ', '+')
            url = f"https://dblp.org/search/publ/api?q={query}&format=json&h=1"
            
            response = requests.get(url, headers=self.get_headers())
            if response.status_code == 200:
                data = response.json()
                hits = data.get('result', {}).get('hits', {}).get('hit', [])
                if hits:
                    info = hits[0].get('info', {})
                    return {
                        'title': info.get('title'),
                        'year': info.get('year'),
                        'venue': info.get('venue'),
                        'dblp_key': info.get('key'),
                        'source': 'dblp',
                    }
        except Exception as e:
            print(f"DBLP error: {e}")
        return None
    
    def fetch_from_microsoft_academic(self, title, authors=None, year=None):
        """
        Fetch from Microsoft Academic (if still available).
        Note: Microsoft Academic was discontinued, but some mirrors might exist.
        """
        # This is a placeholder - Microsoft Academic was discontinued
        # You might find alternative endpoints or archives
        return None
    
    def fetch_from_google_scholar_cache(self, title, authors=None, year=None):
        """
        Use cached Google Scholar data or scholarly library.
        Note: Direct scraping of Google Scholar violates ToS.
        This uses the 'scholarly' library which implements polite crawling.
        """
        try:
            # Install with: pip install scholarly
            from scholarly import scholarly
            
            search_query = title
            if authors and len(authors) > 0:
                search_query = f"{title} {authors[0]}"
            
            search_results = scholarly.search_pubs(search_query)
            first_result = next(search_results, None)
            
            if first_result:
                # Fill the citation data
                filled = scholarly.fill(first_result)
                return {
                    'title': filled.get('bib', {}).get('title'),
                    'citation_count': filled.get('num_citations', 0),
                    'year': filled.get('bib', {}).get('pub_year'),
                    'source': 'google_scholar',
                }
        except Exception as e:
            print(f"Google Scholar error: {e}")
        return None
    
    def fetch_with_fallback(self, title, authors=None, year=None, doi=None):
        """
        Try multiple sources with fallback strategy.
        """
        paper_key = self.get_paper_key(title, authors, year)
        
        # Check cache first
        if paper_key in self.paper_cache:
            return self.paper_cache[paper_key]
        
        # Try each source in order
        sources_priority = [
            'openalex',      # Free, no rate limits
            'crossref',      # Free, generous limits
            'dblp',          # Free, no limits
            'arxiv',         # Free, no strict limits
            'unpaywall',     # Free, requires DOI
            'google_scholar' # Last resort, slow
        ]
        
        for source in sources_priority:
            if source in self.alternative_sources:
                fetcher = self.alternative_sources[source]
                
                # Add random delay to be polite
                time.sleep(random.uniform(0.5, 1.5))
                
                try:
                    if source == 'unpaywall' and doi:
                        result = fetcher(doi)
                    elif source in ['crossref'] and doi:
                        result = fetcher(title, authors, year, doi)
                    else:
                        result = fetcher(title, authors, year)
                    
                    if result and result.get('title'):
                        # Cache the result
                        self.paper_cache[paper_key] = result
                        self.save_cache()
                        return result
                except Exception as e:
                    print(f"Error with {source}: {e}")
                    continue
        
        return None
    
    def get_paper_key(self, title, authors, year):
        """Generate unique key for paper."""
        key_string = f"{title}_{authors[0] if authors else ''}_{year}"
        return hashlib.md5(key_string.encode()).hexdigest()
    
    def save_cache(self):
        """Save cache to disk."""
        cache_file = self.cache_dir / "paper_cache.json"
        with open(cache_file, 'w') as f:
            json.dump(self.paper_cache, f, indent=2)

def main():
    """
    Main function to demonstrate multi-source fetching.
    """
    fetcher = MultiSourceCitationFetcher()
    
    # Example paper
    result = fetcher.fetch_with_fallback(
        title="The Google File System",
        authors=["Sanjay Ghemawat"],
        year=2003
    )
    
    if result:
        print(f"Found: {result['title']}")
        print(f"Citations: {result.get('citation_count', 'N/A')}")
        print(f"Source: {result['source']}")
    else:
        print("Paper not found in any source")

if __name__ == "__main__":
    main()