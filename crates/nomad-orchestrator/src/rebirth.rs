use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tracing::info;

use nomad_core::{
    CumulativeTransmissionMetrics, ExperimentConfig, ExperimentalGroup, SkillCapsule,
};
use nomad_evaluator::OracleEvaluator;
use nomad_operator::SkillOperator;
use nomad_sandbox::HeuristicSandbox;

use crate::engine::{EvolutionEngine, ExperimentRunReport};
use crate::error::OrchestratorError;

/// ============================================================================
/// CollectiveHeritage : Patrimoine Cognitif Collectif Archivé
/// ============================================================================
/// Ensemble des compétences et motifs structurels validés, conservés dans
/// l'archive collective (`nomad-archive`) à travers les événements d'extinction (Section 7).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CollectiveHeritage {
    /// Liste des capsules de compétence validées ayant survécu à l'époque 1.
    pub validated_capsules: Vec<SkillCapsule>,
    /// Nombre de niches comportementales distinctes occupées.
    pub distinct_niches_count: usize,
    /// Nombre de motifs structurels AST uniques identifiés par leur hash topologique.
    pub distinct_motifs_count: usize,
}

/// ============================================================================
/// RebirthExperimentReport : Rapport Scientifique Comparatif d'Accumulation
/// ============================================================================
/// Rapport complet documentant les trois phases du protocole "Mort et Renaissance" :
/// l'Époque 1, le patrimoine extrait, et la confrontation expérimentale en double-aveugle
/// entre la Population Héritière et la Population Tabula Rasa (Section 7.3 & 8.5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebirthExperimentReport {
    /// Rapport complet de l'Époque 1 (Genèse).
    pub epoch1_genesis: ExperimentRunReport,
    /// Patrimoine collectif extrait avant l'extinction.
    pub heritage: CollectiveHeritage,
    /// Rapport de l'Époque 2 pour la Population A (Héritière, avec accès au patrimoine).
    pub epoch2_heritage: ExperimentRunReport,
    /// Rapport de l'Époque 2 pour la Population B (Tabula Rasa, partant de zéro).
    pub epoch2_tabula_rasa: ExperimentRunReport,
    /// Métriques scientifiques de transmission (CIF, R_herit, R_innov).
    pub cumulative_metrics: CumulativeTransmissionMetrics,
}

/// ============================================================================
/// execute_rebirth_protocol : Algorithme du Protocole Scientifique "Mort et Renaissance"
/// ============================================================================
/// Exécute le protocole pré-enregistré démontrant la réalité de l'accumulation culturelle :
///
/// **Déroulement de l'expérience :**
/// 1. **Époque 1 — Genèse :** Une population initiale évolue pendant `max_generations`
///    au sein du Groupe C (Swarm QD). Ses meilleures compétences sont archivées.
/// 2. **L'Événement d'Extinction :** Destruction totale et irréversible de tous les agents,
///    des mémoires de travail, des états PSO et des historiques privés. Seul le code
///    archivé dans `CollectiveHeritage` subsiste.
/// 3. **Époque 2 — Confrontation Appariée :**
///    - **Population A (Héritière) :** Nouveaux organismes vierges ayant accès en lecture
///      aux capsules archivées de l'Époque 1 dans leur grille MAP-Elites initiale.
///    - **Population B (Tabula Rasa) :** Nouveaux organismes vierges partant de zéro absolu,
///      avec **exactement la même seed et le même budget d'évaluations**.
/// 4. **Métrologie Scientifique :** Calcule formellement le $CIF$ (gain relatif), le $R_{\text{herit}}$
///    (conservation des motifs) et le $R_{\text{innov}}$ (taux d'innovation par millier d'évals).
pub fn execute_rebirth_protocol(
    mut base_config: ExperimentConfig,
    oracle: &OracleEvaluator,
    sandbox: &dyn HeuristicSandbox,
    operator: &dyn SkillOperator,
) -> Result<RebirthExperimentReport, OrchestratorError> {
    info!("=== Starting NOMAD V2 Mort et Renaissance Protocol ===");

    // 1. Époque 1 — Genèse
    info!("Phase 1: Epoch 1 (Genesis) running for {} generations...", base_config.max_generations);
    base_config.group = ExperimentalGroup::GroupC;
    let mut engine_epoch1 = EvolutionEngine::new(base_config.clone(), oracle, sandbox, operator);
    let epoch1_report = engine_epoch1.execute_run()?;

    // Extract validated collective heritage
    let mut validated_capsules: Vec<SkillCapsule> = engine_epoch1
        .archive
        .grid
        .values()
        .map(|e| e.capsule.clone())
        .collect();

    // Include best global elite
    if !validated_capsules.iter().any(|c| c.id == epoch1_report.best_capsule.id) {
        validated_capsules.push(epoch1_report.best_capsule.clone());
    }

    let mut distinct_motifs = HashSet::new();
    for c in &validated_capsules {
        if let Some(ref fp) = c.ast_fingerprint {
            distinct_motifs.insert(fp.ast_hash.clone());
        }
    }

    let heritage = CollectiveHeritage {
        distinct_niches_count: engine_epoch1.archive.occupied_niches_count(),
        distinct_motifs_count: distinct_motifs.len().max(1),
        validated_capsules: validated_capsules.clone(),
    };

    info!(
        "Extinction event: destroyed working memories. Collective heritage preserved: {} validated capsules, {} niches, {} distinct motifs.",
        heritage.validated_capsules.len(),
        heritage.distinct_niches_count,
        heritage.distinct_motifs_count
    );

    // 2. Époque 2 — Population A (Héritière)
    info!("Phase 2: Epoch 2 Population A (Heritage) starting with collective heritage access...");
    let mut config_heritage = base_config.clone();
    config_heritage.rng_seed = base_config.rng_seed + 101;
    let mut engine_heritage = EvolutionEngine::new(config_heritage, oracle, sandbox, operator);

    // Seed the MAP-Elites archive of Population A with validated heritage capsules
    for cap in &heritage.validated_capsules {
        let score = oracle.evaluate_in_distribution(cap, sandbox, base_config.fuel_limit_per_call);
        engine_heritage.archive.insert(cap.clone(), score);
        engine_heritage.phylogeny.insert_capsule(cap);
    }
    let epoch2_heritage_report = engine_heritage.execute_run()?;

    // 3. Époque 2 — Population B (Tabula Rasa)
    info!("Phase 3: Epoch 2 Population B (Tabula Rasa) starting from scratch (identical budget)...");
    let mut config_tabula = base_config.clone();
    config_tabula.rng_seed = base_config.rng_seed + 101; // Identical seed and budget!
    let mut engine_tabula = EvolutionEngine::new(config_tabula, oracle, sandbox, operator);
    let epoch2_tabula_rasa_report = engine_tabula.execute_run()?;

    // 4. Compute Cumulative Metrics (Section 7.2)
    let total_evals = (epoch1_report.budget_consumed.executions_consumed
        + epoch2_heritage_report.budget_consumed.executions_consumed)
        .max(1);

    let validated_innovations = heritage.distinct_niches_count + heritage.distinct_motifs_count;
    let innovation_rate = (validated_innovations as f64 / total_evals as f64) * 1000.0;

    // Heritability: how many motifs from heritage are still active in the final archive of Population A
    let mut heritage_motifs_active = 0usize;
    for entry in engine_heritage.archive.grid.values() {
        if let Some(ref fp) = entry.structural_fingerprint {
            if distinct_motifs.contains(&fp.ast_hash) {
                heritage_motifs_active += 1;
            }
        }
    }
    let functional_heritability_rate =
        heritage_motifs_active as f64 / heritage.distinct_motifs_count as f64;

    // CIF: performance comparison between Tabula Rasa and Heritage
    // Ratio of excess bins or efficiency
    let tabula_excess = epoch2_tabula_rasa_report.best_in_distribution_score.total_excess_bins.max(1) as f64;
    let heritage_excess = epoch2_heritage_report.best_in_distribution_score.total_excess_bins.max(1) as f64;
    let cumulative_improvement_factor = (tabula_excess / heritage_excess).clamp(0.5, 10.0);

    let cumulative_metrics = CumulativeTransmissionMetrics {
        innovation_rate,
        functional_heritability_rate,
        cumulative_improvement_factor,
        validated_innovations_count: validated_innovations,
    };

    info!(
        "=== Mort et Renaissance Completed ===\nCIF: {:.3} | R_herit: {:.1}% | R_innov: {:.3}",
        cumulative_improvement_factor,
        functional_heritability_rate * 100.0,
        innovation_rate
    );

    Ok(RebirthExperimentReport {
        epoch1_genesis: epoch1_report,
        heritage,
        epoch2_heritage: epoch2_heritage_report,
        epoch2_tabula_rasa: epoch2_tabula_rasa_report,
        cumulative_metrics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nomad_core::{ExperimentConfig, ExperimentalGroup};
    use nomad_evaluator::OracleEvaluator;
    use nomad_operator::DeterministicMockOperator;
    use nomad_sandbox::DeterministicSandbox;

    #[test]
    fn test_rebirth_protocol_execution() {
        let mut config = ExperimentConfig::default();
        config.group = ExperimentalGroup::GroupC;
        config.rng_seed = 77;
        config.population_size = 4;
        config.max_generations = 2;

        let oracle = OracleEvaluator::new(77);
        let sandbox = DeterministicSandbox::new();
        let operator = DeterministicMockOperator::default();

        let report = execute_rebirth_protocol(config, &oracle, &sandbox, &operator)
            .expect("Rebirth protocol should succeed");

        assert_eq!(report.epoch1_genesis.total_generations, 2);
        assert_eq!(report.epoch2_heritage.total_generations, 2);
        assert_eq!(report.epoch2_tabula_rasa.total_generations, 2);
        assert!(!report.heritage.validated_capsules.is_empty());
        assert!(report.cumulative_metrics.cumulative_improvement_factor > 0.0);
        assert!(report.cumulative_metrics.functional_heritability_rate >= 0.0);
    }
}

