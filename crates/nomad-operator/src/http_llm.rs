use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::time::Duration;
use chrono::Utc;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use nomad_core::{PolicyKind, SkillCapsule, ValidationStatus};
use nomad_genome::{compute_structural_fingerprint, deterministic_ast_crossover, parse_score_bin};
use crate::meta::ReproducibilityAudit;
use crate::operator::{OperatorError, SkillOperator};

/// Calculates the SHA-256 cryptographic hash of a local model weight file (e.g., GGUF format).
pub fn compute_file_sha256(path: impl AsRef<Path>) -> Result<String, std::io::Error> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hex::encode(hasher.finalize()))
}

/// JSON payload structure for OpenAI-compatible chat completion APIs (llama-server, Ollama, vLLM).
#[derive(Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    temperature: f64,
    top_p: f64,
    seed: i64,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: MessageContent,
}

#[derive(Deserialize)]
struct MessageContent {
    content: String,
}

/// Local LLM operator connecting to llama.cpp (or compatible `/v1/chat/completions` endpoint)
/// with strict seed and hyperparameter tracking.
#[derive(Debug, Clone)]
pub struct HttpLlamaOperator {
    pub endpoint: String,
    pub model_name: String,
    pub model_sha256: String,
    pub engine_version: String,
    pub temperature: f64,
    pub top_p: f64,
    pub timeout: Duration,
}

impl Default for HttpLlamaOperator {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:11434/v1/chat/completions".to_string(),
            model_name: "llama.cpp-local-model".to_string(),
            model_sha256: "unknown_model_hash".to_string(),
            engine_version: "llama.cpp server v1.0".to_string(),
            temperature: 0.7,
            top_p: 0.95,
            timeout: Duration::from_secs(180),
        }
    }
}

impl HttpLlamaOperator {
    pub fn new(
        endpoint: impl Into<String>,
        model_name: impl Into<String>,
        model_sha256: impl Into<String>,
        engine_version: impl Into<String>,
        temperature: f64,
        top_p: f64,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            model_name: model_name.into(),
            model_sha256: model_sha256.into(),
            engine_version: engine_version.into(),
            temperature,
            top_p,
            timeout: Duration::from_secs(180),
        }
    }

    fn call_inference(&self, system_prompt: &str, user_prompt: &str, seed: u64) -> Result<String, OperatorError> {
        let request_body = ChatCompletionRequest {
            model: &self.model_name,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user",
                    content: user_prompt,
                },
            ],
            temperature: self.temperature,
            top_p: self.top_p,
            seed: (seed & 0x7FFF_FFFF) as i64,
        };

        let max_attempts = 3;
        let mut last_err = String::new();

        for attempt in 1..=max_attempts {
            match ureq::post(&self.endpoint)
                .timeout(self.timeout)
                .send_json(&request_body)
            {
                Ok(resp) => {
                    let response: ChatCompletionResponse = resp
                        .into_json()
                        .map_err(|e| OperatorError::InferenceError(format!("Failed to parse JSON response: {}", e)))?;

                    let raw_text = response
                        .choices
                        .first()
                        .map(|c| c.message.content.clone())
                        .ok_or_else(|| OperatorError::InferenceError("Empty choices returned by LLM".to_string()))?;

                    return Ok(raw_text);
                }
                Err(ureq::Error::Status(code, resp)) => {
                    let err_text = resp.into_string().unwrap_or_default();
                    last_err = format!("Ollama HTTP {}: {}", code, err_text);
                }
                Err(e) => {
                    last_err = format!("HTTP request failed: {}", e);
                }
            }

            if attempt < max_attempts {
                tracing::warn!(
                    "Ollama request attempt {}/{} failed ({}). Retrying in 4s...",
                    attempt,
                    max_attempts,
                    last_err
                );
                std::thread::sleep(Duration::from_secs(4));
            }
        }

        Err(OperatorError::InferenceError(format!(
            "All {} attempts failed. Last error: {}",
            max_attempts, last_err
        )))
    }

    fn extract_code_block(raw: &str) -> String {
        let trimmed = raw.trim();
        if let Some(start) = trimmed.find("```") {
            let after_start = &trimmed[start + 3..];
            let code_start = if let Some(newline_pos) = after_start.find('\n') {
                newline_pos + 1
            } else {
                0
            };
            let code_body = &after_start[code_start..];
            if let Some(end) = code_body.find("```") {
                return code_body[..end].trim().to_string();
            }
        }
        trimmed.to_string()
    }

    /// Converts raw LLM output or classical primitives into validated Rust AST `score_bin` function.
    fn normalize_to_ast_score_bin(code: &str, seed: u64, fallback_variant: usize) -> (String, syn::ItemFn) {
        let trimmed = code.trim();
        // 1. Try parsing directly if contains score_bin
        if trimmed.contains("score_bin") {
            if let Ok(item_fn) = parse_score_bin(trimmed) {
                return (trimmed.to_string(), item_fn);
            }
        }

        // 2. Map classical heuristic names
        if trimmed.contains("FIRST_FIT") {
            let code = "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {\n    let mut score = 0.0;\n    score\n}".to_string();
            let item_fn = parse_score_bin(&code).expect("First fit must parse");
            return (code, item_fn);
        }
        if trimmed.contains("BEST_FIT") {
            let code = "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {\n    let mut score = bin_residual as f64;\n    score\n}".to_string();
            let item_fn = parse_score_bin(&code).expect("Best fit must parse");
            return (code, item_fn);
        }

        // 3. Map legacy parameter format: SCORE: alpha=... beta=... dead_space_penalty=...
        if trimmed.contains("alpha") || trimmed.contains("SCORE:") || trimmed.contains("dead_space_penalty") {
            let mut alpha = 1.0f64;
            let mut penalty = 95.0f64;
            let mut bonus = 50.0f64;

            for part in trimmed.split_whitespace() {
                if let Some(val) = part.strip_prefix("alpha=") {
                    if let Ok(v) = val.parse::<f64>() { alpha = v; }
                } else if let Some(val) = part.strip_prefix("dead_space_penalty=") {
                    if let Ok(v) = val.parse::<f64>() { penalty = v; }
                } else if let Some(val) = part.strip_prefix("exact_fit_bonus=") {
                    if let Ok(v) = val.parse::<f64>() { bonus = v; }
                }
            }

            let code = format!(
                "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {{\n    let mut score = bin_residual as f64 * {:.2};\n    if bin_residual < 10 {{\n        score = score + {:.1};\n    }}\n    if bin_residual == 0 {{\n        score = score - {:.1};\n    }}\n    score\n}}",
                alpha, penalty, bonus
            );
            if let Ok(item_fn) = parse_score_bin(&code) {
                return (code, item_fn);
            }
        }

        // 4. Deterministic AST fallback template
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ (fallback_variant as u64));
        let penalty_val: f64 = rng.gen_range(60.0..110.0);
        let bonus_val: f64 = rng.gen_range(30.0..70.0);
        let alpha_val: f64 = rng.gen_range(1.0..1.5);

        let code = match fallback_variant % 4 {
            0 => format!(
                "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {{\n    let mut score = bin_residual as f64 * {:.2};\n    if bin_residual < 10 {{\n        score = score + {:.1};\n    }}\n    score\n}}",
                alpha_val, penalty_val
            ),
            1 => format!(
                "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {{\n    let mut score = bin_residual as f64;\n    if bin_residual == 0 {{\n        score = score - {:.1};\n    }}\n    score\n}}",
                bonus_val
            ),
            2 => "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {\n    let mut score = bin_residual as f64;\n    if (bin_residual as f64) < mean_residual {\n        score = score - 15.0;\n    }\n    score\n}".to_string(),
            _ => "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {\n    let mut score = bin_residual as f64;\n    score\n}".to_string(),
        };

        let item_fn = parse_score_bin(&code).expect("Template must be valid Rust AST");
        (code, item_fn)
    }
}

impl SkillOperator for HttpLlamaOperator {
    fn generate_seed(
        &self,
        index: usize,
        seed: u64,
    ) -> Result<(SkillCapsule, ReproducibilityAudit), OperatorError> {
        let system_prompt = "You are an expert algorithmic scientist specializing in online 1D bin packing heuristics in Rust.\n\
Bins have capacity C=100. Items arrive sequentially (sizes between 10 and 80). Your goal is to minimize total bins used.\n\
Implement the heuristic as a valid Rust function with the following exact signature:\n\
```rust\n\
pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {\n\
    let mut score = bin_residual as f64;\n\
    score\n\
}\n\
```\n\
CRITICAL REQUIREMENTS FOR SOTA PERFORMANCE:\n\
1. The candidate bin with the MINIMUM score is selected for placing item_size.\n\
2. To outperform standard Best-Fit, an advanced heuristic MUST be dynamic and multi-variable:\n\
   - Dynamic horizon awareness: Combine `bin_residual` with progress `step_index as f64 / total_items as f64`.\n\
   - In early stages (low progress): prioritize keeping large flexible spaces for upcoming unknown items.\n\
   - In late stages (high progress): prioritize tightly closing existing bins to avoid opening new empty bins near the end.\n\
   - Global state: You can use `open_bins_count`, `mean_residual`, or compare `bin_residual` to `item_size`.\n\
Return ONLY the executable Rust function inside a ```rust ... ``` code block without markdown prose.";

        let user_prompt = format!("Synthesize heuristic seed #{} with seed parameter {}.", index, seed);

        let raw_response = self.call_inference(system_prompt, &user_prompt, seed)?;
        let raw_code = Self::extract_code_block(&raw_response);
        let (ast_code, item_fn) = Self::normalize_to_ast_score_bin(&raw_code, seed, index);
        let fp = Some(compute_structural_fingerprint(&item_fn));

        let name = format!("LLM-Seed-{:03}", index);
        let desc = format!("AST Seed synthesized by {}", self.model_name);

        let audit = ReproducibilityAudit {
            model_name: self.model_name.clone(),
            model_sha256: self.model_sha256.clone(),
            engine_version: self.engine_version.clone(),
            seed,
            temperature: self.temperature,
            top_p: self.top_p,
            full_prompt: format!("{}\n{}", system_prompt, user_prompt),
            raw_response: raw_response.clone(),
            timestamp: Utc::now(),
        };

        let mut metadata = HashMap::new();
        metadata.insert("operator".to_string(), "HttpLlamaOperator".to_string());
        metadata.insert("seed".to_string(), seed.to_string());

        let capsule = SkillCapsule::new_v2(
            name,
            desc,
            ast_code,
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
        let system_prompt = "You are an evolutionary code optimizer for Online 1D Bin Packing heuristics in Rust.\n\
Apply an algorithmic mutation to improve compaction and reduce total bins used.\n\
Combine multiple input variables: `bin_residual`, `item_size`, `step_index`/`total_items` (time horizon), or `mean_residual`.\n\
Introduce non-linear conditionals, time-decaying penalties, or horizon-aware placement logic.\n\
The candidate bin with the MINIMUM score is selected.\n\
Return ONLY the mutated Rust function inside a ```rust ... ``` code block without markdown prose.";

        let user_prompt = format!(
            "PARENT HEURISTIC (ID: {}):\n```rust\n{}\n```\nINTENT: {}\nMutate this heuristic for Generation {}.",
            parent.id, parent.logic_code, parent.semantic_interface, generation
        );

        let raw_response = self.call_inference(system_prompt, &user_prompt, seed)?;
        let raw_code = Self::extract_code_block(&raw_response);
        let (ast_code, item_fn) = Self::normalize_to_ast_score_bin(&raw_code, seed, generation);
        let fp = Some(compute_structural_fingerprint(&item_fn));

        let name = format!("{}-mut-g{}", parent.name, generation);
        let desc = format!("Mutated from parent {} at gen {}", parent.id, generation);

        let audit = ReproducibilityAudit {
            model_name: self.model_name.clone(),
            model_sha256: self.model_sha256.clone(),
            engine_version: self.engine_version.clone(),
            seed,
            temperature: self.temperature,
            top_p: self.top_p,
            full_prompt: format!("{}\n{}", system_prompt, user_prompt),
            raw_response: raw_response.clone(),
            timestamp: Utc::now(),
        };

        let mut metadata = HashMap::new();
        metadata.insert("operator".to_string(), "HttpLlamaOperator".to_string());
        metadata.insert("parent_hash".to_string(), parent.content_hash.clone());

        let capsule = SkillCapsule::new_v2(
            name,
            desc,
            ast_code,
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
        let name = format!("SwarmCross-g{}-{}-{}", generation, &parent_a.id.0[..4], &parent_b.id.0[..4]);
        let desc = format!("Horizontal Swarm Crossover of [{}] and [{}]", parent_a.name, parent_b.name);

        let mut metadata = HashMap::new();
        metadata.insert("operator".to_string(), "HttpLlamaOperator".to_string());
        metadata.insert("parent_a".to_string(), parent_a.id.to_string());
        metadata.insert("parent_b".to_string(), parent_b.id.to_string());

        // First attempt deterministic AST crossover
        if let Ok((mut child, manifest)) = deterministic_ast_crossover(parent_a, parent_b, seed, generation) {
            child.name = name;
            child.semantic_interface = desc;
            child.metadata = metadata;
            child.graft_manifest = Some(manifest);

            let audit = ReproducibilityAudit {
                model_name: self.model_name.clone(),
                model_sha256: self.model_sha256.clone(),
                engine_version: self.engine_version.clone(),
                seed,
                temperature: 0.0,
                top_p: 1.0,
                full_prompt: format!("Deterministic AST crossover between {} and {}", parent_a.id, parent_b.id),
                raw_response: child.logic_code.clone(),
                timestamp: Utc::now(),
            };

            return Ok((child, audit));
        }

        // Fallback: LLM synthesis
        let system_prompt = "You are an evolutionary synthesizer performing SWARM CROSSOVER between two distinct algorithmic lineages for Online 1D Bin Packing (C=100).\n\
Recombine their complementary heuristic mechanisms into a single, superior hybrid Rust function score_bin.\n\
Return ONLY the hybrid Rust function in a ```rust ... ``` code block.";

        let user_prompt = format!(
            "PARENT A (ID: {}):\n```rust\n{}\n```\n\nPARENT B (ID: {}):\n```rust\n{}\n```\n\nPerform horizontal crossover for Generation {}.",
            parent_a.id, parent_a.logic_code,
            parent_b.id, parent_b.logic_code,
            generation
        );

        let raw_response = self.call_inference(system_prompt, &user_prompt, seed)?;
        let raw_code = Self::extract_code_block(&raw_response);
        let (ast_code, item_fn) = Self::normalize_to_ast_score_bin(&raw_code, seed, generation);
        let fp = Some(compute_structural_fingerprint(&item_fn));

        let audit = ReproducibilityAudit {
            model_name: self.model_name.clone(),
            model_sha256: self.model_sha256.clone(),
            engine_version: self.engine_version.clone(),
            seed,
            temperature: self.temperature,
            top_p: self.top_p,
            full_prompt: format!("{}\n{}", system_prompt, user_prompt),
            raw_response: raw_response.clone(),
            timestamp: Utc::now(),
        };

        let capsule = SkillCapsule::new_v2(
            name,
            desc,
            ast_code,
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
