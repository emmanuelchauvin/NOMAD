use nomad_core::{
    BenchmarkScore, PolicyKind, SkillCapsule, ValidationStatus,
};
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
    let stmts = &mut null_fn.block.stmts;
    let mut removed = false;

    // 0. Multi-loci surgical removal:
    if let Some(loci_str) = manifest.host_locus.strip_prefix("multi_loci_") {
        let parts: Vec<&str> = loci_str.split('_').collect();
        if parts.len() == 2 {
            if let (Ok(idx1), Ok(idx2)) = (parts[0].parse::<usize>(), parts[1].parse::<usize>()) {
                let max_idx = idx1.max(idx2);
                let min_idx = idx1.min(idx2);
                // Supprimer d'abord l'index le plus élevé pour ne pas décaler l'index inférieur
                if max_idx < stmts.len() && stmts.len() > 2 {
                    stmts.remove(max_idx);
                    if min_idx < stmts.len() {
                        stmts.remove(min_idx);
                        removed = true;
                    }
                }
            }
        }
    }

    // 0b. Multi-loci hash matching fallback (format hash1:hash2)
    if !removed && manifest.grafted_fragment_hash.contains(':') {
        let hashes: Vec<&str> = manifest.grafted_fragment_hash.split(':').collect();
        if hashes.len() == 2 {
            let mut to_remove = Vec::new();
            for (i, stmt) in stmts.iter().enumerate() {
                let code_repr = quote::quote!(#stmt).to_string();
                let mut hasher = Sha256::new();
                hasher.update(code_repr.as_bytes());
                let h = hex::encode(hasher.finalize());
                if h == hashes[0] || h == hashes[1] {
                    to_remove.push(i);
                }
            }
            to_remove.sort_unstable();
            for &idx in to_remove.iter().rev() {
                if idx < stmts.len() && stmts.len() > 1 {
                    stmts.remove(idx);
                    removed = true;
                }
            }
        }
    }

    // 1. Surgical removal: try exact removal at host_locus index if it matches the grafted hash
    if let Some(idx_str) = manifest.host_locus.strip_prefix("body_stmt_index_") {
        if let Ok(idx) = idx_str.parse::<usize>() {
            if idx < stmts.len() {
                let target_stmt = &stmts[idx];
                let code_repr = quote::quote!(#target_stmt).to_string();
                let mut hasher = Sha256::new();
                hasher.update(code_repr.as_bytes());
                let hash = hex::encode(hasher.finalize());
                if hash == manifest.grafted_fragment_hash {
                    stmts.remove(idx);
                    removed = true;
                }
            }
        }
    }

    // 2. If locus didn't match the hash, remove ONLY the single matching statement (never use retain on all)
    if !removed {
        let mut target_idx = None;
        for (i, stmt) in stmts.iter().enumerate() {
            let code_repr = quote::quote!(#stmt).to_string();
            let mut hasher = Sha256::new();
            hasher.update(code_repr.as_bytes());
            let hash = hex::encode(hasher.finalize());
            if hash == manifest.grafted_fragment_hash {
                target_idx = Some(i);
                break;
            }
        }
        if let Some(i) = target_idx {
            stmts.remove(i);
            removed = true;
        }
    }

    // 3. Fallback: if hash didn't match, remove at host_locus index
    if !removed {
        if let Some(idx_str) = manifest.host_locus.strip_prefix("body_stmt_index_") {
            if let Ok(idx) = idx_str.parse::<usize>() {
                if idx < stmts.len() {
                    stmts.remove(idx);
                    removed = true;
                }
            }
        }
    }

    if !removed {
        return None;
    }

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
    if capsule.graft_manifest.is_some() {
        if let Err(e) = verify_level1_structural_survival(capsule) {
            let status = ValidationStatus::Failed(format!("Level 1 Structural Failure: {}", e));
            capsule.validation = status.clone();
            return Err(AntiDeadCodeEvalError::Level1Failed(e.to_string()));
        }
    } else {
        // Seeds & mutations: verify AST syntax and non-emptiness
        if let Err(e) = parse_score_bin(&capsule.logic_code) {
            let status = ValidationStatus::Failed(format!("Level 1 Structural Parse Failure: {}", e));
            capsule.validation = status.clone();
            return Err(AntiDeadCodeEvalError::Level1Failed(e.to_string()));
        }
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

        // Check if decision, performance, trajectory or behavioral metrics diverge.
        // Fuel consumption is strictly excluded from Level 3: an extra dead statement
        // consumes fuel without altering the decision policy (intron/redundant arithmetic).
        let differs = score_original.total_bins != score_null.total_bins
            || (score_original.mean_efficiency - score_null.mean_efficiency).abs() > 1e-4
            || (score_original.behavior.dead_space_ratio - score_null.behavior.dead_space_ratio).abs() > 1e-4
            || (score_original.behavior.residual_variance - score_null.behavior.residual_variance).abs() > 1e-4
            || (score_original.behavior.new_bin_ratio - score_null.behavior.new_bin_ratio).abs() > 1e-4
            || (score_original.tldr_score - score_null.tldr_score).abs() > 1e-4;

        if differs {
            let status = ValidationStatus::PassedLevel3Behavioral;
            capsule.validation = status.clone();
            return Ok(status);
        } else {
            // Neutral or redundant graft: executed with measured fuel (Level 2),
            // but produced zero causal behavioral divergence (Level 3 not proven).
            let status = ValidationStatus::PassedLevel2Computational;
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
            "Host", "Interface", host_code, vec![], 1, HashMap::new(),
            "ScorePolicyV1", PolicyKind::ScorePolicy, None, vec![],
            ValidationStatus::Unvalidated, None,
        );

        let donor = SkillCapsule::new_v2(
            "Donor", "Interface", donor_code, vec![], 1, HashMap::new(),
            "ScorePolicyV1", PolicyKind::ScorePolicy, None, vec![],
            ValidationStatus::Unvalidated, None,
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
        assert!(c_null.is_some(), "create_ablation_capsule must succeed on multi-loci child");
    }
}

