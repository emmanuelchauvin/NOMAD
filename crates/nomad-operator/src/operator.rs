use std::collections::HashMap;
use chrono::Utc;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use thiserror::Error;

use nomad_core::SkillCapsule;
use crate::meta::ReproducibilityAudit;

#[derive(Debug, Error)]
pub enum OperatorError {
    #[error("Inference execution failed: {0}")]
    InferenceError(String),
    #[error("Failed to parse synthesized capsule: {0}")]
    ParseError(String),
}

/// Abstract contract for generating, mutating, and crossing over SkillCapsules.
pub trait SkillOperator: Send + Sync {
    /// Generates an initial seed candidate.
    fn generate_seed(
        &self,
        index: usize,
        seed: u64,
    ) -> Result<(SkillCapsule, ReproducibilityAudit), OperatorError>;

    /// Mutates an existing capsule (Group D & Group C).
    fn mutate(
        &self,
        parent: &SkillCapsule,
        generation: usize,
        seed: u64,
    ) -> Result<(SkillCapsule, ReproducibilityAudit), OperatorError>;

    /// Performs horizontal Swarm Crossover between two parent capsules (Group C only).
    fn crossover(
        &self,
        parent_a: &SkillCapsule,
        parent_b: &SkillCapsule,
        generation: usize,
        seed: u64,
    ) -> Result<(SkillCapsule, ReproducibilityAudit), OperatorError>;
}

/// Fully reproducible deterministic operator for testing, validation, and benchmarking.
#[derive(Debug, Clone)]
pub struct DeterministicMockOperator {
    pub model_name: String,
    pub model_sha256: String,
    pub engine_version: String,
}

impl Default for DeterministicMockOperator {
    fn default() -> Self {
        Self {
            model_name: "Mock-NOMAD-Darwin-7B".to_string(),
            model_sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
            engine_version: "nomad-mock-v1.0".to_string(),
        }
    }
}

impl SkillOperator for DeterministicMockOperator {
    fn generate_seed(
        &self,
        index: usize,
        seed: u64,
    ) -> Result<(SkillCapsule, ReproducibilityAudit), OperatorError> {
        let variants = [
            ("BEST_FIT", "Standard best fit minimizing remaining capacity"),
            ("FIRST_FIT", "Classical greedy first fit"),
            ("HARMONIC", "Harmonic partitioning heuristic separating large and small items"),
            ("WORST_FIT", "Spreading heuristic placing in emptiest bin"),
            ("SCORE: alpha=1.0 beta=0.0 tight_fit=true", "Linear remaining capacity penalty"),
            ("SCORE: alpha=1.2 beta=0.3 tight_fit=true", "Tight fit with waste margin"),
        ];

        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ (index as u64));
        let variant_idx = rng.gen_range(0..variants.len());
        let (code, desc) = variants[variant_idx];
        let name = format!("Seed-Heuristic-{:03}", index);

        let prompt = format!("SYSTEM: Synthesize an online bin packing heuristic.\nSEED INDEX: {}", index);
        let audit = ReproducibilityAudit {
            model_name: self.model_name.clone(),
            model_sha256: self.model_sha256.clone(),
            engine_version: self.engine_version.clone(),
            seed,
            temperature: 0.7,
            top_p: 0.95,
            full_prompt: prompt,
            raw_response: code.to_string(),
            timestamp: Utc::now(),
        };

        let mut metadata = HashMap::new();
        metadata.insert("operator".to_string(), "DeterministicMockOperator".to_string());
        metadata.insert("initial_seed".to_string(), seed.to_string());

        let capsule = SkillCapsule::new(
            name,
            desc,
            code,
            vec![],
            0,
            metadata,
        );

        Ok((capsule, audit))
    }

    fn mutate(
        &self,
        parent: &SkillCapsule,
        generation: usize,
        seed: u64,
    ) -> Result<(SkillCapsule, ReproducibilityAudit), OperatorError> {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let delta_alpha: f64 = rng.gen_range(-0.25..0.25);
        let delta_beta: f64 = rng.gen_range(-0.15..0.15);

        let new_code = if parent.logic_code.contains("SCORE") {
            format!(
                "SCORE: alpha={:.3} beta={:.3} tight_fit=true",
                (1.0 + delta_alpha).clamp(0.1, 5.0),
                (0.2 + delta_beta).clamp(0.0, 2.0)
            )
        } else if parent.logic_code.contains("FIRST_FIT") {
            "SCORE: alpha=0.8 beta=0.4 tight_fit=true".to_string()
        } else {
            "SCORE: alpha=1.15 beta=0.15 tight_fit=true".to_string()
        };

        let name = format!("{}-mut-g{}", parent.name, generation);
        let desc = format!("Mutated from parent {} with adjusted parameters", parent.id);

        let prompt = format!(
            "SYSTEM: Mutate heuristic code to improve packing efficiency.\nPARENT CODE: {}\nPARENT ID: {}",
            parent.logic_code, parent.id
        );

        let audit = ReproducibilityAudit {
            model_name: self.model_name.clone(),
            model_sha256: self.model_sha256.clone(),
            engine_version: self.engine_version.clone(),
            seed,
            temperature: 0.7,
            top_p: 0.95,
            full_prompt: prompt,
            raw_response: new_code.clone(),
            timestamp: Utc::now(),
        };

        let mut metadata = HashMap::new();
        metadata.insert("mutation_seed".to_string(), seed.to_string());
        metadata.insert("parent_hash".to_string(), parent.content_hash.clone());

        let capsule = SkillCapsule::new(
            name,
            desc,
            new_code,
            vec![parent.id.clone()],
            generation,
            metadata,
        );

        Ok((capsule, audit))
    }

    fn crossover(
        &self,
        parent_a: &SkillCapsule,
        parent_b: &SkillCapsule,
        generation: usize,
        seed: u64,
    ) -> Result<(SkillCapsule, ReproducibilityAudit), OperatorError> {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let blend_ratio: f64 = rng.gen_range(0.3..0.7);

        // Swarm Crossover: recombines semantic intentions and algorithmic heuristics
        let recombined_code = format!(
            "// Recombination of {} and {}\nSCORE: alpha={:.3} beta={:.3} tight_fit=true",
            parent_a.id,
            parent_b.id,
            1.0 + 0.3 * blend_ratio,
            0.15 + 0.2 * (1.0 - blend_ratio)
        );

        let name = format!("SwarmCross-g{}-{}-{}", generation, &parent_a.id.0[..4], &parent_b.id.0[..4]);
        let desc = format!(
            "Horizontal Swarm Crossover combining [{}] and [{}]",
            parent_a.semantic_interface, parent_b.semantic_interface
        );

        let prompt = format!(
            "SYSTEM: Synthesize a superior hybrid heuristic by crossing over these two parents:\nPARENT A: {}\nPARENT B: {}",
            parent_a.logic_code, parent_b.logic_code
        );

        let audit = ReproducibilityAudit {
            model_name: self.model_name.clone(),
            model_sha256: self.model_sha256.clone(),
            engine_version: self.engine_version.clone(),
            seed,
            temperature: 0.7,
            top_p: 0.95,
            full_prompt: prompt,
            raw_response: recombined_code.clone(),
            timestamp: Utc::now(),
        };

        let mut metadata = HashMap::new();
        metadata.insert("crossover_seed".to_string(), seed.to_string());
        metadata.insert("parent_a".to_string(), parent_a.id.to_string());
        metadata.insert("parent_b".to_string(), parent_b.id.to_string());

        let capsule = SkillCapsule::new(
            name,
            desc,
            recombined_code,
            vec![parent_a.id.clone(), parent_b.id.clone()],
            generation,
            metadata,
        );

        Ok((capsule, audit))
    }
}
