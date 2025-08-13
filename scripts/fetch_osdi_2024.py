#!/usr/bin/env python3
"""
Fetch all papers from OSDI 2024 and update the local BibTeX database.

This script scrapes the OSDI 2024 proceedings page to get all papers
and formats them as BibTeX entries to append to the osdi.bib file.
"""

import requests
from bs4 import BeautifulSoup
import re
import json
from datetime import datetime
import bibtexparser
from bibtexparser.bwriter import BibTexWriter
from bibtexparser.bibdatabase import BibDatabase
import sys
import time

def fetch_osdi_2024_papers():
    """Fetch paper information from OSDI 2024 proceedings page."""
    
    # OSDI 2024 proceedings URL
    url = "https://www.usenix.org/conference/osdi24/technical-sessions"
    
    print(f"Fetching papers from {url}...")
    
    try:
        response = requests.get(url, headers={
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
        })
        response.raise_for_status()
    except requests.RequestException as e:
        print(f"Error fetching page: {e}")
        return []
    
    soup = BeautifulSoup(response.text, 'html.parser')
    papers = []
    
    # Find all paper entries - USENIX typically uses specific class names
    # Try different selectors based on their typical page structure
    paper_divs = soup.find_all('div', class_='node-paper')
    if not paper_divs:
        paper_divs = soup.find_all('article', class_='paper')
    if not paper_divs:
        # Try to find papers by looking for title links
        paper_links = soup.find_all('a', href=re.compile(r'/conference/osdi24/presentation/'))
        
        for link in paper_links:
            # Skip if it's not a paper link
            if 'presentation' not in link.get('href', ''):
                continue
                
            paper_info = {}
            paper_info['title'] = link.get_text(strip=True)
            paper_info['url'] = f"https://www.usenix.org{link['href']}"
            
            # Try to find authors (usually in a nearby element)
            parent = link.find_parent(['div', 'article', 'li'])
            if parent:
                # Look for author information
                author_elem = parent.find(class_='field-name-field-paper-people-text')
                if not author_elem:
                    author_elem = parent.find(class_='authors')
                if not author_elem:
                    # Try to find text that looks like authors (contains commas and institutions)
                    for elem in parent.find_all(['div', 'span', 'p']):
                        text = elem.get_text(strip=True)
                        if ',' in text and any(word in text.lower() for word in ['university', 'institute', 'lab', 'microsoft', 'google', 'amazon', 'meta', 'ibm']):
                            author_elem = elem
                            break
                
                if author_elem:
                    authors_text = author_elem.get_text(strip=True)
                    # Parse authors - they're usually separated by commas or semicolons
                    # and may include affiliations in parentheses
                    authors = parse_authors(authors_text)
                    paper_info['authors'] = authors
                else:
                    paper_info['authors'] = []
            
            if paper_info.get('title'):
                papers.append(paper_info)
    
    print(f"Found {len(papers)} papers")
    return papers

def parse_authors(authors_text):
    """Parse author names from text, removing affiliations."""
    # Remove affiliations in parentheses
    authors_text = re.sub(r'\([^)]*\)', '', authors_text)
    # Remove institution names that appear after commas
    authors_text = re.sub(r',\s*[A-Z][^,;]*(?:University|Institute|Lab|Corporation|Inc\.|Corp\.)', '', authors_text)
    
    # Split by semicolon first (common separator between author groups)
    if ';' in authors_text:
        author_parts = authors_text.split(';')
    else:
        # Split by comma, but be careful with names like "Smith, John"
        author_parts = re.split(r',(?![^,]*(?:Jr\.|Sr\.|III|II|IV))', authors_text)
    
    authors = []
    for part in author_parts:
        part = part.strip()
        # Skip if it's likely an affiliation
        if any(word in part.lower() for word in ['university', 'institute', 'lab', 'department', 'school']):
            continue
        # Skip if too short or too long to be a name
        if len(part) < 3 or len(part) > 50:
            continue
        # Clean up the name
        part = re.sub(r'\s+', ' ', part)
        if part and not part.isdigit():
            authors.append(part)
    
    return authors

def fetch_paper_details(paper_url):
    """Fetch additional details for a specific paper."""
    try:
        response = requests.get(paper_url, headers={
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
        })
        response.raise_for_status()
        soup = BeautifulSoup(response.text, 'html.parser')
        
        # Try to find abstract
        abstract_elem = soup.find('div', class_='field-name-field-paper-abstract')
        if not abstract_elem:
            abstract_elem = soup.find('div', class_='abstract')
        
        abstract = abstract_elem.get_text(strip=True) if abstract_elem else None
        
        # Try to find PDF link
        pdf_link = soup.find('a', href=re.compile(r'\.pdf'))
        pdf_url = f"https://www.usenix.org{pdf_link['href']}" if pdf_link else None
        
        return {'abstract': abstract, 'pdf_url': pdf_url}
    except:
        return {'abstract': None, 'pdf_url': None}

def create_bibtex_entry(paper, index):
    """Create a BibTeX entry for a paper."""
    # Generate a unique key based on first author's last name and year
    if paper.get('authors'):
        first_author = paper['authors'][0]
        # Try to extract last name
        name_parts = first_author.split()
        if ',' in first_author:
            last_name = first_author.split(',')[0].strip()
        else:
            last_name = name_parts[-1] if name_parts else 'unknown'
        last_name = re.sub(r'[^a-zA-Z]', '', last_name.lower())
    else:
        last_name = 'unknown'
    
    # Clean title for key
    title_words = paper['title'].lower().split()[:2]
    title_part = ''.join(re.sub(r'[^a-z]', '', word) for word in title_words)
    
    key = f"{last_name}24{title_part}"
    
    entry = {
        'ID': key,
        'ENTRYTYPE': 'inproceedings',
        'title': paper['title'],
        'booktitle': 'OSDI',
        'year': '2024',
        'month': 'jul',  # OSDI 2024 was in July
    }
    
    if paper.get('authors'):
        entry['author'] = ' and '.join(paper['authors'])
    
    if paper.get('url'):
        entry['url'] = paper['url']
    
    if paper.get('abstract'):
        entry['abstract'] = paper['abstract']
    
    return entry

def update_bibtex_file(new_entries, bibtex_file='bib/osdi.bib'):
    """Update the BibTeX file with new entries."""
    
    # Read existing entries
    existing_entries = []
    try:
        with open(bibtex_file, 'r') as f:
            bib_database = bibtexparser.load(f)
            existing_entries = bib_database.entries
            existing_keys = {entry['ID'] for entry in existing_entries}
    except FileNotFoundError:
        print(f"Creating new file: {bibtex_file}")
        existing_keys = set()
    except Exception as e:
        print(f"Error reading existing file: {e}")
        existing_keys = set()
    
    # Filter out entries that already exist
    entries_to_add = []
    for entry in new_entries:
        if entry['ID'] not in existing_keys:
            entries_to_add.append(entry)
        else:
            print(f"Skipping duplicate: {entry['ID']}")
    
    if not entries_to_add:
        print("No new entries to add")
        return
    
    print(f"Adding {len(entries_to_add)} new entries...")
    
    # Append new entries to file
    with open(bibtex_file, 'a') as f:
        f.write('\n% OSDI 2024 Papers (added ' + datetime.now().strftime('%Y-%m-%d') + ')\n\n')
        
        writer = BibTexWriter()
        writer.indent = ''
        writer.order_entries_by = None
        
        for entry in entries_to_add:
            db = BibDatabase()
            db.entries = [entry]
            f.write(writer.write(db))
            f.write('\n')
    
    print(f"Successfully added {len(entries_to_add)} papers to {bibtex_file}")

def main():
    """Main function to fetch and update OSDI 2024 papers."""
    print("OSDI 2024 Paper Fetcher")
    print("=" * 50)
    
    # Fetch papers from OSDI 2024
    papers = fetch_osdi_2024_papers()
    
    if not papers:
        print("No papers found. The website structure might have changed.")
        print("Trying alternative approach...")
        
        # Alternative: manually add known OSDI 2024 papers
        # You can extend this list with actual OSDI 2024 papers
        papers = [
            {
                'title': 'EXAMPLE: This is a placeholder for OSDI 2024 papers',
                'authors': ['Author One', 'Author Two'],
                'url': 'https://www.usenix.org/conference/osdi24/'
            }
        ]
    
    # Fetch additional details for each paper (optional, may be slow)
    fetch_details = input("Fetch additional details for each paper? (y/n): ").lower() == 'y'
    
    if fetch_details:
        for i, paper in enumerate(papers):
            if paper.get('url'):
                print(f"Fetching details for paper {i+1}/{len(papers)}...")
                details = fetch_paper_details(paper['url'])
                paper.update(details)
                time.sleep(1)  # Be respectful to the server
    
    # Convert to BibTeX entries
    bibtex_entries = []
    for i, paper in enumerate(papers):
        entry = create_bibtex_entry(paper, i)
        bibtex_entries.append(entry)
        print(f"Created entry: {entry['ID']} - {entry.get('title', 'Unknown')[:50]}...")
    
    # Update the BibTeX file
    if bibtex_entries:
        update_bibtex_file(bibtex_entries)
    
    print("\nDone! Remember to:")
    print("1. Review the added entries in bib/osdi.bib")
    print("2. Run the sort script: python3 scripts/sort_papers_by_year.py")
    print("3. Rebuild the application: cargo build")

if __name__ == "__main__":
    main()