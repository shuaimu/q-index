#!/usr/bin/env python3
import re

# Count entries in each file
import os
from pathlib import Path

bib_dir = Path("/Users/shuai/workspace/qindex/bib")

for bib_file in sorted(bib_dir.glob("*.bib")):
    if bib_file.name in ['title.bib', 'title_short.bib']:
        continue
    
    with open(bib_file, 'r', encoding='utf-8', errors='ignore') as f:
        content = f.read()
        # Count @inproceedings and @article entries
        entries = len(re.findall(r'@(?:inproceedings|article|techreport)', content, re.IGNORECASE))
        if entries > 0:
            print(f"{bib_file.name:20s}: {entries:4d} entries")