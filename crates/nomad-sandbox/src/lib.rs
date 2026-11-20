//! NOMAD Sandbox: Resource-bounded deterministic execution engine.

pub mod engine;
pub mod error;
pub mod eval_ast;

pub use engine::*;
pub use error::*;
pub use eval_ast::*;

#[cfg(test)]
mod tests {
    use super::*;
    use nomad_core::PackingDecision;

    #[test]
    fn test_sandbox_first_fit_and_best_fit() {
        let sandbox = DeterministicSandbox::new();
        let bins = vec![40, 70, 95];
        let capacity = 100;
        let fuel_limit = 10_000;

        // Item 25: First fit should place in index 0 (40 + 25 = 65 <= 100)
        let outcome_ff = sandbox
            .execute("FIRST_FIT", 25, &bins, capacity, fuel_limit)
            .unwrap();
        assert_eq!(outcome_ff.decision, PackingDecision::PlaceInExisting(0));

        // Item 25: Best fit should place in index 1 (70 + 25 = 95 <= 100, leaving only 5 space!)
        let outcome_bf = sandbox
            .execute("BEST_FIT", 25, &bins, capacity, fuel_limit)
            .unwrap();
        assert_eq!(outcome_bf.decision, PackingDecision::PlaceInExisting(1));

        // Item 40: Won't fit in 70 or 95. Fits in 40 (40 + 40 = 80).
        let outcome_item40 = sandbox
            .execute("BEST_FIT", 40, &bins, capacity, fuel_limit)
            .unwrap();
        assert_eq!(outcome_item40.decision, PackingDecision::PlaceInExisting(0));

        // Item 70: Doesn't fit in any open bin (40+70 > 100, 70+70 > 100, 95+70 > 100) -> OpenNewBin
        let outcome_item70 = sandbox
            .execute("BEST_FIT", 70, &bins, capacity, fuel_limit)
            .unwrap();
        assert_eq!(outcome_item70.decision, PackingDecision::OpenNewBin);
    }

    #[test]
    fn test_sandbox_overflow_protection() {
        let sandbox = DeterministicSandbox::new();
        // Item exceeds capacity 100
        let res = sandbox.execute("FIRST_FIT", 105, &[], 100, 10_000);
        assert!(res.is_err());
    }

    #[test]
    fn test_sandbox_scheduling_execution() {
        use nomad_core::SchedulingDecision;
        let sandbox = DeterministicSandbox::new();
        let node_levels = vec![(10, 30), (28, 120)]; // (cpu, ram) used
        let capacity = (32, 128); // 32 cores, 128 GB
        let fuel_limit = 20_000;

        // Job requiring 4 cores, 16 GB
        // First fit node should place in index 0 (10+4=14 <= 32, 30+16=46 <= 128)
        let outcome_ff = sandbox
            .execute_scheduling(
                "FIRST_FIT_NODE",
                4,
                16,
                &node_levels,
                capacity,
                fuel_limit,
                0,
                10,
            )
            .unwrap();
        assert_eq!(outcome_ff.decision, SchedulingDecision::PlaceOnNode(0));

        // Job requiring 20 cores, 10 GB (node 1 has only 4 cores left, fits only on node 0)
        let outcome_job20 = sandbox
            .execute_scheduling(
                "BALANCED_FIT",
                20,
                10,
                &node_levels,
                capacity,
                fuel_limit,
                1,
                10,
            )
            .unwrap();
        assert_eq!(outcome_job20.decision, SchedulingDecision::PlaceOnNode(0));

        // Job requiring 30 cores, 100 GB (fits on neither node 0 nor node 1) -> OpenNewNode
        let outcome_new = sandbox
            .execute_scheduling(
                "BALANCED_FIT",
                30,
                100,
                &node_levels,
                capacity,
                fuel_limit,
                2,
                10,
            )
            .unwrap();
        assert_eq!(outcome_new.decision, SchedulingDecision::OpenNewNode);
    }

    #[test]
    fn test_sandbox_baseline_exact_match_protection() {
        let sandbox = DeterministicSandbox::new();
        // Synthesized code mentioning "best_fit_lookahead" in comments and variables
        let code = r#"
        // This is a custom heuristic inspired by best_fit_lookahead
        pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {
            let best_fit_lookahead_factor = 42.0;
            let mut score = (bin_residual as f64) + best_fit_lookahead_factor;
            score
        }
        "#;
        let bins = vec![40, 70];
        let outcome = sandbox.execute(code, 20, &bins, 100, 10_000).unwrap();
        // Since score = bin_residual + 42, for bin 0: 60 + 42 = 102. For bin 1: 30 + 42 = 72.
        // Lowest score is bin 1!
        assert_eq!(outcome.decision, PackingDecision::PlaceInExisting(1));
    }
}
