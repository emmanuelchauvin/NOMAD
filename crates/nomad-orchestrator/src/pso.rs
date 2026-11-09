use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use nomad_core::{CapsuleId, LineageEvolutionState, MomentumAction, SemanticVelocity};

/// ============================================================================
/// LineageManager : Gestionnaire de Lignées et de Mémoire PSO
/// ============================================================================
/// Maintient l'état d'évolution persistant de chaque lignée d'organismes :
/// son Personal Best ($P_{\text{best}}$), son meilleur partenaire ($G_{\text{best\_partner}}$),
/// et sa vélocité sémantique précédente ($V_{\text{previous}}$) (Section 5.3 & 5.4).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LineageManager {
    lineages: HashMap<CapsuleId, LineageEvolutionState>,
}

impl LineageManager {
    /// Crée un nouveau gestionnaire de lignées vide.
    pub fn new() -> Self {
        Self {
            lineages: HashMap::new(),
        }
    }

    /// Enregistre une graine initiale ancestrale comme racine de sa lignée.
    pub fn register_seed(&mut self, root_id: CapsuleId) {
        self.lineages.insert(
            root_id.clone(),
            LineageEvolutionState {
                p_best: root_id,
                g_best_partner: None,
                v_previous: None,
                evolution_history: vec!["Initialized root seed individual".to_string()],
            },
        );
    }

    /// ========================================================================
    /// record_evolution : Algorithme de Mise à Jour de l'État PSO de Lignée
    /// ========================================================================
    /// Après un croisement ou une mutation :
    /// 1. Met à jour le $P_{\text{best}}$ si l'enfant surpasse l'ancêtre.
    /// 2. Ajuste la `SemanticVelocity` et le `MomentumAction` :
    ///    - `ContinueDirection` en cas d'amélioration.
    ///    - `Pivot` en cas de stagnation ou régression.
    /// 3. Met à jour le journal d'historique de la lignée.
    pub fn record_evolution(
        &mut self,
        child_id: CapsuleId,
        host_id: &CapsuleId,
        partner_id: Option<CapsuleId>,
        intent: String,
        is_improved: bool,
    ) {
        let parent_state = self.lineages.get(host_id).cloned();

        let p_best = if is_improved {
            child_id.clone()
        } else {
            parent_state
                .as_ref()
                .map(|s| s.p_best.clone())
                .unwrap_or_else(|| child_id.clone())
        };

        let mut history = parent_state
            .as_ref()
            .map(|s| s.evolution_history.clone())
            .unwrap_or_default();

        let entry = format!(
            "Graft from partner {:?} -> Child {}: {} (improved: {})",
            partner_id, child_id, intent, is_improved
        );
        history.push(entry);

        let action = if is_improved {
            MomentumAction::ContinueDirection
        } else if partner_id.is_some() {
            MomentumAction::Pivot
        } else {
            MomentumAction::Combine
        };

        let velocity = SemanticVelocity {
            weakness_diagnosed: "Suboptimal bin residual space or dead space ratio".to_string(),
            partner_mechanism: "Complementary conditional penalty / bonus fragment".to_string(),
            source_ast_selector: "donor.block.stmts[conditional_unit]".to_string(),
            target_insertion_locus: "host.block.stmts[pre_return]".to_string(),
            expected_behavioral_shift: intent,
            regression_risks: vec![
                "potential edge case overflow".to_string(),
                "redundant penalty".to_string(),
            ],
            momentum_action: action,
        };

        self.lineages.insert(
            child_id,
            LineageEvolutionState {
                p_best,
                g_best_partner: partner_id,
                v_previous: Some(velocity),
                evolution_history: history,
            },
        );
    }

    /// Retrieves lineage state for a given capsule.
    pub fn get_lineage(&self, id: &CapsuleId) -> Option<&LineageEvolutionState> {
        self.lineages.get(id)
    }
}
