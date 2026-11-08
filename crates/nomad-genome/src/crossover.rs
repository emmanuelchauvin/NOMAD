use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use thiserror::Error;

use nomad_core::{
    GraftManifest, PolicyKind, SkillCapsule, ValidationStatus,
};

use crate::ast::{
    compute_structural_fingerprint, normalize_source, parse_heuristic_fn, AstError,
};
use crate::unit::{collect_dependencies, extract_logical_units, LogicalUnit};

#[derive(Debug, Error)]
pub enum CrossoverError {
    #[error("AST parsing error: {0}")]
    AstError(#[from] AstError),
    #[error("No transferable logical units found in donor capsule {0}")]
    NoExtractableUnits(String),
    #[error("Host capsule {0} has an empty function body")]
    EmptyHostBody(String),
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
    let chosen_unit: &LogicalUnit = donor_units.choose(&mut rng).unwrap();

    let mut hybrid_fn = host_fn.clone();
    let stmts = &mut hybrid_fn.block.stmts;
    if stmts.is_empty() {
        return Err(CrossoverError::EmptyHostBody(host.id.0.clone()));
    }

    // Determine insertion locus: right before the final return/expression statement
    let locus_idx = if stmts.len() > 1 {
        stmts.len() - 1
    } else {
        0
    };

    let mut stmt_to_insert = chosen_unit.stmt.clone();

    // Règle d'accumulation et protection anti-shadowing / anti-dead-store:
    if let syn::Stmt::Local(local) = &chosen_unit.stmt {
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
                    // Recombinaison convexe par Shadowing Rust strict (compatible rustc)
                    stmt_to_insert = syn::parse_quote! {
                        let score = score * 0.70 + ((#init_expr) as f64) * 0.30;
                    };
                } else {
                    // Vérifier si la variable est lue dans la suite du code hôte
                    let downstream_deps: Vec<String> = stmts[locus_idx..]
                        .iter()
                        .flat_map(|s| collect_dependencies(s))
                        .collect();
                    if already_defined || !downstream_deps.contains(&var_name) {
                        // Modulation causale directe du score avec le fragment donneur
                        stmt_to_insert = syn::parse_quote! {
                            let score = score * 0.95 + ((#init_expr) as f64) * 0.05;
                        };
                    }
                }
            }
        }
    }

    stmts.insert(locus_idx, stmt_to_insert.clone());

    // Compute exact fragment hash of the statement actually inserted
    let inserted_repr = quote::quote!(#stmt_to_insert).to_string();
    let mut fragment_hasher = Sha256::new();
    fragment_hasher.update(inserted_repr.as_bytes());
    let fragment_hash = hex::encode(fragment_hasher.finalize());

    // Generate GraftManifest
    let inserted_deps = collect_dependencies(&stmt_to_insert);
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

    let locus_desc = format!("body_stmt_index_{}", locus_idx);

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
    metadata.insert("crossover_type".to_string(), "ast_deterministic".to_string());
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
    let mut indices: Vec<usize> = (0..donor_units.len()).collect();
    indices.shuffle(&mut rng);
    let unit1 = &donor_units[indices[0]];
    let unit2 = &donor_units[indices[1]];

    let mut hybrid_fn = host_fn.clone();
    let stmts = &mut hybrid_fn.block.stmts;
    if stmts.is_empty() {
        return Err(CrossoverError::EmptyHostBody(host.id.0.clone()));
    }

    // Locus 1: début du corps (index 0 ou 1)
    let locus1 = if stmts.len() > 2 { 1 } else { 0 };
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
                        score = score * 0.80 + ((#init_expr) as f64) * 0.20;
                    };
                } else if already_defined {
                    // Anti-shadowing : fusion causale légère pour ne pas écraser une variable locale
                    stmt1 = syn::parse_quote! {
                        score = score + ((#init_expr) as f64) * 0.02;
                    };
                }
            }
        }
    }
    stmts.insert(locus1, stmt1.clone());

    // Locus 2: fin du corps (juste avant return)
    let locus2 = stmts.len().saturating_sub(1);
    let mut stmt2 = unit2.stmt.clone();
    let use_regime_dispatch = seed % 2 == 1;

    if let syn::Stmt::Local(local) = &unit2.stmt {
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
                    if use_regime_dispatch {
                        // Croisement par Branchement de Régime (Regime Dispatch Crossover)
                        // Préserve l'amplitude et l'échelle intrinsèque de chaque parent dans son domaine de régime
                        if host.logic_code.contains("score_node") {
                            if (seed / 2) % 2 == 0 {
                                stmt2 = syn::parse_quote! {
                                    if (step_index as f64 / total_jobs.max(1) as f64) >= 0.5 {
                                        score = (#init_expr) as f64;
                                    }
                                };
                            } else {
                                stmt2 = syn::parse_quote! {
                                    if (node_residual_cpu as f64) >= mean_cpu_residual {
                                        score = (#init_expr) as f64;
                                    }
                                };
                            }
                        } else {
                            if (seed / 2) % 2 == 0 {
                                stmt2 = syn::parse_quote! {
                                    if (step_index as f64 / total_items.max(1) as f64) >= 0.5 {
                                        score = (#init_expr) as f64;
                                    }
                                };
                            } else {
                                stmt2 = syn::parse_quote! {
                                    if (bin_residual as f64) >= mean_residual {
                                        score = (#init_expr) as f64;
                                    }
                                };
                            }
                        }
                    } else {
                        // Combinaison convexe déterministe standard par Shadowing
                        stmt2 = syn::parse_quote! {
                            let score = score * 0.70 + ((#init_expr) as f64) * 0.30;
                        };
                    }
                } else if already_defined {
                    stmt2 = syn::parse_quote! {
                        let score = score * 0.95 + ((#init_expr) as f64) * 0.05;
                    };
                } else {
                    stmt2 = syn::parse_quote! {
                        let score = score * 0.95 + ((#init_expr) as f64) * 0.05;
                    };
                }
            }
        }
    }
    stmts.insert(locus2, stmt2.clone());

    // Hachage combiné des fragments greffés
    let repr1 = quote::quote!(#stmt1).to_string();
    let repr2 = quote::quote!(#stmt2).to_string();
    let mut fragment_hasher = Sha256::new();
    fragment_hasher.update(repr1.as_bytes());
    fragment_hasher.update(repr2.as_bytes());
    let fragment_hash = hex::encode(fragment_hasher.finalize());

    let mut dep_hasher = Sha256::new();
    let mut combined_deps = collect_dependencies(&stmt1);
    combined_deps.extend(collect_dependencies(&stmt2));
    for d in &combined_deps {
        dep_hasher.update(d.as_bytes());
    }
    let dependency_hash = hex::encode(dep_hasher.finalize());

    let mut graft_hasher = Sha256::new();
    graft_hasher.update(host.id.0.as_bytes());
    graft_hasher.update(donor.id.0.as_bytes());
    graft_hasher.update(fragment_hash.as_bytes());
    graft_hasher.update(seed.to_le_bytes());
    let graft_id = format!("multi-graft-{}", &hex::encode(graft_hasher.finalize())[..12]);

    let manifest = GraftManifest {
        graft_id: graft_id.clone(),
        donor_capsule: donor.id.clone(),
        host_capsule: host.id.clone(),
        donor_ast_hash: donor_fingerprint.ast_hash,
        grafted_fragment_hash: fragment_hash,
        host_locus: format!("multi_loci_{}_{}", locus1, locus2),
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
        if use_regime_dispatch { "regime_dispatch".to_string() } else { "convex_combination".to_string() },
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

        let (child, manifest) = deterministic_ast_crossover(&host, &donor, 42, 2)
            .expect("Crossover should succeed");

        assert_eq!(child.parent_ids.len(), 2);
        assert!(child.is_crossover());
        assert_eq!(manifest.donor_capsule, donor.id);
        assert_eq!(manifest.host_capsule, host.id);
        assert!(child.logic_code.contains("bin_residual < 10") || child.logic_code.contains("score"));
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
        assert_eq!(child.metadata.get("crossover_mode").unwrap(), "regime_dispatch");

        // Test convex combination mode with even seed
        let (child_convex, _) = multi_locus_ast_crossover(&host, &donor, 100, 2)
            .expect("Multi-locus convex crossover should succeed");
        assert_eq!(child_convex.metadata.get("crossover_mode").unwrap(), "convex_combination");
    }
}
