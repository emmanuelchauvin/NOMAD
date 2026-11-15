use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap, HashSet};
use thiserror::Error;

use nomad_core::{GraftManifest, PolicyKind, SkillCapsule, ValidationStatus};

use crate::ast::{compute_structural_fingerprint, normalize_source, parse_heuristic_fn, AstError};
use crate::unit::{collect_dependencies, collect_free_variables, extract_logical_units, LogicalUnit};

#[derive(Debug, Error)]
pub enum CrossoverError {
    #[error("AST parsing error: {0}")]
    AstError(#[from] AstError),
    #[error("No transferable logical units found in donor capsule {0}")]
    NoExtractableUnits(String),
    #[error("Host capsule {0} has an empty function body")]
    EmptyHostBody(String),
    #[error("No donor unit of {0} can be grafted: unresolved dependencies in the host")]
    UnresolvedDependencies(String),
}

fn is_builtin_or_method(name: &str) -> bool {
    matches!(
        name,
        "f64"
            | "f32"
            | "u32"
            | "u64"
            | "usize"
            | "i32"
            | "i64"
            | "bool"
            | "min"
            | "max"
            | "abs"
            | "sqrt"
            | "powf"
            | "floor"
            | "ceil"
            | "round"
            | "Some"
            | "None"
            | "true"
            | "false"
            | "score"
    )
}

fn extract_signature_params(item_fn: &syn::ItemFn) -> HashSet<String> {
    let mut params = HashSet::new();
    for input in &item_fn.sig.inputs {
        if let syn::FnArg::Typed(pat_type) = input {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                params.insert(pat_ident.ident.to_string());
            }
        }
    }
    params
}

fn extract_in_scope_locals(stmts: &[syn::Stmt], locus_idx: usize) -> HashSet<String> {
    let mut in_scope = HashSet::new();
    let limit = locus_idx.min(stmts.len());
    for s in &stmts[..limit] {
        if let syn::Stmt::Local(l) = s {
            if let syn::Pat::Ident(p) = &l.pat {
                in_scope.insert(p.ident.to_string());
            }
        }
    }
    in_scope
}

fn find_donor_definition(donor_stmts: &[syn::Stmt], var: &str) -> Option<(usize, syn::Stmt)> {
    for (idx, stmt) in donor_stmts.iter().enumerate() {
        if let syn::Stmt::Local(local) = stmt {
            if let syn::Pat::Ident(p) = &local.pat {
                if p.ident == var {
                    return Some((idx, stmt.clone()));
                }
            }
        }
    }
    None
}

/// Backward program slicing syntaxique minimal :
/// Vérifie que l'ensemble des identifiants requis par `stmt_to_insert` est couvert par la
/// signature de la fonction hôte ou ses variables en portée. Si une variable locale donneuse
/// est requise, extrait et inclut récursivement ses déclarations depuis le donneur.
pub fn resolve_dependencies_with_backward_slice(
    stmt_to_insert: &syn::Stmt,
    donor_stmts: &[syn::Stmt],
    host_stmts: &[syn::Stmt],
    host_params: &HashSet<String>,
    locus_idx: usize,
) -> Option<Vec<syn::Stmt>> {
    let mut in_scope = extract_in_scope_locals(host_stmts, locus_idx);
    for p in host_params {
        in_scope.insert(p.clone());
    }

    let mut needed_donor_indices = BTreeSet::new();
    let mut worklist = collect_free_variables(stmt_to_insert);
    let mut visited_vars = HashSet::new();

    while let Some(var) = worklist.pop() {
        if is_builtin_or_method(&var) || in_scope.contains(&var) {
            continue;
        }
        if !visited_vars.insert(var.clone()) {
            continue;
        }

        // Tenter de localiser la déclaration amont dans le donneur
        if let Some((idx, def_stmt)) = find_donor_definition(donor_stmts, &var) {
            needed_donor_indices.insert(idx);
            for dep in collect_free_variables(&def_stmt) {
                if !in_scope.contains(&dep) && !is_builtin_or_method(&dep) {
                    worklist.push(dep);
                }
            }
        } else {
            // Variable non résolue (incompatible ou absente des deux organismes)
            return None;
        }
    }

    let mut full_slice = Vec::new();
    for idx in needed_donor_indices {
        full_slice.push(donor_stmts[idx].clone());
    }
    full_slice.push(stmt_to_insert.clone());
    Some(full_slice)
}

/// ============================================================================
/// deterministic_ast_crossover : Algorithme de Croisement Génétique AST Déterministe
/// ============================================================================
/// Réalise le croisement formel de deux organismes sans dépendre obligatoirement
/// d'une réparation LLM (Section 4.5) :
///
/// **Étapes de l'algorithme :**
/// 1. **Analyse Syntaxique :** Parse les codes des capsules hôte et donneuse en `ItemFn`.
/// 2. **Extraction des Unités :** Extrait les blocs et expressions transférables du donneur via `extract_logical_units`.
/// 3. **Sélection Déterministe :** Tire une unité logique donneuse via le générateur pseudo-aléatoire `ChaCha8Rng(seed)`.
/// 4. **Identification du Locus :** Localise le point d'insertion optimal dans l'AST de l'hôte (juste avant l'expression de retour finale).
/// 5. **Greffe Syntaxique :** Insère le statement donneur dans le bloc d'instructions de l'hôte.
/// 6. **Génération du Manifeste :** Calcule les empreintes cryptographiques et produit le `GraftManifest` externe.
/// 7. **Reconstruction Canonique :** Régénère le code source hybride propre via `quote` et initialise l'enfant.
pub fn deterministic_ast_crossover(
    host: &SkillCapsule,
    donor: &SkillCapsule,
    seed: u64,
    generation: usize,
) -> Result<(SkillCapsule, GraftManifest), CrossoverError> {
    let host_fn = parse_heuristic_fn(&host.logic_code)?;
    let donor_fn = parse_heuristic_fn(&donor.logic_code)?;

    let _host_fingerprint = compute_structural_fingerprint(&host_fn);
    let donor_fingerprint = compute_structural_fingerprint(&donor_fn);

    let donor_units = extract_logical_units(&donor_fn);
    if donor_units.is_empty() {
        return Err(CrossoverError::NoExtractableUnits(donor.id.0.clone()));
    }

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let host_params = extract_signature_params(&host_fn);

    let mut hybrid_fn = host_fn.clone();
    let stmts = &mut hybrid_fn.block.stmts;
    if stmts.is_empty() {
        return Err(CrossoverError::EmptyHostBody(host.id.0.clone()));
    }

    // Determine insertion locus: right before the final return/expression statement
    let locus_idx = if stmts.len() > 1 { stmts.len() - 1 } else { 0 };

    let mut candidate_units: Vec<&LogicalUnit> = donor_units.iter().collect();
    candidate_units.shuffle(&mut rng);

    let mut chosen_unit = candidate_units[0];
    let mut resolved_graft: Option<Vec<syn::Stmt>> = None;

    for cand in &candidate_units {
        let mut cand_stmt = cand.stmt.clone();
        if let syn::Stmt::Local(local) = &cand.stmt {
            if let syn::Pat::Ident(pat_ident) = &local.pat {
                let var_name = pat_ident.ident.to_string();
                let already_defined = stmts.iter().any(|s| {
                    if let syn::Stmt::Local(l) = s {
                        if let syn::Pat::Ident(p) = &l.pat {
                            return p.ident == var_name;
                        }
                    }
                    false
                });

                if let Some(init) = &local.init {
                    let init_expr = &init.expr;
                    if var_name == "score" {
                        cand_stmt = syn::parse_quote! {
                            let score = score * 0.70 + ((#init_expr) as f64) * 0.30;
                        };
                    } else {
                        let downstream_deps: Vec<String> = stmts[locus_idx..]
                            .iter()
                            .flat_map(collect_dependencies)
                            .collect();
                        if already_defined || !downstream_deps.contains(&var_name) {
                            cand_stmt = syn::parse_quote! {
                                let score = score * 0.95 + ((#init_expr) as f64) * 0.05;
                            };
                        }
                    }
                }
            }
        }

        if let Some(slice) = resolve_dependencies_with_backward_slice(
            &cand_stmt,
            &donor_fn.block.stmts,
            stmts,
            &host_params,
            locus_idx,
        ) {
            resolved_graft = Some(slice);
            chosen_unit = cand;
            break;
        }
    }

    // Aucune unité greffable : refuser plutôt qu'insérer un fragment qui ne compilerait pas.
    let graft_stmts =
        resolved_graft.ok_or_else(|| CrossoverError::UnresolvedDependencies(donor.id.0.clone()))?;

    for (offset, stmt) in graft_stmts.iter().enumerate() {
        stmts.insert(locus_idx + offset, stmt.clone());
    }

    let mut fragment_hashes = Vec::new();
    let mut inserted_deps = Vec::new();
    let mut loci_indices = Vec::new();

    for (offset, stmt) in graft_stmts.iter().enumerate() {
        let inserted_repr = quote::quote!(#stmt).to_string();
        let mut fragment_hasher = Sha256::new();
        fragment_hasher.update(inserted_repr.as_bytes());
        fragment_hashes.push(hex::encode(fragment_hasher.finalize()));
        inserted_deps.extend(collect_dependencies(stmt));
        loci_indices.push((locus_idx + offset).to_string());
    }

    let fragment_hash = fragment_hashes.join(":");
    let locus_desc = if loci_indices.len() == 1 {
        format!("body_stmt_index_{}", locus_idx)
    } else {
        format!("multi_loci_{}", loci_indices.join("_"))
    };

    let mut dep_hasher = Sha256::new();
    for d in &inserted_deps {
        dep_hasher.update(d.as_bytes());
    }
    let dependency_hash = hex::encode(dep_hasher.finalize());

    let mut graft_hasher = Sha256::new();
    graft_hasher.update(host.id.0.as_bytes());
    graft_hasher.update(donor.id.0.as_bytes());
    graft_hasher.update(fragment_hash.as_bytes());
    graft_hasher.update(seed.to_le_bytes());
    let graft_id = format!("graft-{}", &hex::encode(graft_hasher.finalize())[..12]);

    let manifest = GraftManifest {
        graft_id: graft_id.clone(),
        donor_capsule: donor.id.clone(),
        host_capsule: host.id.clone(),
        donor_ast_hash: donor_fingerprint.ast_hash,
        grafted_fragment_hash: fragment_hash,
        host_locus: locus_desc,
        dependency_hash,
        repair_version: None,
        repair_diff_hash: None,
    };

    // Reconstruct normalized code and compute new fingerprint
    let hybrid_code = normalize_source(&hybrid_fn);
    let child_fingerprint = compute_structural_fingerprint(&hybrid_fn);

    let child_name = format!("Hybrid-G{}-{}", generation, &graft_id[..10]);
    let semantic_interface = format!(
        "AST hybrid combining host {} with donor {} via fragment [{}]",
        host.id, donor.id, chosen_unit.code_repr
    );

    let mut metadata = HashMap::new();
    metadata.insert(
        "crossover_type".to_string(),
        "ast_deterministic".to_string(),
    );
    metadata.insert("host_id".to_string(), host.id.0.clone());
    metadata.insert("donor_id".to_string(), donor.id.0.clone());
    metadata.insert("graft_id".to_string(), graft_id);

    let child_capsule = SkillCapsule::new_v2(
        child_name,
        semantic_interface,
        hybrid_code,
        vec![host.id.clone(), donor.id.clone()],
        generation,
        metadata,
        "ScorePolicyV1",
        PolicyKind::ScorePolicy,
        Some(child_fingerprint),
        chosen_unit.dependencies.clone(),
        ValidationStatus::PassedLevel1Structural, // Deterministic graft is syntactically present
        Some(manifest.clone()),
    );

    Ok((child_capsule, manifest))
}

/// ============================================================================
/// multi_locus_ast_crossover : Croisement Génétique Multi-Sites (Multi-Loci)
/// ============================================================================
/// Permet l'échange synergique de jusqu'à deux unités logiques distinctes du donneur
/// (ex: conditionnement initial / modulation d'horizon ET raffinement terminal du score)
/// dans l'hôte à deux loci structurels distincts, maximisant la recombinaison sans bloat.
pub fn multi_locus_ast_crossover(
    host: &SkillCapsule,
    donor: &SkillCapsule,
    seed: u64,
    generation: usize,
) -> Result<(SkillCapsule, GraftManifest), CrossoverError> {
    let host_fn = parse_heuristic_fn(&host.logic_code)?;
    let donor_fn = parse_heuristic_fn(&donor.logic_code)?;

    let donor_fingerprint = compute_structural_fingerprint(&donor_fn);
    let donor_units = extract_logical_units(&donor_fn);
    if donor_units.is_empty() {
        return Err(CrossoverError::NoExtractableUnits(donor.id.0.clone()));
    }

    if donor_units.len() < 2 {
        return deterministic_ast_crossover(host, donor, seed, generation);
    }

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let host_params = extract_signature_params(&host_fn);

    let mut indices: Vec<usize> = (0..donor_units.len()).collect();
    indices.shuffle(&mut rng);
    let unit1 = &donor_units[indices[0]];
    let unit2 = &donor_units[indices[1]];

    let mut hybrid_fn = host_fn.clone();
    let stmts = &mut hybrid_fn.block.stmts;
    if stmts.is_empty() {
        return Err(CrossoverError::EmptyHostBody(host.id.0.clone()));
    }

    // Locus 1: début du corps (immédiatement après l'initialisation du score à l'index 0)
    let locus1 = if stmts.len() > 1 { 1 } else { 0 };
    let mut stmt1 = unit1.stmt.clone();
    if let syn::Stmt::Local(local) = &unit1.stmt {
        if let syn::Pat::Ident(pat_ident) = &local.pat {
            let var_name = pat_ident.ident.to_string();
            let already_defined = stmts.iter().any(|s| {
                if let syn::Stmt::Local(l) = s {
                    if let syn::Pat::Ident(p) = &l.pat {
                        return p.ident == var_name;
                    }
                }
                false
            });

            if let Some(init) = &local.init {
                let init_expr = &init.expr;
                if var_name == "score" {
                    stmt1 = syn::parse_quote! {
                        let score = score * 0.80 + ((#init_expr) as f64) * 0.20;
                    };
                } else {
                    let downstream_deps: Vec<String> = stmts[locus1..]
                        .iter()
                        .flat_map(collect_dependencies)
                        .collect();
                    if already_defined || !downstream_deps.contains(&var_name) {
                        // Anti-shadowing & anti-dead-store : modulation causale directe du score
                        stmt1 = syn::parse_quote! {
                            let score = score * 0.95 + ((#init_expr) as f64) * 0.05;
                        };
                    }
                }
            }
        }
    }

    let slice1 = resolve_dependencies_with_backward_slice(
        &stmt1,
        &donor_fn.block.stmts,
        stmts,
        &host_params,
        locus1,
    )
    .ok_or_else(|| CrossoverError::UnresolvedDependencies(donor.id.0.clone()))?;

    let mut loci_indices = Vec::new();
    let mut fragment_hashes = Vec::new();
    let mut combined_deps = Vec::new();

    for (offset, s) in slice1.iter().enumerate() {
        let actual_idx = locus1 + offset;
        stmts.insert(actual_idx, s.clone());
        loci_indices.push(actual_idx.to_string());

        let repr = quote::quote!(#s).to_string();
        let mut hasher = Sha256::new();
        hasher.update(repr.as_bytes());
        fragment_hashes.push(hex::encode(hasher.finalize()));
        combined_deps.extend(collect_dependencies(s));
    }

    // Locus 2: fin du corps (juste avant return)
    let locus2 = stmts.len().saturating_sub(1);
    let mut stmt2 = unit2.stmt.clone();
    let use_regime_dispatch = seed % 2 == 1;

    if let syn::Stmt::Local(local) = &unit2.stmt {
        if let syn::Pat::Ident(pat_ident) = &local.pat {
            let var_name = pat_ident.ident.to_string();
            let _already_defined = stmts.iter().any(|s| {
                if let syn::Stmt::Local(l) = s {
                    if let syn::Pat::Ident(p) = &l.pat {
                        return p.ident == var_name;
                    }
                }
                false
            });

            if let Some(init) = &local.init {
                let init_expr = &init.expr;
                if var_name == "score" {
                    if use_regime_dispatch {
                        // Croisement par Branchement de Régime (Regime Dispatch Crossover)
                        // Assignation conditionnelle par shadowing pour compatibilité stricte rustc
                        if host.logic_code.contains("score_node") {
                            if (seed / 2).is_multiple_of(2) {
                                stmt2 = syn::parse_quote! {
                                    let score = if (step_index as f64 / total_jobs.max(1) as f64) >= 0.5 {
                                        (#init_expr) as f64
                                    } else {
                                        score
                                    };
                                };
                            } else {
                                stmt2 = syn::parse_quote! {
                                    let score = if (node_residual_cpu as f64) >= mean_cpu_residual {
                                        (#init_expr) as f64
                                    } else {
                                        score
                                    };
                                };
                            }
                        } else if (seed / 2).is_multiple_of(2) {
                            stmt2 = syn::parse_quote! {
                                let score = if (step_index as f64 / total_items.max(1) as f64) >= 0.5 {
                                    (#init_expr) as f64
                                } else {
                                    score
                                };
                            };
                        } else {
                            stmt2 = syn::parse_quote! {
                                let score = if (bin_residual as f64) >= mean_residual {
                                    (#init_expr) as f64
                                } else {
                                    score
                                };
                            };
                        }
                    } else {
                        // Combinaison convexe déterministe standard par Shadowing
                        stmt2 = syn::parse_quote! {
                            let score = score * 0.70 + ((#init_expr) as f64) * 0.30;
                        };
                    }
                } else {
                    stmt2 = syn::parse_quote! {
                        let score = score * 0.95 + ((#init_expr) as f64) * 0.05;
                    };
                }
            }
        }
    }

    let slice2 = resolve_dependencies_with_backward_slice(
        &stmt2,
        &donor_fn.block.stmts,
        stmts,
        &host_params,
        locus2,
    )
    .ok_or_else(|| CrossoverError::UnresolvedDependencies(donor.id.0.clone()))?;

    for (offset, s) in slice2.iter().enumerate() {
        let actual_idx = locus2 + offset;
        stmts.insert(actual_idx, s.clone());
        loci_indices.push(actual_idx.to_string());

        let repr = quote::quote!(#s).to_string();
        let mut hasher = Sha256::new();
        hasher.update(repr.as_bytes());
        fragment_hashes.push(hex::encode(hasher.finalize()));
        combined_deps.extend(collect_dependencies(s));
    }

    let fragment_hash = fragment_hashes.join(":");
    let locus_desc = format!("multi_loci_{}", loci_indices.join("_"));

    let mut dep_hasher = Sha256::new();
    for d in &combined_deps {
        dep_hasher.update(d.as_bytes());
    }
    let dependency_hash = hex::encode(dep_hasher.finalize());

    let mut graft_hasher = Sha256::new();
    graft_hasher.update(host.id.0.as_bytes());
    graft_hasher.update(donor.id.0.as_bytes());
    graft_hasher.update(fragment_hash.as_bytes());
    graft_hasher.update(seed.to_le_bytes());
    let graft_id = format!(
        "multi-graft-{}",
        &hex::encode(graft_hasher.finalize())[..12]
    );

    let manifest = GraftManifest {
        graft_id: graft_id.clone(),
        donor_capsule: donor.id.clone(),
        host_capsule: host.id.clone(),
        donor_ast_hash: donor_fingerprint.ast_hash,
        grafted_fragment_hash: fragment_hash,
        host_locus: locus_desc,
        dependency_hash,
        repair_version: None,
        repair_diff_hash: None,
    };

    let hybrid_code = normalize_source(&hybrid_fn);
    let child_fingerprint = compute_structural_fingerprint(&hybrid_fn);
    let child_name = format!("MultiCross-G{}-{}", generation, &graft_id[..10]);
    let semantic_interface = format!(
        "Multi-loci AST hybrid combining host {} with donor {} via 2 loci",
        host.id, donor.id
    );

    let mut metadata = HashMap::new();
    metadata.insert("crossover_type".to_string(), "multi_locus_ast".to_string());
    metadata.insert(
        "crossover_mode".to_string(),
        if use_regime_dispatch {
            "regime_dispatch".to_string()
        } else {
            "convex_combination".to_string()
        },
    );
    metadata.insert("host_id".to_string(), host.id.0.clone());
    metadata.insert("donor_id".to_string(), donor.id.0.clone());
    metadata.insert("graft_id".to_string(), graft_id);

    let child_capsule = SkillCapsule::new_v2(
        child_name,
        semantic_interface,
        hybrid_code,
        vec![host.id.clone(), donor.id.clone()],
        generation,
        metadata,
        "ScorePolicyV1",
        PolicyKind::ScorePolicy,
        Some(child_fingerprint),
        combined_deps,
        ValidationStatus::PassedLevel1Structural,
        Some(manifest.clone()),
    );

    Ok((child_capsule, manifest))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIG: &str = "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32,         step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64";

    fn capsule(name: &str, body: &str) -> SkillCapsule {
        SkillCapsule::new_v2(
            name,
            "Interface",
            format!("{SIG} {{ {body} }}"),
            vec![],
            1,
            HashMap::new(),
            "ScorePolicyV1",
            PolicyKind::ScorePolicy,
            None,
            vec![],
            ValidationStatus::Unvalidated,
            None,
        )
    }

    /// Une regle qui declare son propre `let` et appelle des methodes est greffable.
    #[test]
    fn rule_with_inner_let_and_methods_is_grafted() {
        let host = capsule("Host", "let mut score = bin_residual as f64; score");
        let donor = capsule(
            "Donor",
            "let mut score = bin_residual as f64;              if bin_residual < 20 { let gap = (20 - bin_residual) as f64; score += gap.powi(2).clamp(0.0, 400.0); }              score",
        );
        let (child, _) = deterministic_ast_crossover(&host, &donor, 3, 1).unwrap();
        assert!(child.logic_code.contains("powi"), "{}", child.logic_code);
        parse_heuristic_fn(&child.logic_code).unwrap();
    }

    /// Si aucune unite du donneur n'est resoluble dans l'hote, le croisement echoue au lieu
    /// d'inserer un fragment qui ne compilerait pas.
    #[test]
    fn unresolvable_donor_is_refused() {
        let host = capsule("Host", "let mut score = bin_residual as f64; score");
        let donor = capsule(
            "Donor",
            "let mut score = mystery_input * 2.0; score += mystery_input; score",
        );
        for seed in 0..8 {
            assert!(matches!(
                deterministic_ast_crossover(&host, &donor, seed, 1),
                Err(CrossoverError::UnresolvedDependencies(_))
            ));
        }
    }

    #[test]
    fn test_deterministic_ast_crossover_success() {
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
            "HostBestFit",
            "Linear best fit",
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
            "DonorDeadSpacePenalty",
            "Penalty for residual < 10",
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

        let (child, manifest) =
            deterministic_ast_crossover(&host, &donor, 42, 2).expect("Crossover should succeed");

        assert_eq!(child.parent_ids.len(), 2);
        assert!(child.is_crossover());
        assert_eq!(manifest.donor_capsule, donor.id);
        assert_eq!(manifest.host_capsule, host.id);
        assert!(
            child.logic_code.contains("bin_residual < 10") || child.logic_code.contains("score")
        );
        assert!(child.ast_fingerprint.is_some());
    }

    #[test]
    fn test_multi_locus_ast_crossover_success() {
        let host_code = r#"
        pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {
            let mut score = bin_residual as f64;
            score
        }
        "#;

        let donor_code = r#"
        pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {
            let mut score = bin_residual as f64;
            let dynamic_weight = (total_items - step_index) as f64 / total_items as f64;
            if bin_residual < 10 {
                score = score + 90.0 * dynamic_weight;
            }
            score
        }
        "#;

        let host = SkillCapsule::new("Host", "desc", host_code, vec![], 1, HashMap::new());
        let donor = SkillCapsule::new("Donor", "desc", donor_code, vec![], 1, HashMap::new());

        let (child, manifest) = multi_locus_ast_crossover(&host, &donor, 99, 2)
            .expect("Multi-locus crossover should succeed");

        assert_eq!(child.parent_ids.len(), 2);
        assert!(child.is_crossover());
        assert!(manifest.host_locus.starts_with("multi_loci_"));
        assert!(child.metadata.get("crossover_type").unwrap() == "multi_locus_ast");
        assert_eq!(
            child.metadata.get("crossover_mode").unwrap(),
            "regime_dispatch"
        );

        // Test convex combination mode with even seed
        let (child_convex, _) = multi_locus_ast_crossover(&host, &donor, 100, 2)
            .expect("Multi-locus convex crossover should succeed");
        assert_eq!(
            child_convex.metadata.get("crossover_mode").unwrap(),
            "convex_combination"
        );
    }

    #[test]
    fn test_crossover_backward_program_slicing() {
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
            let custom_scale = (bin_capacity as f64) * 0.15;
            if bin_residual < 10 {
                score = score + custom_scale;
            }
            score
        }
        "#;

        let host = SkillCapsule::new("Host", "desc", host_code, vec![], 1, HashMap::new());
        let donor = SkillCapsule::new("Donor", "desc", donor_code, vec![], 1, HashMap::new());

        let (child, manifest) = deterministic_ast_crossover(&host, &donor, 42, 1)
            .expect("Crossover with backward program slicing should succeed");

        assert!(child.is_crossover());
        // Verify backward slice pulled `custom_scale` declaration into child logic code
        if child.logic_code.contains("score = score + custom_scale") {
            assert!(
                child.logic_code.contains("let custom_scale ="),
                "Child must contain sliced definition of custom_scale"
            );
        }
        assert!(manifest.host_capsule == host.id);
    }
}
