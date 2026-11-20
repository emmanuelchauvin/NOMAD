use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

use crate::types::{CapsuleId, GraftManifest, PolicyKind, StructuralFingerprint, ValidationStatus};

fn default_interface_version() -> String {
    "ScorePolicyV1".to_string()
}

/// ============================================================================
/// SkillCapsule : L'Unité Cognitive Fondamentale et Immuable
/// ============================================================================
/// Une `SkillCapsule` encapsule une compétence algorithmique autonome, reproductible
/// et strictement versionnée.
///
/// **Principe non négociable de NOMAD :**
/// La capsule ne contient AUCUN score de fitness ou métrique de benchmark.
/// Cette séparation hermétique garantit que les organismes ne peuvent ni sur-adapter
/// ni exploiter directement les fonctions d'évaluation (pas de fuite d'oracle).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillCapsule {
    /// Identifiant unique dérivé par hachage cryptographique SHA-256 (16 premiers caractères).
    pub id: CapsuleId,

    /// Nom lisible par l'humain et l'agent décrivant l'heuristique ou la stratégie.
    pub name: String,

    /// 1. Interface Sémantique : spécification en langage naturel de l'intention et des contrats.
    pub semantic_interface: String,

    /// 2. Moteur Logique : code source déterministe et exécutable (AST Rust ou formule).
    pub logic_code: String,

    /// 3. Lignée Phylogénétique : identifiants des parents (0 = racine, 1 = mutation, >=2 = croisement).
    pub parent_ids: Vec<CapsuleId>,

    /// Numéro de génération auquel cette capsule a été synthétisée ou recombinée.
    pub generation: usize,

    /// Empreinte cryptographique complète SHA-256 du contenu (nom + interface + code + parents + manifest).
    pub content_hash: String,

    /// Horodatage précis de création (UTC).
    pub created_at: DateTime<Utc>,

    /// Métadonnées extensibles (modèle d'auteur, stratégie de prompt, tag de lignée, etc.).
    pub metadata: HashMap<String, String>,

    /// --- Extensions V2 (`Spec V2 bis.md`) ---

    /// Version de l'interface de politique (ex: "ScorePolicyV1").
    #[serde(default = "default_interface_version")]
    pub interface_version: String,

    /// Nature de la politique (ScorePolicy, StrategyPolicy, LegacyFormula).
    #[serde(default)]
    pub policy_kind: PolicyKind,

    /// Empreinte structurelle AST facultative pour la diversité secondaire et le départage.
    #[serde(default)]
    pub ast_fingerprint: Option<StructuralFingerprint>,

    /// Liste des dépendances et variables requises identifiées lors du découpage syntaxique.
    #[serde(default)]
    pub dependencies: Vec<String>,

    /// Statut du pipeline Anti-Dead-Code à 3 niveaux (Niv 1 Structurel, Niv 2 Computationnel, Niv 3 Comportemental).
    #[serde(default)]
    pub validation: ValidationStatus,

    /// Manifeste de traçabilité externe de la greffe génétique (`GraftManifest`).
    #[serde(default)]
    pub graft_manifest: Option<GraftManifest>,
}

impl SkillCapsule {
    /// ========================================================================
    /// new : Constructeur rétrocompatible V1
    /// ========================================================================
    /// Construit une capsule de compétence avec les valeurs par défaut de la V1.
    pub fn new(
        name: impl Into<String>,
        semantic_interface: impl Into<String>,
        logic_code: impl Into<String>,
        parent_ids: Vec<CapsuleId>,
        generation: usize,
        metadata: HashMap<String, String>,
    ) -> Self {
        Self::new_v2(
            name,
            semantic_interface,
            logic_code,
            parent_ids,
            generation,
            metadata,
            "LegacyV1",
            PolicyKind::LegacyFormula,
            None,
            Vec::new(),
            ValidationStatus::Unvalidated,
            None,
        )
    }

    /// ========================================================================
    /// new_v2 : Constructeur exhaustif NOMAD V2
    /// ========================================================================
    /// Construit une capsule complète avec validation AST, empreinte topologique
    /// et manifeste de greffe externe (`GraftManifest`).
    #[allow(clippy::too_many_arguments)]
    pub fn new_v2(
        name: impl Into<String>,
        semantic_interface: impl Into<String>,
        logic_code: impl Into<String>,
        parent_ids: Vec<CapsuleId>,
        generation: usize,
        metadata: HashMap<String, String>,
        interface_version: impl Into<String>,
        policy_kind: PolicyKind,
        ast_fingerprint: Option<StructuralFingerprint>,
        dependencies: Vec<String>,
        validation: ValidationStatus,
        graft_manifest: Option<GraftManifest>,
    ) -> Self {
        let name = name.into();
        let semantic_interface = semantic_interface.into();
        let logic_code = logic_code.into();
        let interface_version = interface_version.into();

        // Calcul du hash de contenu cryptographique SHA-256 déterministe
        let mut hasher = Sha256::new();
        hasher.update(name.as_bytes());
        hasher.update(semantic_interface.as_bytes());
        hasher.update(logic_code.as_bytes());
        for p in &parent_ids {
            hasher.update(p.0.as_bytes());
        }
        hasher.update(generation.to_le_bytes());
        hasher.update(interface_version.as_bytes());
        if let Some(ref m) = graft_manifest {
            hasher.update(m.graft_id.as_bytes());
        }

        let hash_bytes = hasher.finalize();
        let hash_hex = hex::encode(hash_bytes);
        let id = CapsuleId(hash_hex[..16].to_string());

        Self {
            id,
            name,
            semantic_interface,
            logic_code,
            parent_ids,
            generation,
            content_hash: hash_hex,
            created_at: Utc::now(),
            metadata,
            interface_version,
            policy_kind,
            ast_fingerprint,
            dependencies,
            validation,
            graft_manifest,
        }
    }

    /// Indique si la capsule est issue d'un croisement inter-lignées (crossover, >= 2 parents).
    pub fn is_crossover(&self) -> bool {
        self.parent_ids.len() >= 2
    }

    /// Indique si la capsule est issue d'une mutation simple d'un parent unique (1 parent).
    pub fn is_mutation(&self) -> bool {
        self.parent_ids.len() == 1
    }

    /// Indique si la capsule est une graine initiale ancestrale (0 parent).
    pub fn is_root(&self) -> bool {
        self.parent_ids.is_empty()
    }
}
