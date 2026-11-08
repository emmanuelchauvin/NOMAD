use std::collections::HashSet;
use syn::{Expr, ItemFn, Local, Pat, PatIdent, Stmt};
use thiserror::Error;

use nomad_core::{GraftManifest, SkillCapsule, ValidationStatus};

use crate::ast::parse_score_bin;
use crate::unit::{collect_dependencies, extract_logical_units};

#[derive(Debug, Error)]
pub enum AntiDeadCodeError {
    #[error("Missing graft manifest on capsule {0}")]
    MissingManifest(String),
    #[error("Failed to parse child AST: {0}")]
    ParseFailed(String),
    #[error("Grafted fragment {0} was not found in child AST (Niveau 1 Survie Structurelle Échouée)")]
    FragmentNotFound(String),
    #[error("Grafted fragment was neutralized, dead-store, or made no-op")]
    FragmentNeutralized,
}

/// Extrait le nom de variable défini par un statement (let ident = ... ou ident = ...)
fn extract_defined_ident(stmt: &Stmt) -> Option<String> {
    match stmt {
        Stmt::Local(Local { pat, .. }) => match pat {
            Pat::Ident(PatIdent { ident, .. }) => Some(ident.to_string()),
            _ => None,
        },
        Stmt::Expr(Expr::Assign(assign), _) => match &*assign.left {
            Expr::Path(path) => path.path.get_ident().map(|i| i.to_string()),
            _ => None,
        },
        _ => None,
    }
}

/// ============================================================================
/// compute_liveness : Analyse de Vivacité Arrière (Backward Dataflow Analysis)
/// ============================================================================
/// Parcourt les instructions de `score_bin` en partant de l'expression de retour
/// pour déterminer si chaque déclaration de variable locale est effectivement lue
/// en aval (live) ou s'il s'agit d'un cul-de-sac (dead store / shadowing inutile).
pub fn compute_liveness(item_fn: &ItemFn) -> Vec<bool> {
    let stmts = &item_fn.block.stmts;
    if stmts.is_empty() {
        return Vec::new();
    }

    let mut is_live = vec![false; stmts.len()];
    let mut needed_vars = HashSet::new();

    // La dernière instruction (expression de retour) est toujours vivante
    let last_idx = stmts.len() - 1;
    is_live[last_idx] = true;
    for dep in collect_dependencies(&stmts[last_idx]) {
        needed_vars.insert(dep);
    }
    // "score" est la variable cible canonique
    needed_vars.insert("score".to_string());

    // Parcours arrière depuis last_idx - 1 jusqu'à 0
    for i in (0..last_idx).rev() {
        let stmt = &stmts[i];
        let defined = extract_defined_ident(stmt);
        let deps = collect_dependencies(stmt);

        match stmt {
            Stmt::Local(_) => {
                if let Some(var) = defined {
                    if needed_vars.contains(&var) {
                        is_live[i] = true;
                        needed_vars.remove(&var); // Définition : tue le besoin aval
                        for d in deps {
                            if d != var {
                                needed_vars.insert(d);
                            }
                        }
                    } else {
                        // Dead store : variable assignée mais jamais lue en aval
                        is_live[i] = false;
                    }
                } else {
                    is_live[i] = true;
                    for d in deps {
                        needed_vars.insert(d);
                    }
                }
            }
            Stmt::Expr(Expr::Assign(_), _) => {
                if let Some(var) = defined {
                    if needed_vars.contains(&var) {
                        is_live[i] = true;
                        for d in deps {
                            needed_vars.insert(d);
                        }
                    } else {
                        is_live[i] = false;
                    }
                } else {
                    is_live[i] = true;
                    for d in deps {
                        needed_vars.insert(d);
                    }
                }
            }
            _ => {
                // Branchements if, blocs : considérés vivants
                is_live[i] = true;
                for d in deps {
                    needed_vars.insert(d);
                }
            }
        }
    }

    is_live
}

/// ============================================================================
/// verify_level1_structural_survival : Validation Anti-Dead-Code Niveau 1 (Structurelle)
/// ============================================================================
/// Vérifie formellement dans l'arbre syntaxique (AST) de l'organisme enfant que le fragment
/// greffé décrit par le `GraftManifest` est bel et bien présent, non vide et qu'il n'est
/// pas un dead store isolé du cône de calcul menant au score final (Section 4.8.A).
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

    // 1. Vérification de la présence structurelle du fragment
    let fragment_found = units.iter().any(|u| {
        u.hash == manifest.grafted_fragment_hash
            || u.code_repr.contains(&manifest.grafted_fragment_hash[..8])
            || (!manifest.grafted_fragment_hash.is_empty() && capsule.logic_code.contains(&u.code_repr))
    });

    if !fragment_found {
        return Err(AntiDeadCodeError::FragmentNotFound(
            manifest.grafted_fragment_hash.clone(),
        ));
    }

    // 2. Passe de liveness : vérifier que l'instruction insérée au locus n'est pas un dead store
    let liveness = compute_liveness(&item_fn);
    if let Some(idx_str) = manifest.host_locus.strip_prefix("body_stmt_index_") {
        if let Ok(idx) = idx_str.parse::<usize>() {
            if idx < liveness.len() && !liveness[idx] {
                return Err(AntiDeadCodeError::FragmentNeutralized);
            }
        }
    }

    Ok(ValidationStatus::PassedLevel1Structural)
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

