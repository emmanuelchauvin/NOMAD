//! NOMAD Core: Foundation structures, types, capsules, and metrics for the DARWIN.RS architecture.

pub mod capsule;
pub mod config;
pub mod metric;
pub mod types;

pub use capsule::*;
pub use config::*;
pub use metric::*;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_capsule_immutability_and_hash() {
        let capsule1 = SkillCapsule::new(
            "HeuristicA",
            "Best fit with waste reduction",
            "BEST_FIT",
            vec![],
            0,
            HashMap::new(),
        );

        let capsule2 = SkillCapsule::new(
            "HeuristicA",
            "Best fit with waste reduction",
            "BEST_FIT",
            vec![],
            0,
            HashMap::new(),
        );

        // Same content yields identical SHA-256 fingerprint and ID
        assert_eq!(capsule1.content_hash, capsule2.content_hash);
        assert_eq!(capsule1.id, capsule2.id);
        assert!(capsule1.is_root());
        assert!(!capsule1.is_crossover());
    }

    #[test]
    fn test_benchmark_score_ordering() {
        let valid_best = BenchmarkScore {
            is_valid: true,
            total_bins: 500,
            total_theoretical_min_bins: 480,
            total_excess_bins: 20,
            mean_efficiency: 0.96,
            total_fuel: 10_000,
            solved_instances: 50,
            total_instances: 50,
            behavior: BehaviorDescriptor::default(),
            tldr_score: 0.95,
            trajectory_metrics: None,
        };

        let valid_suboptimal = BenchmarkScore {
            is_valid: true,
            total_bins: 520,
            total_theoretical_min_bins: 480,
            total_excess_bins: 40,
            mean_efficiency: 0.92,
            total_fuel: 8_000,
            solved_instances: 50,
            total_instances: 50,
            behavior: BehaviorDescriptor::default(),
            tldr_score: 0.90,
            trajectory_metrics: None,
        };

        let invalid = BenchmarkScore::fatal_penalty(50);

        // Valid with fewer bins beats valid with more bins
        assert!(valid_best < valid_suboptimal);
        // Valid beats fatal penalty
        assert!(valid_best < invalid);
    }
}
