# NOMAD V2 (DARWIN.RS)

> **Decentralized Cognitive Swarm, Horizontal Evolutionary Recombination & Cumulative Culture for LLM-Synthesized Heuristics**

[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Status](https://img.shields.io/badge/scientific_study-pre--registered_V2-green.svg)](SPECIFICATIONS.md)

---

## 1. Overview & Scientific Purpose

NOMAD (**DARWIN.RS**) is an evolutionary computing and artificial intelligence framework implemented in Rust. It establishes an experimental platform to rigorously demonstrate whether and how a decentralized population of autonomous LLM agents can produce **cumulative algorithmic intelligence** superior to isolated evolutionary selection, while eliminating false evolutionary discoveries (dead code, overfitting, and local convergence).

### Core Scientific Hypotheses
* **H1 (Cumulative Cultural Evolution):** Lineages with access to collective cultural heritage via cross-epoch transmission achieve higher Cumulative Improvement Factor ($CIF > 1.0$) and discover novel behavioral niches faster than Tabula Rasa lineages ($R_{\text{innov, A}} > R_{\text{innov, B}}$).
* **H2 (In-Distribution Superiority):** Swarm horizontal recombination (**Swarm Crossover**) discovers bin packing heuristics that achieve superior packing efficiency compared to mutation-only selection (**Group C vs Group D**, $p < 0.05$).
* **H3 (Out-Of-Distribution Generalization):** Heuristics evolved via multi-trajectory dynamic evaluation generalize superior packing efficiency on Out-Of-Distribution datasets (scale shift $N=500$ and heavy-tailed Weibull distribution).

---

## 2. Architectural Pillars (V2)

NOMAD V2 resolves the vulnerabilities of genetic LLM frameworks through 4 foundational pillars and 1 transversal cumulative culture protocol:

1. **Pillar 1 — AST-Level Semantic Unit Representation**: Heuristics are represented as validated Abstract Syntax Trees rather than brittle free-form text. Swarm Crossover swaps verified logical functional subtrees (`ScoreModifier`, `FilterCondition`, `SortingKey`).
2. **Pillar 2 — 3-Level Anti-Dead-Code Verification Pipeline**:
   - *Level 1 (Structural)*: AST linting for unreachable branches and tautologies.
   - *Level 2 (Dynamic Branch Coverage)*: Runtime instrumentation tagging executed AST branches.
   - *Level 3 (Functional Semantic Impact)*: Differential ablation proving that replacing a unit with identity significantly degrades performance.
3. **Pillar 3 — Dynamic Trajectory & Behavioral Diversity**: Capsules are profiled along behavioral axes (Dead Space Ratio, New Bin Creation Ratio, Residual Capacity Variance) and dynamic trajectories ($tLDR$).
4. **Pillar 4 — MAP-Elites Quality-Diversity Archive**: An unconstrained $10 \times 10$ behavioral grid maintaining the best-performing heuristic in each unique behavioral niche, preventing premature convergence.
5. **Transversal Pillar — Cumulative Culture & "Mort et Renaissance"**: Extinction protocol evaluating whether lineages inheriting validated cultural heritage systematically outperform scratch populations across successive epochs.

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
# Bin Packing Cumulative Culture Protocol
cargo run --release --bin nomad -- rebirth --domain bin-packing --operator ollama --model qwen2.5-coder:7b --pop-size 6 -n 8 --output rebirth_report.json

# Cloud Scheduling Cumulative Culture Protocol
cargo run --release --bin nomad -- rebirth --domain scheduling --operator mock --pop-size 6 -n 4
```
*Outputs: Cumulative Improvement Factor ($CIF$), Functional Heritability ($R_{\text{herit}}$), and Innovation Rate ($R_{\text{innov}}$).*

### 3. Inspect the MAP-Elites Quality-Diversity Archive
```bash
cargo run --release --bin nomad -- archive --input run_report_qwen_v2.json
```

### 4. Export & Visualize Interactive Phylogenetic Tree (HTML / SVG)
```bash
cargo run --release --bin nomad -- graph --format html --output arbre.html
start arbre.html
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

NOMAD is verified by an automated test suite comprising **20 unit tests** and 4 macro-acceptance protocols:
- `cargo test --workspace` (All 20 unit & integration tests pass deterministically)
- Level 1-3 Anti-dead-code validation with surgical $C_{\text{null}}$ ablation
- Multi-Resource Cloud Job Scheduling (CPU/RAM vector packing)
- MAP-Elites niche competition & replacement
- Cryptographic reproducibility across random seeds

See [TESTS.md](TESTS.md) for detailed experimental benchmarks and [SPECIFICATIONS.md](SPECIFICATIONS.md) for full mathematical definitions.

---

## 6. License
Licensed under either Apache License, Version 2.0 or MIT License at your option.

