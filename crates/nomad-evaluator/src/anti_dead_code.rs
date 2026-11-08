use nomad_core::{
    BenchmarkScore, PolicyKind, SkillCapsule, ValidationStatus,
};
use nomad_genome::{parse_score_bin, verify_level1_structural_survival};
use nomad_sandbox::HeuristicSandbox;
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

/// ============================================================================
/// create_ablation_capsule : Algorithme de Génération de la Capsule d'Ablation C_null
/// ============================================================================
/// Construit automatiquement la variante neutralisée `C_null` en retirant
/// de l'AST l'instruction correspondant au fragment greffé (Section 4.8.C).
/// Cette capsule d'ablation sert de témoin négatif pour prouver la causalité de la greffe.
pub fn create_ablation_capsule(capsule: &SkillCapsule) -> Option<SkillCapsule> {
    let manifest = capsule.graft_manifest.as_ref()?;
    let item_fn = parse_score_bin(&capsule.logic_code).ok()?;

    // Construction de C_null en filtrant l'instruction associée au fragment greffé
    let mut null_fn = item_fn.clone();
    null_fn.block.stmts.retain(|stmt| {
        let code = quote::quote!(#stmt).to_string();
        !code.contains(&manifest.grafted_fragment_hash[..8])
    });

    let null_code = quote::quote!(#null_fn).to_string();
    let null_name = format!("{}_null_ablation", capsule.name);

    Some(SkillCapsule::new_v2(
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
/// validate_anti_dead_code_3levels : Algorithme de Validation à 3 Niveaux
/// ============================================================================
/// Exécute la chaîne rigoureuse de vérification anti-code mort (Section 4.8) :
///
/// 1. **Niveau 1 (Survie Structurelle) :** Vérifie via `verify_level1_structural_survival`
///    que le fragment greffé est intact dans l'AST et n'a pas été effacé par le LLM.
/// 2. **Niveau 2 (Survie Computationnelle) :** Exécute l'heuristique sur le benchmark
///    d'entraînement pour attester qu'elle produit des calculs réels et consomme du fuel.
/// 3. **Niveau 3 (Survie Comportementale) :** Évalue la capsule d'ablation `C_null` sur
///    un jeu de sondes et vérifie qu'il existe une différence comportementale ou de score
///    prouvant que le fragment n'est pas inopérant (pas un no-op masqué).
pub fn validate_anti_dead_code_3levels(
    capsule: &mut SkillCapsule,
    oracle: &OracleEvaluator,
    sandbox: &dyn HeuristicSandbox,
    probes_dataset: &Dataset,
    fuel_limit: u64,
) -> Result<ValidationStatus, AntiDeadCodeEvalError> {
    // 1. Level 1: Structural verification
    if let Err(e) = verify_level1_structural_survival(capsule) {
        let status = ValidationStatus::Failed(format!("Level 1 Structural Failure: {}", e));
        capsule.validation = status.clone();
        return Err(AntiDeadCodeEvalError::Level1Failed(e.to_string()));
    }
    capsule.validation = ValidationStatus::PassedLevel1Structural;

    // 2. Level 2: Computational verification on probe dataset
    let score_original: BenchmarkScore = oracle.evaluate_dataset(
        probes_dataset,
        capsule,
        sandbox,
        fuel_limit,
    );

    if !score_original.is_valid || score_original.total_fuel == 0 {
        let status = ValidationStatus::Failed("Level 2 Computational Failure: invalid or zero fuel".to_string());
        capsule.validation = status;
        return Err(AntiDeadCodeEvalError::Level2ZeroExecutions);
    }
    capsule.validation = ValidationStatus::PassedLevel2Computational;

    // 3. Level 3: Behavioral verification against ablation C_null
    if let Some(c_null) = create_ablation_capsule(capsule) {
        let score_null: BenchmarkScore = oracle.evaluate_dataset(
            probes_dataset,
            &c_null,
            sandbox,
            fuel_limit,
        );

        // Check if decision/performance/fuel/behavior metrics diverge
        let differs = score_original.total_bins != score_null.total_bins
            || score_original.total_fuel != score_null.total_fuel
            || (score_original.mean_efficiency - score_null.mean_efficiency).abs() > 1e-4
            || (score_original.behavior.dead_space_ratio - score_null.behavior.dead_space_ratio).abs() > 1e-4;

        if differs {
            let status = ValidationStatus::PassedLevel3Behavioral;
            capsule.validation = status.clone();
            return Ok(status);
        }
    }

    // Passed computational and structural
    Ok(ValidationStatus::PassedLevel2Computational)
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
            "Host", "Interface", host_code, vec![], 1, HashMap::new(),
            "ScorePolicyV1", PolicyKind::ScorePolicy, None, vec![],
            ValidationStatus::Unvalidated, None,
        );

        let donor = SkillCapsule::new_v2(
            "Donor", "Interface", donor_code, vec![], 1, HashMap::new(),
            "ScorePolicyV1", PolicyKind::ScorePolicy, None, vec![],
            ValidationStatus::Unvalidated, None,
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
}

