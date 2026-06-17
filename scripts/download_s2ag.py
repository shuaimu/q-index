#!/usr/bin/env python3
"""
Download Semantic Scholar Academic Graph (S2AG) datasets
Requires API key from https://www.semanticscholar.org/product/api#Partner-Form
"""

import os
import sys
import json
import time
import urllib.request
import urllib.error
from pathlib import Path
from typing import Optional, List

class S2AGDownloader:
    def __init__(self, api_key: str, output_dir: str = "data/s2ag"):
        self.api_key = api_key
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)
        
        # S2AG API endpoint
        self.base_url = "https://api.semanticscholar.org/datasets/v1"
        
        # Get latest release
        self.release_id = self.get_latest_release()
        print(f"Using S2AG release: {self.release_id}")
    
    def get_latest_release(self) -> str:
        """Get the latest release ID"""
        url = f"{self.base_url}/release/latest"
        req = urllib.request.Request(url)
        
        try:
            with urllib.request.urlopen(req) as response:
                data = json.loads(response.read())
                return data.get('release_id', '2025-08-05')
        except Exception as e:
            print(f"Error getting latest release: {e}")
            return '2025-08-05'  # Fallback to known release
    
    def get_dataset_info(self, dataset_name: str) -> dict:
        """Get information about a specific dataset"""
        url = f"{self.base_url}/release/{self.release_id}"
        req = urllib.request.Request(url)
        
        try:
            with urllib.request.urlopen(req) as response:
                data = json.loads(response.read())
                for dataset in data.get('datasets', []):
                    if dataset['name'] == dataset_name:
                        return dataset
        except Exception as e:
            print(f"Error getting dataset info: {e}")
        
        return {}
    
    def download_dataset(self, dataset_name: str, max_files: Optional[int] = None):
        """Download a specific dataset"""
        
        dataset_dir = self.output_dir / dataset_name
        dataset_dir.mkdir(exist_ok=True)
        
        print(f"\n{'='*60}")
        print(f"Downloading {dataset_name} dataset")
        print(f"Release: {self.release_id}")
        print(f"Output directory: {dataset_dir}")
        print(f"{'='*60}\n")
        
        # Get dataset info
        info = self.get_dataset_info(dataset_name)
        if info:
            print(f"Description: {info.get('description', 'N/A')}")
            print()
        
        # Get list of files for this dataset
        files_url = f"{self.base_url}/release/{self.release_id}/dataset/{dataset_name}"
        
        req = urllib.request.Request(files_url)
        req.add_header('x-api-key', self.api_key)
        
        try:
            with urllib.request.urlopen(req) as response:
                response_data = response.read()
                
                # Check if response is JSON array or object
                try:
                    files_data = json.loads(response_data)
                    if isinstance(files_data, list):
                        files = files_data
                    else:
                        files = files_data.get('files', [])
                except json.JSONDecodeError:
                    # Response might be a list of URLs directly
                    files = response_data.decode().strip().split('\n')
                
                if not files:
                    print(f"No files found for dataset {dataset_name}")
                    return
                
                print(f"Found {len(files)} files to download")
                
                if max_files:
                    files = files[:max_files]
                    print(f"Limiting to first {max_files} files")
                
                print()
                
                # Download each file
                for i, file_info in enumerate(files, 1):
                    # Handle different response formats
                    if isinstance(file_info, str):
                        # Direct URL string
                        file_url = file_info
                        # Extract filename from URL (before query parameters)
                        url_path = file_url.split('?')[0] if '?' in file_url else file_url
                        filename = url_path.split('/')[-1] if '/' in url_path else f'file_{i}.jsonl.gz'
                        # Ensure it has proper extension
                        if not filename.endswith('.gz'):
                            filename = f"papers_{i:03d}.jsonl.gz"
                    elif isinstance(file_info, dict):
                        filename = file_info.get('filename', f'file_{i}.jsonl.gz')
                        file_url = file_info.get('url')
                    else:
                        print(f"Skipping unknown file format: {file_info}")
                        continue
                    
                    if not file_url:
                        print(f"Skipping file {filename}: No URL provided")
                        continue
                    
                    output_path = dataset_dir / filename
                    
                    # Check if already downloaded
                    if output_path.exists():
                        size_mb = output_path.stat().st_size / (1024 * 1024)
                        print(f"[{i}/{len(files)}] {filename} already exists ({size_mb:.1f} MB)")
                        continue
                    
                    print(f"[{i}/{len(files)}] Downloading {filename}...")
                    
                    # Download with API key
                    file_req = urllib.request.Request(file_url)
                    file_req.add_header('x-api-key', self.api_key)
                    
                    try:
                        with urllib.request.urlopen(file_req) as file_response:
                            # Get file size if available
                            total_size = int(file_response.headers.get('Content-Length', 0))
                            
                            # Download with progress
                            downloaded = 0
                            chunk_size = 8192 * 100  # 800KB chunks
                            
                            with open(output_path, 'wb') as f:
                                while True:
                                    chunk = file_response.read(chunk_size)
                                    if not chunk:
                                        break
                                    
                                    f.write(chunk)
                                    downloaded += len(chunk)
                                    
                                    # Show progress
                                    if total_size > 0:
                                        percent = (downloaded / total_size) * 100
                                        size_mb = downloaded / (1024 * 1024)
                                        total_mb = total_size / (1024 * 1024)
                                        print(f"\r  Progress: {percent:.1f}% ({size_mb:.1f}/{total_mb:.1f} MB)", 
                                              end='', flush=True)
                            
                            print()  # New line after progress
                            
                            final_size_mb = output_path.stat().st_size / (1024 * 1024)
                            print(f"  ✓ Downloaded {final_size_mb:.1f} MB")
                            
                    except urllib.error.HTTPError as e:
                        print(f"  ✗ HTTP Error {e.code}: {e.reason}")
                        if e.code == 403:
                            print("  API key may be invalid or expired")
                            return
                    except Exception as e:
                        print(f"  ✗ Error downloading file: {e}")
                    
                    # Small delay between downloads to be polite
                    time.sleep(0.5)
                
                print(f"\n✓ Completed downloading {dataset_name}")
                
        except urllib.error.HTTPError as e:
            print(f"HTTP Error {e.code}: {e.reason}")
            if e.code == 403:
                print("API key may be invalid. Please check your API key.")
        except Exception as e:
            print(f"Error listing dataset files: {e}")
    
    def download_papers_and_citations(self, max_files: Optional[int] = None):
        """Download both papers and citations datasets"""
        
        print("="*60)
        print("S2AG Dataset Downloader")
        print("="*60)
        
        # Download papers dataset (smaller, ~45GB total)
        print("\n1. Papers dataset (metadata for 200M papers)")
        print("   Size: ~1.5GB per file × 30 files = ~45GB total")
        
        choice = input("\nDownload papers dataset? (y/n): ")
        if choice.lower() == 'y':
            self.download_dataset('papers', max_files)
        
        # Download citations dataset (larger, ~255GB total)
        print("\n2. Citations dataset (2.4B citation relationships)")
        print("   Size: ~8.5GB per file × 30 files = ~255GB total")
        
        choice = input("\nDownload citations dataset? (y/n): ")
        if choice.lower() == 'y':
            self.download_dataset('citations', max_files)
    
    def test_api_key(self):
        """Test if the API key is valid"""
        print("Testing API key...")
        
        url = f"{self.base_url}/release/{self.release_id}/dataset/papers"
        req = urllib.request.Request(url)
        req.add_header('x-api-key', self.api_key)
        
        try:
            with urllib.request.urlopen(req) as response:
                data = json.loads(response.read())
                if 'files' in data:
                    print("✓ API key is valid")
                    return True
        except urllib.error.HTTPError as e:
            if e.code == 403:
                print("✗ API key is invalid or expired")
            else:
                print(f"✗ HTTP Error {e.code}: {e.reason}")
        except Exception as e:
            print(f"✗ Error testing API key: {e}")
        
        return False

def main():
    import argparse
    
    parser = argparse.ArgumentParser(description='Download S2AG datasets')
    parser.add_argument('--api-key', help='Semantic Scholar API key (or set S2_API_KEY env var)')
    parser.add_argument('--dataset', choices=['papers', 'citations', 'both'], 
                       default='both', help='Which dataset to download')
    parser.add_argument('--max-files', type=int, help='Maximum number of files to download per dataset')
    parser.add_argument('--output-dir', default='data/s2ag', help='Output directory')
    parser.add_argument('--test', action='store_true', help='Test API key only')
    
    args = parser.parse_args()
    
    # Get API key from args or environment
    api_key = args.api_key or os.environ.get('S2_API_KEY')
    
    if not api_key:
        print("Error: API key required")
        print("Please provide via --api-key or set S2_API_KEY environment variable")
        print("\nGet your API key at: https://www.semanticscholar.org/product/api#Partner-Form")
        sys.exit(1)
    
    downloader = S2AGDownloader(api_key, args.output_dir)
    
    if args.test:
        downloader.test_api_key()
        return
    
    # Test API key first
    if not downloader.test_api_key():
        print("\nPlease check your API key and try again")
        sys.exit(1)
    
    print()
    
    # Download requested datasets
    if args.dataset == 'papers':
        downloader.download_dataset('papers', args.max_files)
    elif args.dataset == 'citations':
        downloader.download_dataset('citations', args.max_files)
    else:
        downloader.download_papers_and_citations(args.max_files)
    
    print("\n" + "="*60)
    print("Download complete!")
    print(f"Data saved to: {downloader.output_dir}")
    print("="*60)

if __name__ == "__main__":
    main()