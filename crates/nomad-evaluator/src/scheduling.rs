use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::anti_dead_code::create_ablation_capsule_res;
use nomad_core::{
    BehaviorDescriptor, BenchmarkScore, InstanceEvaluation, SchedulingDecision, SkillCapsule,
    ValidationStatus,
};
use nomad_genome::verify_level1_structural_survival;
use nomad_sandbox::HeuristicSandbox;

/// ============================================================================
/// JobRequest : Requête de Tâche de Calcul Multi-Ressources
/// ============================================================================
/// Représente un job ou conteneur demandant simultanément du CPU et de la RAM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobRequest {
    pub job_id: usize,
    pub cpu_req: u32,
    pub ram_req: u32,
}

/// ============================================================================
/// SchedulingInstance : Instance de Test d'Allocation de Ressources Cloud
/// ============================================================================
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulingInstance {
    pub name: String,
    pub jobs: Vec<JobRequest>,
    pub node_capacity_cpu: u32,
    pub node_capacity_ram: u32,
}

impl SchedulingInstance {
    /// Borne inférieure théorique multidimensionnelle sur le nombre de machines :
    /// L_sched = max( ceil(sum(cpu) / cap_cpu), ceil(sum(ram) / cap_ram) )
    pub fn theoretical_min_nodes(&self) -> usize {
        let total_cpu: u64 = self.jobs.iter().map(|j| j.cpu_req as u64).sum();
        let total_ram: u64 = self.jobs.iter().map(|j| j.ram_req as u64).sum();

        let min_cpu = ((total_cpu as f64) / (self.node_capacity_cpu as f64)).ceil() as usize;
        let min_ram = ((total_ram as f64) / (self.node_capacity_ram as f64)).ceil() as usize;
        min_cpu.max(min_ram).max(1)
    }
}

/// ============================================================================
/// SchedulingDataset : Jeu de Données Sanctuarisé pour le Scheduling
/// ============================================================================
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulingDataset {
    pub dataset_name: String,
    pub instances: Vec<SchedulingInstance>,
    pub dataset_hash: String,
}

impl SchedulingDataset {
    /// Génère le jeu de données d'entraînement In-Distribution :
    /// 50 instances de 50 tâches hétérogènes (CPU 1..=8, RAM 2..=32, nœuds 32 cores / 128 GB).
    pub fn generate_in_distribution(seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut instances = Vec::with_capacity(50);

        for inst_idx in 0..50 {
            let mut jobs = Vec::with_capacity(50);
            for job_id in 0..50 {
                let cpu_req = rng.gen_range(1..=8);
                let ram_req = rng.gen_range(2..=32);
                jobs.push(JobRequest {
                    job_id,
                    cpu_req,
                    ram_req,
                });
            }
            instances.push(SchedulingInstance {
                name: format!("InDist-Sched-{:03}", inst_idx),
                jobs,
                node_capacity_cpu: 32,
                node_capacity_ram: 128,
            });
        }

        let dataset_hash = Self::compute_hash(&instances);
        Self {
            dataset_name: "In-Distribution (Multi-Resource Scheduling)".to_string(),
            instances,
            dataset_hash,
        }
    }

    /// Pre-registered In-Distribution Validation Dataset :
    /// 50 instances de 50 tâches hétérogènes (seed dérivée distincte).
    pub fn generate_validation(seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0x005A_11DA_710E_u64);
        let mut instances = Vec::with_capacity(50);

        for inst_idx in 0..50 {
            let mut jobs = Vec::with_capacity(50);
            for job_id in 0..50 {
                let cpu_req = rng.gen_range(1..=8);
                let ram_req = rng.gen_range(2..=32);
                jobs.push(JobRequest {
                    job_id,
                    cpu_req,
                    ram_req,
                });
            }
            instances.push(SchedulingInstance {
                name: format!("Val-Sched-{:03}", inst_idx),
                jobs,
                node_capacity_cpu: 32,
                node_capacity_ram: 128,
            });
        }

        let dataset_hash = Self::compute_hash(&instances);
        Self {
            dataset_name: "In-Distribution Validation (Multi-Resource Scheduling)".to_string(),
            instances,
            dataset_hash,
        }
    }

    /// Benchmark Hors-Distribution OOD-1 : Mise à l'échelle (50 instances de 300 tâches).
    pub fn generate_ood_scale(seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0xFEEDFACE);
        let mut instances = Vec::with_capacity(50);

        for inst_idx in 0..50 {
            let mut jobs = Vec::with_capacity(300);
            for job_id in 0..300 {
                let cpu_req = rng.gen_range(1..=8);
                let ram_req = rng.gen_range(2..=32);
                jobs.push(JobRequest {
                    job_id,
                    cpu_req,
                    ram_req,
                });
            }
            instances.push(SchedulingInstance {
                name: format!("OOD1-Scale-Sched-{:03}", inst_idx),
                jobs,
                node_capacity_cpu: 32,
                node_capacity_ram: 128,
            });
        }

        let dataset_hash = Self::compute_hash(&instances);
        Self {
            dataset_name: "OOD-1 Scale (N=300 Jobs)".to_string(),
            instances,
            dataset_hash,
        }
    }

    /// Benchmark Hors-Distribution OOD-2 : Tâches asymétriques et déséquilibre sévère.
    pub fn generate_ood_skewed(seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0xCAFEBABE);
        let mut instances = Vec::with_capacity(50);

        for inst_idx in 0..50 {
            let mut jobs = Vec::with_capacity(100);
            for job_id in 0..100 {
                let (cpu_req, ram_req) = if rng.gen_bool(0.5) {
                    // CPU-heavy, RAM-light
                    (rng.gen_range(8..=24), rng.gen_range(2..=8))
                } else {
                    // RAM-heavy, CPU-light
                    (rng.gen_range(1..=4), rng.gen_range(32..=96))
                };
                jobs.push(JobRequest {
                    job_id,
                    cpu_req,
                    ram_req,
                });
            }
            instances.push(SchedulingInstance {
                name: format!("OOD2-Skewed-Sched-{:03}", inst_idx),
                jobs,
                node_capacity_cpu: 32,
                node_capacity_ram: 128,
            });
        }

        let dataset_hash = Self::compute_hash(&instances);
        Self {
            dataset_name: "OOD-2 Asymmetric/Skewed Resource Profiles".to_string(),
            instances,
            dataset_hash,
        }
    }

    fn compute_hash(instances: &[SchedulingInstance]) -> String {
        let mut hasher = Sha256::new();
        for inst in instances {
            hasher.update(inst.name.as_bytes());
            for job in &inst.jobs {
                hasher.update(job.cpu_req.to_le_bytes());
                hasher.update(job.ram_req.to_le_bytes());
            }
        }
        hex::encode(hasher.finalize())
    }
}

/// ============================================================================
/// SchedulingOracle : Oracle Souverain pour l'Ordonnancement Cloud
/// ============================================================================
pub struct SchedulingOracle {
    pub in_distribution: SchedulingDataset,
    pub validation: SchedulingDataset,
    pub ood1_scale: SchedulingDataset,
    pub ood2_skewed: SchedulingDataset,
}

impl SchedulingOracle {
    pub fn new(seed: u64) -> Self {
        Self {
            in_distribution: SchedulingDataset::generate_in_distribution(seed),
            validation: SchedulingDataset::generate_validation(seed),
            ood1_scale: SchedulingDataset::generate_ood_scale(seed),
            ood2_skewed: SchedulingDataset::generate_ood_skewed(seed),
        }
    }

    /// Évalue une capsule sur le jeu de validation In-Distribution.
    pub fn evaluate_validation(
        &self,
        capsule: &SkillCapsule,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
    ) -> BenchmarkScore {
        self.evaluate_dataset(&self.validation, capsule, sandbox, fuel_limit_per_call)
    }

    /// Évalue une instance unique de placement de tâches.
    pub fn evaluate_instance(
        &self,
        instance: &SchedulingInstance,
        capsule: &SkillCapsule,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
    ) -> (InstanceEvaluation, f64, f64) {
        self.evaluate_instance_with_lookahead(instance, capsule, sandbox, fuel_limit_per_call, true)
    }

    /// Évalue une instance unique de placement de tâches avec option de lookahead.
    pub fn evaluate_instance_with_lookahead(
        &self,
        instance: &SchedulingInstance,
        capsule: &SkillCapsule,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
        lookahead: bool,
    ) -> (InstanceEvaluation, f64, f64) {
        let mut node_levels: Vec<(u32, u32)> = Vec::new();
        let mut total_fuel = 0u64;
        let mut trajectory = Vec::with_capacity(instance.jobs.len());
        let cap = (instance.node_capacity_cpu, instance.node_capacity_ram);

        let mut sum_imbalance = 0.0f64;
        let mut tight_nodes_count = 0usize;

        for (step_index, job) in instance.jobs.iter().enumerate() {
            let (next_job_cpu, next_job_ram) = if lookahead {
                let next_job = instance.jobs.get(step_index + 1);
                (
                    next_job.map(|j| j.cpu_req).unwrap_or(0),
                    next_job.map(|j| j.ram_req).unwrap_or(0),
                )
            } else {
                (0, 0)
            };
            let outcome = match sandbox.execute_scheduling_with_lookahead(
                &capsule.logic_code,
                job.cpu_req,
                job.ram_req,
                next_job_cpu,
                next_job_ram,
                &node_levels,
                cap,
                fuel_limit_per_call,
                step_index,
                instance.jobs.len(),
            ) {
                Ok(out) => out,
                Err(err) => {
                    return (
                        InstanceEvaluation {
                            instance_name: instance.name.clone(),
                            is_valid: false,
                            bins_used: node_levels.len(),
                            theoretical_min_bins: instance.theoretical_min_nodes(),
                            fuel_consumed: total_fuel,
                            error_message: Some(err.to_string()),
                            tldr_score: -100.0,
                            trajectory,
                        },
                        0.0,
                        0.0,
                    );
                }
            };

            total_fuel += outcome.fuel_consumed;

            match outcome.decision {
                SchedulingDecision::PlaceOnNode(idx) => {
                    if idx >= node_levels.len() {
                        return (
                            InstanceEvaluation {
                                instance_name: instance.name.clone(),
                                is_valid: false,
                                bins_used: node_levels.len(),
                                theoretical_min_bins: instance.theoretical_min_nodes(),
                                fuel_consumed: total_fuel,
                                error_message: Some(format!("Invalid node index: {}", idx)),
                                tldr_score: -100.0,
                                trajectory,
                            },
                            0.0,
                            0.0,
                        );
                    }
                    let (u_cpu, u_ram) = node_levels[idx];
                    node_levels[idx] = (u_cpu + job.cpu_req, u_ram + job.ram_req);
                }
                SchedulingDecision::OpenNewNode => {
                    node_levels.push((job.cpu_req, job.ram_req));
                }
            }

            trajectory.push(node_levels.len());
        }

        // Compute imbalance & tight packing
        for &(u_cpu, u_ram) in &node_levels {
            let res_cpu_ratio = (cap.0.saturating_sub(u_cpu) as f64) / (cap.0 as f64);
            let res_ram_ratio = (cap.1.saturating_sub(u_ram) as f64) / (cap.1 as f64);
            sum_imbalance += (res_cpu_ratio - res_ram_ratio).abs();
            if cap.0.saturating_sub(u_cpu) < 2 && cap.1.saturating_sub(u_ram) < 4 {
                tight_nodes_count += 1;
            }
        }

        let n_nodes = node_levels.len().max(1) as f64;
        let mean_imbalance = sum_imbalance / n_nodes;
        let tight_ratio = tight_nodes_count as f64 / n_nodes;

        (
            InstanceEvaluation {
                instance_name: instance.name.clone(),
                is_valid: true,
                bins_used: node_levels.len(),
                theoretical_min_bins: instance.theoretical_min_nodes(),
                fuel_consumed: total_fuel,
                error_message: None,
                tldr_score: -(node_levels.len() as f64),
                trajectory,
            },
            mean_imbalance,
            tight_ratio,
        )
    }

    /// Évalue une capsule sur un dataset de scheduling complet.
    pub fn evaluate_dataset(
        &self,
        dataset: &SchedulingDataset,
        capsule: &SkillCapsule,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
    ) -> BenchmarkScore {
        self.evaluate_dataset_with_lookahead(dataset, capsule, sandbox, fuel_limit_per_call, true)
    }

    /// Évalue une capsule sur un dataset de scheduling complet avec option de lookahead.
    pub fn evaluate_dataset_with_lookahead(
        &self,
        dataset: &SchedulingDataset,
        capsule: &SkillCapsule,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
        lookahead: bool,
    ) -> BenchmarkScore {
        let mut total_nodes = 0usize;
        let mut total_theoretical = 0usize;
        let mut total_fuel = 0u64;
        let mut solved = 0usize;
        let mut sum_eff = 0.0f64;
        let mut sum_imbalance = 0.0f64;
        let mut sum_tight = 0.0f64;

        for inst in &dataset.instances {
            let (eval, imb, tight) = self.evaluate_instance_with_lookahead(
                inst,
                capsule,
                sandbox,
                fuel_limit_per_call,
                lookahead,
            );
            if !eval.is_valid {
                return BenchmarkScore {
                    is_valid: false,
                    total_bins: total_nodes + 9999,
                    total_theoretical_min_bins: total_theoretical + inst.theoretical_min_nodes(),
                    total_excess_bins: 9999,
                    mean_efficiency: 0.0,
                    total_fuel,
                    solved_instances: solved,
                    total_instances: dataset.instances.len(),
                    behavior: BehaviorDescriptor::default(),
                    tldr_score: -100.0,
                    trajectory_metrics: Default::default(),
                };
            }

            total_nodes += eval.bins_used;
            total_theoretical += eval.theoretical_min_bins;
            total_fuel += eval.fuel_consumed;
            solved += 1;
            sum_eff += eval.efficiency_ratio();
            sum_imbalance += imb;
            sum_tight += tight;
        }

        let n = dataset.instances.len().max(1) as f64;
        let mean_imbalance = sum_imbalance / n;
        let mean_tight = sum_tight / n;
        let mean_eff = sum_eff / n;

        BenchmarkScore {
            is_valid: true,
            total_bins: total_nodes,
            total_theoretical_min_bins: total_theoretical,
            total_excess_bins: total_nodes.saturating_sub(total_theoretical),
            mean_efficiency: mean_eff,
            total_fuel,
            solved_instances: solved,
            total_instances: dataset.instances.len(),
            behavior: BehaviorDescriptor {
                residual_variance: mean_imbalance * 100.0, // Multiplied for scale [0..100]
                new_bin_ratio: total_nodes as f64 / (dataset.instances.len() * 50) as f64,
                mean_fullness: mean_eff,
                dead_space_ratio: mean_tight,
            },
            tldr_score: -mean_imbalance,
            trajectory_metrics: Default::default(),
        }
    }

    /// Évalue les heuristiques classiques de référence (FIRST_FIT_NODE, BALANCED_FIT, etc.).
    pub fn evaluate_classical_baseline(
        &self,
        baseline_name: &str,
        dataset: &SchedulingDataset,
        sandbox: &dyn HeuristicSandbox,
    ) -> BenchmarkScore {
        let capsule = SkillCapsule::new(
            baseline_name,
            format!("Classical baseline {}", baseline_name),
            baseline_name,
            vec![],
            0,
            std::collections::HashMap::new(),
        );
        self.evaluate_dataset(dataset, &capsule, sandbox, 50_000)
    }

    /// Validation Anti-Dead-Code Niveau 2 et Niveau 3 pour le Scheduling
    pub fn validate_anti_dead_code_scheduling(
        &self,
        child: &mut SkillCapsule,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit: u64,
    ) -> Result<ValidationStatus, String> {
        // Niveau 1 : Survie structurelle
        if child.graft_manifest.is_some() {
            verify_level1_structural_survival(child)
                .map_err(|e| format!("Level 1 Structural failed: {}", e))?;
        }

        // Niveau 2 : Survie computationnelle (évaluation partielle)
        let probe_instance = &self.in_distribution.instances[0];
        let (eval_child, _, _) = self.evaluate_instance(probe_instance, child, sandbox, fuel_limit);
        if !eval_child.is_valid || eval_child.fuel_consumed == 0 {
            child.validation = ValidationStatus::Failed("Level 2 Computational failed".to_string());
            return Err("Level 2 Computational failed: fuel is 0 or execution invalid".to_string());
        }

        // Niveau 3 : Ablation Causale C_null
        if child.graft_manifest.is_none() {
            child.validation = ValidationStatus::PassedLevel2Computational;
            return Ok(ValidationStatus::PassedLevel2Computational);
        }

        let c_null_capsule = match create_ablation_capsule_res(child) {
            Ok(c) => c,
            Err(e) => {
                let status =
                    ValidationStatus::NotEvaluable(format!("Scheduling ablation failed: {}", e));
                child.validation = status.clone();
                return Ok(status);
            }
        };

        // Comparaison décisionnelle différentielle sur un jeu de sondes
        let mut divergent_decisions = 0usize;
        let mut null_nodes_total = 0usize;
        let mut orig_nodes_total = 0usize;

        for inst in self.in_distribution.instances.iter().take(5) {
            let (eval_c, _, _) = self.evaluate_instance(inst, child, sandbox, fuel_limit);
            let (eval_null, _, _) =
                self.evaluate_instance(inst, &c_null_capsule, sandbox, fuel_limit);

            if !eval_null.is_valid {
                let status = ValidationStatus::NotEvaluable(
                    "Scheduling ablation produced invalid execution on probe instance".to_string(),
                );
                child.validation = status.clone();
                return Ok(status);
            }

            orig_nodes_total += eval_c.bins_used;
            null_nodes_total += eval_null.bins_used;

            if eval_c.bins_used != eval_null.bins_used || eval_c.trajectory != eval_null.trajectory
            {
                divergent_decisions += 1;
            }
        }

        if null_nodes_total < orig_nodes_total {
            let status = ValidationStatus::GraftHarmful(format!(
                "Scheduling ablation improved nodes used (null: {}, orig: {})",
                null_nodes_total, orig_nodes_total
            ));
            child.validation = status.clone();
            Ok(status)
        } else if divergent_decisions > 0 && null_nodes_total > orig_nodes_total {
            child.validation = ValidationStatus::PassedLevel3Behavioral;
            Ok(ValidationStatus::PassedLevel3Behavioral)
        } else {
            child.validation = ValidationStatus::PassedLevel2Computational;
            Ok(ValidationStatus::PassedLevel2Computational)
        }
    }
}
