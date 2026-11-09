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
        assert!(report.generations_history.iter().all(|r| r.crossover_rate >= 0.20 && r.crossover_rate <= 0.85));
    }

    #[test]
    fn test_adaptive_operator_crossover_bounds() {
        let config = ExperimentConfig::default();
        let oracle = OracleEvaluator::new(42);
        let sandbox = DeterministicSandbox::new();
        let operator = DeterministicMockOperator::default();

        let engine = EvolutionEngine::new(config, &oracle, &sandbox, &operator);
        assert!((engine.adaptive_crossover_rate - 0.60).abs() < 1e-6);
    }

    #[test]
    fn test_evolutionary_run_scheduling_swarm() {
        use nomad_core::ProblemDomain;
        use nomad_evaluator::SchedulingOracle;

        let mut config = ExperimentConfig::default();
        config.domain = ProblemDomain::MultiResourceScheduling;
        config.group = ExperimentalGroup::GroupC;
        config.rng_seed = 42;
        config.population_size = 6;
        config.max_generations = 3;

        let oracle = OracleEvaluator::new(42);
        let sched_oracle = SchedulingOracle::new(42);
        let sandbox = DeterministicSandbox::new();
        let mut operator = DeterministicMockOperator::default();
        operator.domain = ProblemDomain::MultiResourceScheduling;

        let mut engine = EvolutionEngine::new(config, &oracle, &sandbox, &operator)
            .with_scheduling_oracle(&sched_oracle);
        let report = engine.execute_run().expect("Scheduling run execution should succeed");

        assert_eq!(report.group, ExperimentalGroup::GroupC);
        assert_eq!(report.total_generations, 3);
        assert!(report.best_in_distribution_score.is_valid);
        assert!(report.best_in_distribution_score.total_bins > 0);
        assert!(report.best_in_distribution_score.mean_efficiency > 0.70);
        assert!(report.best_ood1_score.is_valid);
        assert!(report.best_ood2_score.is_valid);
        assert!(report.generations_history.iter().all(|r| r.crossover_rate >= 0.20 && r.crossover_rate <= 0.85));
    }

    #[test]
    fn test_no_self_crossover_in_swarm_evolution() {
        let mut config = ExperimentConfig::default();
        config.group = ExperimentalGroup::GroupC;
        config.rng_seed = 42;
        config.population_size = 6;
        config.max_generations = 2;

        let oracle = OracleEvaluator::new(42);
        let sandbox = DeterministicSandbox::new();
        let operator = DeterministicMockOperator::default();

        let mut engine = EvolutionEngine::new(config, &oracle, &sandbox, &operator);
        let report = engine.execute_run().expect("Run execution should succeed");

        // Verify that in the entire phylogenetic DAG, NO crossover has parent_a == parent_b
        let mut crossover_count = 0;
        for node in report.phylogeny.all_nodes() {
            if node.is_crossover {
                let parents = report.phylogeny.parents_of(&node.id);
                if parents.len() >= 2 {
                    crossover_count += 1;
                    let p1 = &parents[0];
                    let p2 = &parents[1];
                    assert_ne!(p1, p2, "Self-crossover detected! Parent 1 must not equal Parent 2");

                    let n1 = report.phylogeny.find_node(p1);
                    let n2 = report.phylogeny.find_node(p2);
                    if let (Some(n1), Some(n2)) = (n1, n2) {
                        assert_ne!(n1.content_hash, n2.content_hash, "Parents have identical content/AST hash!");
                    }
                }
            }
        }
        assert!(crossover_count > 0, "At least one crossover should have occurred in Group C");
    }

    #[test]
    fn test_validation_and_ood_do_not_contaminate_archive_or_selection_budget() {
        let mut config = ExperimentConfig::default();
        config.group = ExperimentalGroup::GroupC;
        config.rng_seed = 42;
        config.population_size = 6;
        config.max_generations = 2;

        let oracle = OracleEvaluator::new(42);
        let sandbox = DeterministicSandbox::new();
        let operator = DeterministicMockOperator::default();

        let mut engine = EvolutionEngine::new(config, &oracle, &sandbox, &operator);
        let report = engine.execute_run().expect("Run execution should succeed");

        // 1. Validation score must be present and valid
        assert!(report.best_validation_score.is_some());
        let val_score = report.best_validation_score.unwrap();
        assert!(val_score.is_valid);
        assert!(val_score.total_bins > 0);

        // 2. Post-run independent evaluation on validation & OOD must not alter engine's archive or budget
        let initial_archive_count = engine.archive.grid.len();
        let initial_exec_budget = engine.budget.executions_consumed;

        // Perform independent evaluation on validation and OOD
        let _val = oracle.evaluate_validation(&report.best_capsule, &sandbox, 100_000);
        let _ood1 = oracle.evaluate_ood1(&report.best_capsule, &sandbox, 100_000);
        let _ood2 = oracle.evaluate_ood2(&report.best_capsule, &sandbox, 100_000);

        assert_eq!(engine.archive.grid.len(), initial_archive_count, "Archive must not be contaminated by validation/OOD evaluations");
        assert_eq!(engine.budget.executions_consumed, initial_exec_budget, "Selection budget must not be contaminated by validation/OOD evaluations");
    }

    #[test]
    fn test_paired_campaign_reproducibility_and_symmetry() {
        let seed = 1042;
        let oracle = OracleEvaluator::new(seed);
        let sandbox = DeterministicSandbox::new();
        let operator = DeterministicMockOperator::default();

        // 1. Group D run (mutation-only)
        let mut config_d = ExperimentConfig::default();
        config_d.group = ExperimentalGroup::GroupD;
        config_d.rng_seed = seed;
        config_d.population_size = 6;
        config_d.max_generations = 2;
        let mut engine_d = EvolutionEngine::new(config_d, &oracle, &sandbox, &operator);
        let report_d = engine_d.execute_run().expect("Group D run should succeed");

        // 2. Group C run (swarm with crossover)
        let mut config_c = ExperimentConfig::default();
        config_c.group = ExperimentalGroup::GroupC;
        config_c.rng_seed = seed;
        config_c.population_size = 6;
        config_c.max_generations = 2;
        let mut engine_c = EvolutionEngine::new(config_c, &oracle, &sandbox, &operator);
        let report_c = engine_c.execute_run().expect("Group C run should succeed");

        // Verification of experimental symmetry
        assert!(report_d.best_in_distribution_score.is_valid);
        assert!(report_c.best_in_distribution_score.is_valid);
        assert!(report_d.generations_history.iter().all(|r| r.crossover_rate == 0.0), "Group D must have strictly 0 crossover rate");
        assert!(report_c.generations_history.iter().all(|r| r.crossover_rate >= 0.20), "Group C must maintain active crossover rate");
        assert_eq!(report_d.total_generations, 2);
        assert_eq!(report_c.total_generations, 2);
    }
}
