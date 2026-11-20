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

        // Tie-breaker 1: Equal bins, higher tldr_score beats lower tldr_score even with higher fuel
        let mut tie_higher_tldr = valid_best.clone();
        tie_higher_tldr.tldr_score = 0.98;
        tie_higher_tldr.total_fuel = 50_000; // Complex AST hybrid with higher instruction count

        let mut tie_lower_tldr = valid_best.clone();
        tie_lower_tldr.tldr_score = 0.85;
        tie_lower_tldr.total_fuel = 10_000; // Simple mutant with lower instruction count

        assert!(tie_higher_tldr < tie_lower_tldr, "Higher tLDR (qualitative balance) must defeat lower fuel at equal bins");

        // Tie-breaker 2: Equal bins and equal tldr, higher mean_efficiency beats lower efficiency
        let mut tie_higher_eff = tie_higher_tldr.clone();
        tie_higher_eff.mean_efficiency = 0.99;
        tie_higher_eff.total_fuel = 60_000;

        let mut tie_lower_eff = tie_higher_tldr.clone();
        tie_lower_eff.mean_efficiency = 0.95;
        tie_lower_eff.total_fuel = 20_000;

        assert!(tie_higher_eff < tie_lower_eff, "Higher efficiency must defeat lower fuel at equal bins and tLDR");

        // Tie-breaker 3: Equal bins, equal tldr and equal efficiency, lower fuel wins (parsimony)
        let mut tie_fuel_winner = tie_higher_eff.clone();
        tie_fuel_winner.total_fuel = 30_000;

        let mut tie_fuel_loser = tie_higher_eff.clone();
        tie_fuel_loser.total_fuel = 40_000;

        assert!(tie_fuel_winner < tie_fuel_loser, "Lower fuel must win when bins, tLDR and efficiency are identical");
    }
}
