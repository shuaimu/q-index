import json

data = json.load(open('cache/citations/paper_cache.json'))
print(f'Papers in cache: {len(data)}')

failed = json.load(open('cache/citations/failed_lookups.json'))
print(f'Failed lookups: {len(failed)}')

print('\nRecent failures:')
recent = sorted(failed.items(), key=lambda x: x[1], reverse=True)[:10]
for key, timestamp in recent:
    print(f'  {key[:50]}...')

print(f'\nTotal citations: {sum(p.get("citation_count", 0) for p in data.values()):,}')