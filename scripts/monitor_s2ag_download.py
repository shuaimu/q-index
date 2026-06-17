#!/usr/bin/env python3
"""
Monitor S2AG download progress
"""

import os
import time
from pathlib import Path
import subprocess

def get_folder_size(folder):
    """Get total size of files in folder"""
    total = 0
    for file in Path(folder).glob("*.gz"):
        total += file.stat().st_size
    return total

def format_size(bytes):
    """Format bytes to human readable"""
    for unit in ['B', 'KB', 'MB', 'GB', 'TB']:
        if bytes < 1024.0:
            return f"{bytes:.1f} {unit}"
        bytes /= 1024.0
    return f"{bytes:.1f} PB"

def main():
    papers_dir = Path("data/s2ag/papers")
    citations_dir = Path("data/s2ag/citations")
    
    print("=== S2AG Download Monitor ===")
    print("Press Ctrl+C to stop monitoring\n")
    
    while True:
        # Count files
        papers_count = len(list(papers_dir.glob("*.gz")))
        citations_count = len(list(citations_dir.glob("*.gz")))
        
        # Get sizes
        papers_size = get_folder_size(papers_dir)
        citations_size = get_folder_size(citations_dir)
        
        # Check if download processes are running
        try:
            result = subprocess.run(
                ["ps", "aux"], 
                capture_output=True, 
                text=True
            )
            download_running = "download_s2ag" in result.stdout
        except:
            download_running = False
        
        # Clear screen and show status
        os.system('clear' if os.name == 'posix' else 'cls')
        print("=== S2AG Download Progress ===")
        print(f"Time: {time.strftime('%Y-%m-%d %H:%M:%S')}")
        print(f"Status: {'⚡ DOWNLOADING' if download_running else '⏸  PAUSED/STOPPED'}")
        print()
        print(f"Papers Dataset:")
        print(f"  Files: {papers_count} / ~30 expected")
        print(f"  Size: {format_size(papers_size)} / ~45 GB expected")
        print(f"  Progress: {papers_count/30*100:.1f}%")
        print()
        print(f"Citations Dataset:")
        print(f"  Files: {citations_count} / ~236 files")
        print(f"  Size: {format_size(citations_size)} / ~255 GB expected")
        print(f"  Progress: {citations_count/236*100:.1f}%")
        print()
        print(f"Total Downloaded: {format_size(papers_size + citations_size)}")
        print(f"Expected Total: ~300 GB")
        
        # Check disk space
        import shutil
        stat = shutil.disk_usage("/")
        free_space = stat.free
        print()
        print(f"Disk Space:")
        print(f"  Free: {format_size(free_space)}")
        print(f"  Required: ~{format_size(300 * 1024**3 - papers_size - citations_size)}")
        
        if free_space < 50 * 1024**3:  # Less than 50GB
            print("  ⚠️  WARNING: Low disk space!")
        
        time.sleep(5)  # Update every 5 seconds

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n\nMonitoring stopped.")