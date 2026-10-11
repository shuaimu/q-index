#!/usr/bin/env python3
"""Match bibliography papers and extract citation data entirely from an S2AG snapshot.

Run scan, resolve, then graph with --snapshot-dir data/s2ag/2026-10-06.
The snapshot root contains papers/*.gz and citations/*.gz; intermediate matching
results live in its work/ directory. No command contacts a network service.
"""

import argparse
import datetime
import gzip
import hashlib
import html
import json
import os
import re
import sys
import time
import unicodedata
from multiprocessing import Pool
from pathlib import Path

MIN_TITLE_WORDS = 3
SCAN_VERSION = 3
_dois = _dblps = _titles = _corpus_ids = None
_ENDPOINTS = tuple(
    re.compile(rb'(?:\{|,)\s*"' + field + rb'"\s*:\s*(\d+|null)\s*[,}]')
    for field in (b"citingcorpusid", b"citedcorpusid")
)


def norm_title(title):
    title = unicodedata.normalize("NFKD", html.unescape(title))
    title = "".join(c for c in title if not unicodedata.combining(c))
    title = re.sub(r"\\[a-zA-Z]+", " ", title)
    return re.sub(r"[^a-z0-9]+", " ", title.lower()).strip()


def norm_doi(doi):
    return doi.strip().lower() if doi else None


def load_papers(path):
    with open(path, encoding="utf-8") as stream:
        papers = [json.loads(line) for line in stream]
    for paper in papers:
        paper["ntitle"] = norm_title(paper["title"])
        paper["ndoi"] = norm_doi(paper.get("doi"))
    return papers


def bibliography_digest(path):
    with open(path, "rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def unsigned(value, field):
    if type(value) is not int or not 0 <= value <= 2**64 - 1:
        raise ValueError(f"{field} must be an unsigned integer, got {value!r}")
    return value


def shard_manifest(root, kind):
    files = sorted((root / kind).glob("*.gz"))
    if not files:
        raise ValueError(f"no *.gz files in {root / kind}")
    manifest = []
    for path in files:
        stat = path.stat()
        manifest.append({"name": path.name, "bytes": stat.st_size, "mtime_ns": stat.st_mtime_ns})
    return files, manifest


def write_json(path, data):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(json.dumps(data, indent=0, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(path)


def source(args, kind):
    return f"Semantic Scholar S2AG local {kind} snapshot {args.release_id}"


def _mapped_shards(worker, files, workers, initialize, parameters):
    workers = min(workers, len(files))
    if workers == 1:
        initialize(*parameters)
        yield from map(worker, files)
    else:
        with Pool(workers, initialize, parameters) as pool:
            yield from pool.imap_unordered(worker, files)


def _init_worker(dois, dblps, titles):
    global _dois, _dblps, _titles
    _dois, _dblps, _titles = dois, dblps, titles


def _match_line(line, doi_hits, dblp_hits, title_hits):
    row = json.loads(line)
    external_ids = row.get("externalids") or {}
    doi = norm_doi(external_ids.get("DOI"))
    dblp = external_ids.get("DBLP")
    title = norm_title(row.get("title") or "")
    if doi not in _dois and dblp not in _dblps and title not in _titles:
        return
    candidate = {
        "corpus_id": unsigned(row["corpusid"], "corpusid"),
        "title": row.get("title"),
        "dblp_id": dblp,
        "year": row.get("year"),
        "venue": row.get("venue") or (row.get("journal") or {}).get("name") or "",
        "citations": unsigned(row["citationcount"], "citationcount"),
    }
    if row.get("influentialcitationcount") is not None:
        candidate["influential"] = unsigned(row["influentialcitationcount"], "influentialcitationcount")
    if doi in _dois:
        doi_hits.setdefault(doi, []).append(candidate)
    if dblp in _dblps:
        dblp_hits.setdefault(dblp, []).append(candidate)
    if title in _titles:
        title_hits.setdefault(title, []).append(candidate)


def _scan_file(path):
    doi_hits, dblp_hits, title_hits, records = {}, {}, {}, 0
    try:
        with gzip.open(path, "rt", encoding="utf-8") as stream:
            for line in stream:
                records += 1
                _match_line(line, doi_hits, dblp_hits, title_hits)
    except (EOFError, OSError, ValueError, KeyError, TypeError) as error:
        return {}, {}, {}, records, f"{path.name}: {error}"
    return doi_hits, dblp_hits, title_hits, records, None


def venue_affinity(our_venue, s2_venue):
    ours = norm_title(our_venue).replace("usenix ", "")
    return bool(ours) and ours in norm_title(s2_venue)


def candidate_key(candidate, paper):
    # Lowest corpus ID breaks equal scores, independently of worker completion order.
    return (
        -int(venue_affinity(paper.get("venue") or "", candidate["venue"])),
        -candidate["citations"],
        candidate["corpus_id"],
        json.dumps(candidate, sort_keys=True),
    )


def pick(candidates, paper, exact_id=False):
    year = paper.get("year")
    plausible = [c for c in candidates if not (year and c["year"]) or abs(c["year"] - year) <= 1]
    # A different published DBLP record is a distinct version, not a title alias.
    # CoRR may represent the same work's preprint rather than another publication.
    dblp = paper.get("dblp_id")
    if not exact_id and dblp and not dblp.startswith("journals/corr/"):
        plausible = [
            c for c in plausible
            if not c.get("dblp_id") or c["dblp_id"] == dblp
            or c["dblp_id"].startswith("journals/corr/")
        ]
    # Exact DOI/DBLP identifiers remain authoritative when S2's year differs.
    eligible = plausible or (candidates if exact_id else [])
    return min(eligible, key=lambda c: candidate_key(c, paper)) if eligible else None


def cmd_scan(args):
    papers = load_papers(args.papers)
    dois = {p["ndoi"] for p in papers if p["ndoi"]}
    dblps = {p["dblp_id"] for p in papers if p.get("dblp_id")}
    titles = {p["ntitle"] for p in papers if len(p["ntitle"].split()) >= MIN_TITLE_WORDS}
    files, manifest = shard_manifest(args.snapshot_dir, "papers")
    metadata = {
        "version": SCAN_VERSION,
        "release_id": args.release_id,
        "snapshot_dir": str(args.snapshot_dir),
        "papers_sha256": bibliography_digest(args.papers),
        "shards": manifest,
        "files": len(files),
        "complete": False,
    }
    out = args.work_dir / "scan_matches.json"
    # Invalidate previous results before starting: a failed rescan is never reusable.
    write_json(out, metadata)
    doi_hits, dblp_hits, title_hits, records, errors = {}, {}, {}, 0, []
    start = time.monotonic()
    for index, (d, b, t, count, error) in enumerate(
        _mapped_shards(_scan_file, files, args.workers, _init_worker, (dois, dblps, titles)), 1
    ):
        records += count
        if error:
            errors.append(error)
        for key, values in d.items():
            doi_hits.setdefault(key, []).extend(values)
        for key, values in b.items():
            dblp_hits.setdefault(key, []).extend(values)
        for key, values in t.items():
            title_hits.setdefault(key, []).extend(values)
        print(f"  {index}/{len(files)} shards, {records:,} records, {time.monotonic() - start:.0f}s", flush=True)
    if errors:
        write_json(out, dict(metadata, records=records, errors=sorted(errors)))
        raise ValueError("incomplete papers scan: " + "; ".join(sorted(errors)))
    if shard_manifest(args.snapshot_dir, "papers")[1] != manifest:
        raise ValueError("papers shards changed during scan; rerun scan")
    if bibliography_digest(args.papers) != metadata["papers_sha256"]:
        raise ValueError("bibliography changed during scan; rerun scan")
    matches = {}
    for paper in papers:
        if paper["ndoi"] in doi_hits:
            candidate, method = pick(doi_hits[paper["ndoi"]], paper, exact_id=True), "local-doi"
        elif paper.get("dblp_id") in dblp_hits:
            candidate, method = pick(dblp_hits[paper["dblp_id"]], paper, exact_id=True), "local-dblp"
        else:
            candidate, method = pick(title_hits.get(paper["ntitle"], []), paper), "local-title"
        if candidate:
            matches[paper["id"]] = dict(candidate, match=method)
    write_json(out, dict(metadata, complete=True, records=records, errors=[], matches=matches))
    print(f"Matched {len(matches)}/{len(papers)} papers; wrote {out}")


def load_scan(args):
    path = args.work_dir / "scan_matches.json"
    scan = json.loads(path.read_text(encoding="utf-8"))
    if scan.get("version") != SCAN_VERSION or scan.get("complete") is not True or scan.get("errors"):
        raise ValueError("scan is incomplete or unsupported; rerun scan")
    if scan.get("release_id") != args.release_id or scan.get("snapshot_dir") != str(args.snapshot_dir):
        raise ValueError("scan belongs to another snapshot; rerun scan")
    if scan.get("papers_sha256") != bibliography_digest(args.papers):
        raise ValueError("bibliography differs from scan; rerun scan")
    if scan.get("shards") != shard_manifest(args.snapshot_dir, "papers")[1]:
        raise ValueError("papers shards differ from scan; rerun scan")
    return scan


def cmd_resolve(args):
    scan = load_scan(args)
    papers = load_papers(args.papers)
    output, by_method = {}, {}
    for paper in papers:
        match = scan["matches"].get(paper["id"])
        if match is None:
            continue
        record = {
            "corpus_id": unsigned(match["corpus_id"], "corpus_id"),
            "citations": unsigned(match["citations"], "citations"),
            "s2_title": match["title"],
            "s2_year": match["year"],
            "match": match["match"],
        }
        if "influential" in match:
            record["influential"] = unsigned(match["influential"], "influential")
        output[paper["id"]] = record
        by_method[record["match"]] = by_method.get(record["match"], 0) + 1
    write_json(args.out, {
        "release_id": args.release_id,
        "generated": datetime.date.today().isoformat(),
        "source": source(args, "papers"),
        "matched": len(output),
        "total": len(papers),
        "by_method": by_method,
        "papers": output,
    })
    print(f"Matched {len(output)}/{len(papers)} papers {by_method}; wrote {args.out}")


def _init_graph_worker(corpus_ids):
    global _corpus_ids
    _corpus_ids = corpus_ids


def _graph_file(path):
    edges, records = set(), 0
    try:
        with gzip.open(path, "rb") as stream:
            for line in stream:
                records += 1
                # Endpoint fields are flat; reject almost all rows without parsing
                # their potentially large citation contexts/intents JSON arrays.
                endpoints = []
                for pattern in _ENDPOINTS:
                    found = pattern.search(line)
                    if found is None:
                        raise ValueError(f"missing citation endpoint at record {records}")
                    raw = found.group(1)
                    endpoints.append(None if raw == b"null" else int(raw))
                citing, cited = endpoints
                if citing not in _corpus_ids or cited not in _corpus_ids or citing == cited:
                    continue
                row = json.loads(line)
                actual = (unsigned(row["citingcorpusid"], "citingcorpusid"),
                          unsigned(row["citedcorpusid"], "citedcorpusid"))
                if actual != (citing, cited):
                    raise ValueError(f"ambiguous citation endpoints at record {records}")
                edges.add(actual)
    except (EOFError, OSError, ValueError, KeyError, TypeError) as error:
        return set(), records, f"{path.name}: {error}"
    return edges, records, None


def cmd_graph(args):
    scan = load_scan(args)
    corpus_ids = {unsigned(m["corpus_id"], "corpus_id") for m in scan["matches"].values()}
    files, manifest = shard_manifest(args.snapshot_dir, "citations")
    edges, records, errors = set(), 0, []
    start = time.monotonic()
    for index, (pairs, count, error) in enumerate(
        _mapped_shards(_graph_file, files, args.workers, _init_graph_worker, (corpus_ids,)), 1
    ):
        records += count
        if error:
            errors.append(error)
        edges.update(pairs)
        print(f"  {index}/{len(files)} shards, {records:,} records, {len(edges):,} edges, {time.monotonic() - start:.0f}s", flush=True)
    if errors:
        raise ValueError("incomplete citations scan: " + "; ".join(sorted(errors)))
    if shard_manifest(args.snapshot_dir, "citations")[1] != manifest:
        raise ValueError("citations shards changed during scan; rerun graph")
    if load_scan(args) != scan:
        raise ValueError("paper matching changed during scan; rerun graph")
    write_json(args.graph_out, {
        "release_id": args.release_id,
        "generated": datetime.date.today().isoformat(),
        "source": source(args, "citations"),
        "records": records,
        "files": len(files),
        "shards": manifest,
        "edges": sorted(edges),
    })
    print(f"Extracted {len(edges)} unique internal citation edges; wrote {args.graph_out}")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=["scan", "resolve", "graph"])
    parser.add_argument("--snapshot-dir", required=True, type=Path)
    parser.add_argument("--papers", default="cache/citations/papers.jsonl")
    parser.add_argument("--work-dir", type=Path, help="default: SNAPSHOT_DIR/work")
    parser.add_argument("--workers", type=int, default=16)
    parser.add_argument("--out", default="cache/citations/s2ag_paper_citations.json")
    parser.add_argument("--graph-out", default="cache/citations/s2ag_citation_graph.json")
    args = parser.parse_args()
    if args.workers < 1:
        parser.error("--workers must be positive")
    args.workers = min(args.workers, os.cpu_count() or 1)
    args.snapshot_dir = args.snapshot_dir.resolve()
    args.release_id = args.snapshot_dir.name
    args.work_dir = args.work_dir or args.snapshot_dir / "work"
    try:
        {"scan": cmd_scan, "resolve": cmd_resolve, "graph": cmd_graph}[args.command](args)
    except (OSError, EOFError, ValueError, KeyError, TypeError) as error:
        sys.exit(f"{args.command} failed: {error}")


if __name__ == "__main__":
    main()
