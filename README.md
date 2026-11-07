# NOMAD (DARWIN.RS)

> **Decentralized Cognitive Swarm & Horizontal Evolutionary Recombination for LLM-Synthesized Heuristics**

[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Status](https://img.shields.io/badge/scientific_study-pre--registered-green.svg)](SPECIFICATIONS.md)

---

## 1. Overview & Scientific Purpose

NOMAD (**DARWIN.RS**) is an evolutionary computing framework in Rust designed to prove that a decentralized population of autonomous agents can generate cumulative algorithmic intelligence superior to isolated evolutionary selection.

### Core Scientific Hypotheses
* **H1 (In-Distribution Superiority):** Swarm horizontal recombination (**Swarm Crossover**) discovers bin packing heuristics that use significantly fewer bins than standard mutation-only selection (**Group C vs Group D**, $p < 0.05$).
* **H2 (Out-Of-Distribution Generalization):** Heuristics evolved via Swarm Crossover generalize superior packing efficiency on Out-Of-Distribution datasets (scale shift $N=500$ and heavy-tailed power-law distribution).

---

## 2. Workspace Architecture

```
NOMAD/
├── Cargo.toml                  # Cargo Workspace root
├── SPECIFICATIONS.md           # Full scientific specifications & pre-registration
├── crates/
│   ├── nomad-core/             # SkillCapsule, immutable contracts, metrics & configuration
│   ├── nomad-genome/           # Phylogenetic DAG modeling (PetGraph) & ancestry tracking
│   ├── nomad-sandbox/          # Deterministic execution engine with fuel/gas metering
│   ├── nomad-evaluator/        # Independent Oracle for Online 1D Bin Packing (In-Dist, OOD-1, OOD-2)
│   ├── nomad-operator/         # LLM operator abstraction (GGUF SHA-256, seed & prompt audit trail)
│   ├── nomad-orchestrator/     # Evolutionary engine, 4 experimental groups & budget enforcement
│   └── nomad-cli/              # CLI interface for runs, comparisons, and exports
└── scripts/
    └── analysis/
        └── wilcoxon_analysis.py # Paired Wilcoxon Signed-Rank statistical test & effect size
```

---

## 3. Quickstart & CLI Usage

### Build the Project
```bash
cargo build --release
```

### Inspect Benchmark Datasets & Cryptographic Fingerprints
```bash
cargo run --bin nomad -- dataset
```

### Run Classical Heuristic Baselines (First-Fit, Best-Fit, Worst-Fit)
```bash
cargo run --bin nomad -- baseline
```

### Run an Evolutionary Trial
```bash
# Run Group C (NOMAD Swarm with Crossover)
cargo run --bin nomad -- run --group C --pop-size 20 --generations 10 --seed 42

# Run Group D (Evolutionary Selection with Mutation Only)
cargo run --bin nomad -- run --group D --pop-size 20 --generations 10 --seed 42
```

### Run Paired Study (Group C vs Group D) & Statistical Analysis
```bash
cargo run --bin nomad -- compare --runs 5 --pop-size 20 --generations 10 --output comparison.json
python scripts/analysis/wilcoxon_analysis.py comparison.json
```

---

## 4. Experimental Groups (Protocol)

| Group | Description | Population | Transmission Mechanism |
|---|---|---|---|
| **Group A** | Baseline sequential self-correction | 1 agent | Sequential iteration (no swarm) |
| **Group B** | Parallel independent agents | 100 agents | Independent mutations (0 transmission) |
| **Group D** | Evolutionary Selection | 100 agents | Selection top 20%, **Mutation only** |
| **Group C** | **NOMAD Swarm** | 100 agents | Selection top 20%, **Mutation + Swarm Crossover** |

---

## 5. License
Licensed under either Apache License, Version 2.0 or MIT License at your option.
