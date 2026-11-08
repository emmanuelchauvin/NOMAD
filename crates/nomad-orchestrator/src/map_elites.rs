use std::collections::HashMap;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use nomad_core::{
    BehaviorDescriptor, BenchmarkScore, SkillCapsule, StructuralFingerprint,
};

/// ============================================================================
/// ArchiveEntry : Représentant Élite d'une Niche Comportementale
/// ============================================================================
/// Entrée stockée dans la grille MAP-Elites, conservant la meilleure capsule
/// ayant atteint ces coordonnées comportementales spécifiques.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveEntry {
    /// Capsule de compétence élite de cette niche.
    pub capsule: SkillCapsule,
    /// Coordonnées discrètes dans la grille (x, y).
    pub niche_coord: (usize, usize),
    /// Vecteur de descripteurs comportementaux réels de la capsule.
    pub behavior: BehaviorDescriptor,
    /// Score de benchmark obtenu (fitness et fuel).
    pub fitness: BenchmarkScore,
    /// Empreinte topologique AST de la capsule élite.
    pub structural_fingerprint: Option<StructuralFingerprint>,
    /// Nombre de fois où cette niche a été échantillonnée comme parent (pour UCB).
    pub times_sampled: usize,
    /// Nombre de fois où l'élite de cette niche a été remplacée par un meilleur candidat.
    pub times_replaced: usize,
}

/// ============================================================================
/// MapElitesArchive : Grille Quality-Diversity MAP-Elites 2D
/// ============================================================================
/// Discrétise l'espace des comportements en une grille 2D dense (dim_x × dim_y) :
/// - **Dimension X :** Ratio d'espace mort / perte sèche (< 10).
/// - **Dimension Y :** Ratio d'ouverture de nouveaux bacs.
///
/// **Compétition locale intra-niche :**
/// Les organismes ne s'affrontent pas globalement. Un nouvel individu ne combat que
/// l'occupant de sa propre cellule de niche. Cela empêche l'effondrement de diversité
/// (mode collapse) et préserve les solutions "tremplins génétiques" (Section 3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapElitesArchive {
    /// Résolution de l'axe X (espace mort).
    pub dim_x: usize,
    /// Résolution de l'axe Y (ouverture de bacs).
    pub dim_y: usize,
    /// Grille associative indexée par coordonnées "x,y".
    pub grid: HashMap<String, ArchiveEntry>,
    /// Nombre total d'échantillonnages effectués (compteur global pour UCB).
    pub total_samples: usize,
    /// Facteur d'exploration de la curiosité UCB (typiquement sqrt(2) ≈ 1.414).
    pub ucb_exploration_factor: f64,
}

impl Default for MapElitesArchive {
    fn default() -> Self {
        Self::new(10, 10)
    }
}

impl MapElitesArchive {
    /// Initialise une nouvelle grille MAP-Elites aux dimensions spécifiées (ex: 10x10 = 100 niches).
    pub fn new(dim_x: usize, dim_y: usize) -> Self {
        Self {
            dim_x: dim_x.max(1),
            dim_y: dim_y.max(1),
            grid: HashMap::new(),
            total_samples: 0,
            ucb_exploration_factor: 1.414,
        }
    }

    /// ========================================================================
    /// coord_for_behavior : Algorithme de Projection Continue -> Discrète
    /// ========================================================================
    /// Projette le vecteur de comportement continu d'une heuristique dans les indices
    /// discrets (x, y) de la grille de niches.
    pub fn coord_for_behavior(&self, b: &BehaviorDescriptor) -> (usize, usize) {
        // Axe X : Dead space ratio normalisé sur l'intervalle effectif [0.0, 0.60]
        let norm_x = (b.dead_space_ratio / 0.60).clamp(0.0, 0.999);
        let x = (norm_x * self.dim_x as f64).floor() as usize;

        // Axe Y : New bin ratio normalisé sur son domaine physique effectif [0.45, 0.60]
        // (Pour N=50, C=100 et items U(10, 80), new_bin_ratio oscille naturellement entre 0.46 et 0.58)
        let norm_y = ((b.new_bin_ratio - 0.45) / 0.15).clamp(0.0, 0.999);
        let y = (norm_y * self.dim_y as f64).floor() as usize;

        (x.min(self.dim_x - 1), y.min(self.dim_y - 1))
    }

    /// Convertit des coordonnées (x, y) en clé de hashage textuelle sérialisable "x,y".
    pub fn key(coord: (usize, usize)) -> String {
        format!("{},{}", coord.0, coord.1)
    }

    /// ========================================================================
    /// insert : Algorithme d'Insertion Compétitive Locale
    /// ========================================================================
    /// Tente d'insérer un organisme dans sa niche comportementale :
    /// - Si la cellule est vide : insertion directe (découverte d'une nouvelle niche).
    /// - Si la cellule est occupée : compétition locale. Le candidat ne remplace
    ///   l'élite existante que s'il a une fitness strictement supérieure (moins de bacs,
    ///   puis départage au fuel).
    pub fn insert(&mut self, capsule: SkillCapsule, fitness: BenchmarkScore) -> bool {
        if !fitness.is_valid {
            return false;
        }

        let coord = self.coord_for_behavior(&fitness.behavior);
        let k = Self::key(coord);
        let fp = capsule.ast_fingerprint.clone();

        match self.grid.get_mut(&k) {
            Some(existing) => {
                // Compétition locale intra-niche
                if fitness < existing.fitness {
                    existing.capsule = capsule;
                    existing.behavior = fitness.behavior.clone();
                    existing.fitness = fitness;
                    existing.structural_fingerprint = fp;
                    existing.times_replaced += 1;
                    true
                } else {
                    false
                }
            }
            None => {
                self.grid.insert(
                    k,
                    ArchiveEntry {
                        capsule,
                        niche_coord: coord,
                        behavior: fitness.behavior.clone(),
                        fitness,
                        structural_fingerprint: fp,
                        times_sampled: 0,
                        times_replaced: 0,
                    },
                );
                true
            }
        }
    }

    /// ========================================================================
    /// select_parent_ucb : Algorithme de Sélection par Curiosité UCB
    /// ========================================================================
    /// Échantillonne un parent selon le principe Upper Confidence Bound (Section 3.3) :
    /// UCB(niche) = Qualité(niche) + c * sqrt(ln(N_total) / N_niche)
    /// Équilibre l'exploitation des meilleures niches et l'exploration des niches sous-échantillonnées.
    pub fn select_parent_ucb(&mut self, rng: &mut ChaCha8Rng) -> Option<SkillCapsule> {
        if self.grid.is_empty() {
            return None;
        }

        self.total_samples += 1;
        let total_ln = ((self.total_samples + 1) as f64).ln();

        let mut best_key = None;
        let mut best_ucb = f64::NEG_INFINITY;

        for (k, entry) in &self.grid {
            let q = entry.fitness.mean_efficiency;
            let ucb = q + self.ucb_exploration_factor * (total_ln / (entry.times_sampled + 1) as f64).sqrt();

            let jitter: f64 = rng.gen_range(-1e-6..1e-6);
            if ucb + jitter > best_ucb {
                best_ucb = ucb + jitter;
                best_key = Some(k.clone());
            }
        }

        if let Some(k) = best_key {
            if let Some(entry) = self.grid.get_mut(&k) {
                entry.times_sampled += 1;
                return Some(entry.capsule.clone());
            }
        }

        None
    }

    /// ========================================================================
    /// select_complementary_partner : Algorithme de Ciblage Complémentaire AgentPSO
    /// ========================================================================
    /// Sélectionne pour un parent hôte A un partenaire B maximisant :
    /// Score(B|A) = 0.50 * Qualité(B) + 0.35 * Complémentarité(A, B) + 0.15 * Distance_AST(A, B)
    /// Permet un appariement dirigé vers les niches fortes là où l'hôte est déficitaire.
    pub fn select_complementary_partner(
        &mut self,
        host: &SkillCapsule,
        host_score: &BenchmarkScore,
        rng: &mut ChaCha8Rng,
    ) -> Option<SkillCapsule> {
        if self.grid.len() < 2 {
            // If only 1 elite exists, return it if different
            return self.grid.values().next().map(|e| e.capsule.clone());
        }

        let mut best_partner = None;
        let mut best_score = f64::NEG_INFINITY;

        for entry in self.grid.values() {
            if entry.capsule.id == host.id {
                continue;
            }

            let q_b = entry.fitness.mean_efficiency;

            // Complementarity: difference in dead space behavior and new bin opening
            let dead_space_diff = (host_score.behavior.dead_space_ratio - entry.behavior.dead_space_ratio).abs();
            let bin_ratio_diff = (host_score.behavior.new_bin_ratio - entry.behavior.new_bin_ratio).abs();
            let c_complement = dead_space_diff + bin_ratio_diff;

            // Structural distance: difference in AST statement and branch counts
            let d_ast = match (&host.ast_fingerprint, &entry.structural_fingerprint) {
                (Some(fp_a), Some(fp_b)) => {
                    let diff_stmts = (fp_a.statement_count as f64 - fp_b.statement_count as f64).abs();
                    let diff_branches = (fp_a.branch_count as f64 - fp_b.branch_count as f64).abs();
                    (diff_stmts + diff_branches) / 10.0
                }
                _ => 0.5,
            };

            let pso_score = 0.50 * q_b + 0.35 * c_complement + 0.15 * d_ast;
            let jitter: f64 = rng.gen_range(-1e-6..1e-6);

            if pso_score + jitter > best_score {
                best_score = pso_score + jitter;
                best_partner = Some(entry.capsule.clone());
            }
        }

        best_partner.or_else(|| self.grid.values().next().map(|e| e.capsule.clone()))
    }

    /// Computes the Quality-Diversity score: sum of fitness across all occupied niches.
    pub fn qd_score(&self) -> f64 {
        self.grid
            .values()
            .map(|e| e.fitness.mean_efficiency)
            .sum()
    }

    /// Ratio of occupied niches over total grid capacity.
    pub fn coverage(&self) -> f64 {
        let total_cells = (self.dim_x * self.dim_y) as f64;
        self.grid.len() as f64 / total_cells
    }

    /// Count of active behavioral niches.
    pub fn occupied_niches_count(&self) -> usize {
        self.grid.len()
    }

    /// Returns the global best elite capsule across all niches.
    pub fn best_elite(&self) -> Option<&ArchiveEntry> {
        self.grid
            .values()
            .min_by(|a, b| a.fitness.cmp(&b.fitness))
    }

    /// Returns all current niche elites.
    pub fn all_elites(&self) -> Vec<&ArchiveEntry> {
        self.grid.values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_map_elites_insertion_and_local_competition() {
        let mut archive = MapElitesArchive::new(10, 10);
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        let cap1 = SkillCapsule::new("Cap1", "Desc", "BEST_FIT", vec![], 1, HashMap::new());
        let score1 = BenchmarkScore {
            is_valid: true,
            total_bins: 500,
            total_theoretical_min_bins: 480,
            total_excess_bins: 20,
            mean_efficiency: 0.96,
            total_fuel: 10_000,
            solved_instances: 50,
            total_instances: 50,
            behavior: BehaviorDescriptor {
                residual_variance: 5.0,
                new_bin_ratio: 0.30,
                mean_fullness: 0.90,
                dead_space_ratio: 0.05,
            },
            tldr_score: 0.92,
            trajectory_metrics: None,
        };

        let inserted = archive.insert(cap1.clone(), score1.clone());
        assert!(inserted);
        assert_eq!(archive.occupied_niches_count(), 1);

        // Submit a worse individual into the EXACT SAME NICHE
        let cap2 = SkillCapsule::new("Cap2", "Desc", "FIRST_FIT", vec![], 1, HashMap::new());
        let mut score2 = score1.clone();
        score2.total_bins = 550; // worse!

        let inserted_worse = archive.insert(cap2.clone(), score2.clone());
        assert!(!inserted_worse); // Rejected by local competition!
        assert_eq!(archive.best_elite().unwrap().capsule.id, cap1.id);

        // Submit a better individual into the SAME NICHE
        let cap3 = SkillCapsule::new("Cap3", "Desc", "SUPER_FIT", vec![], 1, HashMap::new());
        let mut score3 = score1.clone();
        score3.total_bins = 490; // better!

        let inserted_better = archive.insert(cap3.clone(), score3.clone());
        assert!(inserted_better); // Replaces cap1!
        assert_eq!(archive.best_elite().unwrap().capsule.id, cap3.id);

        // Test UCB selection
        let parent = archive.select_parent_ucb(&mut rng).unwrap();
        assert_eq!(parent.id, cap3.id);
    }
}

