#!/usr/bin/env python3
"""Match our papers to Semantic Scholar (S2AG) and record their citation counts.

Usage:
    ./target/release/qindex export-papers           # writes cache/citations/papers.jsonl
    python3 -I scripts/match_s2ag.py scan           # local S2AG papers files -> corpus ids
    python3 -I scripts/match_s2ag.py resolve        # S2 Graph API -> current citation counts

`scan` streams the downloaded S2AG papers dataset (data/s2ag/papers/*.gz) in
parallel and finds each paper by DOI, or by normalized title plus year (+-1)
for papers without a DOI (USENIX venues, NeurIPS, ICML...).

`resolve` looks up every paper through the Graph API batch endpoint -- by
CorpusId when the scan found it, otherwise by DOI -- so all counts come from
one snapshot. With S2_API_KEY set it also tries the title-match endpoint for
papers still unmatched (too rate-limited to be useful without a key).

The result, keyed by the paper ids `qindex` uses, is written to
cache/citations/s2ag_paper_citations.json; the site generator does exact
lookups in it. Intermediate results are cached in data/s2ag/work/ so reruns
only redo what's missing.
"""

import argparse
import datetime
import gzip
import html
import json
import os
import re
import ssl
import sys
import time
import unicodedata
import urllib.error
import urllib.parse
import urllib.request
from multiprocessing import Pool
from pathlib import Path

API = "https://api.semanticscholar.org/graph/v1"
API_FIELDS = "title,year,venue,citationCount,influentialCitationCount,externalIds"
BATCH_SIZE = 500
MIN_TITLE_WORDS = 3  # shorter titles ("Front Matter", "Keynote") only match by DOI


def norm_title(title: str) -> str:
    title = html.unescape(title)  # some BibTeX titles contain &apos; etc.
    title = unicodedata.normalize("NFKD", title)
    title = "".join(c for c in title if not unicodedata.combining(c))
    title = re.sub(r"\\[a-zA-Z]+", " ", title)  # LaTeX commands
    return re.sub(r"[^a-z0-9]+", " ", title.lower()).strip()


def norm_doi(doi):
    return doi.strip().lower() if doi else None


def load_papers(path):
    papers = [json.loads(line) for line in open(path, encoding="utf-8")]
    for p in papers:
        p["ntitle"] = norm_title(p["title"])
        p["ndoi"] = norm_doi(p["doi"])
    return papers


# --------------------------------------------------------------------------
# scan: local S2AG papers files

_dois = _titles = None


def _init_worker(dois, titles):
    global _dois, _titles
    _dois, _titles = dois, titles


def _scan_file(path):
    """Returns (doi_hits, title_hits, records, error): hits map our key ->
    candidates. A truncated download still yields the records before the cut."""
    doi_hits, title_hits, records, error = {}, {}, 0, None
    try:
        with gzip.open(path, "rt", encoding="utf-8") as f:
            for line in f:
                records += 1
                _match_line(line, doi_hits, title_hits)
    except (EOFError, OSError) as e:
        error = f"{path.name}: {e}"
    return doi_hits, title_hits, records, error


def _match_line(line, doi_hits, title_hits):
    try:
        r = json.loads(line)
    except ValueError:
        return
    ext = r.get("externalids") or {}
    doi = norm_doi(ext.get("DOI"))
    ntitle = norm_title(r.get("title") or "")
    doi_hit = bool(doi) and doi in _dois
    title_hit = ntitle in _titles
    if not (doi_hit or title_hit):
        return
    cand = {
        "corpus_id": int(r["corpusid"]),
        "title": r.get("title"),
        "year": r.get("year"),
        "venue": r.get("venue") or (r.get("journal") or {}).get("name") or "",
        "citations": r.get("citationcount") or 0,
    }
    if doi_hit:
        doi_hits.setdefault(doi, []).append(cand)
    if title_hit:
        title_hits.setdefault(ntitle, []).append(cand)


def venue_affinity(our_venue: str, s2_venue: str) -> bool:
    ours = norm_title(our_venue).replace("usenix ", "")
    return bool(ours) and ours in norm_title(s2_venue)


def pick(candidates, paper):
    """Best candidate: plausible year, then venue match, then most cited."""
    year = paper["year"]
    ok = [c for c in candidates if not (year and c["year"]) or abs(c["year"] - year) <= 1]
    if not ok:
        return None
    return max(ok, key=lambda c: (venue_affinity(paper["venue"], c["venue"]), c["citations"]))


def cmd_scan(args):
    papers = load_papers(args.papers)
    dois = {p["ndoi"] for p in papers if p["ndoi"]}
    titles = {p["ntitle"] for p in papers if len(p["ntitle"].split()) >= MIN_TITLE_WORDS}
    files = sorted(Path(args.s2ag_dir).glob("*.gz"))
    if not files:
        sys.exit(f"no *.gz files in {args.s2ag_dir}")
    print(f"Scanning {len(files)} S2AG files for {len(dois)} DOIs and {len(titles)} titles...")

    doi_hits, title_hits, records, errors = {}, {}, 0, []
    start = time.time()
    with Pool(min(len(files), os.cpu_count() or 1), _init_worker, (dois, titles)) as pool:
        for i, (d, t, n, err) in enumerate(pool.imap_unordered(_scan_file, files), 1):
            if err:
                errors.append(err)
                print(f"  warning: {err} (kept {n:,} records read before the error)", flush=True)
            for k, v in d.items():
                doi_hits.setdefault(k, []).extend(v)
            for k, v in t.items():
                title_hits.setdefault(k, []).extend(v)
            records += n
            print(f"  {i}/{len(files)} files, {records:,} records, {time.time() - start:.0f}s", flush=True)

    matches = {}
    for p in papers:
        cand, how = None, None
        if p["ndoi"] in doi_hits:
            cand, how = pick(doi_hits[p["ndoi"]], p) or doi_hits[p["ndoi"]][0], "local-doi"
        elif len(p["ntitle"].split()) >= MIN_TITLE_WORDS and p["ntitle"] in title_hits:
            cand, how = pick(title_hits[p["ntitle"]], p), "local-title"
        if cand:
            matches[p["id"]] = dict(cand, match=how)

    work = Path(args.work_dir)
    work.mkdir(parents=True, exist_ok=True)
    out = work / "scan_matches.json"
    out.write_text(
        json.dumps({"files": len(files), "records": records, "errors": errors, "matches": matches})
    )
    by = {}
    for m in matches.values():
        by[m["match"]] = by.get(m["match"], 0) + 1
    print(f"Matched {len(matches)}/{len(papers)} papers locally {by}; wrote {out}")


# --------------------------------------------------------------------------
# resolve: Graph API


def ssl_context():
    cafile = os.environ.get("SSL_CERT_FILE")
    if not cafile and not ssl.get_default_verify_paths().cafile:
        system = "/etc/ssl/certs/ca-certificates.crt"
        cafile = system if os.path.exists(system) else None
    return ssl.create_default_context(cafile=cafile)


class Api:
    def __init__(self):
        self.key = os.environ.get("S2_API_KEY")
        self.ctx = ssl_context()

    def request(self, url, body=None, tries=8):
        headers = {"Content-Type": "application/json"}
        if self.key:
            headers["x-api-key"] = self.key
        delay = 5
        for attempt in range(tries):
            req = urllib.request.Request(url, data=body, headers=headers)
            try:
                with urllib.request.urlopen(req, timeout=120, context=self.ctx) as r:
                    return json.load(r)
            except urllib.error.HTTPError as e:
                if e.code == 404:
                    return None
                if e.code not in (429, 500, 502, 503, 504) or attempt == tries - 1:
                    raise
            except urllib.error.URLError:
                if attempt == tries - 1:
                    raise
            time.sleep(delay)
            delay = min(delay * 2, 120)

    def batch(self, ids):
        body = json.dumps({"ids": ids}).encode()
        return self.request(f"{API}/paper/batch?fields={API_FIELDS}", body)

    def title_match(self, title):
        query = urllib.parse.urlencode({"query": title, "fields": API_FIELDS})
        data = self.request(f"{API}/paper/search/match?{query}")
        return (data or {}).get("data", [None])[0]


def titles_agree(ours: str, theirs: str) -> bool:
    a, b = set(norm_title(ours).split()), set(norm_title(theirs or "").split())
    return bool(a) and len(a & b) / len(a | b) >= 0.9


def cmd_resolve(args):
    papers = load_papers(args.papers)
    work = Path(args.work_dir)
    scan_file = work / "scan_matches.json"
    scan = json.loads(scan_file.read_text())["matches"] if scan_file.exists() else {}
    if not scan:
        print("warning: no local scan results; papers without a DOI can only be title-matched")

    cache_file = work / "api_cache.json"
    cache = json.loads(cache_file.read_text()) if cache_file.exists() else {}
    api = Api()

    # One query per paper: CorpusId from the scan, else DOI
    query = {}
    for p in papers:
        if p["id"] in scan:
            query[p["id"]] = f"CorpusId:{scan[p['id']]['corpus_id']}"
        elif p["ndoi"]:
            query[p["id"]] = f"DOI:{p['ndoi']}"
    todo = sorted({q for q in query.values() if q not in cache})
    print(f"Batch lookups: {len(query)} papers, {len(todo)} ids not cached")
    for i in range(0, len(todo), BATCH_SIZE):
        chunk = todo[i : i + BATCH_SIZE]
        for qid, result in zip(chunk, api.batch(chunk) or [None] * len(chunk)):
            cache[qid] = result
        cache_file.write_text(json.dumps(cache))
        print(f"  {min(i + BATCH_SIZE, len(todo))}/{len(todo)}", flush=True)
        time.sleep(1.5)

    out = {}
    for p in papers:
        r = cache.get(query.get(p["id"]))
        if r:
            how = scan[p["id"]]["match"] if p["id"] in scan else "doi"
            out[p["id"]] = record(r, how)

    # Title-match leftovers (only practical with an API key)
    left = [p for p in papers if p["id"] not in out and len(p["ntitle"].split()) >= MIN_TITLE_WORDS]
    if api.key and args.title_match:
        print(f"Title-matching {len(left)} remaining papers via the API...")
        for n, p in enumerate(left, 1):
            qid = f"title:{p['ntitle']}"
            if qid not in cache:
                cache[qid] = api.title_match(p["title"])
                if n % 50 == 0:
                    cache_file.write_text(json.dumps(cache))
                    print(f"  {n}/{len(left)}", flush=True)
                time.sleep(1.1)
            r = cache[qid]
            if r and titles_agree(p["title"], r.get("title")) and (
                not (p["year"] and r.get("year")) or abs(r["year"] - p["year"]) <= 1
            ):
                out[p["id"]] = record(r, "api-title")
        cache_file.write_text(json.dumps(cache))
    elif left:
        print(f"{len(left)} papers unmatched; set S2_API_KEY and pass --title-match to try the API")

    by = {}
    for r in out.values():
        by[r["match"]] = by.get(r["match"], 0) + 1
    Path(args.out).write_text(
        json.dumps(
            {
                "generated": datetime.date.today().isoformat(),
                "source": "Semantic Scholar Graph API citation counts; S2AG papers dataset for title matching",
                "matched": len(out),
                "total": len(papers),
                "by_method": by,
                "papers": dict(sorted(out.items())),
            },
            indent=0,
        )
    )
    print(f"Matched {len(out)}/{len(papers)} papers {by}; wrote {args.out}")


def record(r, how):
    return {
        "corpus_id": (r.get("externalIds") or {}).get("CorpusId"),
        "citations": r.get("citationCount") or 0,
        "influential": r.get("influentialCitationCount") or 0,
        "s2_title": r.get("title"),
        "s2_year": r.get("year"),
        "match": how,
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", choices=["scan", "resolve"])
    ap.add_argument("--papers", default="cache/citations/papers.jsonl")
    ap.add_argument("--s2ag-dir", default="data/s2ag/papers")
    ap.add_argument("--work-dir", default="data/s2ag/work")
    ap.add_argument("--out", default="cache/citations/s2ag_paper_citations.json")
    ap.add_argument("--title-match", action="store_true", help="also query the title-match API (needs S2_API_KEY)")
    args = ap.parse_args()
    {"scan": cmd_scan, "resolve": cmd_resolve}[args.command](args)


if __name__ == "__main__":
    main()
