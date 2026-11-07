use nomad_core::{BenchmarkScore, InstanceEvaluation, PackingDecision, SkillCapsule};
use nomad_sandbox::{DeterministicSandbox, HeuristicSandbox};
use crate::dataset::{BinPackingInstance, Dataset};

/// The Independent Oracle for the NOMAD scientific benchmarks.
///
/// Fully decoupled from agents, evaluating heuristics in isolation and enforcing
/// zero metric exploitation.
#[derive(Debug, Clone)]
pub struct OracleEvaluator {
    pub in_distribution: Dataset,
    pub ood1_scale: Dataset,
    pub ood2_powerlaw: Dataset,
}

impl OracleEvaluator {
    /// Creates an oracle with pre-registered standard datasets using a fixed benchmark seed.
    pub fn new(seed: u64) -> Self {
        Self {
            in_distribution: Dataset::generate_in_distribution(seed),
            ood1_scale: Dataset::generate_ood1_scale(seed),
            ood2_powerlaw: Dataset::generate_ood2_distribution_shift(seed),
        }
    }

    /// Evaluates a SkillCapsule against the training In-Distribution benchmark.
    pub fn evaluate_in_distribution(
        &self,
        capsule: &SkillCapsule,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
    ) -> BenchmarkScore {
        self.evaluate_dataset(&self.in_distribution, capsule, sandbox, fuel_limit_per_call)
    }

    /// Evaluates a SkillCapsule against the OOD-1 (Scale) benchmark.
    pub fn evaluate_ood1(
        &self,
        capsule: &SkillCapsule,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
    ) -> BenchmarkScore {
        self.evaluate_dataset(&self.ood1_scale, capsule, sandbox, fuel_limit_per_call)
    }

    /// Evaluates a SkillCapsule against the OOD-2 (Distribution Shift) benchmark.
    pub fn evaluate_ood2(
        &self,
        capsule: &SkillCapsule,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
    ) -> BenchmarkScore {
        self.evaluate_dataset(&self.ood2_powerlaw, capsule, sandbox, fuel_limit_per_call)
    }

    /// Evaluates a heuristic across an entire dataset.
    pub fn evaluate_dataset(
        &self,
        dataset: &Dataset,
        capsule: &SkillCapsule,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
    ) -> BenchmarkScore {
        let mut total_bins = 0usize;
        let mut total_theoretical = 0usize;
        let mut total_fuel = 0u64;
        let mut solved_instances = 0usize;
        let mut sum_efficiency = 0.0f64;

        for inst in &dataset.instances {
            let eval = self.evaluate_single_instance(inst, &capsule.logic_code, sandbox, fuel_limit_per_call);
            if !eval.is_valid {
                // Fatal penalty: Any failure disqualifies the entire run on this dataset
                return BenchmarkScore::fatal_penalty(dataset.instances.len());
            }

            total_bins += eval.bins_used;
            total_theoretical += eval.theoretical_min_bins;
            total_fuel += eval.fuel_consumed;
            sum_efficiency += eval.efficiency_ratio();
            solved_instances += 1;
        }

        let mean_efficiency = if solved_instances > 0 {
            sum_efficiency / solved_instances as f64
        } else {
            0.0
        };

        BenchmarkScore {
            is_valid: true,
            total_bins,
            total_theoretical_min_bins: total_theoretical,
            total_excess_bins: total_bins.saturating_sub(total_theoretical),
            mean_efficiency,
            total_fuel,
            solved_instances,
            total_instances: dataset.instances.len(),
        }
    }

    /// Evaluates a single online bin packing instance sequentially.
    pub fn evaluate_single_instance(
        &self,
        instance: &BinPackingInstance,
        logic_code: &str,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
    ) -> InstanceEvaluation {
        let mut bin_levels: Vec<u32> = Vec::new();
        let mut total_fuel = 0u64;

        for &item in &instance.items {
            let res = sandbox.execute(
                logic_code,
                item,
                &bin_levels,
                instance.capacity,
                fuel_limit_per_call,
            );

            match res {
                Ok(outcome) => {
                    total_fuel = total_fuel.saturating_add(outcome.fuel_consumed);
                    match outcome.decision {
                        PackingDecision::PlaceInExisting(idx) => {
                            if idx >= bin_levels.len() {
                                return InstanceEvaluation {
                                    instance_name: instance.name.clone(),
                                    is_valid: false,
                                    bins_used: 0,
                                    theoretical_min_bins: instance.theoretical_min_bins,
                                    fuel_consumed: total_fuel,
                                    error_message: Some(format!(
                                        "Invalid bin index {} for {} open bins",
                                        idx,
                                        bin_levels.len()
                                    )),
                                };
                            }
                            if bin_levels[idx] + item > instance.capacity {
                                return InstanceEvaluation {
                                    instance_name: instance.name.clone(),
                                    is_valid: false,
                                    bins_used: 0,
                                    theoretical_min_bins: instance.theoretical_min_bins,
                                    fuel_consumed: total_fuel,
                                    error_message: Some(format!(
                                        "Capacity exceeded in bin {}: {} + {} > {}",
                                        idx, bin_levels[idx], item, instance.capacity
                                    )),
                                };
                            }
                            bin_levels[idx] += item;
                        }
                        PackingDecision::OpenNewBin => {
                            if item > instance.capacity {
                                return InstanceEvaluation {
                                    instance_name: instance.name.clone(),
                                    is_valid: false,
                                    bins_used: 0,
                                    theoretical_min_bins: instance.theoretical_min_bins,
                                    fuel_consumed: total_fuel,
                                    error_message: Some(format!(
                                        "Item {} exceeds capacity {}",
                                        item, instance.capacity
                                    )),
                                };
                            }
                            bin_levels.push(item);
                        }
                    }
                }
                Err(err) => {
                    return InstanceEvaluation {
                        instance_name: instance.name.clone(),
                        is_valid: false,
                        bins_used: 0,
                        theoretical_min_bins: instance.theoretical_min_bins,
                        fuel_consumed: total_fuel,
                        error_message: Some(err.to_string()),
                    };
                }
            }
        }

        InstanceEvaluation {
            instance_name: instance.name.clone(),
            is_valid: true,
            bins_used: bin_levels.len(),
            theoretical_min_bins: instance.theoretical_min_bins,
            fuel_consumed: total_fuel,
            error_message: None,
        }
    }

    /// Evaluates a baseline classical heuristic (e.g. "FIRST_FIT", "BEST_FIT", "WORST_FIT").
    pub fn evaluate_classical_baseline(
        &self,
        heuristic_name: &str,
        dataset: &Dataset,
    ) -> BenchmarkScore {
        let sandbox = DeterministicSandbox::new();
        let capsule = SkillCapsule::new(
            heuristic_name,
            format!("Baseline heuristic {}", heuristic_name),
            heuristic_name,
            vec![],
            0,
            std::collections::HashMap::new(),
        );
        self.evaluate_dataset(dataset, &capsule, &sandbox, 100_000)
    }
}
