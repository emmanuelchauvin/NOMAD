//! Banc hors ligne : elagage du champion final, en une passe (comme le moteur) ou plus fort
//! (aucun appel au modele).
//!
//! Pour chaque seed, prend le champion final d'un tir termine (derniere proposition marquee
//! `is_new_best`) et compare trois elagages. Tous gardent un retrait si le programme reste
//! valide sans perdre de bac sur l'entrainement, comme `prune_best_elite` dans le moteur :
//!   - `une_passe` : instructions du corps de la fonction (sauf l'expression rendue), du bas
//!     vers le haut, une seule passe : c'est l'elagage du moteur ;
//!   - `point_fixe` : la meme chose, repetee tant qu'un retrait passe ;
//!   - `profond` : en partant du resultat de `point_fixe`, toute instruction a toute
//!     profondeur (dans les `if`, les boucles, les blocs), repete jusqu'au point fixe.
//! Chaque resultat est aussi note sur la validation.
//!
//! Ecrit une ligne JSON par seed sur la sortie standard, l'avancement sur la sortie d'erreur.
//!
//! Usage : prune_probe <dossier> <suffixe, ex. evolution> <seed>...
//! Lit <dossier>/run_seed_<seed>_<suffixe>.json.
//! Variables : NOMAD_BP_PROFILE=weibull, NOMAD_FUEL_LIMIT (defaut 400000).

use std::collections::HashMap;
use std::time::Instant;

use nomad_core::{BenchmarkScore, ExperimentConfig, SkillCapsule};
use nomad_evaluator::OracleEvaluator;
use nomad_genome::{normalize_source, parse_heuristic_fn};
use nomad_sandbox::DeterministicSandbox;
use serde_json::{json, Value};
use syn::visit_mut::VisitMut;
use syn::{Block, ItemFn};

/// Parcourt les instructions de tous les blocs en ordre prefixe et retire celle de rang
/// `target`, sauf la derniere instruction du corps de la fonction (l'expression rendue).
/// Avec `target = usize::MAX`, ne retire rien et compte les instructions dans `seen`.
struct Remover {
    target: usize,
    seen: usize,
    depth: usize,
    removed: bool,
}

impl VisitMut for Remover {
    fn visit_block_mut(&mut self, block: &mut Block) {
        self.depth += 1;
        let mut i = 0;
        while i < block.stmts.len() && self.seen <= self.target && !self.removed {
            if self.seen == self.target {
                self.seen += 1;
                let protected = self.depth == 1 && i + 1 == block.stmts.len();
                if !protected {
                    block.stmts.remove(i);
                    self.removed = true;
                }
                break;
            }
            self.seen += 1;
            syn::visit_mut::visit_stmt_mut(self, &mut block.stmts[i]);
            i += 1;
        }
        self.depth -= 1;
    }
}

fn count_stmts(f: &ItemFn) -> usize {
    let mut copy = f.clone();
    let mut r = Remover {
        target: usize::MAX,
        seen: 0,
        depth: 0,
        removed: false,
    };
    r.visit_item_fn_mut(&mut copy);
    r.seen
}

fn remove_deep(f: &ItemFn, target: usize) -> Option<ItemFn> {
    let mut copy = f.clone();
    let mut r = Remover {
        target,
        seen: 0,
        depth: 0,
        removed: false,
    };
    r.visit_item_fn_mut(&mut copy);
    r.removed.then_some(copy)
}

fn remove_top(f: &ItemFn, i: usize) -> Option<ItemFn> {
    if i + 1 >= f.block.stmts.len() {
        return None;
    }
    let mut copy = f.clone();
    copy.block.stmts.remove(i);
    Some(copy)
}

struct Outcome {
    f: ItemFn,
    score: BenchmarkScore,
    removed: usize,
    evals: usize,
    secs: f64,
}

/// Retire les instructions une a une, du dernier rang vers le premier ; un retrait est garde
/// si le programme reste valide sans perdre de bac sur l'entrainement.
fn prune(
    start: &ItemFn,
    start_score: &BenchmarkScore,
    deep: bool,
    fixpoint: bool,
    train: &dyn Fn(&str) -> BenchmarkScore,
) -> Outcome {
    let t = Instant::now();
    let mut cur = start.clone();
    let mut score = start_score.clone();
    let (mut removed, mut evals) = (0, 0);
    loop {
        let mut changed = false;
        let n = if deep {
            count_stmts(&cur)
        } else {
            cur.block.stmts.len()
        };
        for k in (0..n).rev() {
            let trial = if deep {
                remove_deep(&cur, k)
            } else {
                remove_top(&cur, k)
            };
            let Some(trial) = trial else { continue };
            evals += 1;
            let s = train(&normalize_source(&trial));
            if s.is_valid && s.total_bins <= score.total_bins {
                cur = trial;
                score = s;
                removed += 1;
                changed = true;
            }
        }
        if !fixpoint || !changed {
            break;
        }
    }
    Outcome {
        f: cur,
        score,
        removed,
        evals,
        secs: t.elapsed().as_secs_f64(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = &args[1];
    let suffix = &args[2];
    let seeds: Vec<u64> = args[3..].iter().map(|s| s.parse().unwrap()).collect();
    let fuel: u64 = std::env::var("NOMAD_FUEL_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(400_000);
    let lookahead = ExperimentConfig::default().lookahead;
    let sandbox = DeterministicSandbox::new();

    for &seed in &seeds {
        let path = format!("{dir}/run_seed_{seed}_{suffix}.json");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path} : {e}"));
        let report: Value = serde_json::from_str(&text).unwrap();
        let champion = report["proposals"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["is_new_best"].as_bool() == Some(true))
            .last()
            .expect("aucun champion");
        let code = champion["logic_code"].as_str().unwrap();
        let original = parse_heuristic_fn(code).expect("champion illisible");

        let oracle = OracleEvaluator::new(seed);
        let capsule =
            |code: &str| SkillCapsule::new("p", "elagage", code, vec![], 0, HashMap::new());
        let train = |code: &str| {
            oracle.evaluate_dataset_with_lookahead(
                &oracle.in_distribution,
                &capsule(code),
                &sandbox,
                fuel,
                lookahead,
            )
        };
        let describe = |f: &ItemFn, s: &BenchmarkScore| {
            let src = normalize_source(f);
            let v = oracle.evaluate_validation(&capsule(&src), &sandbox, fuel);
            json!({
                "chars": src.chars().count(),
                "stmts_top": f.block.stmts.len(),
                "stmts_all": count_stmts(f),
                "train_valid": s.is_valid,
                "train_bins": s.total_bins,
                "val_valid": v.is_valid,
                "val_bins": v.total_bins,
            })
        };

        let t = Instant::now();
        let orig_score = train(&normalize_source(&original));
        let mut line = json!({
            "seed": seed,
            "champion_kind": champion["kind"],
            "logic_code_chars": code.chars().count(),
            "original": describe(&original, &orig_score),
        });
        eprintln!(
            "seed {seed} : original note en {:.0} s",
            t.elapsed().as_secs_f64()
        );

        let one = prune(&original, &orig_score, false, false, &train);
        let fix = prune(&original, &orig_score, false, true, &train);
        eprintln!(
            "seed {seed} : une passe et point fixe faits ({} + {} evaluations)",
            one.evals, fix.evals
        );
        let deep = prune(&fix.f, &fix.score, true, true, &train);
        eprintln!(
            "seed {seed} : profond fait ({} evaluations, {:.0} s)",
            deep.evals, deep.secs
        );

        for (name, o) in [
            ("une_passe", &one),
            ("point_fixe", &fix),
            ("profond", &deep),
        ] {
            let mut d = describe(&o.f, &o.score);
            d["removed"] = json!(o.removed);
            d["evals"] = json!(o.evals);
            d["secs"] = json!(o.secs);
            line[name] = d;
        }
        line["profond"]["code"] = json!(normalize_source(&deep.f));
        println!("{line}");
    }
}
