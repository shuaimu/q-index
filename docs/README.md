# QIndex Documentation

This directory contains the QIndex technical book — a combined **design specification and
implementation reference**. It is written as an [mdBook](https://rust-lang.github.io/mdBook/).

## Reading it

Every chapter is plain Markdown under [`src/`](src/), so you can read it directly on
GitHub or in any editor. Start with the [Preface](src/preface.md) and
[Table of Contents](src/SUMMARY.md).

## Chapters

| # | Chapter | File |
|---|---------|------|
| — | Preface | [`src/preface.md`](src/preface.md) |
| 1 | Introduction and Motivation | [`src/01-introduction.md`](src/01-introduction.md) |
| 2 | Background and Related Work | [`src/02-background.md`](src/02-background.md) |
| 3 | The Data Model | [`src/03-data-model.md`](src/03-data-model.md) |
| 4 | The Bibliographic Database | [`src/04-bibliographic-database.md`](src/04-bibliographic-database.md) |
| 5 | The Ranking Algorithm | [`src/05-ranking-algorithm.md`](src/05-ranking-algorithm.md) |
| 6 | Citation Data Integration | [`src/06-citation-data-integration.md`](src/06-citation-data-integration.md) |
| 7 | System Architecture | [`src/07-system-architecture.md`](src/07-system-architecture.md) |
| 8 | The Web Interface and HTTP API | [`src/08-web-interface-and-api.md`](src/08-web-interface-and-api.md) |
| 9 | The Command-Line Interface | [`src/09-command-line-interface.md`](src/09-command-line-interface.md) |
| 10 | Building, Running, and Deployment | [`src/10-building-running-deployment.md`](src/10-building-running-deployment.md) |
| 11 | Limitations, Known Issues, and Roadmap | [`src/11-limitations-and-roadmap.md`](src/11-limitations-and-roadmap.md) |
| 12 | Appendices | [`src/12-appendices.md`](src/12-appendices.md) |

## Building the rendered book (optional)

```bash
# Install mdBook once (https://rust-lang.github.io/mdBook/guide/installation.html)
cargo install mdbook

# From the repo root:
mdbook build docs     # outputs static HTML to docs/book/
mdbook serve docs     # live-reloading preview at http://localhost:3000
```

> Note: `mdbook build` writes the rendered site to `docs/book/`. Add that path to
> `.gitignore` if you do not want to commit generated output.

## Scope and honesty

This book documents both the intended design and the **actual current behavior** of the
code, which diverge in places (most notably: venue ranking currently uses a prestige
fallback rather than live PageRank). See the [Preface](src/preface.md) for the headline
caveats. It intentionally supersedes the older, partly aspirational claims in the
top-level [`../README.md`](../README.md).
