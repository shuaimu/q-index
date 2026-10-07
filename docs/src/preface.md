# Preface

This book is the design specification and implementation reference for **QIndex**, a
PageRank-based academic quality index calculator written in Rust. It is intended for
engineers and researchers who want to understand how QIndex works, evaluate its
methodology, and extend it.

The name *QIndex* refers both to the project as a whole and, more narrowly, to the
per-scholar quality metric it computes. The book disambiguates the two on first use in
each chapter.

## What this book is — and what it is honest about

QIndex is part research prototype and part working application. To be useful as an
implementation reference, this book documents **both the intended design and the actual
current behavior**, even where the two diverge. The most important divergences, which
recur throughout the text, are:

- The venue-to-venue citation graph is currently empty (the BibTeX parser never populates
  per-paper citations), so the PageRank power iteration does not run on real data. Venue
  scores are produced today by a **prestige-based fallback**, not by PageRank. See
  Chapter 5, *The Ranking Algorithm*.
- The Semantic Scholar (S2AG) integration supplies **per-paper citation counts** to the
  web UI, but its extracted citation *graph* is effectively empty, so it does not feed the
  ranking algorithm. See Chapter 6, *Citation Data Integration*.
- The web interface and the CLI can produce different scores because they load citation
  data differently. See Chapter 8, *The Web Interface and HTTP API*.

Where a feature is aspirational, stubbed, or running a fallback, the book says so plainly.
It does not repeat unverified claims from the project `README.md`.

## How to read it

- **New to the project?** Read Chapters 1–2 for motivation and background, then Chapter 5
  for the core method.
- **Implementing or extending?** Chapters 3, 5, 6, and 7 cover the data model, algorithm,
  citation pipeline, and architecture; Chapter 11 lists the known issues worth fixing.
- **Operating it?** Chapters 9 and 10 cover the CLI and how to build, run, and deploy.
- **Reference?** Chapter 12 collects data formats, configuration, a glossary, and further
  reading.

This is a working draft; figures and counts reflect the repository state at the time of
writing and will drift as the corpus and code evolve.
