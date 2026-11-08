use std::collections::HashMap;
use chrono::Utc;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use thiserror::Error;

use nomad_core::{
    PolicyKind, SkillCapsule, ValidationStatus,
};
use nomad_genome::{
    compute_structural_fingerprint, deterministic_ast_crossover, parse_score_bin,
};
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
            engine_version: "nomad-mock-v2.0".to_string(),
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
            (
                "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {\n    let mut score = bin_residual as f64;\n    score\n}",
                "Best Fit linear remaining capacity baseline",
            ),
            (
                "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {\n    let mut score = bin_residual as f64;\n    if bin_residual < 10 {\n        score = score + 95.0;\n    }\n    score\n}",
                "Best Fit with dead space penalty (< 10)",
            ),
            (
                "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {\n    let mut score = bin_residual as f64;\n    if bin_residual == 0 {\n        score = score - 50.0;\n    }\n    score\n}",
                "Best Fit with exact closure bonus",
            ),
            (
                "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {\n    let mut score = bin_residual as f64 * 1.25;\n    score\n}",
                "Scaled tightness scoring",
            ),
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

        let parsed_fn = parse_score_bin(code).ok();
        let fp = parsed_fn.as_ref().map(compute_structural_fingerprint);

        let capsule = SkillCapsule::new_v2(
            name,
            desc,
            code,
            vec![],
            0,
            metadata,
            "ScorePolicyV1",
            PolicyKind::ScorePolicy,
            fp,
            Vec::new(),
            ValidationStatus::PassedLevel1Structural,
            None,
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
        let penalty_val: f64 = rng.gen_range(50.0..120.0);

        let new_code = if parent.logic_code.contains("score_bin") {
            if parent.logic_code.contains("bin_residual < 10") {
                parent.logic_code.replace("95.0", &format!("{:.1}", penalty_val))
            } else {
                // Introduce dead space conditional branch
                format!(
                    "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {{\n    let mut score = bin_residual as f64;\n    if bin_residual < 10 {{\n        score = score + {:.1};\n    }}\n    score\n}}",
                    penalty_val
                )
            }
        } else {
            // Legacy fallback
            format!("SCORE: alpha={:.3} beta=0.2 dead_space_penalty={:.1} tight_fit=true", 1.0 + rng.gen_range(-0.1..0.1), penalty_val)
        };

        let name = format!("{}-mut-g{}", parent.name, generation);
        let desc = format!("Mutated from parent {} with targeted parameter or condition tuning", parent.id);

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

        let parsed_fn = parse_score_bin(&new_code).ok();
        let fp = parsed_fn.as_ref().map(compute_structural_fingerprint);

        let capsule = SkillCapsule::new_v2(
            name,
            desc,
            new_code,
            vec![parent.id.clone()],
            generation,
            metadata,
            "ScorePolicyV1",
            PolicyKind::ScorePolicy,
            fp,
            Vec::new(),
            ValidationStatus::PassedLevel1Structural,
            None,
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
        // Attempt deterministic AST crossover first if both parents have parseable score_bin
        if parent_a.logic_code.contains("score_bin") && parent_b.logic_code.contains("score_bin") {
            if let Ok((child, manifest)) = deterministic_ast_crossover(parent_a, parent_b, seed, generation) {
                let audit = ReproducibilityAudit {
                    model_name: self.model_name.clone(),
                    model_sha256: self.model_sha256.clone(),
                    engine_version: self.engine_version.clone(),
                    seed,
                    temperature: 0.0,
                    top_p: 1.0,
                    full_prompt: format!("Deterministic AST crossover: host={}, donor={}", parent_a.id, parent_b.id),
                    raw_response: child.logic_code.clone(),
                    timestamp: Utc::now(),
                };
                let mut child_with_manifest = child;
                child_with_manifest.graft_manifest = Some(manifest);
                return Ok((child_with_manifest, audit));
            }
        }

        // Fallback for hybrid or legacy
        let recombined_code = format!(
            "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {{\n    let mut score = bin_residual as f64;\n    if bin_residual < 10 {{\n        score = score + 95.0;\n    }}\n    score\n}}"
        );

        let name = format!("SwarmCross-g{}-{}-{}", generation, &parent_a.id.0[..4.min(parent_a.id.0.len())], &parent_b.id.0[..4.min(parent_b.id.0.len())]);
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

        let parsed_fn = parse_score_bin(&recombined_code).ok();
        let fp = parsed_fn.as_ref().map(compute_structural_fingerprint);

        let capsule = SkillCapsule::new_v2(
            name,
            desc,
            recombined_code,
            vec![parent_a.id.clone(), parent_b.id.clone()],
            generation,
            metadata,
            "ScorePolicyV1",
            PolicyKind::ScorePolicy,
            fp,
            Vec::new(),
            ValidationStatus::PassedLevel1Structural,
            None,
        );

        Ok((capsule, audit))
    }
}
