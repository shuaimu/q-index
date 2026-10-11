"""Local S2AG behavior regressions using only temporary, synthetic gzip shards."""

import gzip
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "match_s2ag.py"
SPEC = importlib.util.spec_from_file_location("match_s2ag", SCRIPT)
pipeline = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(pipeline)


class SnapshotMatchingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "2026-10-06"
        (self.root / "papers").mkdir(parents=True)
        (self.root / "citations").mkdir()
        self.papers = self.root / "bibliography.jsonl"
        self.counts = self.root / "counts.json"
        self.graph = self.root / "graph.json"

    def write_bibliography(self, papers):
        self.papers.write_text("".join(json.dumps(p) + "\n" for p in papers), encoding="utf-8")

    def write_shard(self, kind, name, rows):
        path = self.root / kind / name
        with gzip.open(path, "wt", encoding="utf-8") as stream:
            for row in rows:
                stream.write(json.dumps(row) + "\n")
        return path

    def invoke(self, command, success=True, extra=()):
        result = subprocess.run([
            sys.executable, "-I", str(SCRIPT), command,
            "--snapshot-dir", str(self.root), "--workers", "1",
            "--papers", str(self.papers), "--out", str(self.counts),
            "--graph-out", str(self.graph), *extra,
        ], capture_output=True, text=True, timeout=30)
        if success:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        return result

    @staticmethod
    def paper(identifier, doi=None, title="Reliable Distributed Storage Systems", year=2020):
        return {"id": identifier, "doi": doi, "title": title, "year": year, "venue": "OSDI"}

    @staticmethod
    def record(corpus_id, citations=7, doi=None, title="Reliable Distributed Storage Systems", year=2020):
        return {"corpusid": corpus_id, "externalids": {"DOI": doi}, "title": title,
                "year": year, "venue": "OSDI", "citationcount": citations,
                "influentialcitationcount": 2}

    def test_doi_precedence_and_deterministic_title_ambiguity(self):
        self.write_bibliography([self.paper("doi", "10.1/exact"), self.paper("title"),
                                 self.paper("wrong-year", title="Another Distinct Research Title")])
        self.write_shard("papers", "a.gz", [
            self.record(30, citations=500),
            self.record(20, doi="10.1/EXACT", title="A Different DOI Title", year=2010),
            self.record(41, title="Another Distinct Research Title", year=2010),
        ])
        self.write_shard("papers", "b.gz", [self.record(10, citations=500)])
        self.invoke("scan")
        self.invoke("resolve")
        data = json.loads(self.counts.read_text())
        self.assertEqual(data["release_id"], "2026-10-06")
        self.assertEqual(data["papers"]["doi"]["corpus_id"], 20)
        self.assertEqual(data["papers"]["doi"]["match"], "local-doi")
        self.assertEqual(data["papers"]["doi"]["citations"], 7)
        self.assertEqual(data["papers"]["doi"]["influential"], 2)
        self.assertEqual(data["papers"]["title"]["corpus_id"], 10)
        self.assertNotIn("wrong-year", data["papers"])
        candidates = [dict(corpus_id=i, title="Same Title", year=2020, venue="OSDI", citations=7)
                      for i in (2, 1)]
        self.assertEqual(pipeline.pick(candidates, self.paper("x")),
                         pipeline.pick(list(reversed(candidates)), self.paper("x")))

    def test_dblp_fallback_is_exact_and_doi_has_precedence(self):
        identifier = "journals/example/Research20"
        self.write_bibliography([
            dict(self.paper("dblp"), dblp_id=identifier),
            dict(self.paper("doi", "10.1/exact"), dblp_id=identifier),
            dict(self.paper("near-miss"), dblp_id=identifier + "x"),
        ])
        self.write_shard("papers", "a.gz", [
            dict(self.record(21, citations=17, title="A defective catalogue title", year=2010),
                 externalids={"DOI": None, "DBLP": identifier}),
            self.record(31, citations=9, doi="10.1/EXACT",
                        title="Another defective catalogue title", year=2010),
        ])
        self.invoke("scan")
        self.invoke("resolve")
        papers = json.loads(self.counts.read_text())["papers"]
        self.assertIn("dblp", papers)
        self.assertEqual(papers["dblp"]["corpus_id"], 21)
        self.assertEqual(papers["dblp"]["citations"], 17)
        self.assertEqual(papers["dblp"]["match"], "local-dblp")
        self.assertEqual(papers["doi"]["corpus_id"], 31)
        self.assertEqual(papers["doi"]["match"], "local-doi")
        self.assertNotIn("near-miss", papers)

    def test_title_fallback_rejects_conflicting_published_version(self):
        conference = "conf/usenix/Research17"
        journal = "journals/tos/Research21"
        self.write_bibliography([
            dict(self.paper("conference", year=2017), dblp_id=conference),
            dict(self.paper("preprint", title="Another Reliable Research System"),
                 dblp_id="conf/osdi/Another20"),
        ])
        self.write_shard("papers", "a.gz", [
            dict(self.record(21, citations=100, year=2017),
                 externalids={"DBLP": journal}),
            dict(self.record(31, citations=9, title="Another Reliable Research System"),
                 externalids={"DBLP": "journals/corr/abs-2001-12345"}),
        ])
        self.invoke("scan")
        self.invoke("resolve")
        papers = json.loads(self.counts.read_text())["papers"]
        self.assertNotIn("conference", papers)
        self.assertEqual(papers["preprint"]["corpus_id"], 31)
        self.assertEqual(papers["preprint"]["citations"], 9)

    def test_graph_deduplicates_aliases_rows_and_shards(self):
        self.write_bibliography([self.paper("a", "10.1/a"), self.paper("alias", "10.1/a"),
                                 self.paper("b", "10.1/b")])
        self.write_shard("papers", "a.gz", [self.record(1, doi="10.1/a"),
                                             self.record(2, doi="10.1/b")])
        edge = {"citingcorpusid": 1, "citedcorpusid": 2, "contexts": ["Long citation context"]}
        self.write_shard("citations", "a.gz", [edge, edge,
            {"citingcorpusid": 2, "citedcorpusid": 1},
            {"citingcorpusid": 1, "citedcorpusid": 1},
            {"citingcorpusid": 999, "citedcorpusid": 1},
            {"citingcorpusid": 2, "citedcorpusid": 999},
            {"citingcorpusid": None, "citedcorpusid": 1}])
        self.write_shard("citations", "b.gz", [edge])
        self.invoke("scan")
        self.invoke("graph")
        graph = json.loads(self.graph.read_text())
        self.assertEqual(graph["edges"], [[1, 2], [2, 1]])
        self.assertEqual(graph["records"], 8)
        self.assertEqual(graph["release_id"], "2026-10-06")

    def test_failed_rescan_invalidates_previous_complete_scan(self):
        self.write_bibliography([self.paper("a", "10.1/a")])
        shard = self.write_shard("papers", "a.gz", [self.record(1, doi="10.1/a")])
        self.invoke("scan")
        shard.write_bytes(shard.read_bytes()[:-8])
        self.invoke("scan", success=False)
        scan = json.loads((self.root / "work" / "scan_matches.json").read_text())
        self.assertFalse(scan["complete"])
        self.invoke("resolve", success=False)
        self.assertFalse(self.counts.exists())

    def test_partial_citations_do_not_publish_graph(self):
        self.write_bibliography([self.paper("a", "10.1/a"), self.paper("b", "10.1/b")])
        self.write_shard("papers", "a.gz", [self.record(1, doi="10.1/a"), self.record(2, doi="10.1/b")])
        shard = self.write_shard("citations", "a.gz", [{"citingcorpusid": 1, "citedcorpusid": 2}])
        shard.write_bytes(shard.read_bytes()[:-8])
        self.invoke("scan")
        self.invoke("graph", success=False)
        self.assertFalse(self.graph.exists())

    def test_stale_bibliography_shards_and_release_are_rejected(self):
        self.write_bibliography([self.paper("a", "10.1/a")])
        self.write_shard("papers", "a.gz", [self.record(1, doi="10.1/a")])
        self.invoke("scan")
        self.write_bibliography([self.paper("b", "10.1/a")])
        self.invoke("resolve", success=False)
        self.write_bibliography([self.paper("a", "10.1/a")])
        self.write_shard("papers", "b.gz", [self.record(2)])
        self.invoke("resolve", success=False)
        self.invoke("scan")
        scan_path = self.root / "work" / "scan_matches.json"
        scan = json.loads(scan_path.read_text())
        scan["release_id"] = "2025-08-08"
        scan_path.write_text(json.dumps(scan), encoding="utf-8")
        self.invoke("resolve", success=False)
        self.assertFalse(self.counts.exists())


if __name__ == "__main__":
    unittest.main()
