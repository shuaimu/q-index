#!/usr/bin/env python3
"""
Parse S2AG (Semantic Scholar Academic Graph) data to extract citations for our papers
"""

import os
import json
import gzip
from pathlib import Path
from typing import Dict, List, Optional
from collections import defaultdict
import re
import time

class S2AGParser:
    def __init__(self, s2ag_dir: str = "data/s2ag", bib_dir: str = "bib", output_dir: str = "cache/citations"):
        self.s2ag_dir = Path(s2ag_dir)
        self.bib_dir = Path(bib_dir)
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)
        
        # Load our papers from BibTeX
        self.our_papers = self.load_bibtex_papers()
        print(f"Loaded {len(self.our_papers)} papers from BibTeX files")
        
        # Citation data
        self.citation_data = {}
        self.paper_id_mapping = {}  # Map our paper IDs to S2AG corpus IDs
        
    def load_bibtex_papers(self) -> Dict[str, dict]:
        """Load all papers from BibTeX files"""
        papers = {}
        
        for bib_file in self.bib_dir.glob("*.bib"):
            try:
                with open(bib_file, 'r', encoding='utf-8') as f:
                    content = f.read()
                
                # Simple BibTeX parsing
                entries = content.split('@')[1:]
                
                for entry in entries:
                    if not entry.strip():
                        continue
                    
                    # Extract title
                    title_match = re.search(r'title\s*=\s*[{"]([^}"]+)[}"]', entry, re.IGNORECASE)
                    if not title_match:
                        continue
                    
                    title = title_match.group(1).strip('{}')
                    
                    # Extract year
                    year_match = re.search(r'year\s*=\s*[{"]?(\d{4})[}"]?', entry, re.IGNORECASE)
                    year = year_match.group(1) if year_match else None
                    
                    # Extract DOI
                    doi_match = re.search(r'doi\s*=\s*[{"]([^}"]+)[}"]', entry, re.IGNORECASE)
                    doi = doi_match.group(1) if doi_match else None
                    
                    # Create normalized keys for matching
                    title_key = self.normalize_title(title)
                    
                    paper_info = {
                        'title': title,
                        'year': year,
                        'venue': bib_file.stem,
                        'doi': doi,
                        'title_key': title_key
                    }
                    
                    # Store with multiple keys
                    papers[title_key] = paper_info
                    if year:
                        papers[f"{title_key}_{year}"] = paper_info
                    if doi:
                        papers[doi.lower()] = paper_info
                        
            except Exception as e:
                print(f"Error loading {bib_file}: {e}")
        
        return papers
    
    def normalize_title(self, title: str) -> str:
        """Normalize title for matching"""
        # Remove latex commands
        title = re.sub(r'\\[a-zA-Z]+\{([^}]*)\}', r'\1', title)
        title = re.sub(r'[{}]', '', title)
        # Remove punctuation and convert to lowercase
        title = title.lower()
        title = re.sub(r'[^\w\s]', '', title)
        title = re.sub(r'\s+', ' ', title).strip()
        return title
    
    def match_paper(self, s2ag_paper: dict) -> Optional[dict]:
        """Try to match an S2AG paper with our BibTeX papers"""
        # Try to match by title
        s2ag_title = s2ag_paper.get('title', '')
        if s2ag_title:
            title_key = self.normalize_title(s2ag_title)
            
            # Try exact title match
            if title_key in self.our_papers:
                return self.our_papers[title_key]
            
            # Try title + year match
            year = s2ag_paper.get('year')
            if year:
                title_year_key = f"{title_key}_{year}"
                if title_year_key in self.our_papers:
                    return self.our_papers[title_year_key]
        
        # Try to match by DOI
        external_ids = s2ag_paper.get('externalids', {}) or {}
        doi = external_ids.get('DOI', '')
        if doi:
            doi_key = doi.lower()
            if doi_key in self.our_papers:
                return self.our_papers[doi_key]
        
        return None
    
    def process_papers_file(self, filepath: Path) -> int:
        """Process a single S2AG papers file"""
        matches_found = 0
        total_processed = 0
        
        print(f"\nProcessing {filepath.name}...")
        file_size_mb = filepath.stat().st_size / (1024 * 1024)
        print(f"  File size: {file_size_mb:.1f} MB")
        
        try:
            with gzip.open(filepath, 'rt') as f:
                for line_num, line in enumerate(f, 1):
                    total_processed += 1
                    
                    # Show progress every 10000 papers
                    if total_processed % 10000 == 0:
                        print(f"\r  Processed {total_processed:,} papers, found {matches_found} matches", 
                              end='', flush=True)
                    
                    try:
                        s2ag_paper = json.loads(line)
                        
                        # Check if this paper matches one of ours
                        our_paper = self.match_paper(s2ag_paper)
                        
                        if our_paper:
                            matches_found += 1
                            
                            # Extract citation data
                            corpus_id = s2ag_paper.get('corpusid')
                            
                            # Store the mapping
                            paper_key = our_paper['title_key']
                            self.paper_id_mapping[paper_key] = corpus_id
                            
                            # Extract paper data
                            self.citation_data[paper_key] = {
                                'title': our_paper['title'],
                                'year': our_paper['year'],
                                'venue': our_paper['venue'],
                                'corpus_id': corpus_id,
                                's2ag_title': s2ag_paper.get('title'),
                                's2ag_year': s2ag_paper.get('year'),
                                'doi': s2ag_paper.get('externalids', {}).get('DOI'),
                                'citation_count': s2ag_paper.get('citationcount', 0),
                                'reference_count': s2ag_paper.get('referencecount', 0),
                                'authors': [a.get('name', '') for a in s2ag_paper.get('authors', [])],
                                'venue_info': s2ag_paper.get('venue'),
                                'fields': s2ag_paper.get('s2fieldsofstudy', []),
                                'abstract': s2ag_paper.get('abstract'),
                                'url': s2ag_paper.get('url')
                            }
                            
                            # Print first few matches
                            if matches_found <= 5:
                                print(f"\n  ✓ Match #{matches_found}: {our_paper['title'][:60]}...")
                                print(f"    Venue: {our_paper['venue']}, Year: {our_paper['year']}")
                                print(f"    Citations: {s2ag_paper.get('citationcount', 0)}")
                    
                    except json.JSONDecodeError:
                        continue
                    except Exception as e:
                        if total_processed % 100000 == 0:
                            print(f"\n  Error at line {line_num}: {e}")
                        continue
                
                print()  # New line after progress
                    
        except Exception as e:
            print(f"\nError reading file {filepath}: {e}")
            return matches_found
        
        print(f"  ✓ Processed {total_processed:,} papers")
        print(f"  ✓ Found {matches_found} matches with our database")
        
        return matches_found
    
    def process_all_papers(self):
        """Process all S2AG papers files"""
        papers_dir = self.s2ag_dir / "papers"
        
        if not papers_dir.exists():
            print(f"Papers directory not found: {papers_dir}")
            print("Please download S2AG papers dataset first")
            return
        
        paper_files = sorted(papers_dir.glob("*.gz"))
        
        if not paper_files:
            print(f"No paper files found in {papers_dir}")
            return
        
        print(f"\nFound {len(paper_files)} S2AG papers files")
        print("="*60)
        
        total_matches = 0
        for i, filepath in enumerate(paper_files, 1):
            print(f"\n[{i}/{len(paper_files)}] Processing file...")
            matches = self.process_papers_file(filepath)
            total_matches += matches
            
            # Save progress after each file
            self.save_citations()
            
            print(f"\nTotal matches so far: {total_matches}/{len(self.our_papers)}")
            
            # If we've found most papers, we can stop
            if total_matches >= len(self.our_papers) * 0.9:
                print(f"\n✓ Found 90% of papers, stopping early")
                break
        
        print("\n" + "="*60)
        print("Final Results:")
        print(f"  Total papers in BibTeX: {len(self.our_papers)}")
        print(f"  Papers found in S2AG: {total_matches}")
        print(f"  Match rate: {100 * total_matches / len(self.our_papers):.1f}%")
        
    def save_citations(self):
        """Save citation data to JSON file"""
        output_file = self.output_dir / "s2ag_citations.json"
        
        with open(output_file, 'w') as f:
            json.dump(self.citation_data, f, indent=2)
        
        print(f"  ✓ Saved citation data to {output_file}")
        
        # Also save ID mapping
        mapping_file = self.output_dir / "s2ag_id_mapping.json"
        with open(mapping_file, 'w') as f:
            json.dump(self.paper_id_mapping, f, indent=2)
        
        # Create summary
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
        summary_file = self.output_dir / "s2ag_summary.json"
        with open(summary_file, 'w') as f:
            json.dump(summary, f, indent=2, default=str)
        
        # Print summary
        print("\n=== Citation Summary ===")
        print(f"Papers matched: {summary['total_papers_matched']}")
        print(f"Total citations: {summary['total_citations']:,}")
        
        if summary['citations_by_venue']:
            print(f"\nTop 5 venues by citations:")
            venue_citations = sorted(
                summary['citations_by_venue'].items(), 
                key=lambda x: x[1], 
                reverse=True
            )
            
            for venue, citations in venue_citations[:5]:
                papers = summary['papers_by_venue'][venue]
                print(f"  {venue}: {citations:,} citations ({papers} papers)")
        
        if summary['top_cited_papers']:
            print(f"\nTop 5 most cited papers:")
            for paper in summary['top_cited_papers'][:5]:
                print(f"  [{paper['citations']:,}] {paper['title'][:60]}...")
                print(f"         {paper['venue']} {paper['year']}")

def main():
    import argparse
    
    parser = argparse.ArgumentParser(description='Parse S2AG dataset for citations')
    parser.add_argument('--s2ag-dir', default='data/s2ag', help='S2AG data directory')
    parser.add_argument('--bib-dir', default='bib', help='BibTeX directory')
    parser.add_argument('--test', action='store_true', help='Test with first file only')
    
    args = parser.parse_args()
    
    parser = S2AGParser(args.s2ag_dir, args.bib_dir)
    
    if args.test:
        # Test with first file only
        papers_dir = Path(args.s2ag_dir) / "papers"
        paper_files = sorted(papers_dir.glob("*.gz"))
        if paper_files:
            parser.process_papers_file(paper_files[0])
            parser.save_citations()
        else:
            print("No paper files found for testing")
    else:
        parser.process_all_papers()

if __name__ == "__main__":
    main()