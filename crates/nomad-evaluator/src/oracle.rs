use crate::dataset::{BinPackingInstance, Dataset};
use nomad_core::{
    BehaviorDescriptor, BenchmarkScore, InstanceEvaluation, PackingDecision, SkillCapsule,
    TrajectoryMetrics,
};
use nomad_sandbox::{DeterministicSandbox, HeuristicSandbox};
use rayon::prelude::*;

/// Longueur d'instance pour laquelle `fuel_limit_per_call` est calibre (jeu d'entrainement
/// du profil Weibull : 1 000 objets).
pub const FUEL_REFERENCE_ITEMS: usize = 1_000;

/// Limite de fuel par decision, proportionnelle a la longueur de l'instance.
///
/// Une decision note chaque bac ouvert : son cout croit avec le nombre de bacs, donc avec
/// le nombre d'objets. Une limite fixe invalide sur les longues instances (OOD-1, 5 000
/// objets) des heuristiques valides sur 1 000 objets. La limite est multipliee par
/// `ceil(n_items / FUEL_REFERENCE_ITEMS)` et reste inchangee jusqu'a 1 000 objets.
pub fn scaled_fuel_limit(fuel_limit_per_call: u64, n_items: usize) -> u64 {
    let factor = n_items.div_ceil(FUEL_REFERENCE_ITEMS).max(1) as u64;
    fuel_limit_per_call.saturating_mul(factor)
}

/// ============================================================================
/// compute_trajectory_tldr : Algorithme Mathématique du tLDR (Sections 6.4 & 6.5)
/// ============================================================================
/// Calcule le taux expérimental de décroissance de Lyapunov (tLDR) sur la trajectoire :
///
/// **Formulation exacte :**
/// 1. **Borne Volumétrique Préfixe :**
///    L_vol(t) = ceil( sum_{i=1}^t w_i / C )
/// 2. **Écart Relatif (Waste) :**
///    V(t) = (B(t) - L_vol(t)) / L_vol(t)
/// 3. **Fenêtre d'Observation :**
///    t in [t0, N] avec t0 = floor(N / 5)
/// 4. **Transformation Logarithmique :**
///    y(t) = ln(V(t) + epsilon), epsilon = 10^-4
/// 5. **Régression Linéaire :**
///    Ajustement de y(t) sur le temps normalisé tau = t / N par moindres carrés,
///    fournissant la pente beta_1 et l'écart-type des résidus sigma_res.
/// 6. **Score Combiné :**
///    tLDR = -beta_1 - lambda * sigma_res (avec lambda = 0.5)
pub fn compute_trajectory_tldr(
    items: &[u32],
    trajectory: &[usize],
    capacity: u32,
) -> (f64, TrajectoryMetrics) {
    compute_trajectory_tldr_with_epsilon(items, trajectory, capacity, 1e-4)
}

/// Calcule le taux expérimental de décroissance de Lyapunov (tLDR) avec un paramètre epsilon configurable.
pub fn compute_trajectory_tldr_with_epsilon(
    items: &[u32],
    trajectory: &[usize],
    capacity: u32,
    epsilon: f64,
) -> (f64, TrajectoryMetrics) {
    let n = items.len();
    if n == 0 || trajectory.is_empty() {
        return (0.0, TrajectoryMetrics::default());
    }

    let mut prefix_sum = 0u64;
    let mut v_values = Vec::with_capacity(n);

    for (t_idx, &item) in items.iter().enumerate() {
        prefix_sum += item as u64;
        let l_vol = ((prefix_sum as f64) / (capacity as f64)).ceil().max(1.0);
        let b_t = trajectory[t_idx] as f64;
        let v_t = ((b_t - l_vol) / l_vol).max(0.0);
        v_values.push(v_t);
    }

    // Window [t0, N] with t0 = floor(N / 5) (Section 6.5)
    let t0 = (n / 5).max(1);
    let lambda = 0.5;

    let mut taus = Vec::new();
    let mut ys = Vec::new();

    for t in t0..=n {
        let tau = (t as f64) / (n as f64);
        let v_t = v_values[t - 1];
        let y = (v_t + epsilon).ln();
        taus.push(tau);
        ys.push(y);
    }

    let m = taus.len() as f64;
    if m < 2.0 {
        return (0.0, TrajectoryMetrics::default());
    }

    let mean_tau: f64 = taus.iter().sum::<f64>() / m;
    let mean_y: f64 = ys.iter().sum::<f64>() / m;

    let mut s_tau_tau = 0.0;
    let mut s_tau_y = 0.0;

    for i in 0..taus.len() {
        let d_tau = taus[i] - mean_tau;
        let d_y = ys[i] - mean_y;
        s_tau_tau += d_tau * d_tau;
        s_tau_y += d_tau * d_y;
    }

    let beta1 = if s_tau_tau > 1e-12 {
        s_tau_y / s_tau_tau
    } else {
        0.0
    };

    let mut s_res = 0.0;
    for i in 0..taus.len() {
        let y_hat = mean_y + beta1 * (taus[i] - mean_tau);
        let residual = ys[i] - y_hat;
        s_res += residual * residual;
    }
    let sigma_res = (s_res / m).sqrt();

    // Section 6.5: tLDR = -beta_1 - lambda * sigma_res
    let tldr = -beta1 - (lambda * sigma_res);

    let metrics = TrajectoryMetrics {
        beta1_slope: beta1,
        sigma_res,
        tldr,
        prefix_waste: v_values,
    };

    (tldr, metrics)
}

/// Calcule l'entropie de Shannon de la distribution des résidus discrétisée en 10 buckets
fn compute_residual_entropy(bin_levels: &[u32], capacity: u32) -> f64 {
    if bin_levels.is_empty() {
        return 0.0;
    }
    let num_buckets = 10;
    let mut counts = [0usize; 10];
    for &lvl in bin_levels {
        let rem = capacity.saturating_sub(lvl) as f64;
        let b = ((rem / capacity as f64) * num_buckets as f64).floor() as usize;
        let idx = b.min(num_buckets - 1);
        counts[idx] += 1;
    }
    let total = bin_levels.len() as f64;
    let mut entropy = 0.0;
    for &c in &counts {
        if c > 0 {
            let p = c as f64 / total;
            entropy -= p * p.ln();
        }
    }
    entropy
}

/// ============================================================================
/// OracleEvaluator : Évaluateur Souverain et Sanctuarisé de NOMAD
/// ============================================================================
/// Entité indépendante et déterministe chargée d'évaluer les heuristiques sans
/// aucune fuite d'information vers les agents d'optimisation.
///
/// **Jeux de données sanctuarisés :**
/// - `in_distribution` : 50 instances, N=50, tailles U(10, 80), capacité C=100.
/// - `validation` : 50 instances de validation in-distribution (seed dérivée sanctuarisée).
/// - `ood1_scale` : 50 instances de passage à l'échelle, N=500, U(10, 80).
/// - `ood2_powerlaw` : 50 instances de rupture de distribution (loi de puissance).
#[derive(Debug, Clone)]
pub struct OracleEvaluator {
    pub in_distribution: Dataset,
    pub validation: Dataset,
    pub ood1_scale: Dataset,
    pub ood2_powerlaw: Dataset,
}

impl OracleEvaluator {
    /// Initialise l'évaluateur avec les jeux de données pré-enregistrés générés via une seed fixe.
    ///
    /// La variable d'environnement `NOMAD_BP_PROFILE=weibull` selectionne le profil Weibull
    /// (longues instances, marge mesurable) a la place du profil uniforme historique.
    pub fn new(seed: u64) -> Self {
        if std::env::var("NOMAD_BP_PROFILE").map(|v| v == "weibull").unwrap_or(false) {
            return Self::new_weibull(seed);
        }
        Self::new_uniform(seed)
    }

    /// Profil Weibull : entrainement et validation Weibull(3, 45) ; OOD-1 = instances 5x plus
    /// longues ; OOD-2 = changement de forme, Weibull(2, 40).
    pub fn new_weibull(seed: u64) -> Self {
        let n = |k: &str, d: usize| -> usize {
            std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
        };
        let inst = n("NOMAD_WB_INSTANCES", 10);
        let items = n("NOMAD_WB_ITEMS", 1000);
        Self {
            in_distribution: Dataset::generate_weibull(seed, "weibull_train", inst, items, 3.0, 45.0),
            validation: Dataset::generate_weibull(
                seed ^ 0x005A_11DA_710E_u64,
                "weibull_val",
                inst,
                items,
                3.0,
                45.0,
            ),
            ood1_scale: Dataset::generate_weibull(
                seed ^ 0x00D1_5CA1E,
                "weibull_ood1_scale",
                5,
                items * 5,
                3.0,
                45.0,
            ),
            ood2_powerlaw: Dataset::generate_weibull(
                seed ^ 0x00D2_5E1F7,
                "weibull_ood2_shape",
                inst,
                items,
                2.0,
                40.0,
            ),
        }
    }

    /// Profil uniforme historique U(10, 80).
    pub fn new_uniform(seed: u64) -> Self {
        Self {
            in_distribution: Dataset::generate_in_distribution(seed),
            validation: Dataset::generate_validation(seed),
            ood1_scale: Dataset::generate_ood1_scale(seed),
            ood2_powerlaw: Dataset::generate_ood2_distribution_shift(seed),
        }
    }

    /// Évalue une capsule sur le jeu d'entraînement In-Distribution.
    pub fn evaluate_in_distribution(
        &self,
        capsule: &SkillCapsule,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
    ) -> BenchmarkScore {
        self.evaluate_dataset(&self.in_distribution, capsule, sandbox, fuel_limit_per_call)
    }

    /// Évalue une capsule sur le jeu de validation In-Distribution (non utilisé pour la sélection ni MAP-Elites).
    pub fn evaluate_validation(
        &self,
        capsule: &SkillCapsule,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
    ) -> BenchmarkScore {
        self.evaluate_dataset(&self.validation, capsule, sandbox, fuel_limit_per_call)
    }

    /// Évalue une capsule sur le benchmark hors-distribution OOD-1 (Échelle N=500).
    pub fn evaluate_ood1(
        &self,
        capsule: &SkillCapsule,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
    ) -> BenchmarkScore {
        self.evaluate_dataset(&self.ood1_scale, capsule, sandbox, fuel_limit_per_call)
    }

    /// Évalue une capsule sur le benchmark hors-distribution OOD-2 (Décalage de loi de puissance).
    pub fn evaluate_ood2(
        &self,
        capsule: &SkillCapsule,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
    ) -> BenchmarkScore {
        self.evaluate_dataset(&self.ood2_powerlaw, capsule, sandbox, fuel_limit_per_call)
    }

    /// ========================================================================
    /// evaluate_dataset : Algorithme d'Évaluation sur un Dataset Complet
    /// ========================================================================
    /// Parcourt chaque instance du dataset, exécute l'heuristique dans le sandbox,
    /// accumule le fuel et les bacs utilisés, et agrège le tLDR et le BehaviorDescriptor.
    pub fn evaluate_dataset(
        &self,
        dataset: &Dataset,
        capsule: &SkillCapsule,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
    ) -> BenchmarkScore {
        self.evaluate_dataset_with_lookahead(dataset, capsule, sandbox, fuel_limit_per_call, true)
    }

    /// Évalue une capsule sur un dataset avec option d'activation ou masquage du lookahead (k=2).
    pub fn evaluate_dataset_with_lookahead(
        &self,
        dataset: &Dataset,
        capsule: &SkillCapsule,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
        lookahead: bool,
    ) -> BenchmarkScore {
        self.evaluate_dataset_with_evals(dataset, capsule, sandbox, fuel_limit_per_call, lookahead).0
    }

    /// Évalue une capsule sur un dataset et retourne à la fois le BenchmarkScore et les évaluations d'instances.
    pub fn evaluate_dataset_with_evals(
        &self,
        dataset: &Dataset,
        capsule: &SkillCapsule,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
        lookahead: bool,
    ) -> (BenchmarkScore, Vec<InstanceEvaluation>) {
        let mut total_bins = 0usize;
        let mut total_theoretical = 0usize;
        let mut total_fuel = 0u64;
        let mut solved_instances = 0usize;
        let mut sum_efficiency = 0.0f64;
        let mut sum_tldr = 0.0f64;

        let mut sum_residual_var = 0.0f64;
        let mut sum_new_bin_ratio = 0.0f64;
        let mut sum_mean_fullness = 0.0f64;
        let mut sum_dead_space_ratio = 0.0f64;

        let mut sum_beta1 = 0.0f64;
        let mut sum_sigma_res = 0.0f64;
        let mut sample_prefix_waste = Vec::new();

        let results: Vec<_> = dataset
            .instances
            .par_iter()
            .map(|inst| {
                self.evaluate_single_instance_with_lookahead(
                    inst,
                    &capsule.logic_code,
                    sandbox,
                    fuel_limit_per_call,
                    lookahead,
                )
            })
            .collect();

        let mut instance_evals = Vec::with_capacity(results.len());

        for (eval, behavior, traj_metrics) in results {
            if !eval.is_valid {
                instance_evals.push(eval);
                // Fatal penalty: Any failure disqualifies the entire run on this dataset
                return (BenchmarkScore::fatal_penalty(dataset.instances.len()), instance_evals);
            }

            total_bins += eval.bins_used;
            total_theoretical += eval.theoretical_min_bins;
            total_fuel += eval.fuel_consumed;
            sum_efficiency += eval.efficiency_ratio();
            sum_tldr += traj_metrics.tldr;
            sum_beta1 += traj_metrics.beta1_slope;
            sum_sigma_res += traj_metrics.sigma_res;
            if sample_prefix_waste.is_empty() && !traj_metrics.prefix_waste.is_empty() {
                sample_prefix_waste = traj_metrics.prefix_waste;
            }

            sum_residual_var += behavior.residual_variance;
            sum_new_bin_ratio += behavior.new_bin_ratio;
            sum_mean_fullness += behavior.mean_fullness;
            sum_dead_space_ratio += behavior.dead_space_ratio;

            instance_evals.push(eval);
            solved_instances += 1;
        }

        let n = if solved_instances > 0 {
            solved_instances as f64
        } else {
            1.0
        };

        let mean_efficiency = if solved_instances > 0 {
            sum_efficiency / n
        } else {
            0.0
        };

        let mean_behavior = BehaviorDescriptor {
            residual_variance: sum_residual_var / n,
            new_bin_ratio: sum_new_bin_ratio / n,
            mean_fullness: sum_mean_fullness / n,
            dead_space_ratio: sum_dead_space_ratio / n,
        };

        let mean_tldr = if solved_instances > 0 {
            sum_tldr / n
        } else {
            0.0
        };
        let mean_beta1 = if solved_instances > 0 {
            sum_beta1 / n
        } else {
            0.0
        };
        let mean_sigma_res = if solved_instances > 0 {
            sum_sigma_res / n
        } else {
            0.0
        };

        let trajectory_metrics = TrajectoryMetrics {
            beta1_slope: mean_beta1,
            sigma_res: mean_sigma_res,
            tldr: mean_tldr,
            prefix_waste: sample_prefix_waste,
        };

        let score = BenchmarkScore {
            is_valid: true,
            total_bins,
            total_theoretical_min_bins: total_theoretical,
            total_excess_bins: total_bins.saturating_sub(total_theoretical),
            mean_efficiency,
            total_fuel,
            solved_instances,
            total_instances: dataset.instances.len(),
            behavior: mean_behavior,
            tldr_score: mean_tldr,
            trajectory_metrics: Some(trajectory_metrics),
        };

        (score, instance_evals)
    }

    /// Evaluates a single online bin packing instance sequentially, computing trajectory, tLDR and behavior.
    pub fn evaluate_single_instance(
        &self,
        instance: &BinPackingInstance,
        logic_code: &str,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
    ) -> (InstanceEvaluation, BehaviorDescriptor, TrajectoryMetrics) {
        self.evaluate_single_instance_with_lookahead(
            instance,
            logic_code,
            sandbox,
            fuel_limit_per_call,
            true,
        )
    }

    /// Evaluates a single online bin packing instance with configurable lookahead.
    pub fn evaluate_single_instance_with_lookahead(
        &self,
        instance: &BinPackingInstance,
        logic_code: &str,
        sandbox: &dyn HeuristicSandbox,
        fuel_limit_per_call: u64,
        lookahead: bool,
    ) -> (InstanceEvaluation, BehaviorDescriptor, TrajectoryMetrics) {
        let fuel_limit_per_call = scaled_fuel_limit(fuel_limit_per_call, instance.items.len());
        let mut bin_levels: Vec<u32> = Vec::new();
        let mut mid_levels: Vec<u32> = Vec::new();
        let mid_course_step = instance.items.len() / 2;
        let mut total_fuel = 0u64;
        let mut trajectory = Vec::with_capacity(instance.items.len());
        let mut new_bins_opened = 0usize;

        for (step_idx, &item) in instance.items.iter().enumerate() {
            let next_item = if lookahead {
                instance.items.get(step_idx + 1).copied().unwrap_or(0)
            } else {
                0
            };
            let res = sandbox.execute_with_lookahead(
                logic_code,
                item,
                next_item,
                &bin_levels,
                instance.capacity,
                fuel_limit_per_call,
                step_idx,
                instance.items.len(),
            );

            match res {
                Ok(outcome) => {
                    total_fuel = total_fuel.saturating_add(outcome.fuel_consumed);
                    match outcome.decision {
                        PackingDecision::PlaceInExisting(idx) => {
                            if idx >= bin_levels.len() {
                                return (
                                    InstanceEvaluation {
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
                                        tldr_score: 0.0,
                                        trajectory: Vec::new(),
                                    },
                                    BehaviorDescriptor::default(),
                                    TrajectoryMetrics::default(),
                                );
                            }
                            if bin_levels[idx] + item > instance.capacity {
                                return (
                                    InstanceEvaluation {
                                        instance_name: instance.name.clone(),
                                        is_valid: false,
                                        bins_used: 0,
                                        theoretical_min_bins: instance.theoretical_min_bins,
                                        fuel_consumed: total_fuel,
                                        error_message: Some(format!(
                                            "Capacity exceeded in bin {}: {} + {} > {}",
                                            idx, bin_levels[idx], item, instance.capacity
                                        )),
                                        tldr_score: 0.0,
                                        trajectory: Vec::new(),
                                    },
                                    BehaviorDescriptor::default(),
                                    TrajectoryMetrics::default(),
                                );
                            }
                            bin_levels[idx] += item;
                        }
                        PackingDecision::OpenNewBin => {
                            if item > instance.capacity {
                                return (
                                    InstanceEvaluation {
                                        instance_name: instance.name.clone(),
                                        is_valid: false,
                                        bins_used: 0,
                                        theoretical_min_bins: instance.theoretical_min_bins,
                                        fuel_consumed: total_fuel,
                                        error_message: Some(format!(
                                            "Item {} exceeds capacity {}",
                                            item, instance.capacity
                                        )),
                                        tldr_score: 0.0,
                                        trajectory: Vec::new(),
                                    },
                                    BehaviorDescriptor::default(),
                                    TrajectoryMetrics::default(),
                                );
                            }
                            bin_levels.push(item);
                            new_bins_opened += 1;
                        }
                    }

                    trajectory.push(bin_levels.len());
                    if step_idx == mid_course_step {
                        mid_levels = bin_levels.clone();
                    }
                }
                Err(err) => {
                    return (
                        InstanceEvaluation {
                            instance_name: instance.name.clone(),
                            is_valid: false,
                            bins_used: 0,
                            theoretical_min_bins: instance.theoretical_min_bins,
                            fuel_consumed: total_fuel,
                            error_message: Some(err.to_string()),
                            tldr_score: 0.0,
                            trajectory: Vec::new(),
                        },
                        BehaviorDescriptor::default(),
                        TrajectoryMetrics::default(),
                    );
                }
            }
        }

        let num_items = instance.items.len().max(1);
        let (tldr_score, traj_metrics) =
            compute_trajectory_tldr(&instance.items, &trajectory, instance.capacity);

        // Calculate end-state behavioral descriptors
        let open_count = bin_levels.len().max(1);
        let residuals: Vec<f64> = bin_levels
            .iter()
            .map(|&lvl| (instance.capacity - lvl) as f64)
            .collect();

        let mean_residual: f64 = residuals.iter().sum::<f64>() / open_count as f64;
        let _ = mean_residual;

        let mean_fullness: f64 = bin_levels
            .iter()
            .map(|&lvl| lvl as f64 / instance.capacity as f64)
            .sum::<f64>()
            / open_count as f64;

        // Axe X : dead space dans la zone critique [1..15]
        let dead_space_count = bin_levels
            .iter()
            .filter(|&&lvl| {
                let rem = instance.capacity - lvl;
                (1..=15).contains(&rem)
            })
            .count();

        // Axe Y : Entropie de Shannon composite (mi-parcours & fin de parcours)
        let entropy_mid = compute_residual_entropy(&mid_levels, instance.capacity);
        let entropy_end = compute_residual_entropy(&bin_levels, instance.capacity);
        let combined_entropy = 0.5 * entropy_mid + 0.5 * entropy_end;

        let behavior = BehaviorDescriptor {
            residual_variance: combined_entropy,
            new_bin_ratio: new_bins_opened as f64 / num_items as f64,
            mean_fullness,
            dead_space_ratio: dead_space_count as f64 / open_count as f64,
        };

        (
            InstanceEvaluation {
                instance_name: instance.name.clone(),
                is_valid: true,
                bins_used: bin_levels.len(),
                theoretical_min_bins: instance.theoretical_min_bins,
                fuel_consumed: total_fuel,
                error_message: None,
                tldr_score,
                trajectory,
            },
            behavior,
            traj_metrics,
        )
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

#[cfg(test)]
mod fuel_tests {
    use super::scaled_fuel_limit;

    #[test]
    fn scaled_fuel_limit_is_proportional_to_instance_length() {
        assert_eq!(scaled_fuel_limit(400_000, 50), 400_000);
        assert_eq!(scaled_fuel_limit(400_000, 1_000), 400_000);
        assert_eq!(scaled_fuel_limit(400_000, 1_001), 800_000);
        assert_eq!(scaled_fuel_limit(400_000, 5_000), 2_000_000);
        assert_eq!(scaled_fuel_limit(u64::MAX, 5_000), u64::MAX);
    }
}
