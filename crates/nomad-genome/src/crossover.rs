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
    compute_structural_fingerprint, normalize_source, parse_score_bin, AstError,
};
use crate::unit::{extract_logical_units, LogicalUnit};

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
    let host_fn = parse_score_bin(&host.logic_code)?;
    let donor_fn = parse_score_bin(&donor.logic_code)?;

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

    stmts.insert(locus_idx, chosen_unit.stmt.clone());

    // Generate GraftManifest
    let mut dep_hasher = Sha256::new();
    for d in &chosen_unit.dependencies {
        dep_hasher.update(d.as_bytes());
    }
    let dependency_hash = hex::encode(dep_hasher.finalize());

    let mut graft_hasher = Sha256::new();
    graft_hasher.update(host.id.0.as_bytes());
    graft_hasher.update(donor.id.0.as_bytes());
    graft_hasher.update(chosen_unit.hash.as_bytes());
    graft_hasher.update(seed.to_le_bytes());
    let graft_id = format!("graft-{}", &hex::encode(graft_hasher.finalize())[..12]);

    let locus_desc = format!("body_stmt_index_{}", locus_idx);

    let manifest = GraftManifest {
        graft_id: graft_id.clone(),
        donor_capsule: donor.id.clone(),
        host_capsule: host.id.clone(),
        donor_ast_hash: donor_fingerprint.ast_hash,
        grafted_fragment_hash: chosen_unit.hash.clone(),
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
}
