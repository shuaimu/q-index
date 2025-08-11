#!/usr/bin/env python3
"""
Fetch papers from DBLP for CSRankings conferences and convert to BibTeX format.
"""

import requests
import xml.etree.ElementTree as ET
import time
import os
import re
from typing import List, Dict, Optional
import json

# CSRankings conference list
CONFERENCES = {
    'systems': [
        'SOSP', 'OSDI', 'EuroSys', 'USENIX ATC', 'FAST',
        'NSDI', 'SIGCOMM', 'ASPLOS', 'ISCA', 'MICRO',
        'PLDI', 'POPL', 'OOPSLA', 'ICFP',
        'SIGMOD', 'VLDB', 'ICDE', 'PODS',
        'CCS', 'IEEE S&P', 'USENIX Security', 'NDSS',
        'SC', 'HPDC', 'ICS',
        'MobiCom', 'MobiSys', 'SenSys', 'IMC', 'SIGMETRICS',
        'FSE', 'ICSE', 'ASE', 'ISSTA'
    ],
    'ai_ml': [
        'NeurIPS', 'ICML', 'ICLR', 'AAAI', 'IJCAI',
        'CVPR', 'ICCV', 'ECCV',
        'ACL', 'EMNLP', 'NAACL',
        'KDD', 'SIGIR', 'WWW'
    ],
    'theory': [
        'STOC', 'FOCS', 'SODA',
        'CRYPTO', 'EUROCRYPT',
        'CAV', 'LICS'
    ],
    'interdisciplinary': [
        'CHI', 'UIST', 'UbiComp',
        'SIGGRAPH', 'VIS', 'VR',
        'ICRA', 'IROS', 'RSS'
    ]
}

# DBLP venue keys mapping (common conference names to DBLP keys)
DBLP_VENUE_MAPPING = {
    'SOSP': 'conf/sosp',
    'OSDI': 'conf/osdi',
    'EuroSys': 'conf/eurosys',
    'USENIX ATC': 'conf/usenix',
    'FAST': 'conf/fast',
    'NSDI': 'conf/nsdi',
    'SIGCOMM': 'conf/sigcomm',
    'ASPLOS': 'conf/asplos',
    'ISCA': 'conf/isca',
    'MICRO': 'conf/micro',
    'PLDI': 'conf/pldi',
    'POPL': 'conf/popl',
    'OOPSLA': 'conf/oopsla',
    'ICFP': 'conf/icfp',
    'SIGMOD': 'conf/sigmod',
    'VLDB': 'conf/vldb',
    'ICDE': 'conf/icde',
    'PODS': 'conf/pods',
    'CCS': 'conf/ccs',
    'IEEE S&P': 'conf/sp',
    'USENIX Security': 'conf/uss',
    'NDSS': 'conf/ndss',
    'SC': 'conf/sc',
    'HPDC': 'conf/hpdc',
    'ICS': 'conf/ics',
    'MobiCom': 'conf/mobicom',
    'MobiSys': 'conf/mobisys',
    'SenSys': 'conf/sensys',
    'IMC': 'conf/imc',
    'SIGMETRICS': 'conf/sigmetrics',
    'FSE': 'conf/fse',
    'ICSE': 'conf/icse',
    'ASE': 'conf/kbse',
    'ISSTA': 'conf/issta',
    'NeurIPS': 'conf/nips',
    'ICML': 'conf/icml',
    'ICLR': 'conf/iclr',
    'AAAI': 'conf/aaai',
    'IJCAI': 'conf/ijcai',
    'CVPR': 'conf/cvpr',
    'ICCV': 'conf/iccv',
    'ECCV': 'conf/eccv',
    'ACL': 'conf/acl',
    'EMNLP': 'conf/emnlp',
    'NAACL': 'conf/naacl',
    'KDD': 'conf/kdd',
    'SIGIR': 'conf/sigir',
    'WWW': 'conf/www',
    'STOC': 'conf/stoc',
    'FOCS': 'conf/focs',
    'SODA': 'conf/soda',
    'CRYPTO': 'conf/crypto',
    'EUROCRYPT': 'conf/eurocrypt',
    'CAV': 'conf/cav',
    'LICS': 'conf/lics',
    'CHI': 'conf/chi',
    'UIST': 'conf/uist',
    'UbiComp': 'conf/ubicomp',
    'SIGGRAPH': 'conf/siggraph',
    'VIS': 'conf/visualization',
    'VR': 'conf/vr',
    'ICRA': 'conf/icra',
    'IROS': 'conf/iros',
    'RSS': 'conf/rss'
}

def fetch_dblp_papers(venue_key: str, years: List[int]) -> List[Dict]:
    """
    Fetch papers from DBLP for a specific venue and years.
    """
    papers = []
    
    for year in years:
        url = f"https://dblp.org/search/publ/api?q=venue:{venue_key}%20year:{year}&format=xml&h=1000"
        
        try:
            print(f"  Fetching {venue_key} {year}...")
            response = requests.get(url, timeout=30)
            response.raise_for_status()
            
            root = ET.fromstring(response.content)
            hits = root.findall('.//hit')
            
            for hit in hits:
                info = hit.find('info')
                if info is not None:
                    paper = extract_paper_info(info)
                    if paper:
                        papers.append(paper)
            
            # Be polite to DBLP API
            time.sleep(1)
            
        except Exception as e:
            print(f"    Error fetching {venue_key} {year}: {e}")
    
    return papers

def extract_paper_info(info: ET.Element) -> Optional[Dict]:
    """
    Extract paper information from DBLP XML element.
    """
    try:
        paper = {}
        
        # Extract basic fields
        title = info.find('title')
        if title is not None and title.text:
            paper['title'] = clean_text(title.text)
        
        # Authors
        authors = []
        for author in info.findall('authors/author'):
            if author.text:
                authors.append(author.text)
        if authors:
            paper['authors'] = authors
        
        # Year
        year = info.find('year')
        if year is not None and year.text:
            paper['year'] = year.text
        
        # Venue
        venue = info.find('venue')
        if venue is not None and venue.text:
            paper['venue'] = venue.text
        
        # Key (for citation)
        key = info.find('key')
        if key is not None and key.text:
            paper['key'] = key.text.split('/')[-1]  # Get last part as key
        
        # Pages
        pages = info.find('pages')
        if pages is not None and pages.text:
            paper['pages'] = pages.text
        
        # DOI
        doi = info.find('doi')
        if doi is not None and doi.text:
            paper['doi'] = doi.text
        
        # URL
        url = info.find('url')
        if url is not None and url.text:
            paper['url'] = url.text
        
        # Only return if we have essential fields
        if 'title' in paper and 'authors' in paper:
            return paper
    
    except Exception as e:
        print(f"    Error extracting paper info: {e}")
    
    return None

def clean_text(text: str) -> str:
    """Clean text for BibTeX format."""
    # Remove HTML tags
    text = re.sub(r'<[^>]+>', '', text)
    # Fix special characters
    text = text.replace('&amp;', '&')
    text = text.replace('&lt;', '<')
    text = text.replace('&gt;', '>')
    text = text.replace('&quot;', '"')
    # Remove trailing period if present
    if text.endswith('.'):
        text = text[:-1]
    return text.strip()

def paper_to_bibtex(paper: Dict, conf_name: str) -> str:
    """
    Convert paper dictionary to BibTeX format.
    """
    # Generate a unique key
    first_author = paper['authors'][0].split()[-1].lower() if paper['authors'] else 'unknown'
    year = paper.get('year', '0000')
    title_words = paper.get('title', '').split()[:3]
    title_part = ''.join(word.lower() for word in title_words if word.isalpha())[:15]
    key = f"{first_author}{year}{title_part}"
    
    # Clean key
    key = re.sub(r'[^a-z0-9]', '', key)
    
    # Build BibTeX entry
    bibtex = f"@inproceedings{{{key},\n"
    
    if 'title' in paper:
        bibtex += f"  title = {{{paper['title']}}},\n"
    
    if 'authors' in paper:
        authors_str = ' and '.join(paper['authors'])
        bibtex += f"  author = {{{authors_str}}},\n"
    
    if 'venue' in paper:
        bibtex += f"  booktitle = {{{paper['venue']}}},\n"
    
    if 'year' in paper:
        bibtex += f"  year = {{{paper['year']}}},\n"
    
    if 'pages' in paper:
        bibtex += f"  pages = {{{paper['pages']}}},\n"
    
    if 'doi' in paper:
        bibtex += f"  doi = {{{paper['doi']}}},\n"
    
    if 'url' in paper:
        bibtex += f"  url = {{{paper['url']}}},\n"
    
    bibtex += "}\n"
    
    return bibtex

def main():
    """Main function to fetch papers and save as BibTeX."""
    
    # Years to fetch (recent years for better data)
    years = list(range(2019, 2025))
    
    # Create output directory
    output_dir = "../bib/csrankings"
    os.makedirs(output_dir, exist_ok=True)
    
    # Track statistics
    total_papers = 0
    processed_confs = []
    failed_confs = []
    
    # Process each conference area
    for area, conferences in CONFERENCES.items():
        print(f"\nProcessing {area} conferences...")
        
        for conf in conferences:
            if conf not in DBLP_VENUE_MAPPING:
                print(f"  Skipping {conf} - no DBLP mapping")
                failed_confs.append(conf)
                continue
            
            venue_key = DBLP_VENUE_MAPPING[conf]
            papers = fetch_dblp_papers(venue_key, years)
            
            if papers:
                # Save to BibTeX file
                filename = f"{conf.lower().replace(' ', '_').replace('/', '_')}_recent.bib"
                filepath = os.path.join(output_dir, filename)
                
                with open(filepath, 'w', encoding='utf-8') as f:
                    f.write(f"% Papers from {conf} ({years[0]}-{years[-1]})\n")
                    f.write(f"% Fetched from DBLP\n\n")
                    
                    for paper in papers:
                        f.write(paper_to_bibtex(paper, conf))
                        f.write("\n")
                
                print(f"  Saved {len(papers)} papers from {conf} to {filename}")
                total_papers += len(papers)
                processed_confs.append(conf)
            else:
                print(f"  No papers found for {conf}")
                failed_confs.append(conf)
    
    # Print summary
    print(f"\n{'='*60}")
    print(f"Summary:")
    print(f"  Total papers fetched: {total_papers}")
    print(f"  Conferences processed: {len(processed_confs)}")
    print(f"  Conferences failed/skipped: {len(failed_confs)}")
    
    if processed_confs:
        print(f"\nSuccessfully processed:")
        for conf in processed_confs:
            print(f"  - {conf}")
    
    if failed_confs:
        print(f"\nFailed/Skipped:")
        for conf in failed_confs:
            print(f"  - {conf}")

if __name__ == "__main__":
    main()