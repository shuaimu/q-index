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
running system does not yet populate the venue citation graph, so a prestige
fallback currently stands in for PageRank and the h-index is largely zero for
want of per-paper citation counts — the implementation-reference sections below
give the exact equations for what runs today. For how citation data is (and is
not) wired in, see Chapter 6, *Citation Data Integration*; for the data model the
algorithm consumes, see Chapter 3, *The Data Model*.

## A paper-level description of the method

QIndex ranks scholars by composing two layers. The lower layer learns a *prestige* score for every academic venue from a citation graph defined over venues rather than individual papers. The upper layer aggregates that prestige across a scholar's publications, weighting each contribution by recency and by the scholar's byline position, and tempering the sum by a sublinear measure of total productivity. The guiding intuition throughout is the one that animates web search: prestige is not a raw vote count but a recursive quantity that flows from prestigious sources, and a contributor's standing is the accumulated prestige of where they have published, corrected for when, with whom, and how often. This section presents the method as designed; we develop the two layers in turn, then a familiar baseline, then the rationale behind each choice.

### The model: a two-layer prestige aggregation

Let $V$ be the set of academic venues under consideration and $n = |V|$, and let $S$ be the set of scholars. For a paper $p$ we write $y_p$ for its publication year, $a(p)$ for its ordered author list, and $k = |a(p)|$ for the author count. For a scholar $s$, let $\mathcal{P}(s)$ be the set of all papers $s$ authored and $N_s = |\mathcal{P}(s)|$ their total publication count across the entire corpus. Let $\mathcal{C} \subseteq V$ be a fixed allowlist of *top* venues in the CSRankings style, and for $v \in \mathcal{C}$ let $\mathcal{P}(s, v)$ denote the papers $s$ published at $v$. Finally, let $Y_{\mathrm{ref}}$ be a fixed reference (current) year used to gauge recency.

The first layer produces a prestige vector $\rho \in \mathbb{R}^{V}_{\ge 0}$, where $\rho(v)$ is the prestige of venue $v$. The second layer maps $\rho$ together with a scholar's publication record to a relative score $\mathrm{QIndex}(s) \in [0, 100]$. The two layers are deliberately separated: venue prestige is a property of the field that can be estimated once and reused, while the scholar score is a lightweight aggregation over that fixed prestige.

### Layer 1: venue prestige as eigenvector centrality

We model prestige as a flow of authority between venues. Define a directed, weighted graph $G = (V, E, w)$ whose nodes are venues. Whenever a paper published in venue $u$ cites a paper published in venue $v$, authority flows from $u$ to $v$. We aggregate these paper-level events into a single venue-level edge weight, discounting each citation by the temporal distance between the citing and cited papers:

$$
w(u, v) \;=\; \sum_{\substack{p \in u,\; q \in v \\ p \,\to\, q}} 0.95^{\,\lvert y_p - y_q \rvert},
$$

where $p \to q$ denotes that paper $p$ cites paper $q$. The factor $0.95^{|y_p - y_q|}$ gently upweights citations between temporally proximate papers: a one-year gap is discounted only to $0.95$, while a decade-wide gap retains roughly $0.95^{10} \approx 0.60$ of full weight. The intuition is that a citation between contemporaneous works signals a live intellectual exchange between communities, a cleaner indicator of standing than a citation reaching far back into the historical record.

We then treat prestige as the stationary behavior of a random surfer over this venue graph: an idealized reader who, starting at some venue, repeatedly follows a citation to whichever venue the cited work appeared in, occasionally teleporting to a uniformly random venue to avoid getting stuck. Let $A_{uv} = w(u, v)$ and define the out-weight-normalized transition matrix $P$ by

$$
P_{uv} \;=\; \frac{w(u, v)}{\sum_{v' \in V} w(u, v')}
\qquad\text{whenever } \textstyle\sum_{v'} w(u, v') > 0 ,
$$

so $P_{uv}$ is the probability that a surfer at $u$, following a citation, lands at $v$. A *dangling* venue $u$ with no outgoing weight is treated as linking uniformly to all venues, $P_{uv} = 1/n$, so that authority cannot leak out of the system. With damping factor $d$, the prestige vector $r$ is the stationary distribution

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

**A tier prior.** The learned vector $r$ captures only what citation structure encodes. We additionally inject a coarse, widely agreed-upon distinction between broad venue classes that the data may underweight. Each venue carries a hand-assigned tier multiplier

$$
\tau(v) \in \{\, \mathrm{A^\ast} \mapsto 2.0,\;\; \mathrm{A} \mapsto 1.5,\;\; \mathrm{B} \mapsto 1.2,\;\; \mathrm{C} \mapsto 1.0 \,\},
$$

and we scale and renormalize:

$$
\rho(v) \;=\; \frac{\tau(v)\, r(v)}{\sum_{v' \in V} \tau(v')\, r(v')}.
$$

Conceptually this is a Bayesian-flavored move: $r$ is the data-driven estimate and $\tau$ is a soft prior over venue classes, multiplied in and renormalized to recover a distribution. Multiplying rather than adding keeps the prior and the learned signal on a common scale, and renormalization restores a probability vector. The result $\rho$ is the venue prestige consumed by Layer 2.

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

First authorship, signaling primary intellectual labor, receives full credit; last authorship, by convention the senior or advising role, receives $0.8$; the remaining $k - 2$ middle authors *split* a fixed pool of $0.6$, so adding more middle coauthors cannot manufacture credit. A single-author paper carries no positional discount.

**Raw aggregate and productivity.** Restricting attention to the top-venue allowlist $\mathcal{C}$, the raw score sums weighted venue prestige over a scholar's qualifying papers:

$$
\mathrm{raw}(s) \;=\; \sum_{v \in \mathcal{C}} \;\; \sum_{p \in \mathcal{P}(s, v)} \rho(v)\,\cdot\,\mathrm{decay}(p)\,\cdot\,\mathrm{pos}(p, s).
$$

We then scale by a sublinear function of the scholar's *total* output across all venues,

$$
\mathrm{score}(s) \;=\; \mathrm{raw}(s)\,\cdot\,\ln\!\left(N_s + 1\right),
$$

where $N_s$ counts every paper by $s$, not only those at top venues. The logarithm rewards sustained productivity with sharply diminishing returns, so a prolific record is acknowledged yet cannot, by volume alone, overwhelm the prestige-weighted quality in $\mathrm{raw}(s)$; the $+1$ keeps the factor well defined and non-negative for scholars with few papers. Finally we report on a relative $0$–$100$ scale by normalizing against the field maximum,

$$
\mathrm{QIndex}(s) \;=\; \frac{\mathrm{raw}(s)\cdot \ln(N_s + 1)}{\displaystyle\max_{s' \in S}\,\bigl[\mathrm{raw}(s')\cdot \ln(N_{s'} + 1)\bigr]} \;\times\; 100 .
$$

The top scholar is pinned at $100$ and everyone else is expressed as a percentage of that leader. Absolute prestige units depend on graph scale and venue counts and are not directly interpretable, so a relative scale yields a stable, legible index whose meaning does not drift as the corpus grows.

> **Algorithm 2 — Scholar QIndex.**
> **Input:** prestige $\rho$; top-venue set $\mathcal{C}$; reference year $Y_{\mathrm{ref}}$; scholars $S$ with total counts $\{N_{s}\}$.
> 1. **for each** $s \in S$:
> 2. $\quad$ $\mathrm{raw}(s) \leftarrow 0$
> 3. $\quad$ **for each** $v \in \mathcal{C}$ with $\mathcal{P}(s,v) \neq \varnothing$, **for each** $p \in \mathcal{P}(s, v)$:
> 4. $\quad\quad$ $\mathrm{raw}(s) \mathrel{+}= \rho(v)\cdot \mathrm{decay}(p)\cdot \mathrm{pos}(p, s)$
> 5. $\quad$ $\mathrm{score}(s) \leftarrow \mathrm{raw}(s)\cdot \ln(N_s + 1)$
> 6. $M \leftarrow \max_{s' \in S}\,\mathrm{score}(s')$
> 7. **for each** $s \in S$: $\ \mathrm{QIndex}(s) \leftarrow 100 \cdot \mathrm{score}(s)/M$
> 8. **return** $\{\mathrm{QIndex}(s)\}_{s \in S}$.

### The h-index baseline

As a familiar, citation-count-based point of comparison we also compute the classical Hirsch $h$-index for each scholar, restricted to their publications in top venues. Let $c(p)$ be the citation count of paper $p$ and $\mathcal{P}_{\mathcal C}(s)$ the scholar's papers at venues in $\mathcal{C}$. Then

$$
h(s) \;=\; \max\Bigl\{\, k \in \mathbb{N} \;:\; \bigl|\{\, p \in \mathcal{P}_{\mathcal C}(s) : c(p) \ge k \,\}\bigr| \ge k \,\Bigr\} ,
$$

the largest $k$ such that at least $k$ of the scholar's top-venue papers each have at least $k$ citations. The $h$-index is insensitive to venue prestige, recency, and author position; comparing it against QIndex isolates what the prestige-flow, recency, and contribution corrections add beyond raw citation counting.

### Design rationale and assumptions

Each layer chooses robustness and interpretability over fine resolution. Prestige is computed at the *venue* level rather than the *paper* level because paper-level citation data is sparse and noisy — many papers have few or no recorded citations, and a per-paper random walk inherits that noise — whereas aggregating into venue edges pools evidence across hundreds of papers per node, yielding a low-variance estimate of inter-venue authority and directly producing the venue ranking we want; the cost, an inability to distinguish a landmark paper from a routine one within a venue, is recovered by the recency and authorship corrections in Layer 2. Reading the stationary distribution as eigenvector centrality makes prestige recursive rather than a popularity count, and the year-decayed edges privilege contemporaneous exchange over ceremonial historical citation. The tier prior multiplicatively encodes curated community consensus that the citation graph cannot fully express, while leaving the within-tier ordering to the data. At the scholar level, restricting to a CSRankings-style allowlist measures impact *at the field's most selective venues* on a common, community-vetted footing rather than rewarding a long tail of marginal output; the gentle recency decay respects foundational work; the byline-position scheme rewards the positions that, by convention in computer science, signal primary or senior responsibility; the logarithmic productivity term and the relative $0$–$100$ normalization keep scores comparable and resistant to gaming by volume. The fixed parameters are the damping $d = 0.85$, convergence tolerance $10^{-6}$ and iteration cap $100$, the recency base $0.95$ with a five-year exponent scaling, and the tier multipliers $(\mathrm{A^\ast}, \mathrm{A}, \mathrm{B}, \mathrm{C}) = (2.0, 1.5, 1.2, 1.0)$.

The description above presents the method *as designed*; in the running system the venue citation graph is not yet populated, so venue prestige is currently produced by a prestige fallback in place of live PageRank (and per-paper citation counts, hence the $h$-index, are largely absent), as detailed in the implementation-reference sections later in this chapter.

## Implementation reference

The remainder of this chapter documents how the method above is realized in
`src/algorithm.rs`, and — with equal candor — where the running system diverges
from the design. The single most important divergence is the one flagged above:
because the venue citation graph is empty at runtime (the parser never populates
per-paper citations, and the singular cache file the calculator looks for does
not exist), the PageRank power iteration of Section 5.3 is dead code, and the
**prestige fallback** of Section 5.4 is what actually computes every venue score
today. The scholar QIndex of Section 5.6 then runs on top of those fallback
scores.

## 5.1 Parameters and pipeline

`PageRankCalculator::new` is constructed with `AlgorithmParams::default()`
(`src/algorithm.rs:75`), whose values (`src/models.rs:108-125`) are:

| Parameter | Value | Role |
|---|---|---|
| `damping_factor` | `0.85` | PageRank damping `d` |
| `max_iterations` | `100` | power-iteration cap |
| `tolerance` | `1e-6` | convergence threshold |
| `venue_weight` | `0.7` | declared, **unused** in `algorithm.rs` |
| `year_decay` | `0.95` | recency decay base |
| `tier_bonus` | `{A*:2.0, A:1.5, B:1.2, C:1.0}` | per-tier multiplier |

`PageRankCalculator::calculate()` (`src/algorithm.rs:99-122`) runs a fixed
five-stage pipeline:

```text
build_venue_graph        -> construct venue-to-venue edges
calculate_venue_pagerank -> power iteration OR prestige fallback
apply_tier_bonus         -> multiply by tier_bonus, re-normalize
calculate_scholar_scores -> scholar QIndex from venue scores
calculate_h_indices      -> per-scholar h-index (CSRankings-filtered)
```

It returns a `QIndexMetrics` whose `algorithm` field is the literal string
`"PageRank-based QIndex"` regardless of which branch actually executed — a
label that, given the empty graph, currently overstates the method in use.

## 5.2 Building the venue graph

`build_venue_graph` (`src/algorithm.rs:124-253`) has two branches selected by
`has_cached_citations`, true only when an external citation cache was loaded
and is non-empty (`src/algorithm.rs:126-130`).

### Cached branch (rarely taken; degenerate when taken)

When a cache is present, the code iterates each cached paper and accumulates
weights against a synthetic `_aggregate` node: references contribute
`ln(reference_count) + 1` from the paper's venue to `_aggregate`
(`src/algorithm.rs:150-158`); citations contribute `ln(citation_count) + 1`
from `_aggregate` to the venue (`src/algorithm.rs:164-170`). The `_aggregate`
node is then removed and each collected weight becomes a **self-loop** on its
venue with weight `× 0.1` (`src/algorithm.rs:176-185`). A comment at
`src/algorithm.rs:152-153` admits the weight is "distributed uniformly" rather
than per-reference. The result is a graph of self-loops only, with no true
venue-to-venue edges, so even this branch would not yield a meaningful
PageRank. It is also dead in the site build: `qindex build-site` never calls
`load_citation_cache` (see Chapter 8), and the CLI looks for a file that does
not exist (Section 5.6).

### BibTeX fallback branch (selected today, produces zero edges)

With no cache, the code walks `paper.citations` and `paper.cited_by` to build
directed venue edges. For each cited paper in a different, non-empty venue it
adds an edge with year-decayed weight (`src/algorithm.rs:202-224`):

$$
w(f \to t) = 1.0 \times 0.95^{\,|\text{year}_f - \text{year}_t|}
\quad\text{(decay applied only when the year difference} > 0)
$$

and symmetrically for `cited_by` (`src/algorithm.rs:227-248`). Venue IDs are
normalized with `normalize_venue_id` (uppercase, spaces/hyphens to `_`, strip
apostrophes; `src/algorithm.rs:623-629`).

This branch is selected today, **but it produces zero edges.** The parser
initializes every `Paper` with `citations: Vec::new()` and
`cited_by: Vec::new()` (`src/parser.rs:166-167`) and no BibTeX field ever
writes to `citations`. `build_citation_network` (`src/models.rs:174-192`)
derives `cited_by` by traversing `citations`, which is empty, so `cited_by`
stays empty too. The S2AG integration (Chapter 6) supplies per-paper citation
**counts** for display but does not populate these graph fields. Net effect:
`self.venue_graph` is empty after `build_venue_graph`.

## 5.3 Venue PageRank (the intended path)

`calculate_venue_pagerank` (`src/algorithm.rs:255-378`) takes the venue set
from `graph.venues.keys()` (`n` venues) and branches on
`has_citations = !venue_graph.is_empty()` (`src/algorithm.rs:264`). When true,
it runs power iteration. Scores initialize to `1/n`
(`src/algorithm.rs:272-274`); each iteration seeds every venue with the random
restart term `(1 - d)/n` (`src/algorithm.rs:282-284`) and then adds, for each
edge `f -> t`, the contribution `d · score(f) · w(f,t) / Σ_e w(f,·)`
(`src/algorithm.rs:293-295`). The exact update equation is:

$$
\text{new}(t) = \frac{1 - d}{n}
+ d \sum_{f \to t} \text{score}(f)\,\frac{w(f, t)}{\sum_{e} w(f, e)}
$$

with `d = 0.85`. A dangling venue (no outgoing edges) instead distributes
`d · score(f) / n` to every venue (`src/algorithm.rs:299-304`). Convergence is
the max absolute per-venue change; the loop breaks when that drop below
`tolerance = 1e-6` or after `max_iterations = 100`
(`src/algorithm.rs:321-324`). Finally scores are L1-normalized to sum to 1
(`src/algorithm.rs:328-333`).

### Control-flow quirk in the iteration body

The power-iteration body is structurally malformed. The inner
`for (to_venue, weight) in edges` loop's closing brace at
`src/algorithm.rs:296` prematurely closes the `if total_weight > 0.0` block, so
the `} else {` at line 297 binds against the inner-for nesting in a way that
pushes the convergence check, the `self.venue_scores = new_scores` assignment,
the L1 normalization, and the `break` (lines 308-333) **inside** the
`for (from_venue, edges)` loop rather than inside the outer iteration loop. In
other words, the per-iteration bookkeeping that should run once per power-iter
step instead runs once per source venue. The code compiles because the braces
nest into a self-consistent (if unintended) shape, and the bug is never
exercised at runtime because the graph is empty and this branch is not taken.
Anyone reviving PageRank must fix this nesting first; see Chapter 11,
*Limitations, Known Issues, and Roadmap*.

A secondary hazard: `search_venues` and the ranking sorts use
`partial_cmp().unwrap()` on `pagerank`, which would panic on a `NaN` score
(`src/models.rs` caveats). The fallback below cannot produce `NaN`, but a
revived PageRank with a zero-weight dangling set could.

## 5.4 The prestige fallback (what actually runs today)

When `venue_graph` is empty, `calculate_venue_pagerank` takes its `else` branch
(`src/algorithm.rs:334-367`). This is the code path that **actually computes
every venue score in the running system.** For each venue it computes:

$$
\text{score}(v) =
\underbrace{\left[\, \text{papers}_v > 0 \;?\; \frac{\ln(\text{papers}_v + 1)}{10} : 0 \,\right]}_{\text{log paper count}}
\times \underbrace{(\text{is\_csrankings}(v) \;?\; 2.0 : 1.0)}_{\text{CSRankings boost}}
+ \underbrace{\frac{\big(\sum \text{charcodes}(\text{name}_v)\big) \bmod 100}{10000}}_{\text{name-hash jitter}}
$$

In code (`src/algorithm.rs:338-358`):

```rust
let paper_count = venue.papers.len() as f64;
if paper_count > 0.0 {
    score = (paper_count + 1.0).ln() / 10.0;
}
if is_csrankings_venue(&venue.name) {
    score *= 2.0;
}
let name_hash = venue.name.chars().fold(0u32, |acc, c| acc.wrapping_add(c as u32));
let variation = ((name_hash % 100) as f64) / 10000.0;
score += variation;
```

The name-hash term (range `0.0` to `0.0099`) exists only to break ties between
venues with identical paper counts — the comment calls it "variation ... to
avoid identical scores." Scores are then L1-normalized to sum to 1
(`src/algorithm.rs:361-366`).

The practical consequence is that venue ranking today is essentially a
monotonic function of `ln(paper_count)`, doubled for CSRankings venues, with a
tiny deterministic jitter. It carries **no citation signal whatsoever.** The
`"PageRank"` label is therefore nominal: there is no eigenvector, no link
structure, and no convergence — just a normalized log-count heuristic.

## 5.5 Tier bonus

After scoring, `apply_tier_bonus` (`src/algorithm.rs:380-403`) multiplies each
venue score by `tier_bonus[venue.tier]` when the tier key is present
(`A*` → 2.0, `A` → 1.5, `B` → 1.2, `C` → 1.0), then re-normalizes to sum to 1.
A quantity `_impact = score · ln(paper_count + 1)` is computed and immediately
discarded (`src/algorithm.rs:391`); it is dead. Tiers come from
`get_venue_tier` (`src/models.rs:359-389`), described in Chapter 3.

Because the prestige fallback already doubles CSRankings venues and most
A\*/A venues are also CSRankings venues, today's effective venue weighting
stacks two prestige multipliers (the `×2` CSRankings boost and the `×2.0`/`×1.5`
tier bonus) on top of `ln(paper_count)`.

## 5.6 Scholar QIndex

`calculate_scholar_scores` (`src/algorithm.rs:405-476`) computes each scholar's
raw score by summing over their CSRankings-venue publications. For scholar `s`:

$$
\text{raw}(s) =
\sum_{\substack{v \in \text{pubs}(s) \\ \text{is\_csrankings}(v)}}
\;\sum_{p \in \text{papers}(s, v)}
\text{venuescore}(v)\;\cdot\;\text{decay}(p)\;\cdot\;\text{pos}(p, s)
$$

then a log-scaling and a 0–100 normalization:

$$
\text{QIndex}(s) =
\frac{\text{raw}(s)\;\cdot\;\ln(\text{papers}_s + 1)}{\max_{s'} \big(\text{raw}(s')\cdot\ln(\text{papers}_{s'}+1)\big)} \times 100
$$

The pieces, exactly as coded:

- **CSRankings filter.** A venue is skipped entirely unless
  `is_csrankings_venue(venue.name)` (`src/algorithm.rs:410-416`). See
  Section 5.8.
- **Venue score.** `venue_scores[venue_id]`, defaulting to `0.01` if the venue
  is missing from the score map (`src/algorithm.rs:418`).
- **Year decay (note the `/5.0`).** With `current_year` **hardcoded to 2024**
  (`src/algorithm.rs:426`), `year_diff = 2024 - year`, and only when
  `year_diff > 0`:

  $$
  \text{decay}(p) = 0.95^{\,(2024 - \text{year}_p)\,/\,5.0}
  $$

  This is **not** a plain `0.95^age`: the exponent is `age / 5`
  (`src/algorithm.rs:429`). The reference year is frozen at 2024, and there is a
  latent bug in how the difference is computed. `current_year` is inferred as
  `u32` to match `paper.year: Option<u32>`, so `current_year - year` is an
  **unsigned** subtraction evaluated *before* the `year_diff > 0.0` check
  (`src/algorithm.rs:426-427`). For any paper dated 2025 or later this
  underflows: a debug build panics with `attempt to subtract with overflow`, and
  the release build wraps to roughly `4.29e9`, making `year_diff > 0.0` true and
  `0.95^(4.29e9 / 5)` collapse to ~0 — so the paper's contribution is effectively
  **zeroed** rather than left undecayed. (The BibTeX-graph path at
  `src/algorithm.rs:211` casts `as i32` and avoids this; only this scholar-score
  path is affected.) Because the calendar is now past 2024, any 2025-or-later
  paper in the corpus triggers this today.
- **Author-position weight.** Applied only when a paper has more than one author
  (`src/algorithm.rs:434-450`). The scholar's position is found by matching
  `normalize_author_name(a) == scholar.normalized_name`, defaulting to
  `author_count` if not found:

  | Position | Multiplier |
  |---|---|
  | first (index 0) | `1.0` |
  | last (`author_count - 1`) | `0.8` |
  | middle | `0.6 / (author_count - 2)` |
  | single-author paper | no factor applied |

  The middle-author divisor means a 3-author paper's middle author gets
  `0.6/1 = 0.6`, a 4-author paper's middles get `0.6/2 = 0.3` each, and so on —
  middle credit shrinks with team size.
- **Log-scaling inconsistency.** After summing, the raw score is multiplied by
  `ln(total_papers + 1)` where `total_papers = scholar.papers.len()`
  (`src/algorithm.rs:408,458-460`) — that is, **all** of the scholar's papers
  across every venue, even though the summation above only counted CSRankings
  venues. The two halves of the formula use different paper sets.
- **Normalization to 0–100.** The maximum raw score across all scholars is
  found, and every score is rescaled to `(score / max_score) * 100`
  (`src/algorithm.rs:465-475`). The top scholar is pinned to 100.

Pseudocode for the whole computation:

```python
for s in scholars:
    raw = 0
    for (venue, papers) in s.publications_by_venue:
        if not is_csrankings(venue): continue
        vscore = venue_scores.get(venue, 0.01)
        for p in papers:
            ps = vscore
            if p.year and 2024 - p.year > 0:   # NOTE: real code is u32 math; year > 2024 underflows (see above)
                ps *= 0.95 ** ((2024 - p.year) / 5.0)
            if len(p.authors) > 1:
                ps *= position_weight(p, s)   # 1.0 / 0.8 / 0.6/(k-2)
            raw += ps
    raw *= ln(len(s.papers) + 1)              # ALL papers, not CSRankings-only
    scores[s] = raw
max_raw = max(scores.values())
for s in scores:
    scores[s] = scores[s] / max_raw * 100
```

Because venue scores today come from the prestige fallback (Section 5.4), the
scholar QIndex is ultimately a recency- and authorship-weighted sum of
log-paper-count prestige across an author's CSRankings venues, log-scaled by
their total output and rescaled to 0–100. It reflects publication volume and
venue prestige, not citation impact.

## 5.7 H-index

`calculate_scholar_h_index` (`src/algorithm.rs:486-512`) collects
`paper.cited_by.len()` for each of a scholar's papers whose venue (resolved via
`normalize_venue_id(paper.venue)`) is a CSRankings venue, sorts the counts
descending, and returns the largest `k` such that the `k`-th paper (1-indexed)
has at least `k` citations:

$$
h = \max\{\,k : \text{citations}_{(k)} \ge k\,\}
$$

breaking on the first failure. There are two important caveats:

1. **Every h-index is 0 today.** `cited_by` is empty for every paper
   (Section 5.2), so all counts are 0 and `h = 0` for all scholars.
2. **The computed h-index is discarded.** `calculate_h_indices`
   (`src/algorithm.rs:478-484`) calls `calculate_scholar_h_index` and writes the
   result only to a `debug!` log — it never stores it back on the `Scholar`.
   The `Scholar.h_index` field stays at its parser-initialized `0`
   (`src/parser.rs`). The CLI rankings and the website recompute the h-index on demand in
   `get_top_scholars` (`src/algorithm.rs:544-620`), which is still `0` for the
   same `cited_by` reason. A second standalone free function
   `calculate_h_index` (`src/algorithm.rs:631-651`) counts **all** papers
   without the CSRankings filter and is unused dead code.

## 5.8 CSRankings venue filtering

Both metrics gate on `is_csrankings_venue` (`src/algorithm.rs:15-60`). It
uppercases the venue name, strips a trailing whitespace-separated 4-digit year
token, and matches against a hardcoded allowlist of roughly 85 venue tokens
(plus two partial-match rules for `USENIX`+`ATC` and `USENIX`+`SECURITY`). The
allowlist follows the CSRankings subarea set — AAAI, ICML, NEURIPS, CVPR, ACL,
SIGCOMM, NSDI, CCS, OAKLAND/SP/S&P, NDSS, SIGMOD, VLDB, ICDE, OSDI, SOSP,
EUROSYS, FAST, USENIX ATC, PLDI, POPL, OOPSLA, ICSE, FSE, STOC, FOCS, SODA,
PODC, SPAA, CHI, and so on.

This filter is applied in three places with subtly different inputs, which is
worth noting for anyone extending the venue mapping:

- venue scoring boost in the fallback, on `venue.name` (`src/algorithm.rs:348`);
- scholar QIndex per-venue inclusion, on `venue.name` (`src/algorithm.rs:413`);
- h-index inclusion and `get_top_*` listing, also on `venue.name`.

Because the year-strip only removes a 4-digit token preceded by whitespace, a
venue name without that exact shape (e.g. a name with no space before the year,
or with an extra suffix) can fail the allowlist match and be silently excluded
from both metrics. The substring-based tier/field classifiers in `models.rs`
(Chapter 3) use a separate, larger keyword set, so `is_csrankings_venue` and
`get_venue_tier`/`get_venue_field` can disagree about a given venue.

## 5.9 Where the cache should come from, and why it does not load

`load_citation_cache` (`src/algorithm.rs:84-97`) acts only if its path
argument `exists()`; on any error it logs at debug level and continues with no
cache. The CLI commands `calculate`, `venues`, `scholars`, and `search` call it
with the hardcoded path `./cache/citations.json`
(`src/main.rs:89, 118, 136, 154`). **That singular file does not exist** — the
repository has only a `cache/citations/` directory containing other JSON files
(`citation_data.json`, `combined_cache.json`, `s2ag_citations.json`, and an
effectively empty `citation_graph.json`). So even on the CLI path the cache is
never loaded, `has_cached_citations` is false, and the prestige fallback runs.

The site generator is different and even more decisive: `build_site`
(`src/site/mod.rs`) constructs `PageRankCalculator::new(&graph)` and calls
`calculate()` **without ever calling `load_citation_cache`**, as the removed web
server did before it. The published rankings are therefore always computed from
the in-memory BibTeX graph (empty edges → prestige fallback), and can differ from
a future CLI run that does load a cache. This divergence, and the relative-path
requirement (the build must run from the repo root or the S2AG cache path
resolves to nothing), are covered further in Chapter 8, *The Static Website and
Its Data Files*.

## 5.10 Summary: intended vs. actual

| Stage | Intended behavior | Actual behavior today |
|---|---|---|
| Venue graph | weighted venue-to-venue edges from citations | empty (no `citations`/`cited_by` populated) |
| Venue score | PageRank power iteration, `d=0.85`, tol `1e-6` | prestige fallback: `ln(papers+1)/10 × (CSR?2:1) + name-jitter` |
| Tier bonus | multiply by tier, re-normalize | same (active) |
| Scholar QIndex | `Σ venuescore·decay·pos`, ×`ln(papers+1)`, 0–100 | active, but over fallback venue scores; 2024 ref-year frozen |
| H-index | CSRankings-filtered citation h-index | always 0 (`cited_by` empty); also discarded after compute |

The honest reading: QIndex today is a **prestige-and-volume ranking dressed in
PageRank vocabulary.** The graph machinery, the power-iteration equation, and
the h-index are all present and specified, but none of them carry citation
signal in the running system because the citation graph is empty. Restoring the
intended behavior requires (1) populating `Paper.citations`/`cited_by` from a
real citation source (Chapter 6), (2) fixing the brace-nesting bug in
`calculate_venue_pagerank` (Section 5.3), (3) un-freezing `current_year`, and
(4) actually persisting the computed h-index. These are tracked in Chapter 11.
