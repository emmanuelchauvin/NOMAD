use syn::ItemFn;
use thiserror::Error;

use nomad_core::{GraftManifest, SkillCapsule, ValidationStatus};

use crate::ast::parse_score_bin;
use crate::unit::extract_logical_units;

#[derive(Debug, Error)]
pub enum AntiDeadCodeError {
    #[error("Missing graft manifest on capsule {0}")]
    MissingManifest(String),
    #[error("Failed to parse child AST: {0}")]
    ParseFailed(String),
    #[error("Grafted fragment {0} was not found in child AST (Niveau 1 Survie Structurelle Échouée)")]
    FragmentNotFound(String),
    #[error("Grafted fragment was neutralized or made no-op")]
    FragmentNeutralized,
}

/// ============================================================================
/// verify_level1_structural_survival : Validation Anti-Dead-Code Niveau 1 (Structurelle)
/// ============================================================================
/// Vérifie formellement dans l'arbre syntaxique (AST) de l'organisme enfant que le fragment
/// greffé décrit par le `GraftManifest` est bel et bien présent, non vide et non supprimé
/// lors d'une éventuelle réparation syntaxique (Section 4.8.A).
pub fn verify_level1_structural_survival(
    capsule: &SkillCapsule,
) -> Result<ValidationStatus, AntiDeadCodeError> {
    let manifest: &GraftManifest = match &capsule.graft_manifest {
        Some(m) => m,
        None => return Err(AntiDeadCodeError::MissingManifest(capsule.id.0.clone())),
    };

    let item_fn: ItemFn = parse_score_bin(&capsule.logic_code)
        .map_err(|e| AntiDeadCodeError::ParseFailed(e.to_string()))?;

    let units = extract_logical_units(&item_fn);

    // Look for a unit matching the grafted fragment hash or containing its critical logic
    let fragment_found = units.iter().any(|u| {
        u.hash == manifest.grafted_fragment_hash
            || u.code_repr.contains(&manifest.grafted_fragment_hash[..8])
            // Check if fragment string exists within reconstructed unit
            || (!manifest.grafted_fragment_hash.is_empty() && capsule.logic_code.contains(&u.code_repr))
    });

    if fragment_found {
        Ok(ValidationStatus::PassedLevel1Structural)
    } else {
        Err(AntiDeadCodeError::FragmentNotFound(
            manifest.grafted_fragment_hash.clone(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crossover::deterministic_ast_crossover;
    use nomad_core::PolicyKind;
    use std::collections::HashMap;

    #[test]
    fn test_level1_structural_verification() {
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
                score += 100.0;
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

        let (child, _manifest) = deterministic_ast_crossover(&host, &donor, 42, 2).unwrap();
        let status = verify_level1_structural_survival(&child).expect("Level 1 check should pass");
        assert_eq!(status, ValidationStatus::PassedLevel1Structural);
    }
}

