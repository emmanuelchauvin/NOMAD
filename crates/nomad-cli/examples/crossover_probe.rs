//! Mesure hors ligne du croisement AST sur de vrais programmes (aucun appel au modele).
//!
//! Prend les archives finales de runs d'evolution deja termines et croise leurs elites :
//!   - `intra` : hote et donneur dans l'archive du meme run ;
//!   - `inter` : hote dans l'archive du run A, donneur = champion d'un autre run (ce que
//!     ferait une migration entre iles), le tout evalue sur les instances du run A.
//! Chaque enfant passe par la meme evaluation que dans le moteur (controle a 3 niveaux).
//! Ecrit une ligne JSON par enfant sur la sortie standard.
//!
//! Les hotes sont tires parmi les `top` meilleures elites de l'archive : la question utile est
//! de savoir si le croisement ameliore les meilleurs programmes, pas les plus faibles.
//!
//! Usage : crossover_probe <dossier des run_seed_*_evolution.json> <paires par condition> <top> <seed>...
//! Variables : NOMAD_BP_PROFILE=weibull, NOMAD_FUEL_LIMIT (defaut 400000).

use std::collections::HashSet;

use nomad_core::{ExperimentConfig, SkillCapsule, ValidationStatus};
use nomad_evaluator::{validate_anti_dead_code_3levels_with_score, OracleEvaluator};
use nomad_genome::{
    deterministic_ast_crossover, multi_locus_ast_crossover, regime_dispatch_crossover,
    REGIME_VARIANTS,
};
use nomad_sandbox::DeterministicSandbox;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // splitmix64
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Elites de l'archive finale d'un run, du meilleur au moins bon (score d'entrainement du journal).
fn load_archive(dir: &str, seed: u64) -> Vec<SkillCapsule> {
    let path = format!("{dir}/run_seed_{seed}_evolution.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path} : {e}"));
    let report: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut elites: Vec<(u64, SkillCapsule)> = report["archive"]["grid"]
        .as_object()
        .unwrap()
        .values()
        .map(|e| {
            let bins = e["fitness"]["total_bins"].as_u64().unwrap();
            let capsule: SkillCapsule = serde_json::from_value(e["capsule"].clone()).unwrap();
            (bins, capsule)
        })
        .collect();
    elites.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.id.0.cmp(&b.1.id.0)));
    let mut seen = HashSet::new();
    elites
        .into_iter()
        .filter(|(_, c)| seen.insert(c.logic_code.clone()))
        .map(|(_, c)| c)
        .collect()
}

fn status_label(s: &ValidationStatus) -> &'static str {
    match s {
        ValidationStatus::Unvalidated => "unvalidated",
        ValidationStatus::PassedLevel1Structural => "level1",
        ValidationStatus::PassedLevel2Computational => "level2_neutral",
        ValidationStatus::PassedLevel3Behavioral => "level3_passed",
        ValidationStatus::NotEvaluable(_) => "not_evaluable",
        ValidationStatus::GraftHarmful(_) => "graft_harmful",
        ValidationStatus::Failed(_) => "failed",
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = &args[1];
    let pairs: usize = args[2].parse().unwrap();
    let top: usize = args[3].parse().unwrap();
    let seeds: Vec<u64> = args[4..].iter().map(|s| s.parse().unwrap()).collect();
    let fuel: u64 = std::env::var("NOMAD_FUEL_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(400_000);
    let config = ExperimentConfig::default();
    // NOMAD_PROBE_DISPATCH=1 : mesure l'aiguillage de regime au lieu de la greffe.
    let dispatch = std::env::var("NOMAD_PROBE_DISPATCH").is_ok();
    let sandbox = DeterministicSandbox::new();

    let archives: Vec<Vec<SkillCapsule>> = seeds.iter().map(|s| load_archive(dir, *s)).collect();

    for (si, &seed) in seeds.iter().enumerate() {
        let oracle = OracleEvaluator::new(seed);
        let train = |c: &SkillCapsule| {
            oracle.evaluate_dataset_with_lookahead(
                &oracle.in_distribution,
                c,
                &sandbox,
                fuel,
                config.lookahead,
            )
        };
        let archive = &archives[si];
        let host_scores: Vec<_> = archive.iter().map(&train).collect();
        let mut rng = Rng(seed ^ 0xC0FF_EE);

        for cond in ["intra", "inter"] {
            for _ in 0..pairs {
                let hi = rng.below(top.min(archive.len()));
                let host = &archive[hi];
                let (donor, donor_origin, donor_rank) = if cond == "intra" {
                    let mut di = rng.below(archive.len());
                    while di == hi {
                        di = rng.below(archive.len());
                    }
                    (&archive[di], seed, di)
                } else {
                    let mut oi = rng.below(seeds.len());
                    while oi == si {
                        oi = rng.below(seeds.len());
                    }
                    (&archives[oi][0], seeds[oi], 0)
                };
                let cross_seed = rng.next();
                // Meme choix d'operateur que HttpLlamaOperator::crossover.
                let res = if cross_seed % 2 == 0 {
                    multi_locus_ast_crossover(host, donor, cross_seed, 1)
                        .or_else(|_| deterministic_ast_crossover(host, donor, cross_seed, 1))
                } else {
                    deterministic_ast_crossover(host, donor, cross_seed, 1)
                        .or_else(|_| multi_locus_ast_crossover(host, donor, cross_seed, 1))
                };
                if dispatch {
                    // Aiguillage de regime : memes paires, toutes les conditions, pas de greffe.
                    // Donneur note une fois par paire ; conditions retenues : avancement,
                    // taille de l'objet, bac (une par famille).
                    let donor_score = train(donor);
                    for v in [0usize, 3, 5] {
                        debug_assert!(v < REGIME_VARIANTS);
                        let mut row = serde_json::json!({
                            "cond": cond, "seed": seed, "host_rank": hi, "donor_seed": donor_origin,
                            "donor_rank": donor_rank, "kind": "regime_dispatch", "variant": v,
                            "host_bins": host_scores[hi].total_bins,
                            "champion_bins": host_scores.iter().filter(|s| s.is_valid).map(|s| s.total_bins).min(),
                        });
                        match regime_dispatch_crossover(host, donor, v, 1) {
                            Err(e) => row["error"] = e.to_string().into(),
                            Ok(child) => {
                                let score = train(&child);
                                row["regime"] = child.metadata.get("regime").cloned().into();
                                row["child_valid"] = score.is_valid.into();
                                row["child_bins"] = score.total_bins.into();
                                row["child_fuel"] = score.total_fuel.into();
                                row["host_fuel"] = host_scores[hi].total_fuel.into();
                                row["donor_valid"] = donor_score.is_valid.into();
                                row["donor_bins"] = donor_score.total_bins.into();
                                let best_parent = if donor_score.is_valid {
                                    host_scores[hi].total_bins.min(donor_score.total_bins)
                                } else {
                                    host_scores[hi].total_bins
                                };
                                if score.is_valid && score.total_bins < best_parent {
                                    let v_child = oracle.evaluate_validation(&child, &sandbox, fuel);
                                    let v_host = oracle.evaluate_validation(host, &sandbox, fuel);
                                    let v_donor = oracle.evaluate_validation(donor, &sandbox, fuel);
                                    row["val_child_valid"] = v_child.is_valid.into();
                                    row["val_child_bins"] = v_child.total_bins.into();
                                    row["val_host_bins"] = v_host.total_bins.into();
                                    row["val_donor_valid"] = v_donor.is_valid.into();
                                    row["val_donor_bins"] = v_donor.total_bins.into();
                                    row["child_code"] = child.logic_code.clone().into();
                                }
                            }
                        }
                        println!("{row}");
                    }
                    continue;
                }
                let mut row = serde_json::json!({
                    "cond": cond, "seed": seed, "host_rank": hi, "donor_seed": donor_origin,
                    "donor_rank": donor_rank,
                    "host_valid": host_scores[hi].is_valid, "host_bins": host_scores[hi].total_bins,
                    "champion_bins": host_scores.iter().filter(|s| s.is_valid).map(|s| s.total_bins).min(),
                });
                match res {
                    Err(e) => {
                        row["status"] = "crossover_error".into();
                        row["error"] = e.to_string().into();
                    }
                    Ok((mut child, _)) => {
                        row["kind"] = child.metadata.get("crossover_type").cloned().into();
                        row["code_len_host"] = host.logic_code.len().into();
                        row["code_len_child"] = child.logic_code.len().into();
                        let score = match validate_anti_dead_code_3levels_with_score(
                            &mut child,
                            &oracle,
                            &sandbox,
                            &oracle.in_distribution,
                            fuel,
                            config.tldr_epsilon,
                            config.lookahead,
                        ) {
                            Ok((_, s)) => s,
                            Err(_) => train(&child),
                        };
                        row["status"] = status_label(&child.validation).into();
                        row["child_valid"] = score.is_valid.into();
                        row["child_bins"] = score.total_bins.into();
                        if !score.is_valid {
                            // Cause de l'invalidite : premier message d'erreur sur l'entrainement.
                            let (_, evals) = oracle.evaluate_dataset_with_evals(
                                &oracle.in_distribution, &child, &sandbox, fuel, config.lookahead);
                            row["error"] = evals.iter().find_map(|e| e.error_message.clone()).into();
                            row["child_code"] = child.logic_code.clone().into();
                        }
                        if score.is_valid && score.total_bins < host_scores[hi].total_bins {
                            let v_child = oracle.evaluate_validation(&child, &sandbox, fuel);
                            let v_host = oracle.evaluate_validation(host, &sandbox, fuel);
                            let donor_score = train(donor);
                            row["donor_valid"] = donor_score.is_valid.into();
                            row["donor_bins"] = donor_score.total_bins.into();
                            row["val_child_valid"] = v_child.is_valid.into();
                            row["val_child_bins"] = v_child.total_bins.into();
                            row["val_host_bins"] = v_host.total_bins.into();
                            row["child_code"] = child.logic_code.clone().into();
                        }
                    }
                }
                println!("{row}");
            }
        }
    }
}
