//! Elagage et effet par regle des champions de runs termines, hors ligne (aucun appel).
//!
//! Pour chaque champion :
//!   1. Effet de chaque regle : bacs d'entrainement sans cette regle, moins bacs avec.
//!      Une regle dont le retrait rend le programme invalide (dependance, compilation) est
//!      notee `requise`.
//!   2. Elagage glouton : du bas vers le haut, une regle est retiree si l'entrainement reste
//!      valide et ne perd aucun bac. Deux passes au plus.
//!   3. Le champion elague est note sur la validation, comme l'original.
//! La derniere instruction (l'expression rendue) n'est jamais retiree.
//! Ecrit une ligne JSON par champion.
//!
//! Usage : prune_champions <dossier des run_seed_*_evolution.json> <seed>...
//! Variables : NOMAD_BP_PROFILE=weibull, NOMAD_FUEL_LIMIT (defaut 400000).

use nomad_core::{BenchmarkScore, ExperimentConfig, SkillCapsule};
use nomad_evaluator::OracleEvaluator;
use nomad_genome::{normalize_source, parse_heuristic_fn};
use nomad_sandbox::DeterministicSandbox;

fn without(code: &str, keep: &[bool]) -> String {
    let mut f = parse_heuristic_fn(code).unwrap();
    let stmts = std::mem::take(&mut f.block.stmts);
    f.block.stmts = stmts
        .into_iter()
        .zip(keep.iter())
        .filter(|(_, k)| **k)
        .map(|(s, _)| s)
        .collect();
    normalize_source(&f)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = &args[1];
    let fuel: u64 = std::env::var("NOMAD_FUEL_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(400_000);
    let lookahead = ExperimentConfig::default().lookahead;
    let sandbox = DeterministicSandbox::new();

    for seed_arg in &args[2..] {
        let seed: u64 = seed_arg.parse().unwrap();
        let path = format!("{dir}/run_seed_{seed}_evolution.json");
        let report: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let champion: SkillCapsule = serde_json::from_value(report["best_capsule"].clone()).unwrap();
        let oracle = OracleEvaluator::new(seed);

        let eval = |code: &str| -> BenchmarkScore {
            let mut c = champion.clone();
            c.logic_code = code.to_string();
            oracle.evaluate_dataset_with_lookahead(&oracle.in_distribution, &c, &sandbox, fuel, lookahead)
        };
        let eval_val = |code: &str| -> BenchmarkScore {
            let mut c = champion.clone();
            c.logic_code = code.to_string();
            oracle.evaluate_validation(&c, &sandbox, fuel)
        };

        let original = normalize_source(&parse_heuristic_fn(&champion.logic_code).unwrap());
        let n = parse_heuristic_fn(&original).unwrap().block.stmts.len();
        let base = eval(&original);

        // 1. Effet de chaque regle, retiree seule.
        let mut effects = Vec::new();
        for i in 0..n.saturating_sub(1) {
            let mut keep = vec![true; n];
            keep[i] = false;
            let s = eval(&without(&original, &keep));
            effects.push(if s.is_valid {
                serde_json::json!(s.total_bins as i64 - base.total_bins as i64)
            } else {
                serde_json::json!("requise")
            });
        }

        // 2. Elagage glouton.
        let mut keep = vec![true; n];
        let mut current = base.clone();
        for _pass in 0..2 {
            let mut changed = false;
            for i in (0..n.saturating_sub(1)).rev() {
                if !keep[i] {
                    continue;
                }
                keep[i] = false;
                let s = eval(&without(&original, &keep));
                if s.is_valid && s.total_bins <= current.total_bins {
                    current = s;
                    changed = true;
                } else {
                    keep[i] = true;
                }
            }
            if !changed {
                break;
            }
        }
        let pruned = without(&original, &keep);

        // 3. Validation.
        let val_orig = eval_val(&original);
        let val_pruned = eval_val(&pruned);

        let row = serde_json::json!({
            "seed": seed,
            "rules": n - 1,
            "rules_kept": keep.iter().filter(|k| **k).count() - 1,
            "code_len": original.len(),
            "code_len_pruned": pruned.len(),
            "train_bins": base.total_bins,
            "train_bins_pruned": current.total_bins,
            "fuel": base.total_fuel,
            "fuel_pruned": current.total_fuel,
            "val_valid": val_orig.is_valid,
            "val_bins": val_orig.total_bins,
            "val_valid_pruned": val_pruned.is_valid,
            "val_bins_pruned": val_pruned.total_bins,
            "effects": effects,
            "pruned_code": pruned,
        });
        println!("{row}");
    }
}
