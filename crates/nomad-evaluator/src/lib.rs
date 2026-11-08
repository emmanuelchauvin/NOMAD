//! NOMAD Evaluator: Independent Oracle and test instances for Bin Packing Online 1D.

pub mod dataset;
pub mod oracle;
pub mod anti_dead_code;
pub mod scheduling;

pub use dataset::*;
pub use oracle::*;
pub use anti_dead_code::*;
pub use scheduling::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pre_registered_datasets_determinism() {
        let ds1 = Dataset::generate_in_distribution(42);
        let ds2 = Dataset::generate_in_distribution(42);

        // Same seed must produce bit-for-bit identical cryptographic fingerprint
        assert_eq!(ds1.dataset_hash, ds2.dataset_hash);
        assert_eq!(ds1.instances.len(), 50);
        assert_eq!(ds1.instances[0].items.len(), 50);

        let ood1 = Dataset::generate_ood1_scale(42);
        assert_eq!(ood1.instances.len(), 50);
        assert_eq!(ood1.instances[0].items.len(), 500);

        let ood2 = Dataset::generate_ood2_distribution_shift(42);
        assert_eq!(ood2.instances.len(), 50);
        assert_eq!(ood2.instances[0].items.len(), 500);
    }

    #[test]
    fn test_classical_baseline_evaluation() {
        let oracle = OracleEvaluator::new(42);
        let bf_score = oracle.evaluate_classical_baseline("BEST_FIT", &oracle.in_distribution);

        assert!(bf_score.is_valid);
        assert_eq!(bf_score.solved_instances, 50);
        assert!(bf_score.total_bins >= bf_score.total_theoretical_min_bins);
        assert!(bf_score.mean_efficiency > 0.80);
    }

    #[test]
    fn test_scheduling_datasets_and_baselines() {
        use nomad_sandbox::DeterministicSandbox;
        let sandbox = DeterministicSandbox::new();
        let oracle = SchedulingOracle::new(42);

        // Determinism test
        let ds2 = SchedulingDataset::generate_in_distribution(42);
        assert_eq!(oracle.in_distribution.dataset_hash, ds2.dataset_hash);
        assert_eq!(oracle.in_distribution.instances.len(), 50);

        // Baseline evaluations
        let ff_score = oracle.evaluate_classical_baseline("FIRST_FIT_NODE", &oracle.in_distribution, &sandbox);
        assert!(ff_score.is_valid);
        assert_eq!(ff_score.solved_instances, 50);

        let bal_score = oracle.evaluate_classical_baseline("BALANCED_FIT", &oracle.in_distribution, &sandbox);
        assert!(bal_score.is_valid);
        assert_eq!(bal_score.solved_instances, 50);
        assert!(bal_score.total_bins >= bal_score.total_theoretical_min_bins);

        // Vector balanced fit should achieve high efficiency (> 75%)
        assert!(bal_score.mean_efficiency > 0.75);
    }
}
