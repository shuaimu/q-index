#!/usr/bin/env python3
"""
Distributed citation fetching with intelligent rate limit management.
"""

import os
import sys
import time
import json
import random
from pathlib import Path
from datetime import datetime, timedelta
import asyncio
import aiohttp
from typing import List, Dict
import hashlib

class DistributedFetcher:
    def __init__(self):
        self.cache_dir = Path("cache/citations")
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        
        # Track API usage per source
        self.api_usage = {
            'semantic_scholar': {'calls': 0, 'last_reset': time.time(), 'limit': 100, 'window': 300},
            'crossref': {'calls': 0, 'last_reset': time.time(), 'limit': 50, 'window': 1},
            'openalex': {'calls': 0, 'last_reset': time.time(), 'limit': 100, 'window': 1},
        }
        
        # Exponential backoff parameters
        self.backoff = {
            'semantic_scholar': 1.0,
            'crossref': 0.5,
            'openalex': 0.3,
        }
    
    async def fetch_with_backoff(self, session, url, source, headers=None):
        """
        Fetch with exponential backoff and rate limit management.
        """
        # Check rate limit
        if not self.check_rate_limit(source):
            await asyncio.sleep(self.backoff[source])
            self.backoff[source] = min(self.backoff[source] * 2, 60)  # Max 60 seconds
        
        try:
            async with session.get(url, headers=headers) as response:
                if response.status == 429:  # Rate limited
                    retry_after = response.headers.get('Retry-After', 60)
                    print(f"Rate limited on {source}, waiting {retry_after}s")
                    await asyncio.sleep(int(retry_after))
                    return await self.fetch_with_backoff(session, url, source, headers)
                elif response.status == 200:
                    self.backoff[source] = max(0.3, self.backoff[source] * 0.9)  # Reduce backoff on success
                    self.record_api_call(source)
                    return await response.json()
        except Exception as e:
            print(f"Error fetching from {source}: {e}")
            return None
    
    def check_rate_limit(self, source):
        """Check if we can make another API call."""
        usage = self.api_usage.get(source)
        if not usage:
            return True
        
        # Reset counter if window has passed
        if time.time() - usage['last_reset'] > usage['window']:
            usage['calls'] = 0
            usage['last_reset'] = time.time()
        
        return usage['calls'] < usage['limit']
    
    def record_api_call(self, source):
        """Record an API call."""
        if source in self.api_usage:
            self.api_usage[source]['calls'] += 1
    
    async def fetch_batch_async(self, papers: List[Dict], max_concurrent=5):
        """
        Fetch papers in batches with concurrent requests.
        """
        semaphore = asyncio.Semaphore(max_concurrent)
        
        async def fetch_paper(session, paper):
            async with semaphore:
                # Try multiple sources concurrently
                tasks = []
                
                if paper.get('doi'):
                    tasks.append(self.fetch_from_crossref_async(session, paper))
                
                tasks.append(self.fetch_from_openalex_async(session, paper))
                
                # Wait for first successful result
                for coro in asyncio.as_completed(tasks):
                    result = await coro
                    if result:
                        return result
                
                return None
        
        async with aiohttp.ClientSession() as session:
            tasks = [fetch_paper(session, paper) for paper in papers]
            results = await asyncio.gather(*tasks)
            return results
    
    async def fetch_from_openalex_async(self, session, paper):
        """Async fetch from OpenAlex."""
        title = paper.get('title', '')
        year = paper.get('year')
        
        query = f'title.search:"{title}"'
        if year:
            query += f' AND publication_year:{year}'
        
        url = f"https://api.openalex.org/works?filter={query}"
        headers = {'User-Agent': 'mailto:your-email@example.com'}
        
        result = await self.fetch_with_backoff(session, url, 'openalex', headers)
        if result and result.get('results'):
            work = result['results'][0]
            return {
                'title': work.get('title'),
                'citation_count': work.get('cited_by_count', 0),
                'source': 'openalex'
            }
        return None
    
    async def fetch_from_crossref_async(self, session, paper):
        """Async fetch from CrossRef."""
        doi = paper.get('doi')
        if not doi:
            return None
        
        url = f"https://api.crossref.org/works/{doi}"
        headers = {'User-Agent': 'QIndex/1.0 (mailto:your-email@example.com)'}
        
        result = await self.fetch_with_backoff(session, url, 'crossref', headers)
        if result and result.get('message'):
            work = result['message']
            return {
                'title': work.get('title', [''])[0],
                'citation_count': work.get('is-referenced-by-count', 0),
                'source': 'crossref'
            }
        return None

# Strategy 3: Use Tor for IP rotation
def setup_tor_session():
    """
    Setup session with Tor for IP rotation.
    Requires Tor to be installed and running.
    """
    import requests
    
    session = requests.Session()
    # Tor default SOCKS proxy
    session.proxies = {
        'http': 'socks5://127.0.0.1:9050',
        'https': 'socks5://127.0.0.1:9050'
    }
    return session

# Strategy 4: Cache and share results
class CitationCache:
    """
    Distributed cache that can be shared across multiple machines.
    """
    def __init__(self, cache_dir="cache/citations"):
        self.cache_dir = Path(cache_dir)
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        
        # Use multiple cache files to avoid conflicts
        self.cache_files = {
            'primary': self.cache_dir / "paper_cache.json",
            'secondary': self.cache_dir / "paper_cache_2.json",
            'tertiary': self.cache_dir / "paper_cache_3.json",
        }
    
    def get_from_any_cache(self, paper_key):
        """Check all cache files."""
        for name, cache_file in self.cache_files.items():
            if cache_file.exists():
                with open(cache_file, 'r') as f:
                    cache = json.load(f)
                    if paper_key in cache:
                        return cache[paper_key]
        return None
    
    def save_to_rotating_cache(self, paper_key, data):
        """Save to rotating cache files."""
        # Rotate through cache files to distribute writes
        cache_file = random.choice(list(self.cache_files.values()))
        
        if cache_file.exists():
            with open(cache_file, 'r') as f:
                cache = json.load(f)
        else:
            cache = {}
        
        cache[paper_key] = data
        
        with open(cache_file, 'w') as f:
            json.dump(cache, f, indent=2)

def main():
    print("""
    Strategies to bypass rate limits:
    
    1. Use multiple free APIs (OpenAlex, CrossRef, DBLP, arXiv)
    2. Implement exponential backoff and respect Retry-After headers
    3. Use async/concurrent requests with semaphores
    4. Rotate User-Agents and add email for polite crawling
    5. Use Tor for IP rotation (requires Tor installation)
    6. Implement distributed caching across multiple files
    7. Add random delays between requests
    8. Use API keys when available (Semantic Scholar, etc.)
    
    To use:
    - Update email addresses in the code
    - Add any available API keys
    - Install Tor if you want IP rotation
    - Run multiple instances with different configurations
    """)

if __name__ == "__main__":
    main()