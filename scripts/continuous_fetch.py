#!/usr/bin/env python3
"""
Continuous citation fetching script that processes all conferences.
Runs until all papers are fetched or manually stopped.
"""

import os
import sys
import time
import json
import subprocess
from pathlib import Path
from datetime import datetime

# Add project root to path
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

def main():
    # Setup paths
    project_root = Path(__file__).parent.parent
    bib_dir = project_root / "bib"
    log_dir = project_root / "logs"
    cache_dir = project_root / "cache" / "citations"
    
    # Create directories
    log_dir.mkdir(exist_ok=True)
    cache_dir.mkdir(parents=True, exist_ok=True)
    
    # Get all BibTeX files
    all_bib_files = sorted(bib_dir.glob("*.bib"))
    
    # Priority order for conferences
    priority_order = [
        'sigcomm', 'sosp', 'osdi', 'nsdi', 'sigmod', 'vldb',
        'pldi', 'popl', 'icml', 'neurips', 'iclr', 'cvpr',
        'ccs', 'oakland', 'ndss', 'stoc', 'focs', 'podc',
        'eurosys', 'asplos', 'atc', 'fast', 'icde', 'cidr',
        'oopsla', 'ecoop', 'isca', 'micro', 'hpca', 'sc',
        'imc', 'conext', 'mobicom', 'mobisys', 'sensys',
        'kdd', 'www', 'icse', 'fse', 'ase', 'issta',
        'tocs', 'tods', 'jacm', 'toplas', 'cacm'
    ]
    
    # Sort BibTeX files by priority
    def get_priority(bib_file):
        name = bib_file.stem.lower()
        try:
            return priority_order.index(name)
        except ValueError:
            return len(priority_order)
    
    sorted_bib_files = sorted(all_bib_files, key=get_priority)
    
    print("="*60)
    print("CONTINUOUS CITATION FETCHING")
    print(f"Started at: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print("="*60)
    
    # Process each conference
    for bib_file in sorted_bib_files:
        conf_name = bib_file.stem
        log_file = log_dir / f"fetch_{conf_name}.log"
        
        # Check current cache status
        if (cache_dir / "paper_cache.json").exists():
            with open(cache_dir / "paper_cache.json", 'r') as f:
                cache = json.load(f)
            current_papers = len(cache)
            current_citations = sum(p.get("citation_count", 0) for p in cache.values())
        else:
            current_papers = 0
            current_citations = 0
        
        print(f"\n[{datetime.now().strftime('%H:%M:%S')}] Processing {conf_name}...")
        print(f"  Current cache: {current_papers} papers, {current_citations:,} citations")
        
        # Run fetch script
        cmd = [
            sys.executable,
            "scripts/fetch_citations.py",
            str(bib_file),
            "--log", str(log_file)
        ]
        
        try:
            # Run with timeout of 30 minutes per conference
            result = subprocess.run(cmd, capture_output=True, text=True, timeout=1800)
            
            # Check new cache status
            if (cache_dir / "paper_cache.json").exists():
                with open(cache_dir / "paper_cache.json", 'r') as f:
                    new_cache = json.load(f)
                new_papers = len(new_cache)
                new_citations = sum(p.get("citation_count", 0) for p in new_cache.values())
                
                papers_added = new_papers - current_papers
                citations_added = new_citations - current_citations
                
                if papers_added > 0:
                    print(f"  ✓ Added {papers_added} papers with {citations_added:,} citations")
                else:
                    print(f"  → No new papers added (may be already cached)")
            
            if result.returncode != 0:
                print(f"  ⚠ Warning: Non-zero exit code for {conf_name}")
                
        except subprocess.TimeoutExpired:
            print(f"  ⚠ Timeout for {conf_name} (30 minutes)")
        except Exception as e:
            print(f"  ✗ Error processing {conf_name}: {e}")
        
        # Small delay between conferences
        time.sleep(5)
    
    # Final statistics
    print("\n" + "="*60)
    print("FETCHING COMPLETE")
    print("="*60)
    
    if (cache_dir / "paper_cache.json").exists():
        with open(cache_dir / "paper_cache.json", 'r') as f:
            final_cache = json.load(f)
        
        total_papers = len(final_cache)
        total_citations = sum(p.get("citation_count", 0) for p in final_cache.values())
        highly_cited = len([p for p in final_cache.values() if p.get("citation_count", 0) >= 100])
        
        print(f"Total papers fetched: {total_papers:,}")
        print(f"Total citations: {total_citations:,}")
        print(f"Papers with 100+ citations: {highly_cited}")
        
        # Top cited papers
        top_papers = sorted(
            [(p.get("citation_count", 0), p.get("title", "")[:80]) for p in final_cache.values()],
            reverse=True
        )[:10]
        
        print("\nTop 10 most cited papers:")
        for i, (count, title) in enumerate(top_papers, 1):
            print(f"  {i}. {count:,} citations: {title}")
        
        # Build and export the database
        print("\nBuilding citation database...")
        subprocess.run([sys.executable, "scripts/build_citation_database.py"])
        print("✓ Citation database exported")

if __name__ == "__main__":
    main()