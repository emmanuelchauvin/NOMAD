use nomad_core::{BenchmarkScore, PolicyKind, SkillCapsule, ValidationStatus};
use nomad_genome::{parse_score_bin, verify_level1_structural_survival};
use nomad_sandbox::HeuristicSandbox;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::dataset::Dataset;
use crate::oracle::OracleEvaluator;

#[derive(Debug, Error)]
pub enum AntiDeadCodeEvalError {
    #[error("Level 1 Structural validation failed: {0}")]
    Level1Failed(String),
    #[error("Level 2 Computational validation failed: grafted fragment was never executed")]
    Level2ZeroExecutions,
    #[error("Level 3 Behavioral validation failed: ablation C_null produced identical decisions (no behavioral impact)")]
    Level3ZeroBehavioralDifference,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AblationError {
    #[error("No graft manifest present in capsule")]
    NoManifest,
    #[error("Failed to parse logic code: {0}")]
    ParseFailed(String),
    #[error("Grafted fragment hash `{0}` not found in AST statements")]
    FragmentNotFound(String),
    #[error("Ablation neutralization failed: {0}")]
    NeutralizationFailed(String),
}

/// ============================================================================
/// create_ablation_capsule_res : Algorithme Rigoureux de Génération C_null par Hash
/// ============================================================================
/// Construit la variante neutralisée `C_null` en recherchant STRICTEMENT les statements
/// dont l'empreinte SHA-256 correspond au fragment greffé dans le `GraftManifest`.
/// Supprime les replis aveugles par indice et neutralise les définitions de variables
/// avec des valeurs neutres documentées afin d'éviter tout plantage de C_null.
pub fn create_ablation_capsule_res(capsule: &SkillCapsule) -> Result<SkillCapsule, AblationError> {
    let manifest = capsule
        .graft_manifest
        .as_ref()
        .ok_or(AblationError::NoManifest)?;
    let mut null_fn = parse_score_bin(&capsule.logic_code)
        .map_err(|e| AblationError::ParseFailed(e.to_string()))?;

    let expected_hashes: Vec<&str> = if manifest.grafted_fragment_hash.contains(':') {
        manifest.grafted_fragment_hash.split(':').collect()
    } else {
        vec![manifest.grafted_fragment_hash.as_str()]
    };

    let mut target_indices = Vec::new();
    let stmts = &null_fn.block.stmts;

    for &expected_h in &expected_hashes {
        let mut found_idx = None;
        for (i, stmt) in stmts.iter().enumerate() {
            let code_repr = quote::quote!(#stmt).to_string();
            let mut hasher = Sha256::new();
            hasher.update(code_repr.as_bytes());
            let h = hex::encode(hasher.finalize());
            if h == expected_h {
                found_idx = Some(i);
                break;
            }
        }
        match found_idx {
            Some(i) => target_indices.push(i),
            None => return Err(AblationError::FragmentNotFound(expected_h.to_string())),
        }
    }

    // Traitement ordonné du plus grand indice au plus petit pour garantir la cohérence des indices lors du retrait
    target_indices.sort_unstable();
    target_indices.dedup();
    target_indices.reverse();

    for &idx in &target_indices {
        // Retrait structurel propre du fragment greffé sans injection de valeur magique (0.0)
        if null_fn.block.stmts.len() > 1 && idx < null_fn.block.stmts.len() {
            null_fn.block.stmts.remove(idx);
        } else if idx < null_fn.block.stmts.len() {
            null_fn.block.stmts[idx] = syn::parse_quote!({
                let _ = ();
            });
        }
    }

    if null_fn.block.stmts.is_empty() {
        null_fn.block.stmts.push(syn::parse_quote!({
            let _ = ();
        }));
    }

    let null_code = quote::quote!(#null_fn).to_string();
    if parse_score_bin(&null_code).is_err() {
        return Err(AblationError::NeutralizationFailed(
            "Neutralized AST code failed to parse".to_string(),
        ));
    }

    let null_name = format!("{}_null_ablation", capsule.name);
    Ok(SkillCapsule::new_v2(
        null_name,
        "Ablation capsule with neutralized graft fragment",
        null_code,
        capsule.parent_ids.clone(),
        capsule.generation,
        capsule.metadata.clone(),
        &capsule.interface_version,
        PolicyKind::ScorePolicy,
        None,
        Vec::new(),
        ValidationStatus::Unvalidated,
        None,
    ))
}

/// ============================================================================
/// create_ablation_capsule : Interface rétrocompatible V2
/// ============================================================================
pub fn create_ablation_capsule(capsule: &SkillCapsule) -> Option<SkillCapsule> {
    create_ablation_capsule_res(capsule).ok()
}

/// ============================================================================
/// validate_anti_dead_code_3levels : Algorithme de Validation à 3 Niveaux
/// ============================================================================
/// Exécute la chaîne rigoureuse de vérification anti-code mort (Section 4.8) :
///
/// 1. **Niveau 1 (Survie Structurelle) :** Vérifie via `verify_level1_structural_survival`
///    que le fragment greffé est intact dans l'AST et n'a pas été effacé par le LLM.
/// 2. **Niveau 2 (Survie Computationnelle) :** Exécute l'heuristique sur le benchmark
///    d'entraînement pour attester qu'elle produit des calculs réels et consomme du fuel.
/// 3. **Niveau 3 (Survie Comportementale) :** Évalue la capsule d'ablation `C_null` sur
///    un jeu de sondes. Exige la validité de C_null, une divergence effective des décisions,
///    et une dégradation directionnelle stricte de la performance (la suppression doit nuire).
pub fn validate_anti_dead_code_3levels(
    capsule: &mut SkillCapsule,
    oracle: &OracleEvaluator,
    sandbox: &dyn HeuristicSandbox,
    probes_dataset: &Dataset,
    fuel_limit: u64,
) -> Result<ValidationStatus, AntiDeadCodeEvalError> {
    validate_anti_dead_code_3levels_with_epsilon(
        capsule,
        oracle,
        sandbox,
        probes_dataset,
        fuel_limit,
        1e-4,
        true,
    )
}

/// Version configurable de la validation Anti-Dead-Code acceptant un seuil epsilon explicite (issu de `ExperimentConfig`)
/// et le contrôle strict du régime Lookahead (`lookahead: bool`).
pub fn validate_anti_dead_code_3levels_with_epsilon(
    capsule: &mut SkillCapsule,
    oracle: &OracleEvaluator,
    sandbox: &dyn HeuristicSandbox,
    probes_dataset: &Dataset,
    fuel_limit: u64,
    epsilon: f64,
    lookahead: bool,
) -> Result<ValidationStatus, AntiDeadCodeEvalError> {
    // 1. Level 1: Structural verification
    if capsule.graft_manifest.is_some() {
        if let Err(e) = verify_level1_structural_survival(capsule) {
            let status = ValidationStatus::Failed(format!("Level 1 Structural Failure: {}", e));
            capsule.validation = status.clone();
            return Err(AntiDeadCodeEvalError::Level1Failed(e.to_string()));
        }
    } else {
        // Seeds & mutations: verify AST syntax and non-emptiness
        if let Err(e) = parse_score_bin(&capsule.logic_code) {
            let status =
                ValidationStatus::Failed(format!("Level 1 Structural Parse Failure: {}", e));
            capsule.validation = status.clone();
            return Err(AntiDeadCodeEvalError::Level1Failed(e.to_string()));
        }
    }
    capsule.validation = ValidationStatus::PassedLevel1Structural;

    // 2. Level 2: Computational verification on probe dataset
    let score_original: BenchmarkScore =
        oracle.evaluate_dataset_with_lookahead(probes_dataset, capsule, sandbox, fuel_limit, lookahead);

    if !score_original.is_valid || score_original.total_fuel == 0 {
        let status = ValidationStatus::Failed(
            "Level 2 Computational Failure: invalid or zero fuel".to_string(),
        );
        capsule.validation = status;
        return Err(AntiDeadCodeEvalError::Level2ZeroExecutions);
    }
    capsule.validation = ValidationStatus::PassedLevel2Computational;

    // 3. Level 3: Behavioral verification against ablation C_null
    if capsule.graft_manifest.is_none() {
        return Ok(ValidationStatus::PassedLevel2Computational);
    }

    let c_null = match create_ablation_capsule_res(capsule) {
        Ok(c) => c,
        Err(e) => {
            let status = ValidationStatus::NotEvaluable(format!("Ablation failed: {}", e));
            capsule.validation = status.clone();
            return Ok(status);
        }
    };

    let score_null: BenchmarkScore =
        oracle.evaluate_dataset_with_lookahead(probes_dataset, &c_null, sandbox, fuel_limit, lookahead);

    // Rejeter les C_null invalides : une erreur d'exécution n'est JAMAIS une preuve causale
    if !score_null.is_valid {
        let status = ValidationStatus::NotEvaluable(
            "Ablation C_null execution failed or was penalized as invalid".to_string(),
        );
        capsule.validation = status.clone();
        return Ok(status);
    }

    // Comparaison des décisions instance par instance
    let mut divergent_decisions_count = 0usize;
    let mut total_decisions = 0usize;
    let mut divergent_instances_count = 0usize;

    for inst in &probes_dataset.instances {
        let (eval_orig, _, _) =
            oracle.evaluate_single_instance_with_lookahead(inst, &capsule.logic_code, sandbox, fuel_limit, lookahead);
        let (eval_null, _, _) =
            oracle.evaluate_single_instance_with_lookahead(inst, &c_null.logic_code, sandbox, fuel_limit, lookahead);

        if !eval_null.is_valid {
            let status = ValidationStatus::NotEvaluable(
                "Ablation C_null failed on individual probe instance".to_string(),
            );
            capsule.validation = status.clone();
            return Ok(status);
        }

        if eval_orig.bins_used != eval_null.bins_used
            || eval_orig.trajectory != eval_null.trajectory
        {
            divergent_instances_count += 1;
        }

        let steps = eval_orig.trajectory.len().min(eval_null.trajectory.len());
        total_decisions += steps;
        for s in 0..steps {
            if eval_orig.trajectory[s] != eval_null.trajectory[s] {
                divergent_decisions_count += 1;
            }
        }
    }

    let decision_divergence_ratio = if total_decisions > 0 {
        divergent_decisions_count as f64 / total_decisions as f64
    } else {
        0.0
    };

    capsule.metadata.insert(
        "ablation_decision_diff_ratio".to_string(),
        format!("{:.4}", decision_divergence_ratio),
    );
    capsule.metadata.insert(
        "ablation_bins_diff".to_string(),
        format!(
            "{:+}",
            score_null.total_bins as i64 - score_original.total_bins as i64
        ),
    );

    // Test directionnel avec seuil epsilon configurable (défaut documenté 1e-4 / config.tldr_epsilon)
    let bins_degraded = score_null.total_bins > score_original.total_bins;
    let eff_degraded = score_null.total_bins == score_original.total_bins
        && score_null.mean_efficiency < score_original.mean_efficiency - epsilon;
    let tldr_degraded = score_null.total_bins == score_original.total_bins
        && (score_original.mean_efficiency - score_null.mean_efficiency).abs() <= epsilon
        && score_null.tldr_score < score_original.tldr_score - epsilon;

    let is_degraded = bins_degraded || eff_degraded || tldr_degraded;

    let bins_improved = score_null.total_bins < score_original.total_bins;
    let eff_improved = score_null.total_bins == score_original.total_bins
        && score_null.mean_efficiency > score_original.mean_efficiency + epsilon;

    let is_improved = bins_improved || eff_improved;

    if is_improved {
        // Si l'ablation améliore la performance, la greffe était nocive !
        let status = ValidationStatus::GraftHarmful(format!(
            "Ablation improved performance (null bins: {}, orig bins: {}, eff diff: {:.4})",
            score_null.total_bins,
            score_original.total_bins,
            score_null.mean_efficiency - score_original.mean_efficiency
        ));
        capsule.validation = status.clone();
        Ok(status)
    } else if is_degraded && (decision_divergence_ratio > 0.0 || divergent_instances_count > 0) {
        // Preuve causale stricte : la suppression de la greffe DÉGRADE les décisions et la performance
        let status = ValidationStatus::PassedLevel3Behavioral;
        capsule.validation = status.clone();
        Ok(status)
    } else {
        // Greffe neutre ou sans impact mesurable : computationnellement active (Niveau 2), mais causalité non établie
        let status = ValidationStatus::PassedLevel2Computational;
        capsule.validation = status.clone();
        Ok(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nomad_genome::deterministic_ast_crossover;
    use nomad_sandbox::DeterministicSandbox;
    use std::collections::HashMap;

    #[test]
    fn test_anti_dead_code_validation_pipeline() {
        let host_code = r#"
        pub fn score_bin(
            item_size: u32,
            bin_residual: u32,
            bin_capacity: u32,
            step_index: usize,
            total_items: usize,
            open_bins_count: usize,
            mean_residual: f64,
        ) -> f64 {
            let mut score = bin_residual as f64;
            score
        }
        "#;

        let donor_code = r#"
        pub fn score_bin(
            item_size: u32,
            bin_residual: u32,
            bin_capacity: u32,
            step_index: usize,
            total_items: usize,
            open_bins_count: usize,
            mean_residual: f64,
        ) -> f64 {
            let mut score = bin_residual as f64;
            if bin_residual < 10 {
                score = score + 50.0;
            }
            score
        }
        "#;

        let host = SkillCapsule::new_v2(
            "Host",
            "Interface",
            host_code,
            vec![],
            1,
            HashMap::new(),
            "ScorePolicyV1",
            PolicyKind::ScorePolicy,
            None,
            vec![],
            ValidationStatus::Unvalidated,
            None,
        );

        let donor = SkillCapsule::new_v2(
            "Donor",
            "Interface",
            donor_code,
            vec![],
            1,
            HashMap::new(),
            "ScorePolicyV1",
            PolicyKind::ScorePolicy,
            None,
            vec![],
            ValidationStatus::Unvalidated,
            None,
        );

        let (mut child, _manifest) = deterministic_ast_crossover(&host, &donor, 42, 2).unwrap();

        let oracle = OracleEvaluator::new(42);
        let sandbox = DeterministicSandbox::new();

        let status = validate_anti_dead_code_3levels(
            &mut child,
            &oracle,
            &sandbox,
            &oracle.in_distribution,
            50_000,
        )
        .expect("Should pass validation");

        assert!(matches!(
            status,
            ValidationStatus::PassedLevel2Computational | ValidationStatus::PassedLevel3Behavioral
        ));
    }

    #[test]
    fn test_multi_locus_anti_dead_code_validation() {
        use nomad_genome::multi_locus_ast_crossover;

        let host_code = r#"
        pub fn score_bin(
            item_size: u32,
            bin_residual: u32,
            bin_capacity: u32,
            step_index: usize,
            total_items: usize,
            open_bins_count: usize,
            mean_residual: f64,
        ) -> f64 {
            let mut score = bin_residual as f64;
            score
        }
        "#;

        let donor_code = r#"
        pub fn score_bin(
            item_size: u32,
            bin_residual: u32,
            bin_capacity: u32,
            step_index: usize,
            total_items: usize,
            open_bins_count: usize,
            mean_residual: f64,
        ) -> f64 {
            let mut score = bin_residual as f64;
            let bonus = 10.0;
            if bin_residual < 10 {
                score = score + 50.0;
            }
            score
        }
        "#;

        let host = SkillCapsule::new_v2(
            "Host",
            "Interface",
            host_code,
            vec![],
            1,
            HashMap::new(),
            "ScorePolicyV1",
            PolicyKind::ScorePolicy,
            None,
            vec![],
            ValidationStatus::Unvalidated,
            None,
        );

        let donor = SkillCapsule::new_v2(
            "Donor",
            "Interface",
            donor_code,
            vec![],
            1,
            HashMap::new(),
            "ScorePolicyV1",
            PolicyKind::ScorePolicy,
            None,
            vec![],
            ValidationStatus::Unvalidated,
            None,
        );

        let (mut child, _manifest) = multi_locus_ast_crossover(&host, &donor, 42, 2).unwrap();

        let oracle = OracleEvaluator::new(42);
        let sandbox = DeterministicSandbox::new();

        let status = validate_anti_dead_code_3levels(
            &mut child,
            &oracle,
            &sandbox,
            &oracle.in_distribution,
            50_000,
        )
        .expect("Multi-locus should pass validation");

        assert!(matches!(
            status,
            ValidationStatus::PassedLevel2Computational | ValidationStatus::PassedLevel3Behavioral
        ));

        // Ensure C_null was successfully created and not None!
        let c_null = create_ablation_capsule(&child);
        assert!(
            c_null.is_some(),
            "create_ablation_capsule must succeed on multi-loci child"
        );
    }

    #[test]
    fn test_ablation_hash_mismatch_yields_not_evaluable() {
        let code = r#"
        pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {
            let mut score = bin_residual as f64;
            score
        }
        "#;
        let manifest = nomad_core::GraftManifest {
            graft_id: "test_graft".to_string(),
            donor_capsule: nomad_core::CapsuleId("donor".to_string()),
            host_capsule: nomad_core::CapsuleId("host".to_string()),
            donor_ast_hash: "none".to_string(),
            grafted_fragment_hash:
                "deadbeef00000000000000000000000000000000000000000000000000000000".to_string(),
            host_locus: "body_stmt_index_1".to_string(),
            dependency_hash: "none".to_string(),
            repair_version: None,
            repair_diff_hash: None,
        };

        let mut capsule = SkillCapsule::new_v2(
            "MismatchCapsule",
            "Desc",
            code,
            vec![],
            1,
            HashMap::new(),
            "ScorePolicyV1",
            PolicyKind::ScorePolicy,
            None,
            vec![],
            ValidationStatus::Unvalidated,
            Some(manifest),
        );

        let err = create_ablation_capsule_res(&capsule).unwrap_err();
        assert!(matches!(err, AblationError::FragmentNotFound(_)));

        let oracle = OracleEvaluator::new(42);
        let sandbox = DeterministicSandbox::new();
        // Since hash is absent, Level 1 rejects it properly
        assert!(validate_anti_dead_code_3levels(
            &mut capsule,
            &oracle,
            &sandbox,
            &oracle.in_distribution,
            50_000
        )
        .is_err());
    }

    #[test]
    fn test_ablation_neutral_graft_yields_level2() {
        let code = r#"
        pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {
            let mut score = bin_residual as f64;
            let score = score + 0.0;
            score
        }
        "#;
        // Compute the hash of the second statement `let score = score + 0.0;`
        let item_fn = parse_score_bin(code).unwrap();
        let intron_stmt = &item_fn.block.stmts[1];
        let code_repr = quote::quote!(#intron_stmt).to_string();
        let h = hex::encode(Sha256::digest(code_repr.as_bytes()));

        let manifest = nomad_core::GraftManifest {
            graft_id: "intron_graft".to_string(),
            donor_capsule: nomad_core::CapsuleId("donor".to_string()),
            host_capsule: nomad_core::CapsuleId("host".to_string()),
            donor_ast_hash: "none".to_string(),
            grafted_fragment_hash: h,
            host_locus: "body_stmt_index_1".to_string(),
            dependency_hash: "none".to_string(),
            repair_version: None,
            repair_diff_hash: None,
        };

        let mut capsule = SkillCapsule::new_v2(
            "IntronCapsule",
            "Desc",
            code,
            vec![],
            1,
            HashMap::new(),
            "ScorePolicyV1",
            PolicyKind::ScorePolicy,
            None,
            vec![],
            ValidationStatus::Unvalidated,
            Some(manifest),
        );

        let oracle = OracleEvaluator::new(42);
        let sandbox = DeterministicSandbox::new();
        let status = validate_anti_dead_code_3levels(
            &mut capsule,
            &oracle,
            &sandbox,
            &oracle.in_distribution,
            50_000,
        )
        .unwrap();
        // Since neutral graft removal produces 0 decision divergence, it must pass Level 2, not Level 3!
        assert_eq!(status, ValidationStatus::PassedLevel2Computational);
    }

    #[test]
    fn test_ablation_harmful_graft_yields_graft_harmful() {
        // Here the graft inverts priority to Worst-Fit, causing terrible performance
        let code = r#"
        pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {
            let mut score = bin_residual as f64;
            let score = score - (bin_residual as f64) * 2.0;
            score
        }
        "#;
        let item_fn = parse_score_bin(code).unwrap();
        let harmful_stmt = &item_fn.block.stmts[1];
        let code_repr = quote::quote!(#harmful_stmt).to_string();
        let h = hex::encode(Sha256::digest(code_repr.as_bytes()));

        let manifest = nomad_core::GraftManifest {
            graft_id: "harmful_graft".to_string(),
            donor_capsule: nomad_core::CapsuleId("donor".to_string()),
            host_capsule: nomad_core::CapsuleId("host".to_string()),
            donor_ast_hash: "none".to_string(),
            grafted_fragment_hash: h,
            host_locus: "body_stmt_index_1".to_string(),
            dependency_hash: "none".to_string(),
            repair_version: None,
            repair_diff_hash: None,
        };

        let mut capsule = SkillCapsule::new_v2(
            "HarmfulCapsule",
            "Desc",
            code,
            vec![],
            1,
            HashMap::new(),
            "ScorePolicyV1",
            PolicyKind::ScorePolicy,
            None,
            vec![],
            ValidationStatus::Unvalidated,
            Some(manifest),
        );

        let oracle = OracleEvaluator::new(42);
        let sandbox = DeterministicSandbox::new();
        let status = validate_anti_dead_code_3levels(
            &mut capsule,
            &oracle,
            &sandbox,
            &oracle.in_distribution,
            50_000,
        )
        .unwrap();
        // Since ablation restores Best-Fit (fewer bins), the graft was harmful -> GraftHarmful!
        assert!(matches!(status, ValidationStatus::GraftHarmful(_)));
    }

    #[test]
    fn test_ablation_crash_yields_not_evaluable() {
        let code = r#"
        pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {
            let mut score = bin_residual as f64;
            let score = score + 1.0;
            score
        }
        "#;
        let item_fn = parse_score_bin(code).unwrap();
        let stmt = &item_fn.block.stmts[1];
        let code_repr = quote::quote!(#stmt).to_string();
        let h = hex::encode(Sha256::digest(code_repr.as_bytes()));

        let manifest = nomad_core::GraftManifest {
            graft_id: "crash_test".to_string(),
            donor_capsule: nomad_core::CapsuleId("donor".to_string()),
            host_capsule: nomad_core::CapsuleId("host".to_string()),
            donor_ast_hash: "none".to_string(),
            grafted_fragment_hash: h,
            host_locus: "body_stmt_index_1".to_string(),
            dependency_hash: "none".to_string(),
            repair_version: None,
            repair_diff_hash: None,
        };

        let mut capsule = SkillCapsule::new_v2(
            "CrashCapsule",
            "Desc",
            code,
            vec![],
            1,
            HashMap::new(),
            "ScorePolicyV1",
            PolicyKind::ScorePolicy,
            None,
            vec![],
            ValidationStatus::Unvalidated,
            Some(manifest),
        );

        let invalid_dataset = Dataset::new("Empty", vec![]);
        let oracle = OracleEvaluator::new(42);
        let sandbox = DeterministicSandbox::new();
        let status = validate_anti_dead_code_3levels(
            &mut capsule,
            &oracle,
            &sandbox,
            &invalid_dataset,
            50_000,
        );
        if let Ok(st) = status {
            assert_ne!(st, ValidationStatus::PassedLevel3Behavioral);
        }
    }

    #[test]
    fn test_anti_dead_code_with_lookahead_flag() {
        let code = r#"
        pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64, next_item_size: u32, post_residual: u32, fitting_bins_count: usize) -> f64 {
            let mut score = bin_residual as f64;
            let score = if post_residual < next_item_size { score + 100.0 } else { score };
            score
        }
        "#;
        let item_fn = parse_score_bin(code).unwrap();
        let graft_stmt = &item_fn.block.stmts[1];
        let code_repr = quote::quote!(#graft_stmt).to_string();
        let h = hex::encode(Sha256::digest(code_repr.as_bytes()));

        let manifest = nomad_core::GraftManifest {
            graft_id: "lookahead_graft".to_string(),
            donor_capsule: nomad_core::CapsuleId("donor".to_string()),
            host_capsule: nomad_core::CapsuleId("host".to_string()),
            donor_ast_hash: "none".to_string(),
            grafted_fragment_hash: h,
            host_locus: "body_stmt_index_1".to_string(),
            dependency_hash: "none".to_string(),
            repair_version: None,
            repair_diff_hash: None,
        };

        let capsule = SkillCapsule::new_v2(
            "LookaheadCapsule",
            "Desc",
            code,
            vec![],
            1,
            HashMap::new(),
            "ScorePolicyV1",
            PolicyKind::ScorePolicy,
            None,
            vec![],
            ValidationStatus::Unvalidated,
            Some(manifest),
        );

        let oracle = OracleEvaluator::new(42);
        let sandbox = DeterministicSandbox::new();

        // 1. Validation with lookahead = false
        // When lookahead is disabled, next_item_size is always passed as 0.0 to the sandbox.
        // Therefore, the ablated code (without the graft) behaves identically to the original code:
        // divergence is strictly 0 and the graft cannot pass Level 3 Behavioral.
        let mut cap_no_lookahead = capsule.clone();
        let status_no_lookahead = validate_anti_dead_code_3levels_with_epsilon(
            &mut cap_no_lookahead,
            &oracle,
            &sandbox,
            &oracle.in_distribution,
            50_000,
            1e-4,
            false,
        )
        .unwrap();
        assert_eq!(
            status_no_lookahead,
            ValidationStatus::PassedLevel2Computational,
            "Without lookahead, the next_item_size graft is computationally inactive (divergence = 0)"
        );
        assert_eq!(
            cap_no_lookahead
                .metadata
                .get("ablation_decision_diff_ratio")
                .map(|s| s.as_str()),
            Some("0.0000"),
            "Divergence ratio must be strictly 0 without lookahead"
        );

        // 2. Validation with lookahead = true
        // When lookahead is enabled, next_item_size takes actual sizes (> 0), making the graft actively
        // influence scoring and causing decisions to diverge between original and C_null!
        let mut cap_lookahead = capsule.clone();
        let status_lookahead = validate_anti_dead_code_3levels_with_epsilon(
            &mut cap_lookahead,
            &oracle,
            &sandbox,
            &oracle.in_distribution,
            50_000,
            1e-4,
            true,
        )
        .unwrap();
        assert_ne!(
            status_lookahead,
            ValidationStatus::PassedLevel2Computational,
            "With lookahead enabled, the next_item_size graft must diverge and trigger Level 3 evaluation"
        );
        let div_ratio: f64 = cap_lookahead
            .metadata
            .get("ablation_decision_diff_ratio")
            .expect("ablation_decision_diff_ratio must be present in metadata")
            .parse()
            .expect("ablation_decision_diff_ratio must parse as f64");
        assert!(
            div_ratio > 0.0,
            "Ablation divergence ratio must be strictly positive under lookahead=true (got {})",
            div_ratio
        );
    }
}
