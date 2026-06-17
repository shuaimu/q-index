#!/usr/bin/env python3
"""
Build a local citation database from fetched citation data
Generates a format that can be imported into the Rust application
"""

import json
import time
from pathlib import Path
from collections import defaultdict
import sys

class CitationDatabase:
    def __init__(self, cache_dir="cache/citations"):
        self.cache_dir = Path(cache_dir)
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        
        # Load citation data
        self.paper_cache_file = self.cache_dir / "paper_cache.json"
        self.citation_cache_file = self.cache_dir / "citation_graph.json"
        
        self.papers = {}
        self.citations = defaultdict(list)
        self.references = defaultdict(list)
        
        self.load_data()
    
    def load_data(self):
        """Load citation data from cache files"""
        # Load paper metadata
        if self.paper_cache_file.exists():
            with open(self.paper_cache_file, 'r', encoding='utf-8') as f:
                self.papers = json.load(f)
        
        # Load citation graph
        if self.citation_cache_file.exists():
            with open(self.citation_cache_file, 'r', encoding='utf-8') as f:
                citation_graph = json.load(f)
                
                for paper_key, relations in citation_graph.items():
                    # Papers this paper cites
                    if 'cites' in relations:
                        self.references[paper_key] = relations['cites']
                    
                    # Papers that cite this paper
                    if 'cited_by' in relations:
                        self.citations[paper_key] = relations['cited_by']
    
    def build_citation_index(self):
        """Build a searchable citation index"""
        index = {
            'papers': [],
            'citations': [],
            'metadata': {
                'total_papers': len(self.papers),
                'total_citation_links': 0,
                'build_date': time.strftime('%Y-%m-%d %H:%M:%S')
            }
        }
        
        # Build paper index
        for paper_key, paper_data in self.papers.items():
            index['papers'].append({
                'id': paper_key,
                'title': paper_data.get('title'),
                'authors': paper_data.get('authors', []),
                'year': paper_data.get('year'),
                'doi': paper_data.get('doi'),
                'venue': paper_data.get('venue'),
                'citation_count': paper_data.get('citation_count', 0),
                'ss_id': paper_data.get('ss_id')
            })
        
        # Build citation relationships
        for paper_key in self.papers:
            # Add references (papers this paper cites)
            for ref_id in self.references.get(paper_key, []):
                index['citations'].append({
                    'source': paper_key,
                    'target': ref_id,
                    'type': 'cites'
                })
                index['metadata']['total_citation_links'] += 1
            
            # Add citations (papers that cite this paper)
            for cit_id in self.citations.get(paper_key, []):
                index['citations'].append({
                    'source': cit_id,
                    'target': paper_key,
                    'type': 'cites'
                })
                index['metadata']['total_citation_links'] += 1
        
        return index
    
    def export_to_rust_format(self, output_file="cache/citations/citation_data.json"):
        """Export citation data in a format suitable for Rust import"""
        output_path = Path(output_file)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        
        # Build the index
        index = self.build_citation_index()
        
        # Save to file
        with open(output_path, 'w', encoding='utf-8') as f:
            json.dump(index, f, indent=2, ensure_ascii=False)
        
        print(f"Citation database exported to {output_path}")
        print(f"  Total papers: {index['metadata']['total_papers']:,}")
        print(f"  Total citation links: {index['metadata']['total_citation_links']:,}")
        
        return output_path
    
    def generate_statistics(self):
        """Generate detailed statistics about the citation database"""
        stats = {
            'total_papers': len(self.papers),
            'papers_with_citation_count': 0,
            'papers_with_references': 0,
            'papers_with_citations': 0,
            'total_citation_links': 0,
            'max_citations': 0,
            'avg_citations': 0,
            'venues': defaultdict(int),
            'years': defaultdict(int)
        }
        
        citation_counts = []
        
        for paper_key, paper_data in self.papers.items():
            # Citation count
            count = paper_data.get('citation_count', 0)
            if count > 0:
                stats['papers_with_citation_count'] += 1
                citation_counts.append(count)
                stats['max_citations'] = max(stats['max_citations'], count)
            
            # References
            if paper_key in self.references and len(self.references[paper_key]) > 0:
                stats['papers_with_references'] += 1
                stats['total_citation_links'] += len(self.references[paper_key])
            
            # Citations
            if paper_key in self.citations and len(self.citations[paper_key]) > 0:
                stats['papers_with_citations'] += 1
            
            # Venue distribution
            venue = paper_data.get('venue', 'Unknown')
            if venue:
                stats['venues'][venue] += 1
            
            # Year distribution
            year = paper_data.get('year')
            if year:
                stats['years'][year] += 1
        
        # Calculate average
        if citation_counts:
            stats['avg_citations'] = sum(citation_counts) / len(citation_counts)
        
        return stats
    
    def find_highly_cited_papers(self, min_citations=100):
        """Find papers with high citation counts"""
        highly_cited = []
        
        for paper_key, paper_data in self.papers.items():
            count = paper_data.get('citation_count', 0)
            if count >= min_citations:
                highly_cited.append({
                    'title': paper_data.get('title'),
                    'authors': paper_data.get('authors', []),
                    'year': paper_data.get('year'),
                    'venue': paper_data.get('venue'),
                    'citations': count
                })
        
        # Sort by citation count
        highly_cited.sort(key=lambda x: x['citations'], reverse=True)
        
        return highly_cited
    
    def export_graphml(self, output_file="cache/citations/citation_graph.graphml"):
        """Export citation graph in GraphML format for visualization"""
        output_path = Path(output_file)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        
        with open(output_path, 'w', encoding='utf-8') as f:
            f.write('<?xml version="1.0" encoding="UTF-8"?>\n')
            f.write('<graphml xmlns="http://graphml.graphdrawing.org/xmlns">\n')
            
            # Define attributes
            f.write('  <key id="title" for="node" attr.name="title" attr.type="string"/>\n')
            f.write('  <key id="year" for="node" attr.name="year" attr.type="int"/>\n')
            f.write('  <key id="citations" for="node" attr.name="citations" attr.type="int"/>\n')
            f.write('  <key id="venue" for="node" attr.name="venue" attr.type="string"/>\n')
            
            f.write('  <graph id="citations" edgedefault="directed">\n')
            
            # Add nodes
            for paper_key, paper_data in self.papers.items():
                f.write(f'    <node id="{paper_key}">\n')
                f.write(f'      <data key="title">{paper_data.get("title", "")[:100]}</data>\n')
                f.write(f'      <data key="year">{paper_data.get("year", 0)}</data>\n')
                f.write(f'      <data key="citations">{paper_data.get("citation_count", 0)}</data>\n')
                f.write(f'      <data key="venue">{paper_data.get("venue", "")}</data>\n')
                f.write('    </node>\n')
            
            # Add edges
            edge_id = 0
            for paper_key in self.papers:
                for ref_id in self.references.get(paper_key, []):
                    f.write(f'    <edge id="e{edge_id}" source="{paper_key}" target="{ref_id}"/>\n')
                    edge_id += 1
            
            f.write('  </graph>\n')
            f.write('</graphml>\n')
        
        print(f"Citation graph exported to {output_path} (GraphML format)")

def main():
    """Main function"""
    import argparse
    
    parser = argparse.ArgumentParser(description='Build and export citation database')
    parser.add_argument('--export-rust', action='store_true', 
                       help='Export to Rust-compatible JSON format')
    parser.add_argument('--export-graphml', action='store_true',
                       help='Export to GraphML format for visualization')
    parser.add_argument('--stats', action='store_true',
                       help='Show detailed statistics')
    parser.add_argument('--highly-cited', type=int, metavar='MIN',
                       help='Show papers with at least MIN citations')
    
    args = parser.parse_args()
    
    db = CitationDatabase()
    
    if args.stats:
        stats = db.generate_statistics()
        print("\n=== Citation Database Statistics ===")
        print(f"Total papers: {stats['total_papers']:,}")
        print(f"Papers with citation count: {stats['papers_with_citation_count']:,}")
        print(f"Papers with references: {stats['papers_with_references']:,}")
        print(f"Papers with citations: {stats['papers_with_citations']:,}")
        print(f"Total citation links: {stats['total_citation_links']:,}")
        print(f"Maximum citations: {stats['max_citations']:,}")
        print(f"Average citations: {stats['avg_citations']:.1f}")
        
        print("\nTop venues by paper count:")
        venue_list = sorted(stats['venues'].items(), key=lambda x: x[1], reverse=True)
        for venue, count in venue_list[:10]:
            print(f"  {venue}: {count}")
        
        print("\nPapers by year (last 10 years):")
        current_year = 2024
        for year in range(current_year, current_year-10, -1):
            if year in stats['years']:
                print(f"  {year}: {stats['years'][year]}")
    
    if args.highly_cited:
        papers = db.find_highly_cited_papers(args.highly_cited)
        print(f"\n=== Papers with ≥{args.highly_cited} citations ===")
        for i, paper in enumerate(papers[:20], 1):
            print(f"\n{i}. {paper['title'][:80]}...")
            print(f"   Authors: {', '.join(paper['authors'][:3])}")
            print(f"   Year: {paper['year']}, Venue: {paper['venue']}")
            print(f"   Citations: {paper['citations']:,}")
    
    if args.export_rust:
        db.export_to_rust_format()
    
    if args.export_graphml:
        db.export_graphml()
    
    if not any([args.stats, args.highly_cited, args.export_rust, args.export_graphml]):
        parser.print_help()
        print("\nExamples:")
        print("  python3 scripts/build_citation_database.py --stats")
        print("  python3 scripts/build_citation_database.py --highly-cited 100")
        print("  python3 scripts/build_citation_database.py --export-rust")
        print("  python3 scripts/build_citation_database.py --export-graphml")

if __name__ == "__main__":
    main()