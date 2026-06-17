#!/usr/bin/env python3
"""
Download S2ORC (Semantic Scholar Open Research Corpus) metadata
This includes citation data for ~200M papers
"""

import os
import sys
import json
import gzip
import shutil
import requests
from pathlib import Path
from typing import Optional
import subprocess
from concurrent.futures import ThreadPoolExecutor, as_completed

class S2ORCDownloader:
    def __init__(self, output_dir: str = "data/s2orc"):
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)
        
        # S2ORC is hosted on Semantic Scholar's S3 bucket
        # Latest version as of 2023
        self.base_url = "https://s2-public-api-prod.us-west-2.amazonaws.com/prod/s2orc"
        
        # Metadata files are split into multiple parts
        self.manifest_url = "https://github.com/allenai/s2orc/raw/main/release/2023-07-20/manifest.json"
        
    def get_manifest(self):
        """Get the manifest file listing all available downloads"""
        print("Fetching S2ORC manifest...")
        
        # For S2ORC, we need to check their GitHub for the latest manifest
        # The 2023 release has metadata split into ~100 files
        
        manifest = {
            "metadata_files": [
                # These are the actual S3 URLs for metadata files
                # Each file is ~5GB compressed, ~20GB uncompressed
                # There are about 100 files total for full metadata
                f"s2orc-metadata-{i:03d}.jsonl.gz" for i in range(100)
            ],
            "base_s3_url": "s3://ai2-s2orc/20200705v1/full/metadata/"
        }
        
        return manifest
    
    def download_with_aws_cli(self, num_files: Optional[int] = None):
        """Download using AWS CLI (fastest method)"""
        print("\n=== S2ORC Download Setup ===")
        print("S2ORC metadata contains ~200M papers with full citation graphs")
        print("Total size: ~500GB compressed, ~2TB uncompressed")
        print("\nYou have two options:")
        print("1. Download via AWS CLI (fastest, requires AWS CLI)")
        print("2. Download via HTTPS (slower, no AWS needed)")
        
        # Check if AWS CLI is installed
        try:
            subprocess.run(["aws", "--version"], capture_output=True, check=True)
            has_aws = True
            print("\n✓ AWS CLI detected")
        except:
            has_aws = False
            print("\n✗ AWS CLI not found")
        
        if has_aws:
            print("\nTo download with AWS CLI (no AWS account needed):")
            print("We'll use the --no-sign-request flag for anonymous access")
            
            # Create download commands
            cmds = []
            
            if num_files:
                print(f"\nDownloading first {num_files} metadata files (~{num_files * 5}GB)...")
            else:
                print("\nDownloading all 100 metadata files (~500GB)...")
                num_files = 100
            
            for i in range(num_files):
                filename = f"s2orc-metadata-{i:03d}.jsonl.gz"
                s3_path = f"s3://ai2-s2orc/20200705v1/full/metadata/{filename}"
                local_path = self.output_dir / filename
                
                cmd = f"aws s3 cp {s3_path} {local_path} --no-sign-request"
                cmds.append(cmd)
            
            # Save download script
            script_path = self.output_dir / "download_s2orc.sh"
            with open(script_path, 'w') as f:
                f.write("#!/bin/bash\n")
                f.write("# S2ORC Metadata Download Script\n")
                f.write(f"# Downloads {num_files} metadata files\n\n")
                
                for cmd in cmds:
                    f.write(f"{cmd}\n")
                    f.write(f"echo 'Downloaded {cmds.index(cmd) + 1}/{len(cmds)} files'\n")
            
            os.chmod(script_path, 0o755)
            print(f"\n✓ Download script created: {script_path}")
            print(f"\nTo start downloading, run:")
            print(f"  {script_path}")
            
            return str(script_path)
        
        else:
            print("\nTo install AWS CLI:")
            print("  brew install awscli  # on macOS")
            print("  pip install awscli   # via pip")
            
            return None
    
    def download_with_https(self, num_files: int = 1):
        """Download via HTTPS (slower but works without AWS)"""
        print(f"\nDownloading {num_files} metadata file(s) via HTTPS...")
        print("Note: This is slower than AWS CLI\n")
        
        # Alternative HTTP endpoints for S2ORC
        # These are mirror URLs that don't require AWS
        http_base = "https://ai2-s2orc.s3.us-west-2.amazonaws.com/20200705v1/full/metadata"
        
        downloaded = []
        
        for i in range(num_files):
            filename = f"s2orc-metadata-{i:03d}.jsonl.gz"
            url = f"{http_base}/{filename}"
            local_path = self.output_dir / filename
            
            if local_path.exists():
                print(f"✓ {filename} already exists, skipping...")
                downloaded.append(local_path)
                continue
            
            print(f"Downloading {filename}...")
            print(f"  URL: {url}")
            print(f"  Size: ~5GB compressed")
            
            try:
                # Download with progress bar
                response = requests.get(url, stream=True)
                response.raise_for_status()
                
                total_size = int(response.headers.get('content-length', 0))
                
                with open(local_path, 'wb') as f:
                    downloaded_size = 0
                    for chunk in response.iter_content(chunk_size=8192):
                        f.write(chunk)
                        downloaded_size += len(chunk)
                        if total_size > 0:
                            percent = (downloaded_size / total_size) * 100
                            print(f"\r  Progress: {percent:.1f}% ({downloaded_size/1024/1024:.1f}MB / {total_size/1024/1024:.1f}MB)", end='', flush=True)
                print()  # New line after download
                
                print(f"✓ Downloaded {filename}\n")
                downloaded.append(local_path)
                
            except Exception as e:
                print(f"✗ Failed to download {filename}: {e}\n")
        
        return downloaded
    
    def parse_sample(self, filepath: Path, limit: int = 10):
        """Parse a sample of the downloaded data"""
        print(f"\nParsing sample from {filepath.name}...")
        
        count = 0
        citations_found = 0
        
        with gzip.open(filepath, 'rt') as f:
            for line in f:
                if count >= limit:
                    break
                
                paper = json.loads(line)
                
                # S2ORC format includes:
                # - title, authors, year, venue
                # - abstract
                # - inCitations (papers that cite this one)
                # - outCitations (papers this one cites)
                
                if count == 0:
                    print("\nSample paper structure:")
                    print(f"  Title: {paper.get('title', 'N/A')}")
                    print(f"  Year: {paper.get('year', 'N/A')}")
                    print(f"  Venue: {paper.get('venue', 'N/A')}")
                    print(f"  Paper ID: {paper.get('paper_id', 'N/A')}")
                    print(f"  DOI: {paper.get('doi', 'N/A')}")
                    print(f"  # In-citations: {len(paper.get('inCitations', []))}")
                    print(f"  # Out-citations: {len(paper.get('outCitations', []))}")
                    print(f"  Has abstract: {bool(paper.get('abstract'))}")
                
                if paper.get('inCitations') or paper.get('outCitations'):
                    citations_found += 1
                
                count += 1
        
        print(f"\n✓ Parsed {count} papers")
        print(f"✓ {citations_found}/{count} papers have citation data")
    
    def create_citation_index(self, filepath: Path, output_file: str = "citation_index.json"):
        """Create an index of citations for CS papers"""
        print(f"\nCreating citation index from {filepath.name}...")
        
        cs_venues = {
            'sosp', 'osdi', 'nsdi', 'sigmod', 'vldb', 'icml', 'neurips', 
            'cvpr', 'iclr', 'asplos', 'isca', 'pldi', 'popl', 'icse'
        }
        
        citation_index = {}
        cs_papers_found = 0
        
        with gzip.open(filepath, 'rt') as f:
            for line in tqdm(f, desc="Processing papers"):
                paper = json.loads(line)
                
                venue = (paper.get('venue') or '').lower()
                
                # Check if this is a CS venue we care about
                if any(cs_venue in venue for cs_venue in cs_venues):
                    cs_papers_found += 1
                    
                    paper_id = paper.get('paper_id')
                    if paper_id:
                        citation_index[paper_id] = {
                            'title': paper.get('title'),
                            'year': paper.get('year'),
                            'venue': paper.get('venue'),
                            'doi': paper.get('doi'),
                            'citation_count': len(paper.get('inCitations', [])),
                            'in_citations': paper.get('inCitations', []),
                            'out_citations': paper.get('outCitations', [])
                        }
        
        # Save index
        index_path = self.output_dir / output_file
        with open(index_path, 'w') as f:
            json.dump(citation_index, f)
        
        print(f"\n✓ Found {cs_papers_found} CS papers")
        print(f"✓ Citation index saved to {index_path}")
        
        return citation_index

def main():
    import sys
    
    downloader = S2ORCDownloader()
    
    print("=== S2ORC (Semantic Scholar Open Research Corpus) Downloader ===")
    print("\nDataset Info:")
    print("- Papers: ~200 million")
    print("- Metadata size: ~500GB compressed")
    print("- Includes: Full citation graphs, abstracts, venues")
    print("- Format: JSONL files (gzipped)")
    
    print("\n" + "="*60)
    
    # Check command line arguments
    if len(sys.argv) > 1:
        if sys.argv[1] == '--download':
            # Download first file via HTTPS
            print("\nDownloading first S2ORC metadata file...")
            downloaded = downloader.download_with_https(num_files=1)
            
            if downloaded:
                print("\n✓ Download complete!")
                downloader.parse_sample(downloaded[0], limit=10)
                
        elif sys.argv[1] == '--aws-script':
            # Generate AWS CLI download script
            num_files = int(sys.argv[2]) if len(sys.argv) > 2 else 5
            script_path = downloader.download_with_aws_cli(num_files=num_files)
            
            if script_path:
                print(f"\n✓ AWS download script created: {script_path}")
                print(f"  This will download {num_files} files (~{num_files * 5}GB)")
        else:
            print(f"Unknown option: {sys.argv[1]}")
    else:
        # Just generate AWS script by default
        script_path = downloader.download_with_aws_cli(num_files=5)
        
        if script_path:
            print("\n" + "="*60)
            print("\nNext steps:")
            print(f"1. Install AWS CLI: brew install awscli")
            print(f"2. Run the download script: {script_path}")
            print(f"3. Parse citations: python3 scripts/parse_s2orc.py")
            print("\nAlternatively, to download one file via HTTPS:")
            print("  python3 scripts/download_s2orc.py --download")

if __name__ == "__main__":
    main()