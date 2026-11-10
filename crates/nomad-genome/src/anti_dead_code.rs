use std::collections::HashSet;
use syn::{Expr, ItemFn, Local, Pat, PatIdent, Stmt};
use thiserror::Error;

use nomad_core::{GraftManifest, SkillCapsule, ValidationStatus};

use crate::ast::parse_score_bin;
use crate::unit::{collect_dependencies, collect_expr_dependencies, extract_logical_units};

#[derive(Debug, Error)]
pub enum AntiDeadCodeError {
    #[error("Missing graft manifest on capsule {0}")]
    MissingManifest(String),
    #[error("Failed to parse child AST: {0}")]
    ParseFailed(String),
    #[error(
        "Grafted fragment {0} was not found in child AST (Niveau 1 Survie Structurelle Échouée)"
    )]
    FragmentNotFound(String),
    #[error("Grafted fragment was neutralized, dead-store, or made no-op")]
    FragmentNeutralized,
    #[error("Unproductive variable shadowing detected on variable `{0}`")]
    UnproductiveShadowing(String),
}

fn extract_pat_ident(pat: &Pat) -> Option<String> {
    match pat {
        Pat::Ident(PatIdent { ident, .. }) => Some(ident.to_string()),
        Pat::Type(pat_type) => extract_pat_ident(&pat_type.pat),
        _ => None,
    }
}

/// Extrait le nom de variable défini par un statement (let ident = ... ou ident = ...)
fn extract_defined_ident(stmt: &Stmt) -> Option<String> {
    match stmt {
        Stmt::Local(Local { pat, .. }) => extract_pat_ident(pat),
        Stmt::Expr(Expr::Assign(assign), _) => match &*assign.left {
            Expr::Path(path) => path.path.get_ident().map(|i| i.to_string()),
            _ => None,
        },
        Stmt::Expr(Expr::Binary(bin), _) => match bin.op {
            syn::BinOp::AddAssign(_)
            | syn::BinOp::SubAssign(_)
            | syn::BinOp::MulAssign(_)
            | syn::BinOp::DivAssign(_)
            | syn::BinOp::RemAssign(_) => match &*bin.left {
                Expr::Path(path) => path.path.get_ident().map(|i| i.to_string()),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

/// Collecte les identifiants lus (et non définis) par une instruction.
fn collect_read_dependencies(stmt: &Stmt) -> Vec<String> {
    match stmt {
        Stmt::Local(local) => {
            if let Some(init) = &local.init {
                collect_expr_dependencies(&init.expr)
            } else {
                Vec::new()
            }
        }
        Stmt::Expr(Expr::Assign(assign), _) => collect_expr_dependencies(&assign.right),
        Stmt::Expr(Expr::Binary(bin), _) => match bin.op {
            syn::BinOp::AddAssign(_)
            | syn::BinOp::SubAssign(_)
            | syn::BinOp::MulAssign(_)
            | syn::BinOp::DivAssign(_)
            | syn::BinOp::RemAssign(_) => collect_expr_dependencies(&bin.right),
            _ => collect_dependencies(stmt),
        },
        _ => collect_dependencies(stmt),
    }
}

fn expr_if_modifies_needed_vars(if_expr: &syn::ExprIf, needed_vars: &HashSet<String>) -> bool {
    if block_modifies_needed_vars(&if_expr.then_branch, needed_vars) {
        return true;
    }
    if let Some((_, else_expr)) = &if_expr.else_branch {
        return expr_modifies_needed_vars(else_expr, needed_vars);
    }
    false
}

fn block_modifies_needed_vars(block: &syn::Block, needed_vars: &HashSet<String>) -> bool {
    for stmt in &block.stmts {
        if stmt_modifies_needed_vars(stmt, needed_vars) {
            return true;
        }
    }
    false
}

fn stmt_modifies_needed_vars(stmt: &Stmt, needed_vars: &HashSet<String>) -> bool {
    match stmt {
        Stmt::Expr(expr, _) => expr_modifies_needed_vars(expr, needed_vars),
        _ => false,
    }
}

fn expr_modifies_needed_vars(expr: &Expr, needed_vars: &HashSet<String>) -> bool {
    match expr {
        Expr::Assign(assign) => {
            if let Expr::Path(path) = &*assign.left {
                if let Some(ident) = path.path.get_ident() {
                    return needed_vars.contains(&ident.to_string());
                }
            }
            false
        }
        Expr::Binary(bin) => match bin.op {
            syn::BinOp::AddAssign(_)
            | syn::BinOp::SubAssign(_)
            | syn::BinOp::MulAssign(_)
            | syn::BinOp::DivAssign(_)
            | syn::BinOp::RemAssign(_) => {
                if let Expr::Path(path) = &*bin.left {
                    if let Some(ident) = path.path.get_ident() {
                        return needed_vars.contains(&ident.to_string());
                    }
                }
                false
            }
            _ => false,
        },
        Expr::If(nested_if) => expr_if_modifies_needed_vars(nested_if, needed_vars),
        Expr::Block(b) => block_modifies_needed_vars(&b.block, needed_vars),
        _ => false,
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
                        // Les variables lues dans l'initialisation sont requises en amont
                        // (y compris si `var` est référencée sur le membre droit via shadowing)
                        for d in collect_read_dependencies(stmt) {
                            needed_vars.insert(d);
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
            Stmt::Expr(Expr::Assign(_), _)
            | Stmt::Expr(
                Expr::Binary(syn::ExprBinary {
                    op:
                        syn::BinOp::AddAssign(_)
                        | syn::BinOp::SubAssign(_)
                        | syn::BinOp::MulAssign(_)
                        | syn::BinOp::DivAssign(_)
                        | syn::BinOp::RemAssign(_),
                    ..
                }),
                _,
            ) => {
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
            Stmt::Expr(Expr::If(if_expr), _) => {
                if expr_if_modifies_needed_vars(if_expr, &needed_vars) {
                    is_live[i] = true;
                    for d in deps {
                        needed_vars.insert(d);
                    }
                } else {
                    // Branchement conditionnel sans impact sur les variables requises (dead code)
                    is_live[i] = false;
                }
            }
            _ => {
                // Autres blocs ou instructions générales
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
/// detect_unproductive_shadowing : Linter Anti-Shadowing Destructeur
/// ============================================================================
/// Détecte toute redéclaration `let var = ...` qui écrase une variable locale `var`
/// déclarée plus haut sans qu'elle ait été lue entre-temps (variable shadowing inutile).
pub fn detect_unproductive_shadowing(item_fn: &ItemFn) -> Option<String> {
    let stmts = &item_fn.block.stmts;
    let mut declared_vars: std::collections::HashMap<String, (usize, bool)> =
        std::collections::HashMap::new();

    for (idx, stmt) in stmts.iter().enumerate() {
        // Collecter les variables lues par cette instruction (en excluant les définitions de variables)
        let reads = collect_read_dependencies(stmt);
        for read in &reads {
            if let Some(entry) = declared_vars.get_mut(read) {
                entry.1 = true;
            }
        }

        // Vérifier si cette instruction déclare une variable locale `let var = ...`
        if let Stmt::Local(local) = stmt {
            if let Some(var_name) = extract_pat_ident(&local.pat) {
                if var_name != "score" {
                    if let Some((_prev_idx, was_read)) = declared_vars.get(&var_name) {
                        if !*was_read {
                            // La déclaration précédente de `var_name` n'a jamais été lue avant d'être écrasée !
                            return Some(var_name);
                        }
                    }
                    declared_vars.insert(var_name, (idx, false));
                }
            }
        }
    }

    None
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
    let fragment_found = if manifest.grafted_fragment_hash.contains(':') {
        let parts: Vec<&str> = manifest.grafted_fragment_hash.split(':').collect();
        parts.len() >= 2
            && parts.iter().all(|h| {
                units
                    .iter()
                    .any(|u| u.hash == *h || u.code_repr.contains(&h[..8]))
                    || capsule.logic_code.contains(&h[..8])
            })
    } else {
        units.iter().any(|u| {
            u.hash == manifest.grafted_fragment_hash
                || u.code_repr.contains(&manifest.grafted_fragment_hash[..8])
                || (!manifest.grafted_fragment_hash.is_empty()
                    && capsule.logic_code.contains(&u.code_repr))
        })
    };

    if !fragment_found {
        return Err(AntiDeadCodeError::FragmentNotFound(
            manifest.grafted_fragment_hash.clone(),
        ));
    }

    // 2. Passe de liveness : vérifier que les instructions insérées aux locus ne sont pas des dead stores
    let liveness = compute_liveness(&item_fn);
    if let Some(idx_str) = manifest.host_locus.strip_prefix("body_stmt_index_") {
        if let Ok(idx) = idx_str.parse::<usize>() {
            if idx < liveness.len() && !liveness[idx] {
                return Err(AntiDeadCodeError::FragmentNeutralized);
            }
        }
    } else if let Some(loci_str) = manifest.host_locus.strip_prefix("multi_loci_") {
        let parts: Vec<&str> = loci_str.split('_').collect();
        for p in &parts {
            if let Ok(idx) = p.parse::<usize>() {
                if idx < liveness.len() && !liveness[idx] {
                    return Err(AntiDeadCodeError::FragmentNeutralized);
                }
            }
        }
    }

    // 3. Passe anti-shadowing : rejeter toute redéclaration qui écrase une variable locale sans lecture intermédiaire
    if let Some(shadowed_var) = detect_unproductive_shadowing(&item_fn) {
        return Err(AntiDeadCodeError::UnproductiveShadowing(shadowed_var));
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

        let (child, _manifest) = deterministic_ast_crossover(&host, &donor, 42, 2).unwrap();
        let status = verify_level1_structural_survival(&child).expect("Level 1 check should pass");
        assert_eq!(status, ValidationStatus::PassedLevel1Structural);
    }

    #[test]
    fn test_detect_unproductive_shadowing_rejection() {
        let shadowed_code = r#"
        pub fn score_bin(
            item_size: u32,
            bin_residual: u32,
            bin_capacity: u32,
            step_index: usize,
            total_items: usize,
            open_bins_count: usize,
            mean_residual: f64,
        ) -> f64 {
            let horizon_factor = 1.0 + 5.0 * (step_index as f64);
            let horizon_factor = 1.0 + 8.0 * (step_index as f64);
            let score = bin_residual as f64 * horizon_factor;
            score
        }
        "#;

        let parsed = parse_score_bin(shadowed_code).unwrap();
        let detected = detect_unproductive_shadowing(&parsed);
        assert_eq!(detected, Some("horizon_factor".to_string()));

        let capsule = SkillCapsule::new_v2(
            "Shadowed",
            "Interface",
            shadowed_code,
            vec![],
            1,
            HashMap::new(),
            "ScorePolicyV1",
            PolicyKind::ScorePolicy,
            None,
            vec![],
            ValidationStatus::Unvalidated,
            Some(GraftManifest {
                graft_id: "graft-1".to_string(),
                donor_capsule: nomad_core::CapsuleId("donor".to_string()),
                host_capsule: nomad_core::CapsuleId("host".to_string()),
                donor_ast_hash: "hash".to_string(),
                grafted_fragment_hash: "horizon_factor".to_string(),
                host_locus: "body_stmt_index_1".to_string(),
                dependency_hash: "dep".to_string(),
                repair_version: None,
                repair_diff_hash: None,
            }),
        );

        let res = verify_level1_structural_survival(&capsule);
        assert!(
            matches!(res, Err(AntiDeadCodeError::UnproductiveShadowing(v)) if v == "horizon_factor")
        );
    }

    #[test]
    fn test_compute_liveness_conditional_branches() {
        let active_if_code = r#"
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
        let fn_active = parse_score_bin(active_if_code).unwrap();
        let liveness_active = compute_liveness(&fn_active);
        // stmt 0: let mut score ... (live)
        // stmt 1: if bin_residual < 10 { score = score + 50.0; } (live)
        // stmt 2: score (live)
        assert_eq!(liveness_active, vec![true, true, true]);

        let dead_if_code = r#"
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
                let dummy = 42.0;
            }
            score
        }
        "#;
        let fn_dead = parse_score_bin(dead_if_code).unwrap();
        let liveness_dead = compute_liveness(&fn_dead);
        // stmt 0: let mut score ... (live)
        // stmt 1: if bin_residual < 10 { let dummy = 42.0; } (DEAD!)
        // stmt 2: score (live)
        assert_eq!(liveness_dead, vec![true, false, true]);
    }
}
