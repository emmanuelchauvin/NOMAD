use serde::{Deserialize, Serialize};
use nomad_core::{PackingDecision, SchedulingDecision, SchedulingInputs};
use crate::error::SandboxError;
use crate::eval_ast::{AstInterpreter, ScorePolicyContext};

/// Outcome of a single heuristic execution step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionOutcome {
    pub decision: PackingDecision,
    pub fuel_consumed: u64,
}

/// Outcome of a single scheduling heuristic execution step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulingExecutionOutcome {
    pub decision: SchedulingDecision,
    pub fuel_consumed: u64,
}

/// Interface for executing a heuristic within a secure and resource-metered environment.
pub trait HeuristicSandbox: Send + Sync {
    fn execute(
        &self,
        logic_code: &str,
        item_weight: u32,
        bin_levels: &[u32],
        bin_capacity: u32,
        fuel_limit: u64,
    ) -> Result<ExecutionOutcome, SandboxError> {
        self.execute_with_context(
            logic_code,
            item_weight,
            bin_levels,
            bin_capacity,
            fuel_limit,
            0,
            50,
        )
    }

    fn execute_with_context(
        &self,
        logic_code: &str,
        item_weight: u32,
        bin_levels: &[u32],
        bin_capacity: u32,
        fuel_limit: u64,
        step_index: usize,
        total_items: usize,
    ) -> Result<ExecutionOutcome, SandboxError> {
        self.execute_with_lookahead(
            logic_code,
            item_weight,
            0,
            bin_levels,
            bin_capacity,
            fuel_limit,
            step_index,
            total_items,
        )
    }

    fn execute_with_lookahead(
        &self,
        logic_code: &str,
        item_weight: u32,
        next_item_weight: u32,
        bin_levels: &[u32],
        bin_capacity: u32,
        fuel_limit: u64,
        step_index: usize,
        total_items: usize,
    ) -> Result<ExecutionOutcome, SandboxError>;

    fn execute_scheduling(
        &self,
        logic_code: &str,
        job_cpu: u32,
        job_ram: u32,
        node_levels: &[(u32, u32)],
        node_capacity: (u32, u32),
        fuel_limit: u64,
        step_index: usize,
        total_jobs: usize,
    ) -> Result<SchedulingExecutionOutcome, SandboxError> {
        self.execute_scheduling_with_lookahead(
            logic_code,
            job_cpu,
            job_ram,
            0,
            0,
            node_levels,
            node_capacity,
            fuel_limit,
            step_index,
            total_jobs,
        )
    }

    fn execute_scheduling_with_lookahead(
        &self,
        logic_code: &str,
        job_cpu: u32,
        job_ram: u32,
        next_job_cpu: u32,
        next_job_ram: u32,
        node_levels: &[(u32, u32)],
        node_capacity: (u32, u32),
        fuel_limit: u64,
        step_index: usize,
        total_jobs: usize,
    ) -> Result<SchedulingExecutionOutcome, SandboxError>;
}

/// Deterministic, zero-dependency sandboxed evaluation engine with instruction-level fuel metering.
#[derive(Debug, Default, Clone)]
pub struct DeterministicSandbox;

impl DeterministicSandbox {
    pub fn new() -> Self {
        Self
    }
}

impl HeuristicSandbox for DeterministicSandbox {
    fn execute_with_lookahead(
        &self,
        logic_code: &str,
        item_weight: u32,
        next_item_weight: u32,
        bin_levels: &[u32],
        bin_capacity: u32,
        fuel_limit: u64,
        step_index: usize,
        total_items: usize,
    ) -> Result<ExecutionOutcome, SandboxError> {
        let mut fuel_consumed = 0u64;

        if item_weight > bin_capacity {
            return Err(SandboxError::ItemExceedsCapacity {
                weight: item_weight,
                capacity: bin_capacity,
            });
        }

        let trimmed_code = logic_code.trim();

        // 0. Check for V2 Rust AST score_bin function
        let is_score_bin = trimmed_code.contains("fn score_bin") || trimmed_code.contains("score_bin(");
        let parsed_fn = if is_score_bin {
            syn::parse_str::<syn::ItemFn>(trimmed_code)
                .or_else(|_| {
                    syn::parse_file(trimmed_code).map(|f| {
                        for item in f.items {
                            if let syn::Item::Fn(func) = item {
                                if func.sig.ident == "score_bin" {
                                    return func;
                                }
                            }
                        }
                        syn::ItemFn {
                            attrs: Vec::new(),
                            vis: syn::Visibility::Inherited,
                            sig: syn::parse_str("fn score_bin()").unwrap(),
                            block: Box::new(syn::Block {
                                brace_token: Default::default(),
                                stmts: Vec::new(),
                            }),
                        }
                    })
                })
                .ok()
        } else {
            None
        };

        let decision = if let Some(item_fn) = parsed_fn.filter(|f| f.sig.ident == "score_bin") {
            let interpreter = AstInterpreter::new(&item_fn, fuel_limit);
            let total_residual: u32 = bin_levels
                .iter()
                .map(|&l| bin_capacity.saturating_sub(l))
                .sum();
            let mean_residual = if !bin_levels.is_empty() {
                total_residual as f64 / bin_levels.len() as f64
            } else {
                0.0
            };

            let mut best_idx = None;
            let mut best_score = f64::MAX;
            let fitting_bins_count = bin_levels.iter().filter(|&&lvl| lvl + item_weight <= bin_capacity).count();

            for (idx, &level) in bin_levels.iter().enumerate() {
                fuel_consumed += 10;
                if fuel_consumed > fuel_limit {
                    return Err(SandboxError::FuelLimitExceeded {
                        fuel_consumed,
                        fuel_limit,
                    });
                }

                if level + item_weight <= bin_capacity {
                    let current_residual = bin_capacity - level;
                    let post_residual = bin_capacity - (level + item_weight);
                    let ctx = ScorePolicyContext {
                        item_size: item_weight,
                        next_item_size: next_item_weight,
                        bin_residual: current_residual,
                        bin_capacity,
                        step_index,
                        total_items,
                        open_bins_count: bin_levels.len(),
                        mean_residual,
                        post_residual,
                        fitting_bins_count,
                    };
                    let outcome = interpreter
                        .eval(&ctx)
                        .map_err(|e| SandboxError::ExecutionCrash(e.to_string()))?;
                    fuel_consumed += outcome.fuel_consumed;
                    if fuel_consumed > fuel_limit {
                        return Err(SandboxError::FuelLimitExceeded {
                            fuel_consumed,
                            fuel_limit,
                        });
                    }

                    if outcome.score < best_score {
                        best_score = outcome.score;
                        best_idx = Some(idx);
                    }
                }
            }

            match best_idx {
                Some(idx) => PackingDecision::PlaceInExisting(idx),
                None => PackingDecision::OpenNewBin,
            }
        } else if trimmed_code.eq_ignore_ascii_case("FIRST_FIT")
            || trimmed_code.contains("first_fit")
            || trimmed_code.contains("FirstFit")
        {
            // First Fit: first bin where item fits
            let mut chosen = None;
            for (idx, &level) in bin_levels.iter().enumerate() {
                fuel_consumed += 10;
                if fuel_consumed > fuel_limit {
                    return Err(SandboxError::FuelLimitExceeded {
                        fuel_consumed,
                        fuel_limit,
                    });
                }
                if level + item_weight <= bin_capacity {
                    chosen = Some(idx);
                    break;
                }
            }
            match chosen {
                Some(idx) => PackingDecision::PlaceInExisting(idx),
                None => PackingDecision::OpenNewBin,
            }
        } else if trimmed_code.eq_ignore_ascii_case("BEST_FIT")
            || trimmed_code.contains("best_fit")
            || trimmed_code.contains("BestFit")
        {
            // Best Fit: bin with minimum remaining space that fits item
            let mut best_idx = None;
            let mut min_remaining = u32::MAX;

            for (idx, &level) in bin_levels.iter().enumerate() {
                fuel_consumed += 15;
                if fuel_consumed > fuel_limit {
                    return Err(SandboxError::FuelLimitExceeded {
                        fuel_consumed,
                        fuel_limit,
                    });
                }
                if level + item_weight <= bin_capacity {
                    let remaining = bin_capacity - (level + item_weight);
                    if remaining < min_remaining {
                        min_remaining = remaining;
                        best_idx = Some(idx);
                    }
                }
            }
            match best_idx {
                Some(idx) => PackingDecision::PlaceInExisting(idx),
                None => PackingDecision::OpenNewBin,
            }
        } else if trimmed_code.eq_ignore_ascii_case("WORST_FIT")
            || trimmed_code.contains("worst_fit")
            || trimmed_code.contains("WorstFit")
        {
            // Worst Fit: bin with maximum remaining space that fits item
            let mut worst_idx = None;
            let mut max_remaining = 0u32;

            for (idx, &level) in bin_levels.iter().enumerate() {
                fuel_consumed += 15;
                if fuel_consumed > fuel_limit {
                    return Err(SandboxError::FuelLimitExceeded {
                        fuel_consumed,
                        fuel_limit,
                    });
                }
                if level + item_weight <= bin_capacity {
                    let remaining = bin_capacity - (level + item_weight);
                    if remaining >= max_remaining {
                        max_remaining = remaining;
                        worst_idx = Some(idx);
                    }
                }
            }
            match worst_idx {
                Some(idx) => PackingDecision::PlaceInExisting(idx),
                None => PackingDecision::OpenNewBin,
            }
        } else if trimmed_code.contains("HARMONIC") || trimmed_code.contains("harmonic") {
            // Harmonic grouping: item size intervals
            let is_large = item_weight > bin_capacity / 2;
            let mut selected = None;

            for (idx, &level) in bin_levels.iter().enumerate() {
                fuel_consumed += 20;
                if fuel_consumed > fuel_limit {
                    return Err(SandboxError::FuelLimitExceeded {
                        fuel_consumed,
                        fuel_limit,
                    });
                }
                if level + item_weight <= bin_capacity {
                    if is_large && level == 0 {
                        selected = Some(idx);
                        break;
                    } else if !is_large && (level + item_weight) >= (bin_capacity * 8 / 10) {
                        selected = Some(idx);
                        break;
                    } else if selected.is_none() {
                        selected = Some(idx);
                    }
                }
            }
            match selected {
                Some(idx) => PackingDecision::PlaceInExisting(idx),
                None => PackingDecision::OpenNewBin,
            }
        } else {
            // Parameterized scoring function parser
            // Form: SCORE: alpha=<val> beta=<val> dead_space_penalty=<val> exact_fit_bonus=<val> tight_fit=<bool>
            let alpha = extract_float_param(trimmed_code, "alpha").unwrap_or(1.0);
            let beta = extract_float_param(trimmed_code, "beta").unwrap_or(0.0);
            let dead_space_penalty = extract_float_param(trimmed_code, "dead_space_penalty")
                .or_else(|| extract_float_param(trimmed_code, "waste_penalty"))
                .unwrap_or(0.0);
            let exact_fit_bonus = extract_float_param(trimmed_code, "exact_fit_bonus")
                .or_else(|| extract_float_param(trimmed_code, "closure_bonus"))
                .unwrap_or(0.0);
            let prefer_tight = extract_bool_param(trimmed_code, "tight_fit").unwrap_or(true);

            let mut best_idx = None;
            let mut best_score = if prefer_tight { f64::MAX } else { f64::MIN };

            for (idx, &level) in bin_levels.iter().enumerate() {
                fuel_consumed += 25;
                if fuel_consumed > fuel_limit {
                    return Err(SandboxError::FuelLimitExceeded {
                        fuel_consumed,
                        fuel_limit,
                    });
                }
                if level + item_weight <= bin_capacity {
                    let remaining = (bin_capacity - (level + item_weight)) as f64;
                    let fill_ratio = (level + item_weight) as f64 / bin_capacity as f64;
                    let mut score = alpha * remaining + beta * (1.0 - fill_ratio);

                    // Penalize dead space: items are >= 10, so any gap < 10 can never be filled
                    if remaining > 0.0 && remaining < 10.0 {
                        score += dead_space_penalty;
                    }
                    // Reward exact 100% bin closure
                    if remaining == 0.0 {
                        score -= exact_fit_bonus;
                    }

                    if prefer_tight {
                        if score < best_score {
                            best_score = score;
                            best_idx = Some(idx);
                        }
                    } else {
                        if score > best_score {
                            best_score = score;
                            best_idx = Some(idx);
                        }
                    }
                }
            }

            match best_idx {
                Some(idx) => PackingDecision::PlaceInExisting(idx),
                None => PackingDecision::OpenNewBin,
            }
        };

        // Strict verification of validity of returned decision
        match decision {
            PackingDecision::PlaceInExisting(idx) => {
                if idx >= bin_levels.len() {
                    return Err(SandboxError::InvalidBinIndex {
                        index: idx,
                        open_bins: bin_levels.len(),
                    });
                }
                let current_level = bin_levels[idx];
                if current_level + item_weight > bin_capacity {
                    return Err(SandboxError::CapacityOverflow {
                        index: idx,
                        current_level,
                        weight: item_weight,
                        capacity: bin_capacity,
                    });
                }
            }
            PackingDecision::OpenNewBin => {}
        }

        Ok(ExecutionOutcome {
            decision,
            fuel_consumed,
        })
    }

    fn execute_scheduling_with_lookahead(
        &self,
        logic_code: &str,
        job_cpu: u32,
        job_ram: u32,
        next_job_cpu: u32,
        next_job_ram: u32,
        node_levels: &[(u32, u32)],
        node_capacity: (u32, u32),
        fuel_limit: u64,
        step_index: usize,
        total_jobs: usize,
    ) -> Result<SchedulingExecutionOutcome, SandboxError> {
        let mut fuel_consumed = 0u64;

        if job_cpu > node_capacity.0 || job_ram > node_capacity.1 {
            return Err(SandboxError::ItemExceedsCapacity {
                weight: job_cpu.max(job_ram),
                capacity: node_capacity.0.max(node_capacity.1),
            });
        }

        let trimmed_code = logic_code.trim();

        // 1. Classical baseline shortcuts
        if trimmed_code == "FIRST_FIT_NODE" {
            for (idx, &(used_cpu, used_ram)) in node_levels.iter().enumerate() {
                fuel_consumed += 5;
                if used_cpu + job_cpu <= node_capacity.0 && used_ram + job_ram <= node_capacity.1 {
                    return Ok(SchedulingExecutionOutcome {
                        decision: SchedulingDecision::PlaceOnNode(idx),
                        fuel_consumed,
                    });
                }
            }
            return Ok(SchedulingExecutionOutcome {
                decision: SchedulingDecision::OpenNewNode,
                fuel_consumed,
            });
        }

        if trimmed_code == "BEST_FIT_CPU" {
            let mut best_idx = None;
            let mut min_res_cpu = u32::MAX;
            for (idx, &(used_cpu, used_ram)) in node_levels.iter().enumerate() {
                fuel_consumed += 5;
                if used_cpu + job_cpu <= node_capacity.0 && used_ram + job_ram <= node_capacity.1 {
                    let res_cpu = node_capacity.0 - (used_cpu + job_cpu);
                    if res_cpu < min_res_cpu {
                        min_res_cpu = res_cpu;
                        best_idx = Some(idx);
                    }
                }
            }
            let decision = best_idx.map(SchedulingDecision::PlaceOnNode).unwrap_or(SchedulingDecision::OpenNewNode);
            return Ok(SchedulingExecutionOutcome { decision, fuel_consumed });
        }

        if trimmed_code == "BEST_FIT_RAM" {
            let mut best_idx = None;
            let mut min_res_ram = u32::MAX;
            for (idx, &(used_cpu, used_ram)) in node_levels.iter().enumerate() {
                fuel_consumed += 5;
                if used_cpu + job_cpu <= node_capacity.0 && used_ram + job_ram <= node_capacity.1 {
                    let res_ram = node_capacity.1 - (used_ram + job_ram);
                    if res_ram < min_res_ram {
                        min_res_ram = res_ram;
                        best_idx = Some(idx);
                    }
                }
            }
            let decision = best_idx.map(SchedulingDecision::PlaceOnNode).unwrap_or(SchedulingDecision::OpenNewNode);
            return Ok(SchedulingExecutionOutcome { decision, fuel_consumed });
        }

        if trimmed_code == "BALANCED_FIT" {
            let mut best_idx = None;
            let mut min_norm = f64::MAX;
            for (idx, &(used_cpu, used_ram)) in node_levels.iter().enumerate() {
                fuel_consumed += 10;
                if used_cpu + job_cpu <= node_capacity.0 && used_ram + job_ram <= node_capacity.1 {
                    let res_cpu_norm = (node_capacity.0 - (used_cpu + job_cpu)) as f64 / node_capacity.0 as f64;
                    let res_ram_norm = (node_capacity.1 - (used_ram + job_ram)) as f64 / node_capacity.1 as f64;
                    let norm = (res_cpu_norm * res_cpu_norm + res_ram_norm * res_ram_norm).sqrt();
                    if norm < min_norm {
                        min_norm = norm;
                        best_idx = Some(idx);
                    }
                }
            }
            let decision = best_idx.map(SchedulingDecision::PlaceOnNode).unwrap_or(SchedulingDecision::OpenNewNode);
            return Ok(SchedulingExecutionOutcome { decision, fuel_consumed });
        }

        // 2. Synthesized AST evaluation via score_node
        let parsed_fn = syn::parse_str::<syn::ItemFn>(trimmed_code)
            .or_else(|_| {
                syn::parse_file(trimmed_code).map(|f| {
                    for item in f.items {
                        if let syn::Item::Fn(func) = item {
                            if func.sig.ident == "score_node" {
                                return func;
                            }
                        }
                    }
                    syn::ItemFn {
                        attrs: Vec::new(),
                        vis: syn::Visibility::Inherited,
                        sig: syn::parse_str("fn score_node()").unwrap(),
                        block: Box::new(syn::Block {
                            brace_token: Default::default(),
                            stmts: Vec::new(),
                        }),
                    }
                })
            })
            .ok();

        let decision = if let Some(item_fn) = parsed_fn.filter(|f| f.sig.ident == "score_node") {
            let interpreter = AstInterpreter::new(&item_fn, fuel_limit);
            let total_res_cpu: u32 = node_levels.iter().map(|&(u, _)| node_capacity.0.saturating_sub(u)).sum();
            let total_res_ram: u32 = node_levels.iter().map(|&(_, r)| node_capacity.1.saturating_sub(r)).sum();
            let n_nodes = node_levels.len().max(1) as f64;
            let mean_cpu_residual = total_res_cpu as f64 / n_nodes;
            let mean_ram_residual = total_res_ram as f64 / n_nodes;

            let mut best_idx = None;
            let mut best_score = f64::MAX;

            for (idx, &(used_cpu, used_ram)) in node_levels.iter().enumerate() {
                fuel_consumed += 10;
                if fuel_consumed > fuel_limit {
                    return Err(SandboxError::FuelLimitExceeded { fuel_consumed, fuel_limit });
                }

                if used_cpu + job_cpu <= node_capacity.0 && used_ram + job_ram <= node_capacity.1 {
                    let res_cpu = node_capacity.0 - (used_cpu + job_cpu);
                    let res_ram = node_capacity.1 - (used_ram + job_ram);
                    let ctx = SchedulingInputs {
                        job_cpu,
                        job_ram,
                        next_job_cpu,
                        next_job_ram,
                        node_residual_cpu: res_cpu,
                        node_residual_ram: res_ram,
                        node_capacity_cpu: node_capacity.0,
                        node_capacity_ram: node_capacity.1,
                        step_index,
                        total_jobs,
                        active_nodes_count: node_levels.len(),
                        mean_cpu_residual,
                        mean_ram_residual,
                    };
                    let outcome = interpreter.eval_scheduling(&ctx)
                        .map_err(|e| SandboxError::ExecutionCrash(e.to_string()))?;
                    fuel_consumed += outcome.fuel_consumed;
                    if fuel_consumed > fuel_limit {
                        return Err(SandboxError::FuelLimitExceeded { fuel_consumed, fuel_limit });
                    }
                    if outcome.score < best_score {
                        best_score = outcome.score;
                        best_idx = Some(idx);
                    }
                }
            }

            match best_idx {
                Some(idx) => SchedulingDecision::PlaceOnNode(idx),
                None => SchedulingDecision::OpenNewNode,
            }
        } else {
            // Fallback: first fit
            let mut found = None;
            for (idx, &(used_cpu, used_ram)) in node_levels.iter().enumerate() {
                if used_cpu + job_cpu <= node_capacity.0 && used_ram + job_ram <= node_capacity.1 {
                    found = Some(idx);
                    break;
                }
            }
            match found {
                Some(idx) => SchedulingDecision::PlaceOnNode(idx),
                None => SchedulingDecision::OpenNewNode,
            }
        };

        // Safety verification
        if let SchedulingDecision::PlaceOnNode(idx) = decision {
            if idx >= node_levels.len() {
                return Err(SandboxError::InvalidBinIndex { index: idx, open_bins: node_levels.len() });
            }
            let (u_cpu, u_ram) = node_levels[idx];
            if u_cpu + job_cpu > node_capacity.0 || u_ram + job_ram > node_capacity.1 {
                return Err(SandboxError::CapacityOverflow {
                    index: idx,
                    current_level: u_cpu.max(u_ram),
                    weight: job_cpu.max(job_ram),
                    capacity: node_capacity.0.max(node_capacity.1),
                });
            }
        }

        Ok(SchedulingExecutionOutcome { decision, fuel_consumed })
    }
}

fn extract_float_param(text: &str, param_name: &str) -> Option<f64> {
    for line in text.lines() {
        if let Some(pos) = line.find(param_name) {
            let rest = &line[pos + param_name.len()..];
            if let Some(eq_pos) = rest.find('=') {
                let val_str = rest[eq_pos + 1..].trim();
                let num_str: String = val_str
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
                    .collect();
                if let Ok(v) = num_str.parse::<f64>() {
                    return Some(v);
                }
            }
        }
    }
    None
}

fn extract_bool_param(text: &str, param_name: &str) -> Option<bool> {
    for line in text.lines() {
        if let Some(pos) = line.find(param_name) {
            let rest = &line[pos + param_name.len()..];
            if rest.contains("true") {
                return Some(true);
            }
            if rest.contains("false") {
                return Some(false);
            }
        }
    }
    None
}

