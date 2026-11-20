# NOMAD V2 (DARWIN.RS)

> **Decentralized Cognitive Swarm, Horizontal Evolutionary Recombination & Cumulative Culture for LLM-Synthesized Heuristics**

[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org)
[![CI](https://github.com/emmanuelchauvin/NOMAD/actions/workflows/ci.yml/badge.svg)](https://github.com/emmanuelchauvin/NOMAD/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](#6-license)
[![Status](https://img.shields.io/badge/scientific_study-pre--registered_V2-green.svg)](SPECIFICATIONS.md)

---

## Results of the proof phase (closed 20 November 2026)

**Question.** At equal budget, does LLM-guided program evolution (NOMAD) beat drawing independent proposals from the same model, and does the gap grow with the budget? Problem: online bin packing, Weibull profile; effect measured on a held-out validation set as the share of the Best-Fit excess removed. Every protocol was written and committed **before** its runs; raw data were committed before being read. Test: exact two-sided paired Wilcoxon signed-rank over seeds (24 in general).

- **H1**: at equal model calls, evolution beats the draw.
- **H2**: at equal spend (dollars), it also does.
- **H3**: the advantage is larger at 1,359 calls than at 340.

| Protocol | Model | Calls | H1 | H2 | H3 |
|---|---|---|---|---|---|
| [Confirmation](docs/RESULTATS_GEMINI_CONFIRMATION.md) | gemini-3.1-flash-lite | 85 | established (p = 0.0075) | no (p = 0.21) | n/a |
| [Scale](docs/RESULTATS_GEMINI_ECHELLE.md) | gemini-3.1-flash-lite | 340 | established (p = 0.0002) | established (p = 0.0016) | no (p = 0.40) |
| [Curve](docs/RESULTATS_GEMINI_COURBE.md) | gemini-3.1-flash-lite | 1,359 | established (22/24 seeds, p < 0.0001) | established (p = 0.0012) | **established** (p = 0.0042) |
| [GLM 340](docs/RESULTATS_GLM_340.md) | glm-5.3-flash | 344 | established (p = 0.0015) | no (cost ratio 4.36, p = 0.47) | n/a |
| [GLM H2, pruning](docs/RESULTATS_GLM_H2.md) | glm-5.3-flash | 344 | established (20/24, p < 0.0001) | established (ratio 2.12, p = 0.014) | n/a |
| [GLM H3](docs/RESULTATS_GLM_H3.md) | glm-5.3-flash | 1,359 | established (24/24, p < 0.0001) | established (p = 0.0065) | **no** (p = 0.60) |

### Established
1. Evolution beats the draw at equal calls, on two models from two vendors, at every budget tested (85 to 1,359 calls).
2. It also wins at equal spend once the cost per call stays contained (on GLM, after pruning the champion brought the cost ratio from 4.4 to 2.1).
3. On Gemini the advantage grows with the budget: the draw plateaus near 17 % while evolution keeps improving (56.0 % vs 17.7 % at 1,359 calls).

### Not established
1. **H3 on GLM.** The median advantage stays between 20 and 27 points from 85 to 1,359 calls, because the GLM draw keeps improving. Only consistency grows (17/24 seeds won at 344 calls, 24/24 at 1,359).
2. **Crossover: not properly tested, not refuted.** The only test (12 seeds, 344 calls, p = 0.77) confounded crossover with pruning and used the pre-fix graft; meanwhile crossovers did produce 45 new champions out of 1,656 attempts, at no cost in calls. A clean test (corrected graft, crossover alone, 24 seeds) is the open question. None of the established results rests on crossover: the winning system is a population with rank selection, LLM-informed mutation and restarts.

### Limits
- A single problem (online bin packing, Weibull profile) and a single instance set.
- Two small, inexpensive models; answers are not reproducible bit for bit. Engine tuning was done on Gemini.
- Restarts are fresh proposals, so the evolution arm contains a share of draw.
- Equal-spend point `k*` uses the run-average cost per call, which is unfavourable to evolution.
- Pruning costs extra compute (free in calls, not in CPU time).
- No comparison with published state of the art (FunSearch, EoH, ReEvo): different instances and metric.
- Total spend: about 148 USD in API calls.

**Full report:** [docs/SYNTHESE_PREUVE.md](docs/SYNTHESE_PREUVE.md) (in French). **Pre-registered protocols:** `docs/PROTOCOLE_*.md`. **Results per protocol:** `docs/RESULTATS_*.md`. **Handover and open questions:** [docs/REPRISE.md](docs/REPRISE.md). Citable snapshot: tag `v1.0-conclusions`.

---

## 1. Overview & Scientific Purpose

NOMAD (**DARWIN.RS**) is an evolutionary computing and artificial intelligence framework implemented in Rust. It establishes an experimental platform to study whether and how a decentralized population of autonomous LLM agents can produce **cumulative algorithmic intelligence** superior to isolated evolutionary selection, while eliminating false evolutionary discoveries (dead code, overfitting, and local convergence).

### Earlier exploratory framework (V2 specification)

The hypotheses below are the original V2 specification, tested only exploratorily (mostly single seed, local Ollama models). They are **distinct from** the H1/H2/H3 of the proof phase above, which are the results to cite.
* **H1 (Cumulative Cultural Evolution):** Lineages with access to collective cultural heritage via cross-epoch transmission achieve higher Cumulative Improvement Factor ($CIF > 1.0$) and discover novel behavioral niches faster than Tabula Rasa lineages ($R_{\text{innov, A}} > R_{\text{innov, B}}$).
* **H2 (Recombination Superiority & Parsimony):** Swarm horizontal recombination (**Swarm Crossover**) discovers bin packing heuristics that break seed inertia and achieve superior packing efficiency and computational parsimony compared to mutation-only selection (**Group C vs Group D**).
* **H3 (Out-Of-Distribution Generalization):** Heuristics evolved via multi-trajectory dynamic evaluation generalize superior packing efficiency on Out-Of-Distribution datasets (scale shift $N=500$ and heavy-tailed Weibull distribution).

---

## 2. Architectural Pillars (V2)

NOMAD V2 resolves the vulnerabilities of genetic LLM frameworks through 4 foundational pillars and 1 transversal cumulative culture protocol:

1. **Pillar 1 — AST-Level Semantic Unit Representation & Multi-Loci Crossover**: Heuristics are represented as validated Abstract Syntax Trees rather than brittle free-form text. Swarm Crossover combines non-conflicting logical loci (`early_horizon` and `score_calc`) or verified functional subtrees (`ScoreModifier`, `FilterCondition`, `SortingKey`).
2. **Pillar 2 — 3-Level Anti-Dead-Code Verification Pipeline**:
   - *Level 1 (Structural)*: AST linting for unreachable branches and tautologies.
   - *Level 2 (Dynamic Branch Coverage)*: Runtime instrumentation tagging executed AST branches.
   - *Level 3 (Functional Semantic Impact)*: Differential ablation proving that replacing a unit with identity significantly degrades performance.
3. **Pillar 3 — Dynamic Trajectory, Bounded Lookahead ($k=2$) & Behavioral Diversity**: Capsules are profiled along behavioral axes (Dead Space Ratio, New Bin Creation Ratio, Residual Capacity Variance), dynamic trajectories ($tLDR$), and stream lookahead awareness (`next_item_size`, `next_job_cpu`, `next_job_ram`).
4. **Pillar 4 — MAP-Elites Quality-Diversity Archive**: An unconstrained $10 \times 10$ calibrated behavioral grid maintaining the best-performing heuristic in each unique behavioral niche, preventing premature convergence.
5. **Transversal Pillar — Cumulative Culture & 3-Epoch "Mort et Renaissance"**: Multi-civilizational extinction protocol evaluating whether lineages inheriting validated cultural heritage systematically outperform scratch populations across successive epochs ($CIF > 1.0$, $R_{\text{herit}} = 100\%$).

---

## 3. Workspace Architecture

```
NOMAD/
├── Cargo.toml                  # Cargo Workspace root
├── SPECIFICATIONS.md           # Pre-registered V2 scientific specifications & mathematical formalisms
├── TESTS.md                    # Scientific test audit & empirical experimental verification
├── README.md                   # System overview & CLI reference
├── crates/
│   ├── nomad-core/             # SkillCapsule, immutable contracts, metrics & configuration
│   ├── nomad-genome/           # AST representation, PetGraph phylogenetic DAG & lineage tracking
│   ├── nomad-sandbox/          # Deterministic execution engine with fuel/gas metering
│   ├── nomad-evaluator/        # Independent Oracle for Online 1D Bin Packing (In-Dist, OOD-1, OOD-2)
│   ├── nomad-operator/         # LLM operator abstraction (Ollama, GGUF SHA-256, Mock fallback)
│   ├── nomad-orchestrator/     # MAP-Elites archive, evolution engine & "Mort et Renaissance" protocol
│   └── nomad-cli/              # CLI interface for runs, rebirth protocols, archives, and HTML phylogeny
└── scripts/
    └── analysis/
        └── wilcoxon_analysis.py # Paired Wilcoxon Signed-Rank statistical test & effect size
```

---

## 4. CLI Usage & Experimental Protocols

### Build the Project
```bash
cargo build --release
```

### 1. Run an Evolutionary Trial
```bash
# 1D Online Bin Packing (Group C Swarm with Crossover)
cargo run --release --bin nomad -- run --domain bin-packing --operator ollama --model qwen2.5-coder:7b --pop-size 8 -n 10 --group c --output run_report.json

# Multi-Resource Cloud Job Scheduling (CPU & RAM Vector Packing)
cargo run --release --bin nomad -- run --domain scheduling --operator ollama --model qwen2.5-coder:7b --pop-size 8 -n 10 --group c --output run_report_sched.json

# Group D (Evolutionary Selection with Mutation Only)
cargo run --release --bin nomad -- run --operator ollama --model qwen2.5-coder:7b --pop-size 8 -n 10 --group d --output run_group_d.json
```

### 2. Run the "Mort et Renaissance" Protocol (Cumulative Culture)
```bash
# 3-Epoch Bin Packing Cumulative Culture Protocol with Lookahead k=2
cargo run --release --bin nomad -- rebirth --domain bin-packing --operator ollama --model qwen2.5-coder:7b --pop-size 8 -n 8 -e 3 --output grand_rebirth_bp_v2.json

# 3-Epoch Cloud Scheduling Cumulative Culture Protocol
cargo run --release --bin nomad -- rebirth --domain scheduling --operator ollama --model qwen2.5-coder:7b --pop-size 8 -n 8 -e 3 --output grand_rebirth_sched.json
```
*Outputs: Cumulative Improvement Factor ($CIF$), Functional Heritability ($R_{\text{herit}}$), and Innovation Rate ($R_{\text{innov}}$).*

### 3. Inspect the MAP-Elites Quality-Diversity Archive
```bash
cargo run --release --bin nomad -- archive --input grand_rebirth_bp_v2.json
```

### 4. Export & Visualize Interactive Phylogenetic Tree (HTML / SVG)
```bash
# Supports both ExperimentRunReport and RebirthExperimentReport
cargo run --release --bin nomad -- graph --report grand_rebirth_bp_v2.json --format html --output grand_arbre_rebirth_bp.html
cargo run --release --bin nomad -- graph --report grand_rebirth_sched.json --format html --output grand_arbre_sched.html
start grand_arbre_rebirth_bp.html
```

### 5. Datasets & Deterministic Baselines
```bash
# Cryptographic SHA-256 dataset verification (Bin Packing & Scheduling)
cargo run --release --bin nomad -- dataset --domain bin-packing
cargo run --release --bin nomad -- dataset --domain scheduling

# Classical baselines (First-Fit, Best-Fit, Balanced-Fit)
cargo run --release --bin nomad -- baseline --domain bin-packing
cargo run --release --bin nomad -- baseline --domain scheduling
```

### 6. Paired Comparative Study (Group C Swarm vs Group D Mutation)
```bash
# Compare Swarm crossover vs pure mutation across 5 paired seeds
cargo run --release --bin nomad -- compare --domain bin-packing --operator ollama --runs 5
cargo run --release --bin nomad -- compare --domain scheduling --operator mock --runs 5
```

---

## 5. Scientific Validation & Testing

NOMAD is verified by an automated test suite comprising **103 unit and integration tests** and empirical acceptance protocols. The items below are **exploratory, single-seed observations** from the earlier V2 phase, not the proof-phase results above:
- `cargo test --workspace` (All 103 unit & integration tests pass)
- **Controlled Ablation Study (Group C vs Group D, 15 Generations) :** Recombinant champion `SwarmCross-g6-d6a1-8f9d-mut-g12-mut-g12` verified as `PassedLevel2Computational` (computational viability confirmed; behavioral causality of graft remains exploratory), achieving **28.96% fuel reduction** (Fuel: 703 568 vs 990 350, +40.76% extra fuel in Group D) and superior Quality-Diversity coverage (10 niches, QD-Score: 8.770 vs 9 niches, 7.873) over mutation-only baseline locked in single-lineage mode (`ablation_group_c.json` & `ablation_group_d.json`).
- **Multi-Resource Cloud Scheduling :** Exploratory score of **391 servers** observed on a single seed ($CIF_{\text{perf}} = 1.071\text{x}$, $R_{\text{herit}} = 100\%$, OOD-2: 1 731 servers).
- **Online 1D Bin Packing :** Lookahead $k=2$ + Dynamic Horizon with **98.56% OOD-2 efficiency** and $R_{\text{herit}} = 100\%$. Fair comparison against `BEST_FIT_LOOKAHEAD` baseline at equal $k=2$.
- Level 1-3 Anti-dead-code validation with surgical $C_{\text{null}}$ differential ablation (hash-verified statement neutralization, directional degradation threshold).
- Pre-registered paired Wilcoxon signed-rank test and permutation test campaign (`nomad campaign`) for multi-seed statistical significance.
- Adaptive Operator Selection (AOS) for dynamic Swarm crossover control.
- MAP-Elites niche competition & replacement with fuel parsimony tie-breaking.
- Cryptographic reproducibility across random seeds.

See [TESTS.md](TESTS.md) for detailed experimental benchmarks and [SPECIFICATIONS.md](SPECIFICATIONS.md) for full mathematical definitions.

---

## 6. License
Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.

Copyright (c) 2026 Emmanuel Chauvin.

