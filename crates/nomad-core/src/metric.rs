use crate::capsule::SkillCapsule;
use crate::types::BehaviorDescriptor;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

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

/// Classe d'un réel à 1e-4 près, pour les critères 3 et 4 de l'ordre ci-dessous : deux
/// valeurs de la même classe sont à égalité et départagées par le critère suivant.
///
/// Comparer les classes donne un ordre total. L'ancienne comparaison « à 1e-4 près »
/// n'en était pas un (a ≈ b et b ≈ c sans a ≈ c), et le tri de Rust s'arrêtait parfois sur
/// « user-provided comparison function does not correctly implement a total order ».
/// NaN compte comme la pire valeur ; la conversion est saturante pour les infinis.
fn tolerance_class(x: f64) -> i64 {
    if x.is_nan() {
        i64::MIN
    } else {
        (x * 1e4).round() as i64
    }
}

/// Comparison order according to the scientific specifications:
/// 1. Validity (valid beats invalid)
/// 2. Total bins used (fewer bins is better)
/// 3. Dynamic Lyapunov / Imbalance Quality (higher tLDR / lower imbalance is better)
/// 4. Mean efficiency (higher packing efficiency is better)
/// 5. Fuel consumption (lower fuel/gas is ultimate parsimony tie-breaker)
///
/// Criteria 3 and 4 compare classes of 1e-4 (`tolerance_class`), so that the order is total.
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
        self.total_bins
            .cmp(&other.total_bins)
            // 3. Dynamic Lyapunov / Imbalance Quality (tLDR score: higher is better)
            .then_with(|| tolerance_class(other.tldr_score).cmp(&tolerance_class(self.tldr_score)))
            // 4. Mean efficiency (higher is better)
            .then_with(|| {
                tolerance_class(other.mean_efficiency).cmp(&tolerance_class(self.mean_efficiency))
            })
            // 5. Fuel consumption as parsimony tie-breaker (smaller is better)
            .then_with(|| self.total_fuel.cmp(&other.total_fuel))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn score(bins: usize, tldr: f64, efficiency: f64, fuel: u64) -> BenchmarkScore {
        BenchmarkScore {
            is_valid: true,
            total_bins: bins,
            total_theoretical_min_bins: 0,
            total_excess_bins: 0,
            mean_efficiency: efficiency,
            total_fuel: fuel,
            solved_instances: 1,
            total_instances: 1,
            behavior: BehaviorDescriptor::default(),
            tldr_score: tldr,
            trajectory_metrics: None,
        }
    }

    /// Vérifie sur tous les couples et triplets que `cmp` est un ordre total :
    /// antisymétrique et transitif.
    fn assert_total_order(scores: &[BenchmarkScore]) {
        for (i, a) in scores.iter().enumerate() {
            for (j, b) in scores.iter().enumerate() {
                assert_eq!(a.cmp(b), b.cmp(a).reverse(), "antisymetrie : {i} et {j}");
                if a.cmp(b) == Ordering::Greater {
                    continue;
                }
                for (k, c) in scores.iter().enumerate() {
                    if b.cmp(c) != Ordering::Greater {
                        assert_ne!(
                            a.cmp(c),
                            Ordering::Greater,
                            "transitivite : {i} <= {j} et {j} <= {k}, mais {i} > {k}"
                        );
                    }
                }
            }
        }
    }

    /// tLDR deux à deux à moins de 1e-4, sauf les extrêmes : une comparaison « à 1e-4
    /// près » donnait a < b (efficacité), b < c (efficacité) et c < a (tLDR).
    #[test]
    fn near_ties_do_not_form_a_cycle() {
        let a = score(10, 0.0, 0.9, 1);
        let b = score(10, 0.00006, 0.8, 1);
        let c = score(10, 0.00012, 0.7, 1);
        assert_total_order(&[a, b, c]);
    }

    #[test]
    fn order_is_total_on_near_ties_nan_infinities_and_invalid_scores() {
        let tldrs = [
            0.0,
            0.00003,
            0.00006,
            0.00009,
            0.00012,
            0.00015,
            -0.00005,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ];
        let efficiencies = [0.5, 0.50004, 0.50008, 0.50012, f64::NAN];
        let mut scores = Vec::new();
        for &tldr in &tldrs {
            for &efficiency in &efficiencies {
                for fuel in [1, 2] {
                    scores.push(score(10, tldr, efficiency, fuel));
                }
            }
        }
        scores.push(score(9, 0.0, 0.5, 1));
        scores.push(score(11, 0.0, 0.5, 1));
        let mut invalid = score(8, 1.0, 1.0, 0);
        invalid.is_valid = false;
        scores.push(invalid.clone());
        invalid.total_bins = 12;
        scores.push(invalid);
        assert_total_order(&scores);
    }

    /// Situation du tri des 1 359 candidats d'un run de tirage, arrêté par
    /// « user-provided comparison function does not correctly implement a total order »
    /// (protocole GLM H3, seed 10417) : beaucoup de scores aux mêmes bacs et aux tLDR
    /// voisins.
    #[test]
    fn sorting_many_near_ties_does_not_panic() {
        let mut scores: Vec<BenchmarkScore> = (0..2000)
            .map(|i| score(10, i as f64 * 0.00006, 1.0 - i as f64 * 0.0002, 1))
            .collect();
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        for i in (1..scores.len()).rev() {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            scores.swap(i, (state >> 33) as usize % (i + 1));
        }
        scores.sort();
        assert!(scores
            .windows(2)
            .all(|w| w[0].cmp(&w[1]) != Ordering::Greater));
    }
}
