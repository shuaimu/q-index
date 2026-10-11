# 5. The Ranking Algorithm

This chapter presents QIndex's two ranking metrics in two registers. It opens
with a **paper-level description of the method as designed** — the model, the
mathematics, and the rationale, written without reference to the code — and then
turns to an **implementation reference** that documents how the method is
realized and, with equal candor, where the running system diverges from the
design.

The first metric is a **venue prestige** score derived by running PageRank over a
citation graph of venues. The second is the **scholar QIndex**, the metric that
gives the project its name; from here on, "QIndex" without qualification refers
to the project, while "the scholar QIndex" or "the QIndex score" refers to the
per-author metric. One caveat is worth carrying into the design exposition: the
default CLI and website load the local S2AG graph before ranking and use snapshot
counts for reported citation totals and h-indices. See Chapter 6 for the local
extraction path and Chapter 3 for the data model.

## A paper-level description of the method

QIndex ranks scholars by composing two layers. The lower layer learns a *prestige* score for every academic venue from a citation graph defined over venues rather than individual papers. The upper layer aggregates that prestige across a scholar's publications, weighting each contribution by recency and by the scholar's byline position, and tempering the sum by a sublinear measure of total productivity. The guiding intuition throughout is the one that animates web search: prestige is not a raw vote count but a recursive quantity that flows from prestigious sources, and a contributor's standing is the accumulated prestige of where they have published, corrected for when, with whom, and how often. This section presents the method as designed; we develop the two layers in turn, then a familiar baseline, then the rationale behind each choice.

### The model: a two-layer prestige aggregation

Let $V$ be the set of venues, $n=|V|$, and $S$ the scholars. A paper $p$ has publication year $y_p$ and ordered authors $a(p)$. Let $\mathcal C$ be the CSRankings-style venue allowlist. For each scholar, first select one representative per publication identity (S2 corpus id when matched, otherwise local paper id), then retain only representatives whose normalized author list contains the scholar and whose venue belongs to $\mathcal C$. Call this set $\mathcal P_{\mathcal C}(s)$, with $N_s=|\mathcal P_{\mathcal C}(s)|$; $\mathcal P(s,v)$ is its subset at venue $v$. $Y_{\mathrm{ref}}$ is the snapshot release year when valid release metadata is loaded, otherwise the current UTC year.

The first layer produces a prestige vector $\rho \in \mathbb{R}^{V}_{\ge 0}$, where $\rho(v)$ is the prestige of venue $v$. The second layer maps $\rho$ together with a scholar's publication record to a relative score $\mathrm{QIndex}(s) \in [0, 100]$. The two layers are deliberately separated: venue prestige is a property of the field that can be estimated once and reused, while the scholar score is a lightweight aggregation over that fixed prestige.

### Layer 1: venue prestige as eigenvector centrality

We model prestige as a flow of authority between venues. Define a directed, weighted graph $G = (V, E, w)$ whose nodes are venues. Whenever a paper published in venue $u$ cites a paper published in venue $v$, authority flows from $u$ to $v$. We aggregate these paper-level events into a single venue-level edge weight, discounting each citation by the temporal distance between the citing and cited papers:

$$
w(u, v) \;=\; \sum_{\substack{p \in u,\; q \in v \\ p \,\to\, q,\; u\ne v}} 0.95^{\,\max(y_p-y_q,0)},
$$

Each unique directed cross-venue citation contributes once. Its age is the non-negative difference between citing and cited years, not an absolute difference: future-dated references receive weight 1. Missing endpoint years also leave the edge undecayed. A ten-year citation age retains $0.95^{10}\approx0.60$.

We then treat prestige as the stationary behavior of a random surfer over this venue graph: an idealized reader who, starting at some venue, repeatedly follows a citation to whichever venue the cited work appeared in, occasionally teleporting to a uniformly random venue to avoid getting stuck. Let $A_{uv} = w(u, v)$ and define the out-weight-normalized transition matrix $P$ by

$$
P_{uv} \;=\; \frac{w(u, v)}{\sum_{v' \in V} w(u, v')}
\qquad\text{whenever } \textstyle\sum_{v'} w(u, v') > 0 ,
$$

For a dangling venue define the sparse row $P_{uv}=0$ and redistribute its mass uniformly during iteration. Equivalently, the full stochastic transition matrix replaces that row by $1/n$; use that full matrix in the stationary equation below, and the sparse matrix in Algorithm 1 so dangling mass is not counted twice.

$$
r \;=\; \frac{1 - d}{n}\,\mathbf{1} \;+\; d\,P^{\!\top} r,
\qquad d = 0.85,
$$

where $\mathbf{1}$ is the all-ones vector. The two terms encode the surfer's two behaviors: with probability $d = 0.85$ it follows a citation edge (the $d\,P^{\!\top} r$ term), and with probability $1 - d = 0.15$ it teleports uniformly (the $\frac{1-d}{n}\mathbf{1}$ term). The damping factor is the canonical choice: it makes the chain ergodic, so a unique stationary $r$ exists, and it prevents prestige from being permanently trapped inside tightly knit citation cliques. This fixed point is exactly that of a damped eigenvector-centrality recursion, so the right reading of $r(v)$ is centrality: *a venue is prestigious precisely when it is cited by other prestigious venues*. Prestige is therefore endogenous and self-referential, not a raw in-citation count.

We solve the fixed point by power iteration.

> **Algorithm 1 — Venue prestige.**
> **Input:** venues $V$, $n = |V|$; transition matrix $P$ with dangling set $\mathcal{D}$; damping $d = 0.85$; tolerance $\varepsilon = 10^{-6}$; cap $T = 100$.
> 1. $r^{(0)}(v) \leftarrow 1/n$ for all $v \in V$.
> 2. **for** $t = 0, 1, \dots, T-1$:
> 3. $\quad$ $D \leftarrow \sum_{u \in \mathcal{D}} r^{(t)}(u)$  *(mass at dangling venues)*
> 4. $\quad$ $r^{(t+1)}(v) \leftarrow \dfrac{1-d}{n} + d\!\left( \sum_{u} P_{uv}\,r^{(t)}(u) + \dfrac{D}{n}\right)$ for all $v$
> 5. $\quad$ $\delta \leftarrow \max_{v \in V} \bigl| r^{(t+1)}(v) - r^{(t)}(v) \bigr|$
> 6. $\quad$ **if** $\delta < \varepsilon$ **then break**
> 7. $r \leftarrow r^{(t+1)} \big/ \lVert r^{(t+1)} \rVert_1$  *(L1-normalize)*
> 8. **return** $r$.

Iteration begins from the uniform distribution, applies the update until the largest per-venue change falls below $\varepsilon = 10^{-6}$ or the cap $T = 100$ is reached, and L1-normalizes so that $\sum_v r(v) = 1$, allowing prestige to be read as a probability mass over venues.

**Unadjusted Venue Score.** The vector consumed by Layer 2 is $\rho(v)=r(v)$:
normalized PageRank without manual prestige, venue-name or eligibility boosts.
For a graph with no usable cross-venue edges, all nodes are dangling and the
same update gives $\rho(v)=1/n$ for every venue. No substitute prestige heuristic
is used. Automatic field taxonomy and the CSRankings scholar-eligibility
allowlist remain separate policies; neither modifies this vector.

### Layer 2: the scholar QIndex

Given $\rho$, a scholar's standing is the accumulated prestige of the venues they publish in, with three corrections that raw accumulation ignores: recent work should count for more than old work, lead and senior authors deserve more credit than middle authors, and a long career should help but only sublinearly so that sheer volume cannot dominate quality.

**Recency decay.** A paper's contribution is discounted by age relative to $Y_{\mathrm{ref}}$:

$$
\mathrm{decay}(p) \;=\;
\begin{cases}
0.95^{\,(Y_{\mathrm{ref}} - y_p)/5}, & y_p < Y_{\mathrm{ref}},\\[4pt]
1, & y_p \ge Y_{\mathrm{ref}}.
\end{cases}
$$

Dividing the exponent by $5$ makes this a gentle, roughly five-year-scaled decay: a paper twenty years old retains $0.95^{4} \approx 0.81$ of its weight, whereas an unscaled $0.95^{20} \approx 0.36$ would penalize it more than twice as harshly. The aim is to favor current activity without erasing the lasting value of foundational older work.

If the publication year is unknown, decay is 1. The reference difference uses saturating subtraction, so future years do not underflow.

**Author-position credit.** A paper's prestige should not be assigned in full to every coauthor. Let scholar $s$ occupy a byline position among $k$ authors. We assign

$$
\mathrm{pos}(p, s) \;=\;
\begin{cases}
1, & k = 1 \ \text{(sole author)},\\
1.0, & s \text{ is first author},\\
0.8, & s \text{ is last author},\\
\dfrac{0.6}{\,k - 2\,}, & s \text{ is a middle author}.
\end{cases}
$$

These weights assume first/last-author roles that do not hold universally; alphabetical bylines are an important counterexample. Middle authors split a fixed 0.6 pool, while sole authors receive 1. The weights are not normalized to sum to one across a byline and do not measure verified individual contribution.

**Raw aggregate and productivity.** Restricting attention to the top-venue allowlist $\mathcal{C}$, the raw score sums weighted venue prestige over a scholar's qualifying papers:

$$
\mathrm{raw}(s) \;=\; \sum_{v \in \mathcal{C}} \;\; \sum_{p \in \mathcal{P}(s, v)} \rho(v)\,\cdot\,\mathrm{decay}(p)\,\cdot\,\mathrm{pos}(p, s).
$$

We scale by a sublinear function of the same unique, eligible authored works,

$$
\mathrm{score}(s) \;=\; \mathrm{raw}(s)\,\cdot\,\ln\!\left(N_s + 1\right),
$$

where $N_s$ counts only $\mathcal P_{\mathcal C}(s)$. Excluded publications cannot increase the logarithmic factor. The $+1$ makes it well-defined at zero. Normalize relative to the maximum; if all scores are zero, all QIndices stay zero.

$$
\mathrm{QIndex}(s) \;=\; \frac{\mathrm{raw}(s)\cdot \ln(N_s + 1)}{\displaystyle\max_{s' \in S}\,\bigl[\mathrm{raw}(s')\cdot \ln(N_{s'} + 1)\bigr]} \;\times\; 100 .
$$

When the maximum is positive, the top scholar is pinned at 100. This makes the output legible but does not make scores comparable across changing corpora: adding venues, papers or scholars can change both prestige and the normalization maximum.

> **Algorithm 2 — Scholar QIndex.**
> **Input:** prestige $\rho$; unique eligible authored sets $\mathcal P_{\mathcal C}(s)$ and their counts $N_s$; reference year $Y_{\mathrm{ref}}$.
> 1. **for each** $s \in S$:
> 2. $\quad$ $\mathrm{raw}(s) \leftarrow 0$
> 3. $\quad$ **for each** $v \in \mathcal{C}$ with $\mathcal{P}(s,v) \neq \varnothing$, **for each** $p \in \mathcal{P}(s, v)$:
> 4. $\quad\quad$ $\mathrm{raw}(s) \mathrel{+}= \rho(v)\cdot \mathrm{decay}(p)\cdot \mathrm{pos}(p, s)$
> 5. $\quad$ $\mathrm{score}(s) \leftarrow \mathrm{raw}(s)\cdot \ln(N_s + 1)$
> 6. $M \leftarrow \max_{s' \in S}\,\mathrm{score}(s')$
> 7. **for each** $s \in S$: $\mathrm{QIndex}(s)\leftarrow100\,\mathrm{score}(s)/M$ if $M>0$, otherwise $0$.
> 8. **return** scholars sorted by descending QIndex, then ascending scholar id.

### The h-index baseline

As a familiar, citation-count-based point of comparison we also compute the classical Hirsch $h$-index for each scholar, restricted to their publications in top venues. Let $c(p)$ be the citation count of paper $p$ and $\mathcal{P}_{\mathcal C}(s)$ the scholar's papers at venues in $\mathcal{C}$. Then

$$
h(s) \;=\; \max\Bigl\{\, k \in \mathbb{N} \;:\; \bigl|\{\, p \in \mathcal{P}_{\mathcal C}(s) : c(p) \ge k \,\}\bigr| \ge k \,\Bigr\} ,
$$

the largest $k$ such that at least $k$ of the scholar's top-venue papers each have at least $k$ citations. The $h$-index is insensitive to venue prestige, recency, and author position; comparing it against QIndex isolates what the prestige-flow, recency, and contribution corrections add beyond raw citation counting.

### Design rationale and assumptions

Venue-level aggregation pools sparse paper edges but cannot distinguish a landmark paper from routine work in the same venue; recency and byline corrections do not recover that missing distinction. PageRank measures recursive centrality within the selected corpus, not verified scientific quality. The scholar allowlist excludes unlisted venues without boosting venue scores. The first/last-author convention is field-dependent, while logarithmic scaling still rewards volume among eligible works. Relative normalization makes outputs readable, not invariant to corpus changes or resistant to all gaming. The implemented constants remain damping 0.85, tolerance $10^{-6}$, iteration cap 100, and decay base 0.95 (scholar age divided by five).

The method is applied to a bibliography-only graph extracted from the local S2AG snapshot, not to the whole S2 graph; this sampling boundary remains important when interpreting venue prestige.

## Implementation reference

The remainder of this chapter documents the ranking policy in `src/algorithm.rs`.
The local snapshot integration populates directed paper edges before scoring;
whole-S2 counts remain distinct from the internal graph.

## 5.1 Parameters and pipeline

`PageRankCalculator::new` uses `AlgorithmParams::default()` in `src/models.rs`:

| Parameter | Value | Role |
|---|---|---|
| `damping_factor` | `0.85` | PageRank damping `d` |
| `max_iterations` | `100` | power-iteration cap |
| `tolerance` | `1e-6` | convergence threshold |
| `venue_weight` | `0.7` | declared, **unused** in `algorithm.rs` |
| `year_decay` | `0.95` | recency decay base |
| `reference_year` | current UTC year; valid snapshot release year with `with_citations` | scholar recency reference |

`PageRankCalculator::calculate` runs a fixed four-stage pipeline:

```text
build_venue_graph        -> construct venue-to-venue edges
calculate_venue_pagerank -> normalized power iteration
calculate_scholar_scores -> scholar QIndex from venue scores
calculate_h_indices      -> per-scholar h-index (CSRankings-filtered)
```

The metrics identify the algorithm as `"PageRank-based QIndex"`.

## 5.2 Building the venue graph

Each unique directed paper citation contributes once to the venue graph. Forward
`citations` and reverse `cited_by` lists represent the same evidence, not two
independent citations. Duplicate BibTeX aliases are canonicalized by S2 corpus id
before aggregation. The old aggregate-cache/self-loop path has been removed.

Only citations between different, non-empty venues contribute. Venue ids are
normalized consistently. The existing weight is preserved:

$$
w(f \to t) = 0.95^{\,\max(\text{year}_f - \text{year}_t,0)}
$$

with weight 1 when no positive year difference is available. These are internal
edges: papers outside the matched bibliography do not become venue endpoints.

## 5.3 Venue PageRank

`calculate_venue_pagerank` in `src/algorithm.rs` sorts venue ids before iteration.
With usable cross-venue edges it initializes every venue at $1/n$ and applies:

$$
\text{new}(t) = \frac{1 - d}{n}
+ d \sum_{f \to t} \text{score}(f)\,\frac{w(f, t)}{\sum_{e} w(f, e)}
$$

with `d = 0.85`, plus $dD/n$ for dangling mass $D$. Convergence uses the maximum
absolute per-venue change after a complete update; the cap is 100 iterations and
the default tolerance is `1e-6`. Scores are then L1-normalized.

Iteration bookkeeping and convergence checks run once per complete update.
All venues, including those absent from the outgoing-edge map, participate;
dangling mass is distributed uniformly rather than lost.

## 5.4 Graphs without usable edges

If there are N venues but no usable cross-venue edges, all venue mass is
dangling. Uniform teleportation and dangling redistribution preserve the
initial distribution, giving each venue PageRank 1/N. Paper counts, venue
names and CSRankings eligibility do not change that result.

## 5.5 Published venue metrics

The published JSON field `pagerank` contains normalized, unadjusted PageRank
$\rho=r$; CLI and UI call it **Venue Score**. The retained field
`impact_factor` is $\rho(v)\ln(n_v+1)$ for unique venue works $n_v$, a
**Count-Scaled Score**, not a journal impact factor.


## 5.6 Scholar QIndex

`calculate_scholar_scores` in `src/algorithm.rs` uses
`eligible_scholar_papers` for both the sum and logarithmic count:

$$
\mathrm{raw}(s)=\sum_{p\in\mathcal P_{\mathcal C}(s)}
\rho(v_p)\,\mathrm{decay}(p)\,\mathrm{pos}(p,s),\qquad
\mathrm{score}(s)=\mathrm{raw}(s)\ln(N_s+1).
$$

The shared deduplicator prefers metadata completeness (venue, DOI, year, author
count, in that order), then smallest local id. Membership and venue eligibility
are checked on that representative. An alias with conflicting authors therefore
cannot grant credit absent from the selected work.

`reference_year` defaults to the current UTC year; `with_citations` uses the year
of a valid `%Y-%m-%d` snapshot release. Scholar age uses saturating subtraction,
so future papers are undecayed, as are unknown-year papers. Missing venue scores
contribute zero. Author-position weights remain 1 for sole/first author, 0.8 for
last author, and `0.6/(k-2)` for middle authors; this is a heuristic, not verified
contribution attribution.

```python
for s in scholars:
    papers = unique_representatives(resolve(s.papers))
    papers = [p for p in papers if contains_normalized_author(p, s)
              and resolve_venue(p) is not None
              and is_csrankings(resolve_venue(p).name)]
    raw = 0
    for p in papers:
        age = max(reference_year - p.year, 0) if p.year is not None else 0
        raw += venue_scores.get(venue_id(p), 0) * 0.95 ** (age / 5) * position_weight(p, s)
    scores[s] = raw * ln(len(papers) + 1)
maximum = max(scores.values(), default=0)
scores = {s: 100 * value / maximum if maximum > 0 else 0
          for s, value in scores.items()}
rankings = sorted(scores, key=lambda s: (-scores[s], s.id))
```

The same eligible set supplies ranking publication count, citation total,
h-index and top venues. Top venues sort by count descending, then venue id.
Profile `all_indexed` metrics describe the broader indexed record and are
distinct from `ranking` metrics; search and rankings use the eligible scope.
This is not a direct QIndex ranking by whole-S2 paper citations.

## 5.7 H-index

Reported h-indices use the local whole-S2 snapshot counts, deduplicated by corpus
id, rather than assuming that internal `cited_by.len()` is a global count:

$$
h = \max\{\,k : \text{citations}_{(k)} \ge k\,\}
$$

Sort distinct paper counts descending and select the largest qualifying rank.
The default CLI and website share this count source. Internal graph in-degree
can be useful as a different corpus-restricted measure, but is not interchangeable
with the reported snapshot-wide h-index. Neither is a direct QIndex term.

## 5.8 CSRankings venue filtering

Ranking metrics gate on `is_csrankings_venue` in `src/algorithm.rs`. It
uppercases the venue name, strips a trailing whitespace-separated 4-digit year
token, and matches against a hardcoded allowlist of roughly 85 venue tokens
(plus two partial-match rules for `USENIX`+`ATC` and `USENIX`+`SECURITY`). The
allowlist follows the CSRankings subarea set — AAAI, ICML, NEURIPS, CVPR, ACL,
SIGCOMM, NSDI, CCS, OAKLAND/SP/S&P, NDSS, SIGMOD, VLDB, ICDE, OSDI, SOSP,
EUROSYS, FAST, USENIX ATC, PLDI, POPL, OOPSLA, ICSE, FSE, STOC, FOCS, SODA,
PODC, SPAA, CHI, and so on.

This filter is applied in two places:

- `eligible_scholar_papers`, which supplies QIndex, counts, h-index and top venues;
- venue-ranking inclusion, on `venue.name`.

Because the year-strip only removes a 4-digit token preceded by whitespace, a
venue name without that exact shape (e.g. a name with no space before the year,
or with an extra suffix) can fail the allowlist match and be silently excluded
from both metrics. The automatic substring-based field classifier in
`models.rs` (Chapter 3) uses a separate keyword set, so `is_csrankings_venue`
and `get_venue_field` can disagree about a given venue. Neither boosts PageRank.

## 5.9 Shared local citation inputs

Default CLI ranking commands and the site load
`cache/citations/s2ag_paper_citations.json` and
`cache/citations/s2ag_citation_graph.json`. The old singular aggregate cache is
not the ranking input. The two releases must agree; their `release_id` identifies
snapshot provenance, while `generated` identifies processing time.
The site's `--citations-file` and `--citation-graph-file` relocate these inputs.
See Chapter 6 for local-only extraction and Chapter 10 for deployment.

## 5.10 Summary

| Stage | Behavior |
|---|---|
| Venue graph | unique internal directed citations aggregated across venues with year decay |
| Venue score | normalized, unadjusted PageRank; uniform 1/N without usable cross-venue edges |
| Scholar QIndex | existing CSRankings/recency/authorship formula over venue scores |
| Reported counts and h-index | whole-S2 snapshot counts, corpus-id deduplicated |

The snapshot graph supplies genuine citation structure, but it is a selected
bibliography graph rather than all of S2. Scholar eligibility and byline/age
weighting remain explicit policy choices. Those limitations are discussed in Chapter 11.
