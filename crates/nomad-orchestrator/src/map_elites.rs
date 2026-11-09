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
        // Axe X : Dead space ratio critique [1..15] normalisé sur [0.15, 0.75].
        let norm_x = ((b.dead_space_ratio - 0.15) / 0.60).clamp(0.0, 0.999);
        let x = (norm_x * self.dim_x as f64).floor() as usize;

        // Axe Y : Entropie de Shannon composite mi/fin de parcours (ou Imbalance en scheduling)
        // Normalisation continue et monotone évitant les collisions de métriques et discontinuités de seuils
        let norm_y = if b.residual_variance > 0.0 {
            if b.residual_variance <= 2.5 {
                // Bin Packing : Entropie de Shannon composite canonique [0.20, 2.00]
                ((b.residual_variance - 0.20) / 1.80).clamp(0.0, 0.999)
            } else {
                // Scheduling : Déséquilibre CPU/RAM normalisé continu [2.5, 40.0]
                ((b.residual_variance - 2.5) / 37.5).clamp(0.0, 0.999)
            }
        } else {
            // Repli déterministe sur le ratio d'ouverture de bacs [0.30, 0.70]
            ((b.new_bin_ratio - 0.30) / 0.40).clamp(0.0, 0.999)
        };
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
                // Compétition locale intra-niche :
                // 1. Moins de bacs est toujours strictement meilleur (ou meilleure efficacité)
                // 2. En cas d'égalité stricte sur total_bins, départager par tldr_score supérieur (stabilité de Lyapunov)
                // 3. À égalité de total_bins et tldr_score proche (< 1e-3), départager par PARCIMONIE STRICTE : total_fuel inférieur
                let is_strictly_better = if fitness.total_bins < existing.fitness.total_bins {
                    true
                } else if fitness.total_bins == existing.fitness.total_bins {
                    let tldr_diff = fitness.tldr_score - existing.fitness.tldr_score;
                    if tldr_diff > 1e-3 {
                        true
                    } else if tldr_diff < -1e-3 {
                        false
                    } else {
                        fitness.total_fuel < existing.fitness.total_fuel
                    }
                } else {
                    false
                };

                if is_strictly_better {
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
        let host_hash = host.ast_fingerprint.as_ref().map(|f| &f.ast_hash);

        if self.grid.len() < 2 {
            // If only 1 elite exists, return it ONLY if different from host in ID and AST hash
            let candidate = self.grid.values().next().map(|e| e.capsule.clone());
            if let Some(ref c) = candidate {
                if c.id == host.id {
                    return None;
                }
                if let (Some(h1), Some(h2)) = (host_hash, c.ast_fingerprint.as_ref().map(|f| &f.ast_hash)) {
                    if h1 == h2 {
                        return None;
                    }
                }
            }
            return candidate;
        }

        let mut best_partner = None;
        let mut best_score = f64::NEG_INFINITY;

        for entry in self.grid.values() {
            if entry.capsule.id == host.id {
                continue;
            }
            if let (Some(h1), Some(h2)) = (host_hash, entry.structural_fingerprint.as_ref().map(|f| &f.ast_hash)) {
                if h1 == h2 {
                    continue;
                }
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

    #[test]
    fn test_map_elites_fuel_parsimony_and_tldr_tie_breaking() {
        let mut archive = MapElitesArchive::new(10, 10);

        let cap_base = SkillCapsule::new("Base", "Desc", "code", vec![], 5, HashMap::new());
        let score_base = BenchmarkScore {
            is_valid: true,
            total_bins: 500,
            total_theoretical_min_bins: 480,
            total_excess_bins: 20,
            mean_efficiency: 0.96,
            total_fuel: 100_000,
            solved_instances: 50,
            total_instances: 50,
            behavior: BehaviorDescriptor {
                residual_variance: 1.2,
                new_bin_ratio: 0.30,
                mean_fullness: 0.90,
                dead_space_ratio: 0.15,
            },
            tldr_score: 0.80,
            trajectory_metrics: None,
        };
        assert!(archive.insert(cap_base.clone(), score_base.clone()));

        // Candidate 1: Higher tLDR with equal bins replaces incumbent even with higher fuel
        let cap_higher_tldr = SkillCapsule::new("HigherTLDR", "Desc", "code", vec![], 2, HashMap::new());
        let mut score_higher_tldr = score_base.clone();
        score_higher_tldr.tldr_score = 0.85;
        score_higher_tldr.total_fuel = 150_000;
        assert!(archive.insert(cap_higher_tldr.clone(), score_higher_tldr.clone()));
        assert_eq!(archive.best_elite().unwrap().capsule.id, cap_higher_tldr.id);

        // Candidate 2: Higher generation but higher fuel and identical tLDR is REJECTED (bloat prevention)
        let cap_bloated = SkillCapsule::new("Bloated", "Desc", "code", vec![], 9, HashMap::new());
        let mut score_bloated = score_higher_tldr.clone();
        score_bloated.total_fuel = 200_000;
        assert!(!archive.insert(cap_bloated.clone(), score_bloated.clone()));
        assert_eq!(archive.best_elite().unwrap().capsule.id, cap_higher_tldr.id);

        // Candidate 3: Lower generation (Gen 0 seed) with identical bins and tLDR, but lower fuel WINS (strict parsimony)
        let cap_parsimonious = SkillCapsule::new("ParsimoniousSeed", "Desc", "code", vec![], 0, HashMap::new());
        let mut score_parsimonious = score_higher_tldr.clone();
        score_parsimonious.total_fuel = 60_000;
        assert!(archive.insert(cap_parsimonious.clone(), score_parsimonious.clone()));
        assert_eq!(archive.best_elite().unwrap().capsule.id, cap_parsimonious.id);
    }
}

