use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::types::CapsuleId;

/// The immutable cognitive unit of the NOMAD ecosystem.
///
/// Crucially, a `SkillCapsule` contains NO evaluation metrics or fitness scores.
/// This guarantees that agents cannot overfit or exploit objective functions directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillCapsule {
    /// Unique cryptographically derived identifier.
    pub id: CapsuleId,

    /// Human/Agent-readable name of the heuristic or skill.
    pub name: String,

    /// 1. Semantic Interface: Human/LLM-readable specification of intent, problem context, and contracts.
    pub semantic_interface: String,

    /// 2. Logical Engine: Deterministic executable code (e.g., QuickJS script or AST rule).
    pub logic_code: String,

    /// 3. Phylogenetic Lineage: Parent capsule IDs (empty for root, 1 for mutation, >=2 for crossover).
    pub parent_ids: Vec<CapsuleId>,

    /// Generation index at which this capsule was synthesized.
    pub generation: usize,

    /// Cryptographic SHA-256 fingerprint of the content (interface + logic + parents).
    pub content_hash: String,

    /// Creation timestamp (UTC).
    pub created_at: DateTime<Utc>,

    /// Extensible metadata tags (e.g. author agent model, lineage lineage tag, prompt strategy).
    pub metadata: HashMap<String, String>,
}

impl SkillCapsule {
    /// Constructs a new SkillCapsule and automatically computes its cryptographic SHA-256 hash.
    pub fn new(
        name: impl Into<String>,
        semantic_interface: impl Into<String>,
        logic_code: impl Into<String>,
        parent_ids: Vec<CapsuleId>,
        generation: usize,
        metadata: HashMap<String, String>,
    ) -> Self {
        let name = name.into();
        let semantic_interface = semantic_interface.into();
        let logic_code = logic_code.into();

        let mut hasher = Sha256::new();
        hasher.update(name.as_bytes());
        hasher.update(semantic_interface.as_bytes());
        hasher.update(logic_code.as_bytes());
        for p in &parent_ids {
            hasher.update(p.0.as_bytes());
        }
        hasher.update(generation.to_le_bytes());

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
        }
    }

    /// Returns true if this capsule is the result of a cross-lineage recombination (>= 2 parents).
    pub fn is_crossover(&self) -> bool {
        self.parent_ids.len() >= 2
    }

    /// Returns true if this capsule is a direct single-parent mutation.
    pub fn is_mutation(&self) -> bool {
        self.parent_ids.len() == 1
    }

    /// Returns true if this capsule is an ancestral seed (no parents).
    pub fn is_root(&self) -> bool {
        self.parent_ids.is_empty()
    }
}
