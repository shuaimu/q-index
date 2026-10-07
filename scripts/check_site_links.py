#!/usr/bin/env python3
"""Check that every internal link in a generated QIndex site resolves.

Usage: python3 scripts/check_site_links.py <site-dir> <base-url>
   e.g. python3 scripts/check_site_links.py site /q-index/

Every href/src/action in every HTML file is resolved against the page's URL
(so relative links in the mdBook output are checked too). Internal links must
stay under <base-url> and point to an existing file (a trailing slash means
index.html). Links to the scholar page (scholar/?id=...) are also checked
against the scholar JSON shards, using the same FNV-1a sharding as
src/site/mod.rs and static/app.js.

Exits non-zero if anything is broken.
"""

import json
import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import parse_qs, unquote, urljoin, urlsplit

SCHOLAR_SHARDS = 256  # must match src/site/mod.rs
LINK_ATTRS = {"a": "href", "link": "href", "script": "src", "img": "src", "form": "action"}


def scholar_shard(scholar_id: str) -> int:
    h = 0x811C9DC5
    for b in scholar_id.encode("utf-8"):
        h ^= b
        h = (h * 0x01000193) & 0xFFFFFFFF
    return h % SCHOLAR_SHARDS


class LinkCollector(HTMLParser):
    def __init__(self):
        super().__init__()
        self.links = []

    def handle_starttag(self, tag, attrs):
        attr = LINK_ATTRS.get(tag)
        if attr:
            for name, value in attrs:
                if name == attr and value:
                    self.links.append(value)


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    site = Path(sys.argv[1])
    base = "/" + sys.argv[2].strip("/") + "/" if sys.argv[2].strip("/") else "/"
    origin = "http://site.invalid"

    shards = {}

    def scholar_exists(scholar_id: str) -> bool:
        n = scholar_shard(scholar_id)
        if n not in shards:
            shard_file = site / "data" / "scholars" / f"{n:02x}.json"
            shards[n] = json.loads(shard_file.read_text()) if shard_file.exists() else {}
        return scholar_id in shards[n]

    errors = []
    checked = 0
    pages = sorted(site.rglob("*.html"))
    for page in pages:
        rel = page.relative_to(site).as_posix()
        page_url = origin + base + rel
        collector = LinkCollector()
        collector.feed(page.read_text(encoding="utf-8"))
        for link in collector.links:
            if link.startswith(("#", "mailto:", "javascript:", "data:")):
                continue
            url = urlsplit(urljoin(page_url, link))
            if url.scheme + "://" + url.netloc != origin:
                continue  # external link
            checked += 1
            path = unquote(url.path)
            if not path.startswith(base):
                errors.append(f"{rel}: {link} -> outside base {base}")
                continue
            target = site / path[len(base):]
            if path.endswith("/"):
                target = target / "index.html"
            if not target.is_file():
                errors.append(f"{rel}: {link} -> missing {target.relative_to(site)}")
                continue
            if path == base + "scholar/":
                ids = parse_qs(url.query).get("id", [])
                if not ids or not scholar_exists(ids[0]):
                    errors.append(f"{rel}: {link} -> unknown scholar")

    print(f"Checked {checked} internal links in {len(pages)} HTML files")
    for error in errors[:50]:
        print("  BROKEN", error)
    if len(errors) > 50:
        print(f"  ... and {len(errors) - 50} more")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
