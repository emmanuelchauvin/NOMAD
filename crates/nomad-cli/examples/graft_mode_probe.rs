//! Banc hors ligne : greffe avec melanges contre greffe de regles completes (aucun appel au
//! modele).
//!
//! Rejoue les croisements d'un tir deja termine sur les memes paires hote/donneur (lues dans le
//! journal des propositions, `kind == "crossover"`), une fois dans chaque mode de greffe :
//!   - `mixing` : comportement historique, une definition du donneur que l'hote ne lit pas
//!     devient un melange `score = score * 0.95 + e * 0.05` ;
//!   - `rules` : seules les instructions du donneur qui modifient `score` sont greffees.
//! Les deux modes recoivent la meme graine de croisement et le meme choix d'operateur que le
//! moteur. L'enfant est juge comme dans le moteur : garde s'il bat son hote sur l'entrainement.
//! Les enfants gardes sont aussi notes sur la validation, avec leur hote.
//!
//! Ecrit une ligne JSON par paire sur la sortie standard, l'avancement sur la sortie d'erreur.
//!
//! Usage : graft_mode_probe <dossier> <suffixe des fichiers, ex. local> <paires max par seed, 0 = toutes> <seed>...
//! Lit <dossier>/run_seed_<seed>_<suffixe>.json.
//! Variables : NOMAD_BP_PROFILE=weibull, NOMAD_FUEL_LIMIT (defaut 400000).

use std::collections::HashMap;
use std::time::Instant;

use nomad_core::{BenchmarkScore, ExperimentConfig, SkillCapsule};
use nomad_evaluator::OracleEvaluator;
use nomad_genome::{deterministic_ast_crossover_with, multi_locus_ast_crossover_with, GraftMode};
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
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = &args[1];
    let suffix = &args[2];
    let max_pairs: usize = args[3].parse().unwrap();
    let seeds: Vec<u64> = args[4..].iter().map(|s| s.parse().unwrap()).collect();
    let fuel: u64 = std::env::var("NOMAD_FUEL_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(400_000);
    let lookahead = ExperimentConfig::default().lookahead;
    let sandbox = DeterministicSandbox::new();
    let start = Instant::now();

    for &seed in &seeds {
        let path = format!("{dir}/run_seed_{seed}_{suffix}.json");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path} : {e}"));
        let report: serde_json::Value = serde_json::from_str(&text).unwrap();
        let proposals = report["proposals"].as_array().unwrap();
        let code_of: HashMap<&str, &str> = proposals
            .iter()
            .map(|p| (p["capsule_id"].as_str().unwrap(), p["logic_code"].as_str().unwrap()))
            .collect();
        let champion_bins = proposals
            .iter()
            .filter(|p| p["is_valid"].as_bool() == Some(true))
            .filter_map(|p| p["total_bins"].as_u64())
            .min();

        let oracle = OracleEvaluator::new(seed);
        let train = |c: &SkillCapsule| -> BenchmarkScore {
            oracle.evaluate_dataset_with_lookahead(&oracle.in_distribution, c, &sandbox, fuel, lookahead)
        };
        let capsule = |id: &str| {
            SkillCapsule::new(id, "rejoue", code_of[id], vec![], 0, HashMap::new())
        };
        let mut host_cache: HashMap<String, BenchmarkScore> = HashMap::new();
        let mut rng = Rng(seed ^ 0x6EAF_7);
        let seed_start = Instant::now();
        let mut done = 0usize;

        for p in proposals.iter().filter(|p| p["kind"] == "crossover") {
            if max_pairs > 0 && done >= max_pairs {
                break;
            }
            done += 1;
            let parents = p["parent_ids"].as_array().unwrap();
            let (host_id, donor_id) = (parents[0].as_str().unwrap(), parents[1].as_str().unwrap());
            let host = capsule(host_id);
            let donor = capsule(donor_id);
            let host_score = host_cache
                .entry(host_id.to_string())
                .or_insert_with(|| train(&host))
                .clone();
            let cross_seed = rng.next();

            let mut row = serde_json::json!({
                "seed": seed, "index": p["index"], "host_id": host_id, "donor_id": donor_id,
                "cross_seed": cross_seed, "champion_bins": champion_bins,
                "host_valid": host_score.is_valid, "host_bins": host_score.total_bins,
                "host_len": host.logic_code.len(),
                "orig_accepted_journal_valid": p["is_valid"],
            });
            let mut host_val: Option<BenchmarkScore> = None;

            for (label, mode) in [("mixing", GraftMode::Mixing), ("rules", GraftMode::RulesOnly)] {
                // Meme choix d'operateur que le moteur (croisement local du groupe D).
                let res = if cross_seed % 2 == 0 {
                    multi_locus_ast_crossover_with(&host, &donor, cross_seed, 1, mode)
                        .or_else(|_| deterministic_ast_crossover_with(&host, &donor, cross_seed, 1, mode))
                } else {
                    deterministic_ast_crossover_with(&host, &donor, cross_seed, 1, mode)
                        .or_else(|_| multi_locus_ast_crossover_with(&host, &donor, cross_seed, 1, mode))
                };
                match res {
                    Err(e) => {
                        row[format!("{label}_error")] = e.to_string().into();
                    }
                    Ok((child, _)) => {
                        let score = train(&child);
                        let accepted = score.is_valid && score < host_score;
                        row[format!("{label}_kind")] = child.metadata.get("crossover_type").cloned().into();
                        row[format!("{label}_mixed")] = (child.metadata.get("mixed").map(String::as_str) == Some("true")).into();
                        row[format!("{label}_valid")] = score.is_valid.into();
                        row[format!("{label}_bins")] = score.total_bins.into();
                        row[format!("{label}_fuel")] = score.total_fuel.into();
                        row[format!("{label}_len")] = child.logic_code.len().into();
                        row[format!("{label}_accepted")] = accepted.into();
                        if accepted {
                            let hv = host_val
                                .get_or_insert_with(|| oracle.evaluate_validation(&host, &sandbox, fuel));
                            let v = oracle.evaluate_validation(&child, &sandbox, fuel);
                            row["host_val_valid"] = hv.is_valid.into();
                            row["host_val_bins"] = hv.total_bins.into();
                            row[format!("{label}_val_valid")] = v.is_valid.into();
                            row[format!("{label}_val_bins")] = v.total_bins.into();
                            row[format!("{label}_code")] = child.logic_code.clone().into();
                        }
                    }
                }
            }
            println!("{row}");
        }
        eprintln!(
            "seed {seed} : {done} paires en {:.1} s (total {:.1} s)",
            seed_start.elapsed().as_secs_f64(),
            start.elapsed().as_secs_f64()
        );
    }
}
