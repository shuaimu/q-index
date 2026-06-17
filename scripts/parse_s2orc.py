#!/usr/bin/env python3
"""
Parse S2ORC data to extract citations for papers in our BibTeX database
"""

import os
import json
import gzip
import glob
from pathlib import Path
from typing import Dict, List, Set, Optional
from collections import defaultdict
import bibtexparser
import hashlib
import re

class S2ORCParser:
    def __init__(self, s2orc_dir: str = "data/s2orc", bib_dir: str = "bib"):
        self.s2orc_dir = Path(s2orc_dir)
        self.bib_dir = Path(bib_dir)
        self.output_dir = Path("cache/citations")
        self.output_dir.mkdir(parents=True, exist_ok=True)
        
        # Load our papers from BibTeX
        self.our_papers = self.load_bibtex_papers()
        print(f"Loaded {len(self.our_papers)} papers from BibTeX files")
        
        # Citation data
        self.citation_data = {}
        
    def load_bibtex_papers(self) -> Dict[str, dict]:
        """Load all papers from BibTeX files"""
        papers = {}
        
        for bib_file in self.bib_dir.glob("*.bib"):
            try:
                with open(bib_file, 'r', encoding='utf-8') as f:
                    bib_database = bibtexparser.load(f)
                    
                for entry in bib_database.entries:
                    # Create multiple keys for matching
                    title = entry.get('title', '').strip('{}')
                    year = entry.get('year', '')
                    
                    # Generate various matching keys
                    if title:
                        # Key 1: Normalized title
                        title_key = self.normalize_title(title)
                        
                        # Key 2: Title + Year
                        if year:
                            title_year_key = f"{title_key}_{year}"
                        else:
                            title_year_key = None
                        
                        # Store paper with multiple keys
                        paper_info = {
                            'title': title,
                            'year': year,
                            'authors': entry.get('author', ''),
                            'venue': bib_file.stem,
                            'doi': entry.get('doi', ''),
                            'id': entry.get('ID', ''),
                            'original_entry': entry
                        }
                        
                        papers[title_key] = paper_info
                        if title_year_key:
                            papers[title_year_key] = paper_info
                        
                        # Also store by DOI if available
                        if paper_info['doi']:
                            doi_key = paper_info['doi'].lower().strip()
                            papers[doi_key] = paper_info
                            
            except Exception as e:
                print(f"Error loading {bib_file}: {e}")
        
        return papers
    
    def normalize_title(self, title: str) -> str:
        """Normalize title for matching"""
        # Remove punctuation and convert to lowercase
        title = title.lower()
        title = re.sub(r'[^\w\s]', '', title)
        title = re.sub(r'\s+', ' ', title).strip()
        return title
    
    def match_paper(self, s2orc_paper: dict) -> Optional[dict]:
        """Try to match an S2ORC paper with our BibTeX papers"""
        # Try to match by title
        s2orc_title = s2orc_paper.get('title', '')
        if s2orc_title:
            title_key = self.normalize_title(s2orc_title)
            
            # Try exact title match
            if title_key in self.our_papers:
                return self.our_papers[title_key]
            
            # Try title + year match
            year = s2orc_paper.get('year')
            if year:
                title_year_key = f"{title_key}_{year}"
                if title_year_key in self.our_papers:
                    return self.our_papers[title_year_key]
        
        # Try to match by DOI
        s2orc_doi = s2orc_paper.get('doi', '')
        if s2orc_doi:
            doi_key = s2orc_doi.lower().strip()
            if doi_key in self.our_papers:
                return self.our_papers[doi_key]
        
        return None
    
    def process_s2orc_file(self, filepath: Path) -> int:
        """Process a single S2ORC metadata file"""
        matches_found = 0
        total_processed = 0
        
        print(f"\nProcessing {filepath.name}...")
        
        try:
            with gzip.open(filepath, 'rt') as f:
                for line in f:
                    total_processed += 1
                    if total_processed % 10000 == 0:
                        print(f"\r  Processed {total_processed:,} papers, found {matches_found} matches", end='', flush=True)
                    
                    try:
                        s2orc_paper = json.loads(line)
                        
                        # Check if this paper matches one of ours
                        our_paper = self.match_paper(s2orc_paper)
                        
                        if our_paper:
                            matches_found += 1
                            
                            # Extract citation data
                            paper_id = s2orc_paper.get('paper_id', '')
                            
                            self.citation_data[our_paper['id']] = {
                                'title': our_paper['title'],
                                'year': our_paper['year'],
                                'venue': our_paper['venue'],
                                's2orc_id': paper_id,
                                'doi': s2orc_paper.get('doi'),
                                'citation_count': len(s2orc_paper.get('inCitations', [])),
                                'in_citations': s2orc_paper.get('inCitations', []),
                                'out_citations': s2orc_paper.get('outCitations', []),
                                'abstract': s2orc_paper.get('abstract'),
                                's2orc_venue': s2orc_paper.get('venue'),
                                'authors': s2orc_paper.get('authors', [])
                            }
                            
                            # Print match info
                            if matches_found <= 10:  # Show first 10 matches
                                print(f"\n✓ Match #{matches_found}: {our_paper['title'][:60]}...")
                                print(f"  Venue: {our_paper['venue']}")
                                print(f"  Citations: {len(s2orc_paper.get('inCitations', []))}")
                    
                    except json.JSONDecodeError:
                        continue
                    except Exception as e:
                        if total_processed % 100000 == 0:
                            print(f"Error processing paper: {e}")
                        continue
                    
        except Exception as e:
            print(f"Error reading file {filepath}: {e}")
            return matches_found
        
        print(f"✓ Processed {total_processed:,} papers")
        print(f"✓ Found {matches_found} matches with our database")
        
        return matches_found
    
    def process_all_files(self):
        """Process all downloaded S2ORC files"""
        s2orc_files = sorted(self.s2orc_dir.glob("s2orc-metadata-*.jsonl.gz"))
        
        if not s2orc_files:
            print(f"No S2ORC files found in {self.s2orc_dir}")
            print("Please run download_s2orc.py first")
            return
        
        print(f"\nFound {len(s2orc_files)} S2ORC metadata files")
        
        total_matches = 0
        for filepath in s2orc_files:
            matches = self.process_s2orc_file(filepath)
            total_matches += matches
            
            # Save progress after each file
            self.save_citations()
            
            print(f"\nTotal matches so far: {total_matches}/{len(self.our_papers)}")
            
            # If we've found most papers, we can stop
            if total_matches >= len(self.our_papers) * 0.9:
                print(f"\n✓ Found 90% of papers, stopping early")
                break
        
        print(f"\n{'='*60}")
        print(f"Final Results:")
        print(f"  Total papers in BibTeX: {len(self.our_papers)}")
        print(f"  Papers found in S2ORC: {total_matches}")
        print(f"  Match rate: {100 * total_matches / len(self.our_papers):.1f}%")
        
    def save_citations(self):
        """Save citation data to JSON file"""
        output_file = self.output_dir / "s2orc_citations.json"
        
        with open(output_file, 'w') as f:
            json.dump(self.citation_data, f, indent=2)
        
        print(f"✓ Saved citation data to {output_file}")
        
        # Also create a summary
        self.create_summary()
    
    def create_summary(self):
        """Create a summary of citation statistics"""
        summary = {
            'total_papers_matched': len(self.citation_data),
            'total_citations': sum(p['citation_count'] for p in self.citation_data.values()),
            'papers_by_venue': defaultdict(int),
            'citations_by_venue': defaultdict(int),
            'top_cited_papers': []
        }
        
        # Calculate stats by venue
        for paper_id, data in self.citation_data.items():
            venue = data['venue']
            summary['papers_by_venue'][venue] += 1
            summary['citations_by_venue'][venue] += data['citation_count']
        
        # Get top cited papers
        sorted_papers = sorted(
            self.citation_data.items(), 
            key=lambda x: x[1]['citation_count'], 
            reverse=True
        )
        
        for paper_id, data in sorted_papers[:20]:
            summary['top_cited_papers'].append({
                'title': data['title'],
                'venue': data['venue'],
                'year': data['year'],
                'citations': data['citation_count']
            })
        
        # Save summary
        summary_file = self.output_dir / "s2orc_summary.json"
        with open(summary_file, 'w') as f:
            json.dump(summary, f, indent=2)
        
        # Print summary
        print("\n=== Citation Summary ===")
        print(f"Papers matched: {summary['total_papers_matched']}")
        print(f"Total citations: {summary['total_citations']:,}")
        print(f"\nTop 5 venues by citations:")
        
        venue_citations = sorted(
            summary['citations_by_venue'].items(), 
            key=lambda x: x[1], 
            reverse=True
        )
        
        for venue, citations in venue_citations[:5]:
            papers = summary['papers_by_venue'][venue]
            print(f"  {venue}: {citations:,} citations ({papers} papers)")
        
        print(f"\nTop 5 most cited papers:")
        for paper in summary['top_cited_papers'][:5]:
            print(f"  [{paper['citations']:,}] {paper['title'][:60]}...")
            print(f"         {paper['venue']} {paper['year']}")

def main():
    parser = S2ORCParser()
    
    print("=== S2ORC Citation Parser ===")
    print(f"Looking for S2ORC files in: {parser.s2orc_dir}")
    print(f"Loading papers from: {parser.bib_dir}")
    print(f"Output will be saved to: {parser.output_dir}")
    
    parser.process_all_files()

if __name__ == "__main__":
    main()