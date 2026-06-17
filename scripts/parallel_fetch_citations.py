#!/usr/bin/env python3
"""
Parallel citation fetching for all conferences.
Processes multiple conferences in parallel with proper rate limiting.
"""

import os
import sys
import time
import json
import subprocess
from pathlib import Path
from datetime import datetime
import threading
from queue import Queue

# Add project root to path
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

def fetch_conference_citations(conf_name, bib_file, log_dir):
    """Fetch citations for a single conference."""
    log_file = log_dir / f"fetch_{conf_name}.log"
    
    cmd = [
        sys.executable,
        "scripts/fetch_citations.py",
        str(bib_file),
        "--log", str(log_file)
    ]
    
    print(f"[{datetime.now().strftime('%H:%M:%S')}] Starting {conf_name}...")
    
    try:
        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode == 0:
            print(f"[{datetime.now().strftime('%H:%M:%S')}] ✓ Completed {conf_name}")
        else:
            print(f"[{datetime.now().strftime('%H:%M:%S')}] ✗ Failed {conf_name}: {result.stderr[:200]}")
    except Exception as e:
        print(f"[{datetime.now().strftime('%H:%M:%S')}] ✗ Error with {conf_name}: {e}")

def worker(queue, log_dir):
    """Worker thread to process conferences from queue."""
    while True:
        item = queue.get()
        if item is None:
            break
        conf_name, bib_file = item
        fetch_conference_citations(conf_name, bib_file, log_dir)
        queue.task_done()
        # Add delay between conferences to avoid overwhelming APIs
        time.sleep(2)

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
    
    # Priority order for conferences (process important ones first)
    priority_order = [
        'sigcomm', 'sosp', 'osdi', 'nsdi', 'sigmod', 'vldb',
        'pldi', 'popl', 'icml', 'neurips', 'iclr', 'cvpr',
        'ccs', 'oakland', 'ndss', 'stoc', 'focs', 'podc',
        'eurosys', 'asplos', 'atc', 'fast', 'icde', 'cidr',
        'oopsla', 'ecoop', 'isca', 'micro', 'hpca', 'sc',
        'imc', 'conext', 'mobicom', 'mobisys', 'sensys',
        'kdd', 'www', 'icse', 'fse', 'ase', 'issta'
    ]
    
    # Sort BibTeX files by priority
    def get_priority(bib_file):
        name = bib_file.stem.lower()
        try:
            return priority_order.index(name)
        except ValueError:
            return len(priority_order)
    
    sorted_bib_files = sorted(all_bib_files, key=get_priority)
    
    # Check current progress
    if cache_dir.exists() and (cache_dir / "paper_cache.json").exists():
        with open(cache_dir / "paper_cache.json", 'r') as f:
            cache = json.load(f)
        print(f"Starting with {len(cache)} papers already in cache")
    else:
        print("Starting fresh - no existing cache found")
    
    # Create work queue
    work_queue = Queue()
    
    # Add all conferences to queue
    conferences = []
    for bib_file in sorted_bib_files:
        conf_name = bib_file.stem
        conferences.append((conf_name, bib_file))
        work_queue.put((conf_name, bib_file))
    
    print(f"Processing {len(conferences)} conferences...")
    print(f"Order: {', '.join(c[0] for c in conferences[:10])}...")
    
    # Create worker threads (limit to 3 parallel to avoid rate limiting)
    num_workers = 3
    threads = []
    for i in range(num_workers):
        t = threading.Thread(target=worker, args=(work_queue, log_dir))
        t.start()
        threads.append(t)
    
    # Wait for all work to complete
    work_queue.join()
    
    # Stop workers
    for i in range(num_workers):
        work_queue.put(None)
    for t in threads:
        t.join()
    
    # Final statistics
    if (cache_dir / "paper_cache.json").exists():
        with open(cache_dir / "paper_cache.json", 'r') as f:
            final_cache = json.load(f)
        
        total_papers = len(final_cache)
        total_citations = sum(p.get("citation_count", 0) for p in final_cache.values())
        highly_cited = len([p for p in final_cache.values() if p.get("citation_count", 0) >= 100])
        
        print("\n" + "="*60)
        print("FINAL STATISTICS:")
        print(f"  Total papers fetched: {total_papers:,}")
        print(f"  Total citations: {total_citations:,}")
        print(f"  Papers with 100+ citations: {highly_cited}")
        print("="*60)
        
        # Build and export the database
        print("\nBuilding citation database...")
        subprocess.run([sys.executable, "scripts/build_citation_database.py"])

if __name__ == "__main__":
    main()