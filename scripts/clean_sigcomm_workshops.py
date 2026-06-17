#!/usr/bin/env python3
"""
Clean SIGCOMM workshop papers while preserving valid entries
"""

import re
from pathlib import Path

def parse_bibtex_entries(content):
    """Parse BibTeX content into individual entries"""
    entries = []
    current_entry = []
    in_entry = False
    brace_count = 0
    
    for line in content.split('\n'):
        if line.strip().startswith('@'):
            if current_entry and brace_count == 0:
                entries.append('\n'.join(current_entry))
            current_entry = [line]
            in_entry = True
            brace_count = line.count('{') - line.count('}')
        elif in_entry:
            current_entry.append(line)
            brace_count += line.count('{') - line.count('}')
            if brace_count == 0 and line.strip().endswith('}'):
                entries.append('\n'.join(current_entry))
                current_entry = []
                in_entry = False
    
    if current_entry:
        entries.append('\n'.join(current_entry))
    
    return entries

def is_workshop_paper(entry):
    """Check if an entry is a workshop paper"""
    # Workshop indicators in titles
    workshop_titles = [
        'Proceedings of', 'Workshop on', 'Symposium on',
        'hyDNS: Acceleration of DNS Through Kernel',
        'Unsafe kernel extension composition via BPF',
        'Eliminating eBPF Tracing Overhead',
        'An Empirical Study on the Challenges of eBPF',
        'μBPF: Using eBPF for Microcontroller',
        'Understanding Performance of eBPF Maps',
        'Towards Functional Verification of eBPF Programs',
        'Unlocking Path Awareness for Legacy Applications',
        'Honey for the Ice Bear - Dynamic eBPF',
        'Custom Page Fault Handling With eBPF',
        'BOAD', 'Kgent: Kernel Extensions',
        'NetEdit: An Orchestration Platform for eBPF',
        'Design and Implementation of Tag-based Policies in eBPF',
        'Enabling eBPF on Embedded Systems Through Decoupled',
        'Enoki: High Velocity Linux Kernel Scheduler',
        'iproute2-sysrepo', 'LibPreemptible',
        'Network Monitoring as an eBPF Service',
        'Overlay network technologies for next-generation eBPF',
        'Scaling up eBPF processing using hardware-based',
        'SecBPF: Deploying eBPF/XDP Programs Safely',
        'XDP-Firewall using CRAB',
        'Fast In-Kernel Packet Classification with eBPF',
        'Kernel-Bypass Userspace Data Processing',
        'Emerging Multimedia Systems',
        'Hot Topics in Optical Technologies'
    ]
    
    for title in workshop_titles:
        if title in entry:
            return True
    
    # Check for specific DOI patterns that indicate workshops
    if 'doi = {10.1145/3672197' in entry:  # eBPF workshop DOI prefix
        return True
    
    return False

def clean_sigcomm():
    """Clean SIGCOMM file"""
    filepath = Path('bib/sigcomm.bib')
    
    # Read file
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # Parse entries
    entries = parse_bibtex_entries(content)
    
    # Filter out workshop papers
    kept_entries = []
    removed_count = 0
    
    for entry in entries:
        if entry.strip() and not entry.strip().startswith('%'):
            if is_workshop_paper(entry):
                # Extract title for logging
                title_match = re.search(r'title\s*=\s*\{([^}]+)\}', entry)
                if title_match:
                    title = title_match.group(1)[:60]
                    print(f'Removing workshop paper: {title}...')
                removed_count += 1
            else:
                kept_entries.append(entry)
    
    print(f'\nTotal removed: {removed_count} workshop papers')
    print(f'Total kept: {len(kept_entries)} main conference papers')
    
    # Write back
    with open(filepath, 'w', encoding='utf-8') as f:
        f.write('% SIGCOMM Main Conference Papers\n')
        f.write('% Workshop papers removed\n\n')
        
        for entry in kept_entries:
            f.write(entry)
            f.write('\n\n')
    
    return removed_count, len(kept_entries)

if __name__ == "__main__":
    removed, kept = clean_sigcomm()
    print(f"\nDone! File cleaned successfully.")