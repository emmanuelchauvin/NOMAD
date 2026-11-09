use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use crate::capsule::SkillCapsule;
use crate::types::BehaviorDescriptor;

/// ============================================================================
/// InstanceEvaluation : Évaluation Détaillée d'une Instance Unique
/// ============================================================================
/// Résultat complet de l'exécution séquentielle d'une heuristique sur un problème donné.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstanceEvaluation {
    /// Nom ou identifiant de l'instance évaluée.
    pub instance_name: String,
    /// Validité de l'exécution (sans dépassement de capacité ni panic).
    pub is_valid: bool,
    /// Nombre total de bacs utilisés par l'algorithme.
    pub bins_used: usize,
    /// Borne inférieure théorique (volumétrique Martello-Toth).
    pub theoretical_min_bins: usize,
    /// Quantité totale de fuel (instructions logiques) consommée.
    pub fuel_consumed: u64,
    /// Message d'erreur explicite en cas d'échec ou d'invalidité.
    pub error_message: Option<String>,
    /// Score expérimental de décroissance de Lyapunov (tLDR) sur cette instance.
    #[serde(default)]
    pub tldr_score: f64,
    /// Historique pas-à-pas du nombre de bacs ouverts [B(1), B(2), ..., B(N)].
    #[serde(default)]
    pub trajectory: Vec<usize>,
}

impl InstanceEvaluation {
    /// Calcule le nombre de bacs excédentaires au-dessus de l'optimum théorique.
    pub fn excess_bins(&self) -> usize {
        self.bins_used.saturating_sub(self.theoretical_min_bins)
    }

    /// Calcule le ratio d'efficacité de compaction (théorique / utilisé).
    pub fn efficiency_ratio(&self) -> f64 {
        if self.bins_used == 0 {
            0.0
        } else {
            self.theoretical_min_bins as f64 / self.bins_used as f64
        }
    }
}

/// ============================================================================
/// BenchmarkScore : Score Agrégé sur un Jeu de Données Complet
/// ============================================================================
/// Mesure de performance globale agrégée sur un ensemble d'instances de test
/// (In-Distribution d'entraînement, OOD-1 échelle ou OOD-2 décalage de distribution).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkScore {
    /// Validité globale : vraie uniquement si TOUTES les instances ont réussi sans violation.
    pub is_valid: bool,
    /// Nombre total de bacs utilisés sur l'ensemble du dataset (critère principal à minimiser).
    pub total_bins: usize,
    /// Somme des bornes inférieures théoriques sur le dataset.
    pub total_theoretical_min_bins: usize,
    /// Total des bacs excédentaires au-dessus de l'optimum théorique.
    pub total_excess_bins: usize,
    /// Efficacité moyenne de compaction (1.0 = compaction parfaite sans aucun déchet).
    pub mean_efficiency: f64,
    /// Consommation totale de fuel logique (critère secondaire de départage).
    pub total_fuel: u64,
    /// Nombre d'instances résolues avec succès.
    pub solved_instances: usize,
    /// Nombre total d'instances du dataset.
    pub total_instances: usize,
    /// Descripteur comportemental agrégé pour la projection dans la grille MAP-Elites.
    #[serde(default)]
    pub behavior: BehaviorDescriptor,
    /// Score moyen de dynamique temporelle tLDR (Trajectory-aware Lyapunov Decay Rate).
    #[serde(default)]
    pub tldr_score: f64,
    /// Paramètres détaillés de la régression linéaire du tLDR.
    #[serde(default)]
    pub trajectory_metrics: Option<TrajectoryMetrics>,
}

/// ============================================================================
/// TrajectoryMetrics : Paramètres de la Régression Dynamique tLDR (Section 6.5)
/// ============================================================================
/// Détail mathématique de l'ajustement linéaire du potentiel d'espace résiduel :
/// y(t) = ln(V(t) + epsilon) régression sur le temps normalisé tau = t / N.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TrajectoryMetrics {
    /// Pente de régression beta_1 (une pente fortement négative indique une compaction rapide).
    pub beta1_slope: f64,
    /// Écart-type des résidus sigma_res (mesure de la régularité du processus d'insertion).
    pub sigma_res: f64,
    /// Score combiné : tLDR = -beta_1 - 0.5 * sigma_res.
    pub tldr: f64,
    /// Trajectoire de l'écart relatif volumétrique V(t) sur chaque pas de temps.
    pub prefix_waste: Vec<f64>,
}

/// ============================================================================
/// CumulativeTransmissionMetrics : Métriques de Transmission Culturelle (Section 7.2)
/// ============================================================================
/// Indicateurs scientifiques mesurant l'efficacité du patrimoine collectif
/// et la réalité de l'accumulation culturelle intergénérationnelle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CumulativeTransmissionMetrics {
    /// R_innov : Taux d'innovations comportementales et structurelles validées pour 1 000 évaluations.
    pub innovation_rate: f64,
    /// R_herit : Proportion des motifs innovants transmis qui demeurent fonctionnellement actifs.
    pub functional_heritability_rate: f64,
    /// CIF historique (champ déprécié, maintenu pour compatibilité, aligné sur cif_performance).
    #[deprecated(note = "Use cif_performance, cif_qd, or cif_functional_transmission instead")]
    pub cumulative_improvement_factor: f64,
    /// Total count of validated innovations preserved in collective heritage.
    pub validated_innovations_count: usize,
    /// CIF Performance : ratio brut tabula_excess / heritage_excess sans clamp (None si excès nul).
    #[serde(default)]
    pub cif_performance: Option<f64>,
    /// CIF QD : ratio de diversité QD-Score heritage / tabula sans clamp.
    #[serde(default)]
    pub cif_qd: f64,
    /// CIF Transmission Fonctionnelle : conservation des motifs fonctionnels hérités (= functional_heritability_rate).
    #[serde(default)]
    pub cif_functional_transmission: f64,
}

impl BenchmarkScore {
    /// Fatal penalty score assigned when code fails to compile or crashes.
    pub fn fatal_penalty(total_instances: usize) -> Self {
        Self {
            is_valid: false,
            total_bins: usize::MAX / 2,
            total_theoretical_min_bins: 0,
            total_excess_bins: usize::MAX / 2,
            mean_efficiency: 0.0,
            total_fuel: u64::MAX,
            solved_instances: 0,
            total_instances,
            behavior: BehaviorDescriptor::default(),
            tldr_score: 0.0,
            trajectory_metrics: None,
        }
    }
}

/// Comparison order according to the scientific specifications:
/// 1. Validity (valid beats invalid)
/// 2. Total bins used (fewer bins is better)
/// 3. Total fuel consumed (lower gas is tie-breaker)
impl PartialOrd for BenchmarkScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for BenchmarkScore {}

impl Ord for BenchmarkScore {
    fn cmp(&self, other: &Self) -> Ordering {
        // 1. Validity first
        match (self.is_valid, other.is_valid) {
            (true, false) => return Ordering::Less, // self is better (ranks higher)
            (false, true) => return Ordering::Greater,
            (false, false) => return Ordering::Equal,
            (true, true) => {}
        }

        // 2. Total bins used (smaller is better)
        match self.total_bins.cmp(&other.total_bins) {
            Ordering::Less => Ordering::Less,
            Ordering::Greater => Ordering::Greater,
            // 3. Fuel consumption as tie-breaker (smaller is better)
            Ordering::Equal => self.total_fuel.cmp(&other.total_fuel),
        }
    }
}

/// A population member evaluated by the oracle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluatedIndividual {
    pub capsule: SkillCapsule,
    pub in_distribution_score: BenchmarkScore,
    pub ood1_score: Option<BenchmarkScore>,
    pub ood2_score: Option<BenchmarkScore>,
    pub rank: usize,
}

impl EvaluatedIndividual {
    pub fn new(capsule: SkillCapsule, score: BenchmarkScore) -> Self {
        Self {
            capsule,
            in_distribution_score: score,
            ood1_score: None,
            ood2_score: None,
            rank: 0,
        }
    }
}

