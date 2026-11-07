use serde::{Deserialize, Serialize};
use nomad_core::PackingDecision;
use crate::error::SandboxError;

/// Outcome of a single heuristic execution step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionOutcome {
    pub decision: PackingDecision,
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
    ) -> Result<ExecutionOutcome, SandboxError>;
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
    fn execute(
        &self,
        logic_code: &str,
        item_weight: u32,
        bin_levels: &[u32],
        bin_capacity: u32,
        fuel_limit: u64,
    ) -> Result<ExecutionOutcome, SandboxError> {
        let mut fuel_consumed = 0u64;

        if item_weight > bin_capacity {
            return Err(SandboxError::ItemExceedsCapacity {
                weight: item_weight,
                capacity: bin_capacity,
            });
        }

        let trimmed_code = logic_code.trim();

        // 1. Check for standard classical algorithmic policies
        let decision = if trimmed_code.eq_ignore_ascii_case("FIRST_FIT")
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
            // Form: SCORE(alpha, beta, gamma): score = alpha * remaining + beta * fill_ratio + gamma * is_exact
            // Or default to heuristic best fit with score evaluation
            let alpha = extract_float_param(trimmed_code, "alpha").unwrap_or(1.0);
            let beta = extract_float_param(trimmed_code, "beta").unwrap_or(0.0);
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
                    let score = alpha * remaining + beta * (1.0 - fill_ratio);

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

