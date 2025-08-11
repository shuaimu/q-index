#!/usr/bin/env python3
import json
import requests

# Get all venues from the API
r = requests.get("http://127.0.0.1:8080/api/venues_all")
if r.status_code == 200:
    data = r.json()
    venues = data.get('data', [])
    print(f"Total venues loaded: {len(venues)}")
    print("\nAll venue names:")
    for v in sorted(venues, key=lambda x: x['name']):
        print(f"  {v['name']}: {v['paper_count']} papers")
else:
    print(f"Error: {r.status_code}")