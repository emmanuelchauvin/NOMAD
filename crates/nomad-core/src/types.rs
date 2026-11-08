use serde::{Deserialize, Serialize};
use std::fmt;

/// ============================================================================
/// CapsuleId : Identifiant Unique de Capsule
/// ============================================================================
/// Identifiant cryptographique unique d'une capsule de compétence (`SkillCapsule`),
/// calculé de façon déterministe à partir du hash SHA-256 de son contenu ou d'un UUID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct CapsuleId(pub String);

impl CapsuleId {
    /// Crée un nouvel identifiant à partir d'une chaîne ou d'un hash hexadécimal.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Renvoie une référence sous forme de tranche de chaîne (&str).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CapsuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// ============================================================================
/// PackingDecision : Décision de Placement
/// ============================================================================
/// Décision discrète produite par une heuristique pour insérer un objet séquentiel
/// dans le problème de 1D Bin Packing en ligne (Online 1D Bin Packing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackingDecision {
    /// Placer l'objet dans un bac déjà ouvert à l'indice spécifié (0-indexé).
    PlaceInExisting(usize),
    /// Ouvrir un nouveau bac vide et y placer l'objet.
    OpenNewBin,
}

/// ============================================================================
/// SchedulingDecision : Décision d'Allocation de Ressource Cloud
/// ============================================================================
/// Décision discrète produite par une heuristique pour affecter une tâche (Job)
/// à un nœud de calcul (Node) ou allouer une nouvelle machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchedulingDecision {
    /// Placer la tâche sur le nœud existant à l'indice spécifié (0-indexé).
    PlaceOnNode(usize),
    /// Allouer une nouvelle machine (Node) pour y démarrer la tâche.
    OpenNewNode,
}

/// ============================================================================
/// ProblemDomain : Domaine Combinatoire Traité par NOMAD
/// ============================================================================
/// Spécifie la nature du problème d'optimisation abordé par les organismes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProblemDomain {
    /// 1D Online Bin Packing canonique (minimisation de bacs).
    BinPacking1D,
    /// Multi-Resource Cloud Job Scheduling (CPU / RAM multi-dimensionnel).
    MultiResourceScheduling,
}

impl Default for ProblemDomain {
    fn default() -> Self {
        Self::BinPacking1D
    }
}

impl ProblemDomain {
    pub fn name(&self) -> &'static str {
        match self {
            Self::BinPacking1D => "1D Online Bin Packing",
            Self::MultiResourceScheduling => "Multi-Resource Cloud Job Scheduling (CPU/RAM)",
        }
    }
}

/// ============================================================================
/// ExperimentalGroup : Groupes d'Expérimentation NOMAD
/// ============================================================================
/// Désignation des groupes de l'étude scientifique pré-enregistrée (Section 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExperimentalGroup {
    /// Groupe A : 1 seul agent avec auto-correction séquentielle (baseline individuelle).
    GroupA,
    /// Groupe B : N agents indépendants en parallèle sans échange inter-lignées.
    GroupB,
    /// Groupe D : Sélection évolutionnaire avec mutation LLM seule (sans crossover).
    GroupD,
    /// Groupe C : Essaim NOMAD complet combinant Quality-Diversity, AgentPSO et Crossover AST.
    GroupC,
}

impl ExperimentalGroup {
    /// Renvoie le nom formel du groupe pour les affichages et rapports.
    pub fn name(&self) -> &'static str {
        match self {
            Self::GroupA => "Group A (Sequential Baseline)",
            Self::GroupB => "Group B (Parallel Independent)",
            Self::GroupD => "Group D (Mutation-Only Selection)",
            Self::GroupC => "Group C (NOMAD Swarm Crossover)",
        }
    }

    /// Indique si la stratégie de reproduction du groupe autorise le croisement (crossover).
    pub fn allows_crossover(&self) -> bool {
        matches!(self, Self::GroupC)
    }
}

impl fmt::Display for ExperimentalGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// ============================================================================
/// PolicyKind : Nature de l'Interface de Politique
/// ============================================================================
/// Type de représentation de politique régissant le code d'une compétence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PolicyKind {
    /// ScorePolicy V2 canonique : fonction `score_bin(...) -> f64` manipulable par AST.
    ScorePolicy,
    /// StrategyPolicy : politique avec état interne pour décisions complexes (extensions futures).
    StrategyPolicy,
    /// LegacyFormula V1 : formule textuelle paramétrique rétrocompatible (`SCORE: alpha=...`).
    LegacyFormula,
}

impl Default for PolicyKind {
    fn default() -> Self {
        Self::ScorePolicy
    }
}

/// ============================================================================
/// ScoreBinInputs : Paramètres d'Entrée de `score_bin`
/// ============================================================================
/// Ensemble des paramètres passés à la fonction canonique de scoring pour chaque bac ouvert.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScoreBinInputs {
    /// Taille de l'objet à insérer (ex: entre 10 et 80).
    pub item_size: u32,
    /// Espace résiduel disponible dans le bac candidat (capacité - niveau actuel).
    pub bin_residual: u32,
    /// Capacité maximale d'un bac (généralement C = 100).
    pub bin_capacity: u32,
    /// Indice séquentiel de l'objet en cours d'insertion (0-indexé).
    pub step_index: usize,
    /// Nombre total d'objets dans la séquence complète.
    pub total_items: usize,
    /// Nombre actuel de bacs déjà ouverts.
    pub open_bins_count: usize,
    /// Espace résiduel moyen parmi l'ensemble des bacs ouverts.
    pub mean_residual: f64,
}

/// ============================================================================
/// SchedulingInputs : Paramètres d'Entrée de `score_node`
/// ============================================================================
/// Ensemble des paramètres passés à la fonction canonique de scoring pour chaque nœud candidat.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SchedulingInputs {
    /// CPU requis par la tâche courante (cores).
    pub job_cpu: u32,
    /// RAM requise par la tâche courante (GB).
    pub job_ram: u32,
    /// CPU résiduel disponible sur le nœud candidat.
    pub node_residual_cpu: u32,
    /// RAM résiduelle disponible sur le nœud candidat.
    pub node_residual_ram: u32,
    /// Capacité totale CPU du nœud.
    pub node_capacity_cpu: u32,
    /// Capacité totale RAM du nœud.
    pub node_capacity_ram: u32,
    /// Indice séquentiel de la tâche dans la séquence (0-indexé).
    pub step_index: usize,
    /// Nombre total de tâches dans l'instance.
    pub total_jobs: usize,
    /// Nombre actuel de nœuds actifs alloués.
    pub active_nodes_count: usize,
    /// Moyenne d'espace CPU résiduel parmi les nœuds actifs.
    pub mean_cpu_residual: f64,
    /// Moyenne d'espace RAM résiduel parmi les nœuds actifs.
    pub mean_ram_residual: f64,
}

/// ============================================================================
/// ValidationStatus : Statut de Survie Anti-Dead-Code
/// ============================================================================
/// Statut du protocole de validation à 3 niveaux garantissant qu'un fragment greffé
/// n'est ni du code mort, ni une altération inopérante (Section 4.8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationStatus {
    /// Non encore validé.
    Unvalidated,
    /// Niveau 1 validé : présence et intégrité structurelle du fragment dans l'AST.
    PassedLevel1Structural,
    /// Niveau 2 validé : activation effective et calcul dynamique lors de l'évaluation.
    PassedLevel2Computational,
    /// Niveau 3 validé : impact comportemental causal prouvé par ablation différentielle C_null.
    PassedLevel3Behavioral,
    /// Échec de validation avec description explicite de la cause du rejet.
    Failed(String),
}

impl Default for ValidationStatus {
    fn default() -> Self {
        Self::Unvalidated
    }
}

/// ============================================================================
/// StructuralFingerprint : Empreinte Topologique de l'AST
/// ============================================================================
/// Empreinte synthétique caractérisant la complexité syntaxique et topologique
/// du code source d'une heuristique, calculée de manière déterministe via `syn`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StructuralFingerprint {
    /// Profondeur maximale de l'arbre syntaxique (AST).
    pub max_depth: usize,
    /// Nombre de structures conditionnelles et de branchements (`if`, `match`).
    pub branch_count: usize,
    /// Nombre total d'opérations unaires et binaires (+, -, *, /, etc.).
    pub op_count: usize,
    /// Nombre total d'instructions (statements) dans le corps de la fonction.
    pub statement_count: usize,
    /// Hash cryptographique SHA-256 canonique des tokens formatés.
    pub ast_hash: String,
}

/// ============================================================================
/// GraftManifest : Manifeste Génétique de Traçabilité Externe
/// ============================================================================
/// Enregistrement formel de provenance garantissant la traçabilité complète d'une
/// greffe génétique entre deux capsules (Section 4.6), indépendant du code lui-même.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraftManifest {
    /// Identifiant unique de la greffe.
    pub graft_id: String,
    /// Identifiant de la capsule parent donneuse du fragment.
    pub donor_capsule: CapsuleId,
    /// Identifiant de la capsule parent hôte réceptrice.
    pub host_capsule: CapsuleId,
    /// Hash AST canonique du donneur au moment du croisement.
    pub donor_ast_hash: String,
    /// Hash syntaxique du fragment logique greffé.
    pub grafted_fragment_hash: String,
    /// Locus d'insertion dans l'AST de l'hôte (ex: indice d'instruction dans le bloc).
    pub host_locus: String,
    /// Hash des dépendances et variables requises par le fragment.
    pub dependency_hash: String,
    /// Version de l'éventuelle réparation par LLM si intervenue.
    pub repair_version: Option<String>,
    /// Hash du différentiel (diff) appliqué lors d'une réparation éventuelle.
    pub repair_diff_hash: Option<String>,
}

/// ============================================================================
/// BehaviorDescriptor : Descripteurs Comportementaux MAP-Elites
/// ============================================================================
/// Vecteur multidimensionnel caractérisant le comportement d'une heuristique
/// sur les instances d'entraînement, utilisé pour projeter l'individu dans la grille
/// de niches Quality-Diversity (Section 3.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BehaviorDescriptor {
    /// Variance ou dispersion de l'espace résiduel parmi les bacs ouverts.
    pub residual_variance: f64,
    /// Ratio d'ouvertures de nouveaux bacs par rapport au nombre total d'objets.
    pub new_bin_ratio: f64,
    /// Taux moyen de remplissage des bacs (somme objets / capacité).
    pub mean_fullness: f64,
    /// Fréquence des espaces résiduels non remplissables (< 10) créateurs de perte sèche.
    pub dead_space_ratio: f64,
}

/// ============================================================================
/// MomentumAction : Action de Momentum pour AgentPSO
/// ============================================================================
/// Décision d'ajustement dynamique de trajectoire proposée par la boucle de rétroaction
/// d'AgentPSO après évaluation d'un croisement ou d'une mutation (Section 5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MomentumAction {
    /// Poursuivre dans la direction sémantique actuelle (gain observé).
    ContinueDirection,
    /// Pivoter vers une nouvelle direction (stagnation ou régression constatée).
    Pivot,
    /// Combiner plusieurs mécanismes découverts dans des niches complémentaires.
    Combine,
}

impl Default for MomentumAction {
    fn default() -> Self {
        Self::ContinueDirection
    }
}

/// ============================================================================
/// SemanticVelocity : Vélocité Sémantique d'Évolution
/// ============================================================================
/// Vecteur structuré guidant l'orientation des mutations et des croisements
/// en identifiant précisément le défaut à corriger et le fragment donneur cible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticVelocity {
    /// Diagnostic précis de la faiblesse de l'organisme hôte (ex: surplus d'espaces < 10).
    pub weakness_diagnosed: String,
    /// Description du mécanisme complémentaire recherché chez le partenaire.
    pub partner_mechanism: String,
    /// Sélecteur de fragment AST chez le donneur.
    pub source_ast_selector: String,
    /// Locus d'insertion désigné dans l'AST cible.
    pub target_insertion_locus: String,
    /// Modification comportementale attendue suite à l'opération.
    pub expected_behavioral_shift: String,
    /// Risques de régression fonctionnelle identifiés à surveiller.
    pub regression_risks: Vec<String>,
    /// Action de momentum associée (continuer, pivoter, combiner).
    pub momentum_action: MomentumAction,
}

/// ============================================================================
/// LineageEvolutionState : État d'Évolution Persistant d'une Lignée
/// ============================================================================
/// Mémoire d'une lignée d'organismes conservant son meilleur représentant historique (P_best),
/// le meilleur partenaire rencontré et l'historique de ses vélocités sémantiques.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineageEvolutionState {
    /// Meilleure capsule historique découverte par cette lignée (Personal Best).
    pub p_best: CapsuleId,
    /// Meilleur partenaire issu d'une autre niche ayant produit un croisement réussi.
    pub g_best_partner: Option<CapsuleId>,
    /// Dernière vélocité sémantique appliquée à la lignée.
    pub v_previous: Option<SemanticVelocity>,
    /// Journal textuel des étapes majeures d'évolution de la lignée.
    pub evolution_history: Vec<String>,
}


