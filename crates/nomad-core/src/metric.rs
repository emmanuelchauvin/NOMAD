use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use crate::capsule::SkillCapsule;

/// Detailed outcome of evaluating a heuristic on a single bin packing instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstanceEvaluation {
    pub instance_name: String,
    pub is_valid: bool,
    pub bins_used: usize,
    pub theoretical_min_bins: usize,
    pub fuel_consumed: u64,
    pub error_message: Option<String>,
}

impl InstanceEvaluation {
    pub fn excess_bins(&self) -> usize {
        self.bins_used.saturating_sub(self.theoretical_min_bins)
    }

    pub fn efficiency_ratio(&self) -> f64 {
        if self.bins_used == 0 {
            0.0
        } else {
            self.theoretical_min_bins as f64 / self.bins_used as f64
        }
    }
}

/// Aggregated benchmark score across an entire dataset (e.g., In-Distribution or OOD).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkScore {
    /// Overall validity: true if ALL test instances executed successfully without violation.
    pub is_valid: bool,
    /// Total bins used across all instances in the dataset (primary metric to minimize).
    pub total_bins: usize,
    /// Total theoretical lower bound bins across instances.
    pub total_theoretical_min_bins: usize,
    /// Total excess bins above theoretical optimum.
    pub total_excess_bins: usize,
    /// Average packing efficiency ratio (higher is better, 1.0 = perfect packing).
    pub mean_efficiency: f64,
    /// Total computational fuel/gas consumed (secondary tie-breaker metric).
    pub total_fuel: u64,
    /// Number of successfully solved instances out of total.
    pub solved_instances: usize,
    pub total_instances: usize,
}

impl BenchmarkScore {
    /// Fatal penalty score assigned when code fails to compile or crashes.
    pub fn fatal_penalty(total_instances: usize) -> Self {
        Self {
            is_valid: false,
            total_bins: usize::MAX / 2,
            total_theoretical_min_bins: 0,
            total_excess_bins: usize::MAX / 2,
            mean_efficiency: 0.0,
            total_fuel: u64::MAX,
            solved_instances: 0,
            total_instances,
        }
    }
}

/// Comparison order according to the scientific specifications:
/// 1. Validity (valid beats invalid)
/// 2. Total bins used (fewer bins is better)
/// 3. Total fuel consumed (lower gas is tie-breaker)
impl PartialOrd for BenchmarkScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for BenchmarkScore {}

impl Ord for BenchmarkScore {
    fn cmp(&self, other: &Self) -> Ordering {
        // 1. Validity first
        match (self.is_valid, other.is_valid) {
            (true, false) => return Ordering::Less, // self is better (ranks higher)
            (false, true) => return Ordering::Greater,
            (false, false) => return Ordering::Equal,
            (true, true) => {}
        }

        // 2. Total bins used (smaller is better)
        match self.total_bins.cmp(&other.total_bins) {
            Ordering::Less => Ordering::Less,
            Ordering::Greater => Ordering::Greater,
            // 3. Fuel consumption as tie-breaker (smaller is better)
            Ordering::Equal => self.total_fuel.cmp(&other.total_fuel),
        }
    }
}

/// A population member evaluated by the oracle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluatedIndividual {
    pub capsule: SkillCapsule,
    pub in_distribution_score: BenchmarkScore,
    pub ood1_score: Option<BenchmarkScore>,
    pub ood2_score: Option<BenchmarkScore>,
    pub rank: usize,
}

impl EvaluatedIndividual {
    pub fn new(capsule: SkillCapsule, score: BenchmarkScore) -> Self {
        Self {
            capsule,
            in_distribution_score: score,
            ood1_score: None,
            ood2_score: None,
            rank: 0,
        }
    }
}
