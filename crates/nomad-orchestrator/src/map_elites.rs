use rand::Rng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::HashMap;

use nomad_core::{
    BehaviorDescriptor, BenchmarkScore, ParentSelection, ProblemDomain, SkillCapsule,
    StructuralFingerprint,
};

/// Rapport geometrique de la selection par rang : chaque rang a moitie moins de chances
/// que le precedent.
const RANK_SELECTION_RATIO: f64 = 0.5;

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
    /// Résolution de l'axe X (espace mort ou nœuds denses).
    pub dim_x: usize,
    /// Résolution de l'axe Y (entropie ou déséquilibre CPU/RAM).
    pub dim_y: usize,
    /// Grille associative indexée par coordonnées "x,y".
    /// Table ordonnee : l'ordre d'iteration, donc les tirages qui en dependent, est le meme
    /// d'une execution a l'autre.
    pub grid: BTreeMap<String, ArchiveEntry>,
    /// Nombre total d'échantillonnages effectués (compteur global pour UCB).
    pub total_samples: usize,
    /// Facteur d'exploration de la curiosité UCB (typiquement sqrt(2) ≈ 1.414).
    pub ucb_exploration_factor: f64,
    /// Domaine du problème combinatoire pour le calibrage de l'espace des niches.
    #[serde(default)]
    pub domain: ProblemDomain,
}

impl Default for MapElitesArchive {
    fn default() -> Self {
        Self::new(10, 10)
    }
}

impl MapElitesArchive {
    /// Initialise une nouvelle grille MAP-Elites aux dimensions spécifiées (défaut BinPacking1D).
    pub fn new(dim_x: usize, dim_y: usize) -> Self {
        Self::new_with_domain(dim_x, dim_y, ProblemDomain::BinPacking1D)
    }

    /// Initialise une nouvelle grille MAP-Elites avec un domaine combinatoire explicite.
    pub fn new_with_domain(dim_x: usize, dim_y: usize, domain: ProblemDomain) -> Self {
        Self {
            dim_x: dim_x.max(1),
            dim_y: dim_y.max(1),
            grid: BTreeMap::new(),
            total_samples: 0,
            ucb_exploration_factor: 1.414,
            domain,
        }
    }

    /// ========================================================================
    /// coord_for_behavior : Algorithme de Projection Continue -> Discrète
    /// ========================================================================
    /// Projette le vecteur de comportement continu d'une heuristique dans les indices
    /// discrets (x, y) de la grille de niches en utilisant des fonctions de compression
    /// logarithmiques adaptées aux plages physiques réelles pour éviter la saturation.
    pub fn coord_for_behavior(&self, b: &BehaviorDescriptor) -> (usize, usize) {
        let is_scheduling =
            self.domain == ProblemDomain::MultiResourceScheduling || b.residual_variance > 2.5;

        let (norm_x, norm_y) = if is_scheduling {
            // ================================================================
            // Domaine Multi-Resource Scheduling
            // ================================================================
            // Axe X : Ratio de nœuds denses (< 2 CPU et < 4 RAM).
            // Plage observée [0.0, 0.40] comprimée logarithmiquement pour étaler les faibles et forts ratios
            let nx = ((1.0 + b.dead_space_ratio.max(0.0) / 0.04).ln() / (11.0f64).ln())
                .clamp(0.0, 0.999);

            // Axe Y : Déséquilibre moyen CPU/RAM (mean_imbalance * 100).
            // Plage observée [1.0, 40.0] comprimée logarithmiquement pour discriminer les heuristiques équilibrées
            let ny = ((1.0 + b.residual_variance.max(0.0) / 4.0).ln() / (11.0f64).ln())
                .clamp(0.0, 0.999);

            (nx, ny)
        } else {
            // ================================================================
            // Domaine 1D Bin Packing
            // ================================================================
            // Axe X : Ratio d'espace mort critique [1..15] sur les bacs ouverts.
            // Les heuristiques performantes convergent sous 0.08. Échelle logarithmique compressée
            // sur la plage réelle observée [0.0, 0.15] (évite l'écrasement dans la tranche 0).
            let nx = ((1.0 + b.dead_space_ratio.max(0.0) / 0.015).ln() / (11.0f64).ln())
                .clamp(0.0, 0.999);

            // Axe Y : Entropie de Shannon composite du remplissage [0.0, 2.5].
            // Maximum théorique ln(10) ≈ 2.3025. Découpage calibré sur la plage réelle [0.0, 2.50]
            // (évite la division linéaire brute sur [0.0, 1.0]).
            let ny = if b.residual_variance > 0.0 {
                (b.residual_variance / 2.50).clamp(0.0, 0.999)
            } else {
                ((b.new_bin_ratio - 0.30).max(0.0) / 0.40).clamp(0.0, 0.999)
            };

            (nx, ny)
        };

        let x = ((norm_x * self.dim_x as f64).floor() as usize).min(self.dim_x - 1);
        let y = ((norm_y * self.dim_y as f64).floor() as usize).min(self.dim_y - 1);

        (x, y)
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
                // Utilise l'ordre canonique BenchmarkScore (validité, bins, tldr, efficacité, fuel)
                let is_strictly_better = fitness < existing.fitness;

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
    /// Choisit le parent d'une mutation selon la regle demandee.
    pub fn select_parent(
        &mut self,
        mode: ParentSelection,
        rng: &mut ChaCha8Rng,
    ) -> Option<SkillCapsule> {
        match mode {
            ParentSelection::Rank => self.select_parent_rank(rng),
            ParentSelection::Ucb => self.select_parent_ucb(rng),
        }
    }

    /// Selection par rang. Les elites sont classes du meilleur au moins bon ; le rang r est
    /// tire avec un poids `RANK_SELECTION_RATIO^r`. Le meilleur elite recoit ainsi au moins
    /// la moitie des mutations, et un elite tres inferieur n'en recoit presque aucune.
    pub fn select_parent_rank(&mut self, rng: &mut ChaCha8Rng) -> Option<SkillCapsule> {
        if self.grid.is_empty() {
            return None;
        }
        self.total_samples += 1;

        // Tri deterministe : par fitness, puis par cle de niche en cas d'egalite.
        let mut ranked: Vec<(&String, &ArchiveEntry)> = self.grid.iter().collect();
        ranked.sort_by(|a, b| a.1.fitness.cmp(&b.1.fitness).then_with(|| a.0.cmp(b.0)));

        let weights: Vec<f64> = (0..ranked.len())
            .map(|r| RANK_SELECTION_RATIO.powi(r as i32))
            .collect();
        let total: f64 = weights.iter().sum();
        let mut draw = rng.gen_range(0.0..total);
        let mut chosen = ranked.len() - 1;
        for (r, w) in weights.iter().enumerate() {
            if draw < *w {
                chosen = r;
                break;
            }
            draw -= w;
        }

        let key = ranked[chosen].0.clone();
        let entry = self.grid.get_mut(&key)?;
        entry.times_sampled += 1;
        Some(entry.capsule.clone())
    }

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
            let ucb = q + self.ucb_exploration_factor
                * (total_ln / (entry.times_sampled + 1) as f64).sqrt();

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
        let host_coord = self.coord_for_behavior(&host_score.behavior);

        if self.grid.len() < 2 {
            // If only 1 elite exists, return it ONLY if different from host in ID and AST hash
            let candidate = self.grid.values().next().map(|e| e.capsule.clone());
            if let Some(ref c) = candidate {
                if c.id == host.id {
                    return None;
                }
                if let (Some(h1), Some(h2)) =
                    (host_hash, c.ast_fingerprint.as_ref().map(|f| &f.ast_hash))
                {
                    if h1 == h2 {
                        return None;
                    }
                }
            }
            return candidate;
        }

        let mut best_partner = None;
        let mut best_score = f64::NEG_INFINITY;
        let max_niche_dist =
            ((self.dim_x * self.dim_x + self.dim_y * self.dim_y) as f64).sqrt().max(1.0);

        for entry in self.grid.values() {
            if entry.capsule.id == host.id {
                continue;
            }
            if let (Some(h1), Some(h2)) = (
                host_hash,
                entry.structural_fingerprint.as_ref().map(|f| &f.ast_hash),
            ) {
                if h1 == h2 {
                    continue;
                }
            }

            let q_b = entry.fitness.mean_efficiency;

            // Distance euclidienne normalisée entre niches dans la grille MAP-Elites
            let dx = host_coord.0 as f64 - entry.niche_coord.0 as f64;
            let dy = host_coord.1 as f64 - entry.niche_coord.1 as f64;
            let niche_dist = (dx * dx + dy * dy).sqrt() / max_niche_dist;

            // Complémentarité comportementale directe combinant l'écart de niche et les descripteurs
            let dead_space_diff =
                (host_score.behavior.dead_space_ratio - entry.behavior.dead_space_ratio).abs();
            let bin_ratio_diff =
                (host_score.behavior.new_bin_ratio - entry.behavior.new_bin_ratio).abs();
            let c_complement =
                0.60 * niche_dist + 0.40 * (dead_space_diff + bin_ratio_diff).min(1.0);

            // Structural distance: difference in AST statement and branch counts
            let d_ast = match (&host.ast_fingerprint, &entry.structural_fingerprint) {
                (Some(fp_a), Some(fp_b)) => {
                    let diff_stmts =
                        (fp_a.statement_count as f64 - fp_b.statement_count as f64).abs();
                    let diff_branches = (fp_a.branch_count as f64 - fp_b.branch_count as f64).abs();
                    ((diff_stmts + diff_branches) / 10.0).min(1.0)
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
        self.grid.values().map(|e| e.fitness.mean_efficiency).sum()
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
        // Egalite de score : la cle de niche departage, pour un resultat independant de
        // l'ordre d'iteration de la table.
        self.grid
            .iter()
            .min_by(|a, b| a.1.fitness.cmp(&b.1.fitness).then_with(|| a.0.cmp(b.0)))
            .map(|(_, e)| e)
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

    /// Archive de quatre niches dont les elites utilisent 4 100, 4 200, 4 600 et 8 700 bacs.
    fn archive_with_four_elites() -> (MapElitesArchive, Vec<nomad_core::CapsuleId>) {
        let mut archive = MapElitesArchive::new(10, 10);
        let mut ids = Vec::new();
        for (i, bins) in [4_100usize, 4_200, 4_600, 8_700].into_iter().enumerate() {
            let capsule = SkillCapsule::new(
                format!("Cap{}", i),
                "Desc",
                format!("code {}", i),
                vec![],
                1,
                HashMap::new(),
            );
            ids.push(capsule.id.clone());
            let fitness = BenchmarkScore {
                is_valid: true,
                total_bins: bins,
                total_theoretical_min_bins: 4_000,
                total_excess_bins: bins - 4_000,
                mean_efficiency: 4_000.0 / bins as f64,
                total_fuel: 10_000,
                solved_instances: 10,
                total_instances: 10,
                behavior: BehaviorDescriptor {
                    residual_variance: 1.0,
                    new_bin_ratio: 0.30,
                    mean_fullness: 0.90,
                    dead_space_ratio: 0.05,
                },
                tldr_score: 0.9,
                trajectory_metrics: None,
            };
            archive.grid.insert(
                format!("{},0", i),
                ArchiveEntry {
                    capsule,
                    niche_coord: (i, 0),
                    behavior: fitness.behavior.clone(),
                    fitness,
                    structural_fingerprint: None,
                    times_sampled: 0,
                    times_replaced: 0,
                },
            );
        }
        (archive, ids)
    }

    fn parent_shares(mode: ParentSelection, draws: usize) -> Vec<f64> {
        let (mut archive, ids) = archive_with_four_elites();
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let mut counts = vec![0usize; ids.len()];
        for _ in 0..draws {
            let parent = archive.select_parent(mode, &mut rng).unwrap();
            counts[ids.iter().position(|id| *id == parent.id).unwrap()] += 1;
        }
        counts.iter().map(|&c| c as f64 / draws as f64).collect()
    }

    #[test]
    fn rank_selection_concentrates_mutations_on_best_elites() {
        let shares = parent_shares(ParentSelection::Rank, 2_000);
        assert!(shares[0] > 0.45, "best elite share {:?}", shares);
        assert!(shares[0] > shares[1] && shares[1] > shares[2] && shares[2] > shares[3]);
        assert!(shares[3] < 0.12, "worst elite share {:?}", shares);
    }

    #[test]
    fn ucb_selection_wastes_mutations_on_poor_elites_at_small_budget() {
        // Defaut documente de l'ancienne regle au budget d'un run (une centaine de tirages) :
        // le bonus d'exploration domine, le meilleur elite recoit moins de la moitie des
        // mutations et les deux moins bons en recoivent davantage qu'avec la selection par rang.
        let ucb = parent_shares(ParentSelection::Ucb, 100);
        let rank = parent_shares(ParentSelection::Rank, 100);
        assert!(ucb[0] < 0.45 && rank[0] > 0.50, "ucb {:?} rank {:?}", ucb, rank);
        assert!(ucb[2] + ucb[3] > rank[2] + rank[3], "ucb {:?} rank {:?}", ucb, rank);
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
        let cap_higher_tldr =
            SkillCapsule::new("HigherTLDR", "Desc", "code", vec![], 2, HashMap::new());
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
        let cap_parsimonious = SkillCapsule::new(
            "ParsimoniousSeed",
            "Desc",
            "code",
            vec![],
            0,
            HashMap::new(),
        );
        let mut score_parsimonious = score_higher_tldr.clone();
        score_parsimonious.total_fuel = 60_000;
        assert!(archive.insert(cap_parsimonious.clone(), score_parsimonious.clone()));
        assert_eq!(
            archive.best_elite().unwrap().capsule.id,
            cap_parsimonious.id
        );
    }

    #[test]
    fn test_bin_packing_niche_dispersion() {
        let mut archive =
            MapElitesArchive::new_with_domain(10, 10, ProblemDomain::BinPacking1D);

        for i in 0..50 {
            let name = format!("Heuristic-BP-{:03}", i);
            let cap = SkillCapsule::new(name, "desc", "code", vec![], 1, HashMap::new());

            // Variations réalistes observées en Bin Packing :
            // dead_space_ratio entre 0.003 et 0.12
            // Shannon entropy entre 0.25 et 2.25
            let ds = 0.003 + (i as f64 * 0.0022) + ((i % 5) as f64 * 0.004);
            let ent = 0.25 + (((i * 13) % 200) as f64) / 100.0;

            let score = BenchmarkScore {
                is_valid: true,
                total_bins: 500 - (i % 20),
                total_theoretical_min_bins: 480,
                total_excess_bins: 20,
                mean_efficiency: 0.96,
                total_fuel: 10_000,
                solved_instances: 50,
                total_instances: 50,
                behavior: BehaviorDescriptor {
                    residual_variance: ent,
                    new_bin_ratio: 0.45,
                    mean_fullness: 0.90,
                    dead_space_ratio: ds,
                },
                tldr_score: 0.80,
                trajectory_metrics: None,
            };

            let coord = archive.coord_for_behavior(&score.behavior);
            assert!(coord.0 < 10, "Axe X doit être dans [0..9]");
            assert!(coord.1 < 10, "Axe Y doit être dans [0..9]");

            archive.insert(cap, score);
        }

        let occupied = archive.occupied_niches_count();
        let coverage = archive.coverage();
        println!(
            "Bin Packing MAP-Elites dispersion: {}/100 niches (Coverage: {:.1}%)",
            occupied,
            coverage * 100.0
        );

        // Garantie formelle : dispersion sur au moins 25% des niches (résout le verrou 1-2%)
        assert!(
            occupied >= 25,
            "La couverture Bin Packing ({}) doit être >= 25 niches",
            occupied
        );
    }

    #[test]
    fn test_scheduling_niche_dispersion() {
        let mut archive = MapElitesArchive::new_with_domain(
            10,
            10,
            ProblemDomain::MultiResourceScheduling,
        );

        for i in 0..50 {
            let name = format!("Heuristic-Sched-{:03}", i);
            let cap = SkillCapsule::new(name, "desc", "code", vec![], 1, HashMap::new());

            // Variations réalistes observées en Scheduling :
            // dense node ratio entre 0.01 et 0.35
            // mean_imbalance * 100 entre 1.0 et 36.0
            let tight = 0.01 + (i as f64 * 0.0065) + ((i % 4) as f64 * 0.015);
            let imb = 1.0 + (((i * 17) % 350) as f64) / 10.0;

            let score = BenchmarkScore {
                is_valid: true,
                total_bins: 80 - (i % 15),
                total_theoretical_min_bins: 65,
                total_excess_bins: 15,
                mean_efficiency: 0.85,
                total_fuel: 20_000,
                solved_instances: 50,
                total_instances: 50,
                behavior: BehaviorDescriptor {
                    residual_variance: imb,
                    new_bin_ratio: 0.40,
                    mean_fullness: 0.85,
                    dead_space_ratio: tight,
                },
                tldr_score: -imb / 100.0,
                trajectory_metrics: None,
            };

            let coord = archive.coord_for_behavior(&score.behavior);
            assert!(coord.0 < 10, "Axe X doit être dans [0..9]");
            assert!(coord.1 < 10, "Axe Y doit être dans [0..9]");

            archive.insert(cap, score);
        }

        let occupied = archive.occupied_niches_count();
        let coverage = archive.coverage();
        println!(
            "Scheduling MAP-Elites dispersion: {}/100 niches (Coverage: {:.1}%)",
            occupied,
            coverage * 100.0
        );

        // Garantie formelle : dispersion sur au moins 25% des niches
        assert!(
            occupied >= 25,
            "La couverture Scheduling ({}) doit être >= 25 niches",
            occupied
        );
    }

    #[test]
    fn test_complementary_partner_selection_favors_distant_niches() {
        let mut archive =
            MapElitesArchive::new_with_domain(10, 10, ProblemDomain::BinPacking1D);
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        // Host in low dead space, low entropy niche
        let host = SkillCapsule::new("Host", "desc", "host_code", vec![], 1, HashMap::new());
        let host_score = BenchmarkScore {
            is_valid: true,
            total_bins: 500,
            total_theoretical_min_bins: 480,
            total_excess_bins: 20,
            mean_efficiency: 0.96,
            total_fuel: 10_000,
            solved_instances: 50,
            total_instances: 50,
            behavior: BehaviorDescriptor {
                residual_variance: 0.3,
                new_bin_ratio: 0.40,
                mean_fullness: 0.90,
                dead_space_ratio: 0.005,
            },
            tldr_score: 0.80,
            trajectory_metrics: None,
        };

        // Neighbor 1: close niche (0.01, 0.4)
        let cap_close =
            SkillCapsule::new("ClosePartner", "desc", "close_code", vec![], 1, HashMap::new());
        let mut score_close = host_score.clone();
        score_close.behavior.dead_space_ratio = 0.008;
        score_close.behavior.residual_variance = 0.35;
        score_close.mean_efficiency = 0.90;
        archive.insert(cap_close.clone(), score_close);

        // Neighbor 2: distant complementary niche with high quality (0.12, 2.1)
        let cap_distant = SkillCapsule::new(
            "DistantPartner",
            "desc",
            "distant_code",
            vec![],
            1,
            HashMap::new(),
        );
        let mut score_distant = host_score.clone();
        score_distant.behavior.dead_space_ratio = 0.12;
        score_distant.behavior.residual_variance = 2.1;
        score_distant.mean_efficiency = 0.95;
        archive.insert(cap_distant.clone(), score_distant);

        let partner = archive
            .select_complementary_partner(&host, &host_score, &mut rng)
            .expect("Must select partner");

        // The partner chosen should be the distant complementary one
        assert_eq!(partner.id, cap_distant.id);
    }
}
