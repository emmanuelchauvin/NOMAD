use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use syn::visit::Visit;
use syn::visit_mut::VisitMut;
use syn::{Expr, ExprLit, ItemFn, Lit, LitFloat, LitInt};

use nomad_core::{BenchmarkScore, SkillCapsule};
use nomad_genome::ast::{compute_structural_fingerprint, parse_heuristic_fn};
use nomad_sandbox::HeuristicSandbox;

use crate::oracle::OracleEvaluator;

/// Compteur et indexeur de littéraux numériques continus et discrets modifiables
#[derive(Default)]
struct NumericLiteralCounter {
    float_count: usize,
    int_count: usize,
}

impl<'ast> Visit<'ast> for NumericLiteralCounter {
    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let Expr::Lit(ExprLit { lit, .. }) = expr {
            match lit {
                Lit::Float(_) => self.float_count += 1,
                Lit::Int(li) => {
                    if let Ok(v) = li.base10_parse::<i64>() {
                        // On n'optimise que les seuils significatifs > 1 (exclut les 0 et 1 invariants)
                        if v > 1 {
                            self.int_count += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        syn::visit::visit_expr(self, expr);
    }
}

/// Mutateur AST ciblant un littéral spécifique par son index
struct NumericLiteralMutator {
    target_is_float: bool,
    target_idx: usize,
    current_float_idx: usize,
    current_int_idx: usize,
    float_multiplier: f64,
    int_offset: i64,
}

impl VisitMut for NumericLiteralMutator {
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        if let Expr::Lit(ExprLit { lit, .. }) = expr {
            match lit {
                Lit::Float(lf) => {
                    if self.target_is_float && self.current_float_idx == self.target_idx {
                        if let Ok(val) = lf.base10_parse::<f64>() {
                            let new_val = val * self.float_multiplier;
                            if let Ok(new_lit) = syn::parse_str::<LitFloat>(&format!("{:.3}", new_val)) {
                                *lf = new_lit;
                            }
                        }
                    }
                    self.current_float_idx += 1;
                }
                Lit::Int(li) => {
                    if let Ok(val) = li.base10_parse::<i64>() {
                        if val > 1 {
                            if !self.target_is_float && self.current_int_idx == self.target_idx {
                                let new_val = (val + self.int_offset).max(1);
                                if let Ok(new_lit) = syn::parse_str::<LitInt>(&new_val.to_string()) {
                                    *li = new_lit;
                                }
                            }
                            self.current_int_idx += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        syn::visit_mut::visit_expr_mut(self, expr);
    }
}

/// ============================================================================
/// optimize_capsule_constants : Optimiseur Local de Constantes par Hill-Climbing
/// ============================================================================
/// Extrait les littéraux numériques (`f64` et `u32` > 1) de l'AST et exécute une
/// recherche locale déterministe (Coordinate Ascent / Hill-Climbing) pour affiner
/// les constantes continues et les seuils entiers de l'heuristique.
pub fn optimize_capsule_constants(
    capsule: &SkillCapsule,
    oracle: &OracleEvaluator,
    sandbox: &dyn HeuristicSandbox,
    fuel_limit: u64,
    steps: usize,
    seed: u64,
) -> (SkillCapsule, BenchmarkScore) {
    let current_item_fn: ItemFn = match parse_heuristic_fn(&capsule.logic_code) {
        Ok(f) => f,
        Err(_) => {
            let score = oracle.evaluate_in_distribution(capsule, sandbox, fuel_limit);
            return (capsule.clone(), score);
        }
    };

    let current_score = oracle.evaluate_in_distribution(capsule, sandbox, fuel_limit);
    if !current_score.is_valid {
        return (capsule.clone(), current_score);
    }

    let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0x0571_CA1E);
    let mut best_item_fn = current_item_fn;
    let mut best_score = current_score.clone();
    let mut improved = false;

    // Facteurs de perturbation continus et discrets (±15%, ±30%, ±2, ±5)
    let float_deltas = [0.85, 1.15, 0.70, 1.30, 0.90, 1.10];
    let int_deltas = [-2i64, 2, -5, 5, -1, 1];

    for _step in 0..steps {
        let mut counter = NumericLiteralCounter::default();
        counter.visit_item_fn(&best_item_fn);

        if counter.float_count == 0 && counter.int_count == 0 {
            break;
        }

        // Choix du type de littéral et de l'index cible
        let pick_float = if counter.float_count > 0 && counter.int_count > 0 {
            rng.gen_bool(0.65)
        } else {
            counter.float_count > 0
        };

        let mut candidate_fn = best_item_fn.clone();
        if pick_float && counter.float_count > 0 {
            let target_idx = rng.gen_range(0..counter.float_count);
            let multiplier = float_deltas[rng.gen_range(0..float_deltas.len())];
            let mut mutator = NumericLiteralMutator {
                target_is_float: true,
                target_idx,
                current_float_idx: 0,
                current_int_idx: 0,
                float_multiplier: multiplier,
                int_offset: 0,
            };
            mutator.visit_item_fn_mut(&mut candidate_fn);
        } else if counter.int_count > 0 {
            let target_idx = rng.gen_range(0..counter.int_count);
            let offset = int_deltas[rng.gen_range(0..int_deltas.len())];
            let mut mutator = NumericLiteralMutator {
                target_is_float: false,
                target_idx,
                current_float_idx: 0,
                current_int_idx: 0,
                float_multiplier: 1.0,
                int_offset: offset,
            };
            mutator.visit_item_fn_mut(&mut candidate_fn);
        }

        let new_code = quote::quote!(#candidate_fn).to_string();
        let mut temp_capsule = capsule.clone();
        temp_capsule.logic_code = new_code;

        let eval_score = oracle.evaluate_in_distribution(&temp_capsule, sandbox, fuel_limit);
        if !eval_score.is_valid {
            continue;
        }

        let is_strictly_better = if eval_score.total_bins < best_score.total_bins {
            true
        } else if eval_score.total_bins == best_score.total_bins {
            eval_score.tldr_score > best_score.tldr_score + 1e-4
        } else {
            false
        };

        if is_strictly_better {
            best_score = eval_score;
            best_item_fn = candidate_fn;
            improved = true;
        }
    }

    if improved {
        let optimized_code = quote::quote!(#best_item_fn).to_string();
        let fp = Some(compute_structural_fingerprint(&best_item_fn));
        let mut opt_capsule = capsule.clone();
        opt_capsule.logic_code = optimized_code;
        opt_capsule.ast_fingerprint = fp;
        opt_capsule.semantic_interface = format!("{} (constants optimized)", capsule.semantic_interface);
        (opt_capsule, best_score)
    } else {
        (capsule.clone(), current_score)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nomad_core::PolicyKind;
    use nomad_sandbox::DeterministicSandbox;
    use std::collections::HashMap;

    #[test]
    fn test_optimize_capsule_constants_execution() {
        let oracle = OracleEvaluator::new(42);
        let sandbox = DeterministicSandbox::new();

        let initial_code = r#"
        pub fn score_bin(
            item_size: u32,
            bin_residual: u32,
            bin_capacity: u32,
            step_index: usize,
            total_items: usize,
            open_bins_count: usize,
            mean_residual: f64,
            next_item_size: u32,
            post_residual: u32,
            fitting_bins_count: usize,
        ) -> f64 {
            let mut score = post_residual as f64 * 1.0;
            if post_residual < 10 {
                score = score + 50.0;
            }
            score
        }
        "#;

        let capsule = SkillCapsule::new_v2(
            "TestCapsule",
            "Initial unoptimized baseline",
            initial_code,
            vec![],
            0,
            HashMap::new(),
            "ScorePolicyV1",
            PolicyKind::ScorePolicy,
            None,
            vec![],
            nomad_core::ValidationStatus::PassedLevel1Structural,
            None,
        );

        let (optimized, score) = optimize_capsule_constants(&capsule, &oracle, &sandbox, 50_000, 10, 42);
        assert!(score.is_valid);
        assert!(score.total_bins > 0);
        assert!(!optimized.logic_code.is_empty());
    }
}

