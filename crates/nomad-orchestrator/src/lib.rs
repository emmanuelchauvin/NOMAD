//! NOMAD Orchestrator: Multi-agent lifecycle, budget accounting, and evolutionary swarm reproduction.

pub mod error;
pub mod engine;
pub mod map_elites;
pub mod pso;
pub mod rebirth;

pub use error::*;
pub use engine::*;
pub use map_elites::*;
pub use pso::*;
pub use rebirth::*;

#[cfg(test)]
mod tests {
    use super::*;
    use nomad_core::{ExperimentConfig, ExperimentalGroup};
    use nomad_evaluator::OracleEvaluator;
    use nomad_operator::DeterministicMockOperator;
    use nomad_sandbox::DeterministicSandbox;

    #[test]
    fn test_evolutionary_run_group_c_swarm() {
        let mut config = ExperimentConfig::default();
        config.group = ExperimentalGroup::GroupC;
        config.rng_seed = 42;
        config.population_size = 6;
        config.max_generations = 3;

        let oracle = OracleEvaluator::new(42);
        let sandbox = DeterministicSandbox::new();
        let operator = DeterministicMockOperator::default();

        let mut engine = EvolutionEngine::new(config, &oracle, &sandbox, &operator);
        let report = engine.execute_run().expect("Run execution should succeed");

        assert_eq!(report.group, ExperimentalGroup::GroupC);
        assert_eq!(report.total_generations, 3);
        assert!(report.best_in_distribution_score.is_valid);
        assert!(report.best_in_distribution_score.total_bins > 0);
        assert!(report.total_lineage_nodes >= 6);
        assert!(report.best_ood1_score.is_valid);
        assert!(report.best_ood2_score.is_valid);
    }
}
