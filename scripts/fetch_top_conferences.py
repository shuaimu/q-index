#!/usr/bin/env python3
"""
Fetch recent papers from top CS conferences using DBLP API.
Limited version for demonstration.
"""

import requests
import xml.etree.ElementTree as ET
import time
import os
import re
from typing import List, Dict, Optional

# Focus on top conferences from each area
TOP_CONFERENCES = {
    'SOSP': 'conf/sosp',
    'OSDI': 'conf/osdi', 
    'SIGCOMM': 'conf/sigcomm',
    'PLDI': 'conf/pldi',
    'SIGMOD': 'conf/sigmod',
    'VLDB': 'conf/vldb',
    'CCS': 'conf/ccs',
    'NeurIPS': 'conf/nips',
    'ICML': 'conf/icml',
    'CVPR': 'conf/cvpr',
    'STOC': 'conf/stoc',
    'FOCS': 'conf/focs'
}

def fetch_conference_papers(conf_name: str, venue_key: str, year: int) -> List[Dict]:
    """Fetch papers from a conference for a specific year."""
    papers = []
    
    # DBLP API URL
    url = f"https://dblp.org/search/publ/api?q=venue:{venue_key}%20year:{year}&format=xml&h=100"
    
    try:
        print(f"  Fetching {conf_name} {year}...")
        response = requests.get(url, timeout=30)
        response.raise_for_status()
        
        root = ET.fromstring(response.content)
        hits = root.findall('.//hit')
        
        for hit in hits[:50]:  # Limit to 50 papers per year
            info = hit.find('info')
            if info is not None:
                paper = {}
                
                # Title
                title = info.find('title')
                if title is not None and title.text:
                    paper['title'] = clean_text(title.text)
                
                # Authors
                authors = []
                for author in info.findall('authors/author'):
                    if author.text:
                        authors.append(author.text)
                if authors:
                    paper['authors'] = ' and '.join(authors)
                
                # Year
                paper['year'] = str(year)
                
                # Venue
                paper['booktitle'] = f"{conf_name} {year}"
                
                # Pages
                pages = info.find('pages')
                if pages is not None and pages.text:
                    paper['pages'] = pages.text
                
                # DOI
                doi = info.find('doi')
                if doi is not None and doi.text:
                    paper['doi'] = doi.text
                
                if 'title' in paper and 'authors' in paper:
                    papers.append(paper)
        
        print(f"    Found {len(papers)} papers")
        time.sleep(0.5)  # Rate limiting
        
    except Exception as e:
        print(f"    Error: {e}")
    
    return papers

def clean_text(text: str) -> str:
    """Clean text for BibTeX."""
    text = re.sub(r'<[^>]+>', '', text)
    text = text.replace('&amp;', '&')
    text = text.replace('&lt;', '<')
    text = text.replace('&gt;', '>')
    text = text.replace('&quot;', '"')
    if text.endswith('.'):
        text = text[:-1]
    return text.strip()

def generate_bibtex_key(paper: Dict, conf: str) -> str:
    """Generate a unique BibTeX key."""
    authors = paper.get('authors', '').split(' and ')
    first_author = authors[0].split()[-1].lower() if authors else 'unknown'
    year = paper.get('year', '0000')
    
    # Clean first author name
    first_author = re.sub(r'[^a-z]', '', first_author)
    
    # Get first word of title
    title_words = paper.get('title', '').split()
    first_word = ''
    for word in title_words:
        clean = re.sub(r'[^a-zA-Z]', '', word).lower()
        if len(clean) > 3:
            first_word = clean[:8]
            break
    
    return f"{first_author}{year}{conf.lower()}{first_word}"

def papers_to_bibtex(papers: List[Dict], conf: str) -> str:
    """Convert papers to BibTeX format."""
    bibtex = f"% Papers from {conf}\n"
    bibtex += f"% Fetched from DBLP\n\n"
    
    for i, paper in enumerate(papers):
        key = generate_bibtex_key(paper, conf)
        if i > 0 and key in bibtex:  # Avoid duplicate keys
            key = f"{key}{i}"
        
        bibtex += f"@inproceedings{{{key},\n"
        
        for field in ['title', 'authors', 'booktitle', 'year', 'pages', 'doi']:
            if field in paper:
                field_name = 'author' if field == 'authors' else field
                bibtex += f"  {field_name} = {{{paper[field]}}},\n"
        
        bibtex = bibtex.rstrip(',\n') + '\n}\n\n'
    
    return bibtex

def main():
    """Main function."""
    print("Fetching papers from top CS conferences...")
    print("=" * 60)
    
    # Create output directory
    output_dir = "../bib/csrankings"
    os.makedirs(output_dir, exist_ok=True)
    
    total_papers = 0
    
    # Process each conference
    for conf_name, venue_key in TOP_CONFERENCES.items():
        print(f"\nProcessing {conf_name}...")
        
        all_papers = []
        
        # Fetch recent years
        for year in [2023, 2022, 2021]:
            papers = fetch_conference_papers(conf_name, venue_key, year)
            all_papers.extend(papers)
        
        if all_papers:
            # Save to file
            filename = f"{conf_name.lower()}_recent.bib"
            filepath = os.path.join(output_dir, filename)
            
            bibtex_content = papers_to_bibtex(all_papers, conf_name)
            
            with open(filepath, 'w', encoding='utf-8') as f:
                f.write(bibtex_content)
            
            print(f"  Saved {len(all_papers)} papers to {filename}")
            total_papers += len(all_papers)
    
    print("\n" + "=" * 60)
    print(f"Total papers fetched: {total_papers}")
    print(f"Files saved to: {output_dir}")

if __name__ == "__main__":
    main()