#!/usr/bin/env python3
"""
Fetch citations for all papers in batches with better error handling
"""

import subprocess
import time
import json
from pathlib import Path
from datetime import datetime

def get_conference_paper_count(conf):
    """Get number of papers in a conference file"""
    filepath = Path(f"bib/{conf}.bib")
    if not filepath.exists():
        return 0
    
    with open(filepath, 'r') as f:
        content = f.read()
    
    return content.count('@inproceedings')

def fetch_conference_citations(conf, batch_size=50):
    """Fetch citations for a conference in batches"""
    total = get_conference_paper_count(conf)
    if total == 0:
        print(f"Skipping {conf}: no papers found")
        return
    
    print(f"\n{'='*60}")
    print(f"Fetching citations for {conf.upper()} ({total} papers)")
    print(f"{'='*60}")
    
    # Fetch in batches
    fetched = 0
    batch_num = 0
    
    while fetched < total:
        batch_num += 1
        remaining = min(batch_size, total - fetched)
        
        print(f"\nBatch {batch_num}: Processing papers {fetched+1}-{fetched+remaining}")
        
        # Run the fetcher
        cmd = f"python3 scripts/fetch_citations.py {conf} --limit {remaining}"
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        
        if result.returncode != 0:
            print(f"Error in batch {batch_num}: {result.stderr}")
        else:
            # Parse output to see how many were successful
            output = result.stdout
            if "Completed:" in output:
                completed_line = [l for l in output.split('\n') if 'Completed:' in l]
                if completed_line:
                    print(f"  {completed_line[0].strip()}")
        
        fetched += remaining
        
        # Show current stats
        stats_result = subprocess.run(
            "python3 scripts/fetch_citations.py --stats", 
            shell=True, capture_output=True, text=True
        )
        
        if "total_papers:" in stats_result.stdout:
            for line in stats_result.stdout.split('\n'):
                if 'total_papers:' in line:
                    print(f"  Current total papers in cache: {line.split(':')[1].strip()}")
                    break
        
        # Sleep between batches to avoid rate limiting
        if fetched < total:
            print(f"  Sleeping 5 seconds before next batch...")
            time.sleep(5)
    
    print(f"\nCompleted {conf.upper()}")

def main():
    """Main function"""
    # Priority conferences - systems and networking first
    priority_conferences = [
        'sigcomm',  # ~1,700 papers
        'sosp',     # ~800 papers
        'osdi',     # ~800 papers
        'nsdi',     # ~700 papers
        'sigmod',   # ~1,000 papers
        'vldb',     # ~900 papers
    ]
    
    # Other major conferences
    other_conferences = [
        'pldi',     # ~750 papers
        'popl',     # ~500 papers
        'icml',     # ~1,200 papers
        'neurips',  # ~1,400 papers
        'ccs',      # ~2,700 papers
        'stoc',     # ~1,900 papers
        'podc',     # ~1,100 papers
        'eurosys',  # ~1,300 papers
        'asplos',   # ~1,100 papers
        'atc',      # ~700 papers
        'fast',     # ~400 papers
        'icde',     # ~1,400 papers
    ]
    
    start_time = datetime.now()
    
    print("=" * 60)
    print("CITATION FETCHING - FULL DATABASE")
    print(f"Started at: {start_time.strftime('%Y-%m-%d %H:%M:%S')}")
    print("=" * 60)
    
    # Process priority conferences first
    for conf in priority_conferences:
        fetch_conference_citations(conf, batch_size=50)
        
        # Show progress
        print("\n" + "-" * 40)
        subprocess.run("python3 scripts/fetch_citations.py --stats", shell=True)
        print("-" * 40)
        
        # Sleep between conferences
        time.sleep(10)
    
    # Check if we should continue with other conferences
    stats_result = subprocess.run(
        "python3 scripts/fetch_citations.py --stats", 
        shell=True, capture_output=True, text=True
    )
    
    total_papers = 0
    if "total_papers:" in stats_result.stdout:
        for line in stats_result.stdout.split('\n'):
            if 'total_papers:' in line:
                total_papers = int(line.split(':')[1].strip().replace(',', ''))
                break
    
    print(f"\n{'='*60}")
    print(f"Priority conferences completed!")
    print(f"Total papers fetched so far: {total_papers:,}")
    
    # Ask whether to continue
    print(f"\nContinue with remaining {len(other_conferences)} conferences? (y/n)")
    # For automation, we'll continue
    
    # Process other conferences
    for conf in other_conferences:
        fetch_conference_citations(conf, batch_size=30)  # Smaller batches
        time.sleep(10)
    
    # Final statistics
    end_time = datetime.now()
    duration = end_time - start_time
    
    print("\n" + "=" * 60)
    print("CITATION FETCHING COMPLETED")
    print(f"Duration: {duration}")
    print("=" * 60)
    
    # Show final stats
    subprocess.run("python3 scripts/fetch_citations.py --stats", shell=True)
    
    # Export the database
    print("\nExporting citation database...")
    subprocess.run("python3 scripts/build_citation_database.py --export-rust", shell=True)
    subprocess.run("python3 scripts/build_citation_database.py --export-graphml", shell=True)
    
    # Show highly cited papers
    print("\nTop cited papers (≥1000 citations):")
    subprocess.run("python3 scripts/build_citation_database.py --highly-cited 1000", shell=True)

if __name__ == "__main__":
    main()