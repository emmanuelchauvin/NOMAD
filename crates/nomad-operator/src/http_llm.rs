use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::time::Duration;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use nomad_core::SkillCapsule;
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
}

impl SkillOperator for HttpLlamaOperator {
    fn generate_seed(
        &self,
        index: usize,
        seed: u64,
    ) -> Result<(SkillCapsule, ReproducibilityAudit), OperatorError> {
        let system_prompt = "You are an expert algorithmic scientist specializing in online 1D bin packing heuristics.\n\
Bins have capacity C=100. Items arrive sequentially (sizes between 10 and 80). Your goal is to minimize total bins used.\n\
Available primitives & parameterized scoring:\n\
- FIRST_FIT, BEST_FIT, WORST_FIT, HARMONIC\n\
- Advanced scoring formula:\n\
  SCORE: alpha=<float> beta=<float> dead_space_penalty=<float> exact_fit_bonus=<float> tight_fit=<bool>\n\
  * alpha: weight on minimizing remaining space (e.g. 1.0 to 2.5)\n\
  * beta: weight on fullness ratio (e.g. 0.0 to 1.0)\n\
  * dead_space_penalty: penalty when remaining space < 10 (since items are >=10, space < 10 can never be filled and wastes bins)\n\
  * exact_fit_bonus: bonus when item fills the bin to exactly 100%\n\
  * tight_fit: true or false\n\
Return ONLY the strategy name or formula in a code block.";

        let user_prompt = format!("Synthesize heuristic seed #{} with seed parameter {}.", index, seed);

        let raw_response = self.call_inference(system_prompt, &user_prompt, seed)?;
        let code = Self::extract_code_block(&raw_response);
        let name = format!("LLM-Seed-{:03}", index);
        let desc = format!("Initial seed synthesized by {}", self.model_name);

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

        let capsule = SkillCapsule::new(name, desc, code, vec![], 0, metadata);

        Ok((capsule, audit))
    }

    fn mutate(
        &self,
        parent: &SkillCapsule,
        generation: usize,
        seed: u64,
    ) -> Result<(SkillCapsule, ReproducibilityAudit), OperatorError> {
        let system_prompt = "You are an evolutionary code optimizer for Online 1D Bin Packing heuristics (Capacity C=100, item sizes 10 to 80).\n\
Apply a targeted mutation (e.g. adjust alpha/beta, increase dead_space_penalty to prevent unfillable gaps < 10, or tune exact_fit_bonus to close bins) to reduce total bins used.\n\
Formula format: SCORE: alpha=<float> beta=<float> dead_space_penalty=<float> exact_fit_bonus=<float> tight_fit=<bool>\n\
Return ONLY the mutated heuristic in a code block.";

        let user_prompt = format!(
            "PARENT HEURISTIC (ID: {}):\n{}\nINTENT: {}\nMutate this heuristic for Generation {}.",
            parent.id, parent.logic_code, parent.semantic_interface, generation
        );

        let raw_response = self.call_inference(system_prompt, &user_prompt, seed)?;
        let code = Self::extract_code_block(&raw_response);
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

        let capsule = SkillCapsule::new(name, desc, code, vec![parent.id.clone()], generation, metadata);

        Ok((capsule, audit))
    }

    fn crossover(
        &self,
        parent_a: &SkillCapsule,
        parent_b: &SkillCapsule,
        generation: usize,
        seed: u64,
    ) -> Result<(SkillCapsule, ReproducibilityAudit), OperatorError> {
        let system_prompt = "You are an evolutionary synthesizer performing SWARM CROSSOVER between two distinct algorithmic lineages for Online 1D Bin Packing (C=100).\n\
Analyze the strengths of Parent A and Parent B. Recombine their complementary heuristic mechanisms (e.g. blend alpha/beta, take the strongest dead_space_penalty or exact_fit_bonus) into a single, superior hybrid strategy.\n\
Formula format: SCORE: alpha=<float> beta=<float> dead_space_penalty=<float> exact_fit_bonus=<float> tight_fit=<bool>\n\
Return ONLY the recombined hybrid strategy in a code block.";

        let user_prompt = format!(
            "PARENT A (ID: {}):\nCode: {}\nIntent: {}\n\nPARENT B (ID: {}):\nCode: {}\nIntent: {}\n\nPerform horizontal crossover to produce child for Generation {}.",
            parent_a.id, parent_a.logic_code, parent_a.semantic_interface,
            parent_b.id, parent_b.logic_code, parent_b.semantic_interface,
            generation
        );

        let raw_response = self.call_inference(system_prompt, &user_prompt, seed)?;
        let code = Self::extract_code_block(&raw_response);
        let name = format!("SwarmCross-g{}-{}-{}", generation, &parent_a.id.0[..4], &parent_b.id.0[..4]);
        let desc = format!("Horizontal Swarm Crossover of [{}] and [{}]", parent_a.name, parent_b.name);

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
        metadata.insert("parent_a".to_string(), parent_a.id.to_string());
        metadata.insert("parent_b".to_string(), parent_b.id.to_string());

        let capsule = SkillCapsule::new(
            name,
            desc,
            code,
            vec![parent_a.id.clone(), parent_b.id.clone()],
            generation,
            metadata,
        );

        Ok((capsule, audit))
    }
}
