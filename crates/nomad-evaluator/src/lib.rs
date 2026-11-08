//! NOMAD Evaluator: Independent Oracle and test instances for Bin Packing Online 1D.

pub mod dataset;
pub mod oracle;
pub mod anti_dead_code;
pub mod scheduling;
pub mod optimizer;

pub use dataset::*;
pub use oracle::*;
pub use anti_dead_code::*;
pub use scheduling::*;
pub use optimizer::*;

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

    #[test]
    fn test_lookahead_bin_packing_combinations() {
        use nomad_sandbox::DeterministicSandbox;
        use nomad_core::SkillCapsule;
        let sandbox = DeterministicSandbox::new();
        let oracle = OracleEvaluator::new(42);

        let bf_capsule = SkillCapsule::new("BF", "", "BEST_FIT", vec![], 0, std::collections::HashMap::new());
        let bf_score = oracle.evaluate_in_distribution(&bf_capsule, &sandbox, 50_000);
        println!(">>> BEST FIT In-Dist: {} bins", bf_score.total_bins);

        // Test 1: Best Fit + Exact closure bonus
        let c1 = r#"pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64, next_item_size: u32) -> f64 {
            let mut score = bin_residual as f64;
            if bin_residual == 0 {
                score -= 50.0;
            }
            score
        }"#;
        let s1 = oracle.evaluate_in_distribution(&SkillCapsule::new("C1", "", c1, vec![], 0, Default::default()), &sandbox, 50_000);
        println!(">>> Best Fit + Exact Closure: {} bins", s1.total_bins);

        // Test 2: Best Fit + Horizon Dynamic
        let c2 = r#"pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64, next_item_size: u32) -> f64 {
            let progress = step_index as f64 / total_items as f64;
            let mut score = bin_residual as f64;
            if progress > 0.7 && bin_residual < 15 {
                score -= 20.0;
            }
            if bin_residual == 0 {
                score -= 30.0;
            }
            score
        }"#;
        let s2 = oracle.evaluate_in_distribution(&SkillCapsule::new("C2", "", c2, vec![], 0, Default::default()), &sandbox, 50_000);
        println!(">>> Best Fit + Horizon: {} bins", s2.total_bins);

        // Test 3: The 1264 Champion from run_report_qwen_v2.json
        // Test 3: The 1264 Champion from run_v2 with updated signature using post_residual
        let c3 = r#"pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64, next_item_size: u32, post_residual: u32, fitting_bins_count: usize) -> f64 {
            let progress = step_index as f64 / total_items as f64;
            let dynamic_factor = if progress < 0.5 { 1.0 } else { 2.0 };
            let decay_penalty = (1.0 - progress).powi(3);
            let mut score = post_residual as f64 
                - item_size as f64 * dynamic_factor 
                - open_bins_count as f64 / total_items as f64 * 5.0 
                + mean_residual * (1.0 - decay_penalty);

            if progress < 0.3 {
                score -= open_bins_count as f64 / total_items as f64 * 5.0;
            } else if progress > 0.7 {
                score += mean_residual * decay_penalty;
            }

            let non_linear_factor = if post_residual < item_size { 2.0 } else { 1.0 };
            let adjusted_score = score * non_linear_factor;
            let time_decay_penalty = (1.0 - progress).powi(4);
            let penalized_score = adjusted_score * (1.0 + time_decay_penalty);
            let horizon_multiplier = (total_items - step_index) as f64 / total_items as f64;
            penalized_score * (1.0 + horizon_multiplier)
        }"#;
        let s3 = oracle.evaluate_in_distribution(&SkillCapsule::new("C3", "", c3, vec![], 0, Default::default()), &sandbox, 50_000);
        println!(">>> 1264 Champion from v2: {} bins", s3.total_bins);

        // Test 4: The 1264 Champion WITH lookahead pairing bonus
        let c4 = r#"pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64, next_item_size: u32, post_residual: u32, fitting_bins_count: usize) -> f64 {
            let progress = step_index as f64 / total_items as f64;
            let dynamic_factor = if progress < 0.5 { 1.0 } else { 2.0 };
            let decay_penalty = (1.0 - progress).powi(3);
            let mut score = post_residual as f64 
                - item_size as f64 * dynamic_factor 
                - open_bins_count as f64 / total_items as f64 * 5.0 
                + mean_residual * (1.0 - decay_penalty);

            if progress < 0.3 {
                score -= open_bins_count as f64 / total_items as f64 * 5.0;
            } else if progress > 0.7 {
                score += mean_residual * decay_penalty;
            }

            // Lookahead micro-matching
            if post_residual == next_item_size && next_item_size > 0 {
                score -= 35.0 * (1.0 + progress);
            }

            let non_linear_factor = if post_residual < item_size { 2.0 } else { 1.0 };
            let adjusted_score = score * non_linear_factor;
            let time_decay_penalty = (1.0 - progress).powi(4);
            let penalized_score = adjusted_score * (1.0 + time_decay_penalty);
            let horizon_multiplier = (total_items - step_index) as f64 / total_items as f64;
            penalized_score * (1.0 + horizon_multiplier)
        }"#;
        let s4 = oracle.evaluate_in_distribution(&SkillCapsule::new("C4", "", c4, vec![], 0, Default::default()), &sandbox, 50_000);
        assert_eq!(s3.total_bins, 1264);
        assert_eq!(s4.total_bins, 1264);
    }
}
