//! NOMAD Orchestrator: Multi-agent lifecycle, budget accounting, and evolutionary swarm reproduction.

pub mod engine;
pub mod error;
pub mod map_elites;
pub mod pso;
pub mod rebirth;

pub use engine::*;
pub use error::*;
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
        let config = ExperimentConfig {
            group: ExperimentalGroup::GroupC,
            rng_seed: 42,
            population_size: 6,
            max_generations: 3,
            ..Default::default()
        };

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
        assert!(report
            .generations_history
            .iter()
            .all(|r| r.crossover_rate >= 0.20 && r.crossover_rate <= 0.85));
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

        let config = ExperimentConfig {
            domain: ProblemDomain::MultiResourceScheduling,
            group: ExperimentalGroup::GroupC,
            rng_seed: 42,
            population_size: 6,
            max_generations: 3,
            ..Default::default()
        };

        let oracle = OracleEvaluator::new(42);
        let sched_oracle = SchedulingOracle::new(42);
        let sandbox = DeterministicSandbox::new();
        let operator = DeterministicMockOperator {
            domain: ProblemDomain::MultiResourceScheduling,
            ..Default::default()
        };

        let mut engine = EvolutionEngine::new(config, &oracle, &sandbox, &operator)
            .with_scheduling_oracle(&sched_oracle);
        let report = engine
            .execute_run()
            .expect("Scheduling run execution should succeed");

        assert_eq!(report.group, ExperimentalGroup::GroupC);
        assert_eq!(report.total_generations, 3);
        assert!(report.best_in_distribution_score.is_valid);
        assert!(report.best_in_distribution_score.total_bins > 0);
        assert!(report.best_in_distribution_score.mean_efficiency > 0.70);
        assert!(report.best_ood1_score.is_valid);
        assert!(report.best_ood2_score.is_valid);
        assert!(report
            .generations_history
            .iter()
            .all(|r| r.crossover_rate >= 0.20 && r.crossover_rate <= 0.85));
    }

    #[test]
    fn test_no_self_crossover_in_swarm_evolution() {
        let config = ExperimentConfig {
            group: ExperimentalGroup::GroupC,
            rng_seed: 42,
            population_size: 6,
            max_generations: 2,
            ..Default::default()
        };

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
                    assert_ne!(
                        p1, p2,
                        "Self-crossover detected! Parent 1 must not equal Parent 2"
                    );

                    let n1 = report.phylogeny.find_node(p1);
                    let n2 = report.phylogeny.find_node(p2);
                    if let (Some(n1), Some(n2)) = (n1, n2) {
                        assert_ne!(
                            n1.content_hash, n2.content_hash,
                            "Parents have identical content/AST hash!"
                        );
                    }
                }
            }
        }
        assert!(
            crossover_count > 0,
            "At least one crossover should have occurred in Group C"
        );
    }

    #[test]
    fn test_validation_and_ood_do_not_contaminate_archive_or_selection_budget() {
        let config = ExperimentConfig {
            group: ExperimentalGroup::GroupC,
            rng_seed: 42,
            population_size: 6,
            max_generations: 2,
            ..Default::default()
        };

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

        assert_eq!(
            engine.archive.grid.len(),
            initial_archive_count,
            "Archive must not be contaminated by validation/OOD evaluations"
        );
        assert_eq!(
            engine.budget.executions_consumed, initial_exec_budget,
            "Selection budget must not be contaminated by validation/OOD evaluations"
        );
    }

    #[test]
    fn test_paired_campaign_reproducibility_and_symmetry() {
        let seed = 1042;
        let oracle = OracleEvaluator::new(seed);
        let sandbox = DeterministicSandbox::new();
        let operator = DeterministicMockOperator::default();

        // 1. Group D run (mutation-only)
        let config_d = ExperimentConfig {
            group: ExperimentalGroup::GroupD,
            rng_seed: seed,
            population_size: 6,
            max_generations: 2,
            ..Default::default()
        };
        let mut engine_d = EvolutionEngine::new(config_d, &oracle, &sandbox, &operator);
        let report_d = engine_d.execute_run().expect("Group D run should succeed");

        // 2. Group C run (swarm with crossover)
        let config_c = ExperimentConfig {
            group: ExperimentalGroup::GroupC,
            rng_seed: seed,
            population_size: 6,
            max_generations: 2,
            ..Default::default()
        };
        let mut engine_c = EvolutionEngine::new(config_c, &oracle, &sandbox, &operator);
        let report_c = engine_c.execute_run().expect("Group C run should succeed");

        // Verification of experimental symmetry
        assert!(report_d.best_in_distribution_score.is_valid);
        assert!(report_c.best_in_distribution_score.is_valid);
        assert!(
            report_d
                .generations_history
                .iter()
                .all(|r| r.crossover_rate == 0.0),
            "Group D must have strictly 0 crossover rate"
        );
        assert!(
            report_c
                .generations_history
                .iter()
                .all(|r| r.crossover_rate >= 0.20),
            "Group C must maintain active crossover rate"
        );
        assert_eq!(report_d.total_generations, 2);
        assert_eq!(report_c.total_generations, 2);
    }

    #[test]
    fn test_crossover_acceptance_rate_and_not_evaluable_monitoring() {
        let seed = 42;
        let oracle = OracleEvaluator::new(seed);
        let sandbox = DeterministicSandbox::new();
        let operator = DeterministicMockOperator::default();

        let config = ExperimentConfig {
            group: ExperimentalGroup::GroupC,
            rng_seed: seed,
            population_size: 8,
            max_generations: 3,
            ..Default::default()
        };

        let mut engine = EvolutionEngine::new(config, &oracle, &sandbox, &operator);
        let report = engine.execute_run().expect("Run execution should succeed");

        // Verify that crossover attempts and rejections are actively tracked
        assert!(
            report.crossover_attempts > 0,
            "Group C must attempt crossovers"
        );
        assert_eq!(report.crossover_attempts, engine.crossover_attempts);
        assert_eq!(report.crossover_not_evaluable_count, engine.crossover_not_evaluable_count);
        assert_eq!(report.crossover_rejected_count, engine.crossover_rejected_count);

        let evaluable_crossovers = report.crossover_attempts.saturating_sub(report.crossover_not_evaluable_count);
        let evaluable_rate = evaluable_crossovers as f64 / report.crossover_attempts as f64;

        // Evaluable rate must remain optimal (> 80%, attrition strictly bounded)
        assert!(
            evaluable_rate >= 0.80,
            "Crossover evaluable rate must remain optimal (> 80%), got {:.2}% (attempts: {}, not_evaluable: {})",
            evaluable_rate * 100.0,
            report.crossover_attempts,
            report.crossover_not_evaluable_count
        );

        // Attrition due to ablation parsing/execution failure (NotEvaluable) must remain near zero
        assert_eq!(
            report.crossover_not_evaluable_count, 0,
            "Structural ablation must cleanly delete fragments without inducing NotEvaluable runtime failures"
        );

        // Verify generation records reflect the metrics
        for gen_rec in &report.generations_history {
            assert!(gen_rec.crossover_rate >= 0.20);
        }
    }

    #[test]
    fn test_initial_population_size_is_independent_of_population_size() {
        let config = ExperimentConfig {
            group: ExperimentalGroup::GroupD,
            rng_seed: 42,
            population_size: 4,
            max_generations: 3,
            initial_population_size: 10,
            stagnation_patience: 0,
            informed_mutation: false,
            ..Default::default()
        };
        let oracle = OracleEvaluator::new(42);
        let sandbox = DeterministicSandbox::new();
        let operator = DeterministicMockOperator::default();

        let mut engine = EvolutionEngine::new(config, &oracle, &sandbox, &operator);
        let report = engine.execute_run().expect("Run execution should succeed");

        // 10 propositions de depart, puis 2 generations de 3 mutations (l'elite est gratuit).
        assert_eq!(report.generations_history[0].llm_calls_consumed, 10);
        assert_eq!(report.budget_consumed.llm_calls_consumed, 10 + 2 * 3);
        assert_eq!(engine.fresh_seed_counter, 10);
    }

    #[test]
    fn test_stagnation_injects_fresh_proposals_within_the_same_budget() {
        let config = ExperimentConfig {
            group: ExperimentalGroup::GroupD,
            rng_seed: 42,
            population_size: 5,
            max_generations: 8,
            initial_population_size: 0,
            stagnation_patience: 1,
            informed_mutation: true,
            ..Default::default()
        };
        let oracle = OracleEvaluator::new(42);
        let sandbox = DeterministicSandbox::new();
        let operator = DeterministicMockOperator::default();

        let mut engine = EvolutionEngine::new(config, &oracle, &sandbox, &operator);
        let report = engine.execute_run().expect("Run execution should succeed");

        // Le budget ne depend pas de la relance : 5 au depart, puis 7 generations de 4 appels.
        assert_eq!(report.budget_consumed.llm_calls_consumed, 5 + 7 * 4);
        // Avec une patience de 1, l'operateur de test stagne et declenche des relances.
        assert!(
            engine.fresh_seed_counter > 5,
            "aucune proposition fraiche injectee (compteur {})",
            engine.fresh_seed_counter
        );
    }

    /// Operateur de test qui compte les mutations recues avec et sans contexte.
    struct ContextProbe {
        inner: DeterministicMockOperator,
        with_context: std::sync::atomic::AtomicUsize,
        with_best_other: std::sync::atomic::AtomicUsize,
        without_context: std::sync::atomic::AtomicUsize,
    }

    impl nomad_operator::SkillOperator for ContextProbe {
        fn generate_seed(
            &self,
            index: usize,
            seed: u64,
        ) -> Result<(nomad_core::SkillCapsule, nomad_operator::ReproducibilityAudit), nomad_operator::OperatorError>
        {
            self.inner.generate_seed(index, seed)
        }

        fn mutate_with_lineage(
            &self,
            parent: &nomad_core::SkillCapsule,
            generation: usize,
            seed: u64,
            lineage: Option<&nomad_core::LineageEvolutionState>,
        ) -> Result<(nomad_core::SkillCapsule, nomad_operator::ReproducibilityAudit), nomad_operator::OperatorError>
        {
            self.inner.mutate_with_lineage(parent, generation, seed, lineage)
        }

        fn mutate_informed(
            &self,
            parent: &nomad_core::SkillCapsule,
            generation: usize,
            seed: u64,
            lineage: Option<&nomad_core::LineageEvolutionState>,
            context: Option<&nomad_operator::MutationContext>,
        ) -> Result<(nomad_core::SkillCapsule, nomad_operator::ReproducibilityAudit), nomad_operator::OperatorError>
        {
            use std::sync::atomic::Ordering;
            match context {
                Some(ctx) => {
                    assert!(ctx.parent_bins > 0 && ctx.lower_bound_bins > 0);
                    self.with_context.fetch_add(1, Ordering::Relaxed);
                    if ctx.best_other.is_some() {
                        self.with_best_other.fetch_add(1, Ordering::Relaxed);
                    }
                }
                None => {
                    self.without_context.fetch_add(1, Ordering::Relaxed);
                }
            }
            self.inner.mutate_with_lineage(parent, generation, seed, lineage)
        }

        fn crossover(
            &self,
            parent_a: &nomad_core::SkillCapsule,
            parent_b: &nomad_core::SkillCapsule,
            generation: usize,
            seed: u64,
        ) -> Result<(nomad_core::SkillCapsule, nomad_operator::ReproducibilityAudit), nomad_operator::OperatorError>
        {
            self.inner.crossover(parent_a, parent_b, generation, seed)
        }
    }

    fn probe_run(informed: bool) -> (usize, usize, usize) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let config = ExperimentConfig {
            group: ExperimentalGroup::GroupD,
            rng_seed: 42,
            population_size: 5,
            max_generations: 6,
            initial_population_size: 12,
            stagnation_patience: 0,
            informed_mutation: informed,
            ..Default::default()
        };
        let oracle = OracleEvaluator::new(42);
        let sandbox = DeterministicSandbox::new();
        let operator = ContextProbe {
            inner: DeterministicMockOperator::default(),
            with_context: AtomicUsize::new(0),
            with_best_other: AtomicUsize::new(0),
            without_context: AtomicUsize::new(0),
        };
        let mut engine = EvolutionEngine::new(config, &oracle, &sandbox, &operator);
        engine.execute_run().expect("Run execution should succeed");
        (
            operator.with_context.load(Ordering::Relaxed),
            operator.with_best_other.load(Ordering::Relaxed),
            operator.without_context.load(Ordering::Relaxed),
        )
    }

    #[test]
    fn test_informed_mutation_passes_measured_context_to_the_operator() {
        // 5 generations de 4 mutations, toutes informees.
        let (with_context, with_best_other, without_context) = probe_run(true);
        assert_eq!((with_context, without_context), (20, 0));
        assert!(with_best_other > 0, "aucune mutation n'a recu le meilleur programme connu");

        let (with_context, _, without_context) = probe_run(false);
        assert_eq!((with_context, without_context), (0, 20));
    }
}
