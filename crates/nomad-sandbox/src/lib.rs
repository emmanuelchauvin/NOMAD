//! NOMAD Sandbox: Resource-bounded deterministic execution engine.

pub mod error;
pub mod engine;
pub mod eval_ast;

pub use error::*;
pub use engine::*;
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
}
