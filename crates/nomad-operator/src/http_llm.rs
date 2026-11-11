use chrono::Utc;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::time::Duration;

use crate::meta::ReproducibilityAudit;
use crate::operator::{OperatorError, SkillOperator};
use nomad_core::{
    LineageEvolutionState, MomentumAction, PolicyKind, ProblemDomain, SkillCapsule, ValidationStatus,
};
use nomad_genome::{
    compute_structural_fingerprint, deterministic_ast_crossover, multi_locus_ast_crossover,
    parse_heuristic_fn, parse_score_bin,
};

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
    pub domain: ProblemDomain,
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
            domain: ProblemDomain::BinPacking1D,
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
            domain: ProblemDomain::BinPacking1D,
        }
    }

    pub fn with_domain(mut self, domain: ProblemDomain) -> Self {
        self.domain = domain;
        self
    }

    fn call_inference(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        seed: u64,
    ) -> Result<String, OperatorError> {
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
                    let response: ChatCompletionResponse = resp.into_json().map_err(|e| {
                        OperatorError::InferenceError(format!(
                            "Failed to parse JSON response: {}",
                            e
                        ))
                    })?;

                    let raw_text = response
                        .choices
                        .first()
                        .map(|c| c.message.content.clone())
                        .ok_or_else(|| {
                            OperatorError::InferenceError(
                                "Empty choices returned by LLM".to_string(),
                            )
                        })?;

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
    fn normalize_to_ast_score_bin(
        code: &str,
        seed: u64,
        fallback_variant: usize,
    ) -> (String, syn::ItemFn) {
        let trimmed = code.trim();
        // 1. Try parsing directly if contains score_bin
        if trimmed.contains("score_bin") {
            if let Ok(item_fn) = parse_score_bin(trimmed) {
                return (trimmed.to_string(), item_fn);
            }
        }

        // 2. Map classical heuristic names
        if trimmed.contains("FIRST_FIT") {
            let code = "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64, next_item_size: u32, post_residual: u32, fitting_bins_count: usize) -> f64 {\n    let score = 0.0;\n    score\n}".to_string();
            let item_fn = parse_score_bin(&code).expect("First fit must parse");
            return (code, item_fn);
        }
        if trimmed.contains("BEST_FIT") {
            let code = "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64, next_item_size: u32, post_residual: u32, fitting_bins_count: usize) -> f64 {\n    let score = post_residual as f64;\n    score\n}".to_string();
            let item_fn = parse_score_bin(&code).expect("Best fit must parse");
            return (code, item_fn);
        }

        // 3. Map legacy parameter format: SCORE: alpha=... beta=... dead_space_penalty=...
        if trimmed.contains("alpha")
            || trimmed.contains("SCORE:")
            || trimmed.contains("dead_space_penalty")
        {
            let mut alpha = 1.0f64;
            let mut penalty = 95.0f64;
            let mut bonus = 50.0f64;

            for part in trimmed.split_whitespace() {
                if let Some(val) = part.strip_prefix("alpha=") {
                    if let Ok(v) = val.parse::<f64>() {
                        alpha = v;
                    }
                } else if let Some(val) = part.strip_prefix("dead_space_penalty=") {
                    if let Ok(v) = val.parse::<f64>() {
                        penalty = v;
                    }
                } else if let Some(val) = part.strip_prefix("exact_fit_bonus=") {
                    if let Ok(v) = val.parse::<f64>() {
                        bonus = v;
                    }
                }
            }

            let code = format!(
                "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64, next_item_size: u32, post_residual: u32, fitting_bins_count: usize) -> f64 {{\n    let mut score = post_residual as f64 * {:.2};\n    if post_residual < 10 {{\n        score = score + {:.1};\n    }}\n    if post_residual == 0 {{\n        score = score - {:.1};\n    }}\n    score\n}}",
                alpha, penalty, bonus
            );
            if let Ok(item_fn) = parse_score_bin(&code) {
                return (code, item_fn);
            }
        }

        // 4. Deterministic AST fallback template (Strict Rule 0: Pure naive baselines, no human lookahead or magic bonuses)
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ (fallback_variant as u64));
        let weight: f64 = rng.gen_range(0.8..1.2);

        let code = match fallback_variant % 4 {
            0 => "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64, next_item_size: u32, post_residual: u32, fitting_bins_count: usize) -> f64 {\n    let score = 0.0;\n    score\n}".to_string(),
            1 => "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64, next_item_size: u32, post_residual: u32, fitting_bins_count: usize) -> f64 {\n    let score = post_residual as f64;\n    score\n}".to_string(),
            2 => "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64, next_item_size: u32, post_residual: u32, fitting_bins_count: usize) -> f64 {\n    let score = post_residual as f64 / bin_capacity.max(1) as f64;\n    score\n}".to_string(),
            _ => format!(
                "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64, next_item_size: u32, post_residual: u32, fitting_bins_count: usize) -> f64 {{\n    let score = (post_residual as f64) * {:.2};\n    score\n}}",
                weight
            ),
        };

        let item_fn = parse_score_bin(&code).expect("Template must be valid Rust AST");
        (code, item_fn)
    }

    /// Complement de prompt decrivant l'interface quand le bac vide est candidat
    /// (`NOMAD_BP_ALLOW_OPEN=1` ou `NOMAD_BP_PROFILE=weibull`). Decrit le mecanisme
    /// uniquement, jamais une strategie.
    fn bin_packing_profile_note() -> String {
        let on = |k: &str, v: &str| std::env::var(k).map(|x| x == v).unwrap_or(false);
        let active = !on("NOMAD_BP_ALLOW_OPEN", "0")
            && (on("NOMAD_BP_ALLOW_OPEN", "1") || on("NOMAD_BP_PROFILE", "weibull"));
        if !active {
            return String::new();
        }
        "

INTERFACE NOTE (overrides any size range stated above):
- Item sizes are integers between 1 and 100 drawn from an UNKNOWN distribution; streams are long (see `total_items`).
- NEW-BIN CANDIDATE: besides every open bin that can fit the item, `score_bin` is also called once for a brand-new EMPTY bin, recognizable by `bin_residual == bin_capacity`. If that call returns the LOWEST score, a new bin is opened even though some open bins could fit the item. Your heuristic therefore decides when opening a new bin is better than accepting a poor fit."
            .to_string()
    }

    /// Converts raw LLM output or classical primitives into validated Rust AST `score_node` function.
    fn normalize_to_ast_score_node(
        code: &str,
        seed: u64,
        fallback_variant: usize,
    ) -> (String, syn::ItemFn) {
        let trimmed = code.trim();
        // 1. Try parsing directly if contains score_node
        if trimmed.contains("score_node") {
            if let Ok(item_fn) = parse_heuristic_fn(trimmed) {
                return (trimmed.to_string(), item_fn);
            }
        }

        // 2. Map classical heuristic names
        if trimmed.contains("FIRST_FIT") {
            let code = "pub fn score_node(job_cpu: u32, job_ram: u32, node_residual_cpu: u32, node_residual_ram: u32, node_capacity_cpu: u32, node_capacity_ram: u32, step_index: usize, total_jobs: usize, active_nodes_count: usize, mean_cpu_residual: f64, mean_ram_residual: f64, next_job_cpu: u32, next_job_ram: u32) -> f64 {\n    let mut score = 0.0;\n    score\n}".to_string();
            let item_fn = parse_heuristic_fn(&code).expect("First fit must parse");
            return (code, item_fn);
        }
        if trimmed.contains("BEST_FIT_CPU") {
            let code = "pub fn score_node(job_cpu: u32, job_ram: u32, node_residual_cpu: u32, node_residual_ram: u32, node_capacity_cpu: u32, node_capacity_ram: u32, step_index: usize, total_jobs: usize, active_nodes_count: usize, mean_cpu_residual: f64, mean_ram_residual: f64, next_job_cpu: u32, next_job_ram: u32) -> f64 {\n    let mut score = node_residual_cpu as f64;\n    score\n}".to_string();
            let item_fn = parse_heuristic_fn(&code).expect("Best fit CPU must parse");
            return (code, item_fn);
        }
        if trimmed.contains("BEST_FIT_RAM") {
            let code = "pub fn score_node(job_cpu: u32, job_ram: u32, node_residual_cpu: u32, node_residual_ram: u32, node_capacity_cpu: u32, node_capacity_ram: u32, step_index: usize, total_jobs: usize, active_nodes_count: usize, mean_cpu_residual: f64, mean_ram_residual: f64, next_job_cpu: u32, next_job_ram: u32) -> f64 {\n    let mut score = node_residual_ram as f64;\n    score\n}".to_string();
            let item_fn = parse_heuristic_fn(&code).expect("Best fit RAM must parse");
            return (code, item_fn);
        }
        if trimmed.contains("BALANCED_FIT") {
            let code = "pub fn score_node(job_cpu: u32, job_ram: u32, node_residual_cpu: u32, node_residual_ram: u32, node_capacity_cpu: u32, node_capacity_ram: u32, step_index: usize, total_jobs: usize, active_nodes_count: usize, mean_cpu_residual: f64, mean_ram_residual: f64, next_job_cpu: u32, next_job_ram: u32) -> f64 {\n    let cpu_norm = (node_residual_cpu - job_cpu) as f64 / node_capacity_cpu as f64;\n    let ram_norm = (node_residual_ram - job_ram) as f64 / node_capacity_ram as f64;\n    let mut score = cpu_norm * cpu_norm + ram_norm * ram_norm;\n    score\n}".to_string();
            let item_fn = parse_heuristic_fn(&code).expect("Balanced fit must parse");
            return (code, item_fn);
        }

        // 3. Deterministic AST fallback template (Strict Rule 0: Pure naive baselines, no human imbalance/horizon weights)
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ (fallback_variant as u64));
        let weight: f64 = rng.gen_range(0.8..1.2);

        let code = match fallback_variant % 4 {
            0 => "pub fn score_node(job_cpu: u32, job_ram: u32, node_residual_cpu: u32, node_residual_ram: u32, node_capacity_cpu: u32, node_capacity_ram: u32, step_index: usize, total_jobs: usize, active_nodes_count: usize, mean_cpu_residual: f64, mean_ram_residual: f64, next_job_cpu: u32, next_job_ram: u32) -> f64 {\n    let score = (node_residual_cpu + node_residual_ram) as f64;\n    score\n}".to_string(),
            1 => "pub fn score_node(job_cpu: u32, job_ram: u32, node_residual_cpu: u32, node_residual_ram: u32, node_capacity_cpu: u32, node_capacity_ram: u32, step_index: usize, total_jobs: usize, active_nodes_count: usize, mean_cpu_residual: f64, mean_ram_residual: f64, next_job_cpu: u32, next_job_ram: u32) -> f64 {\n    let score = (node_residual_cpu as f64 / node_capacity_cpu.max(1) as f64) + (node_residual_ram as f64 / node_capacity_ram.max(1) as f64);\n    score\n}".to_string(),
            2 => format!(
                "pub fn score_node(job_cpu: u32, job_ram: u32, node_residual_cpu: u32, node_residual_ram: u32, node_capacity_cpu: u32, node_capacity_ram: u32, step_index: usize, total_jobs: usize, active_nodes_count: usize, mean_cpu_residual: f64, mean_ram_residual: f64, next_job_cpu: u32, next_job_ram: u32) -> f64 {{\n    let score = (node_residual_cpu as f64 * {:.2}) + node_residual_ram as f64;\n    score\n}}",
                weight
            ),
            _ => "pub fn score_node(job_cpu: u32, job_ram: u32, node_residual_cpu: u32, node_residual_ram: u32, node_capacity_cpu: u32, node_capacity_ram: u32, step_index: usize, total_jobs: usize, active_nodes_count: usize, mean_cpu_residual: f64, mean_ram_residual: f64, next_job_cpu: u32, next_job_ram: u32) -> f64 {\n    let score = node_residual_cpu as f64;\n    score\n}".to_string(),
        };

        let item_fn = parse_heuristic_fn(&code).expect("Template must be valid Rust AST");
        (code, item_fn)
    }
}

impl SkillOperator for HttpLlamaOperator {
    fn generate_seed(
        &self,
        index: usize,
        seed: u64,
    ) -> Result<(SkillCapsule, ReproducibilityAudit), OperatorError> {
        let (system_prompt, user_prompt) = if self.domain == ProblemDomain::MultiResourceScheduling
        {
            let archetype_desc = match index % 5 {
                0 => "ARCHETYPE: Dominant bottleneck & cross-product alignment (align job_cpu/job_ram ratio with node_residual_cpu/node_residual_ram using dot-product or cosine-like coupling).",
                1 => "ARCHETYPE: Strict normalized imbalance minimization (penalize |(post_cpu / cap_cpu) - (post_ram / cap_ram)| non-linearly after subtracting job demands).",
                2 => "ARCHETYPE: Aggressive 2-step lookahead reservation (reward placements where remaining (post_cpu, post_ram) can still fit (next_job_cpu, next_job_ram), penalize stranding the upcoming job).",
                3 => "ARCHETYPE: Dynamic time-horizon & cluster pressure (modulate tight packing vs imbalance avoidance based on step_index / total_jobs and mean_cpu_residual / mean_ram_residual).",
                _ => "ARCHETYPE: Exploratory multi-regime conditional policy (use distinct scoring branches for CPU-heavy jobs vs RAM-heavy jobs).",
            };
            (
                "You are an expert algorithmic scientist specializing in multi-resource cloud job scheduling heuristics in Rust.\n\
Nodes have CPU capacity C_cpu=32 and RAM capacity C_ram=128. Jobs arrive sequentially with (cpu, ram) demands. Your goal is to minimize total nodes used and prevent multi-dimensional resource stranding.\n\
Implement the heuristic as a valid Rust function with the following exact signature:\n\
```rust\n\
pub fn score_node(job_cpu: u32, job_ram: u32, node_residual_cpu: u32, node_residual_ram: u32, node_capacity_cpu: u32, node_capacity_ram: u32, step_index: usize, total_jobs: usize, active_nodes_count: usize, mean_cpu_residual: f64, mean_ram_residual: f64, next_job_cpu: u32, next_job_ram: u32) -> f64 {\n\
    let mut score = node_residual_cpu as f64 + node_residual_ram as f64;\n\
    score\n\
}\n\
```\n\
CRITICAL REQUIREMENTS FOR SOTA PERFORMANCE:\n\
1. The candidate open node with the MINIMUM score is selected for placing the job.\n\
2. To outperform standard baselines, an advanced heuristic MUST balance multi-resource usage:\n\
   - `node_residual_cpu` / `node_residual_ram` are the free capacities BEFORE placing the job (the job is guaranteed to fit). ALWAYS reason on the POST-PLACEMENT state: `let post_cpu = (node_residual_cpu - job_cpu) as f64 / node_capacity_cpu as f64;` and `let post_ram = (node_residual_ram - job_ram) as f64 / node_capacity_ram as f64;`.\n\
   - Penalize POST-PLACEMENT resource imbalance: `(post_cpu - post_ram).abs()` to avoid stranded resource capacity. A node that is currently imbalanced is a GOOD target for a complementary job that restores balance; never penalize pre-placement imbalance.\n\
   - Dynamic horizon awareness: Combine `step_index` and `total_jobs`.\n\
   - Lookahead awareness (k=2): `next_job_cpu` and `next_job_ram` provide lookahead of the immediately upcoming job. If residual capacity can perfectly fit the next job, reserve or prioritize pairing complementary jobs!\n\
3. Use ONLY variables declared in the function signature. Do NOT declare pseudo-random seeds or inject ungrounded constant scales.\n\
Return ONLY the executable Rust function inside a ```rust ... ``` code block without markdown prose.".to_string(),
                format!("Synthesize candidate cloud scheduling heuristic #{}. {}", index, archetype_desc)
            )
        } else {
            let archetype_desc = match index % 5 {
                0 => "ARCHETYPE: Non-linear polynomial coupling post_residual with normalized stream progress (step_index as f64 / total_items as f64). Avoid hardcoded piecewise constants; use smooth non-linear interaction terms.",
                1 => "ARCHETYPE: Harmonic classes logic (explore modulo arithmetic, capacity fractions, integer division thresholds on post_residual and next_item_size).",
                2 => "ARCHETYPE: Strict dead space avoidance (penalizing post_residual in unusable small gaps) coupled with open_bins_count pressure.",
                3 => "ARCHETYPE: Aggressive 2-step lookahead matching (comparing post_residual against next_item_size and relative to mean_residual).",
                _ => "ARCHETYPE: Free exploratory multi-variable heuristic combining lookahead, global state, and dynamic time horizon without preset templates.",
            };
            (
                "You are an expert algorithmic scientist specializing in online 1D bin packing heuristics in Rust.\n\
Bins have capacity C=100. Items arrive sequentially (sizes between 10 and 80). Your goal is to synthesize a heuristic function that minimizes total bins used.\n\
Implement the heuristic as a valid Rust function with the following exact signature:\n\
```rust\n\
pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64, next_item_size: u32, post_residual: u32, fitting_bins_count: usize) -> f64 {\n\
    let mut score = post_residual as f64;\n\
    score\n\
}\n\
```\n\
AVAILABLE INPUT VARIABLES:\n\
- `item_size: u32` : Current item size (10..80).\n\
- `bin_residual: u32` : Space currently available in the bin before placing the item.\n\
- `post_residual: u32` : Remaining space in the bin after placing the item (bin_residual - item_size).\n\
- `bin_capacity: u32` : Total bin capacity (C=100).\n\
- `step_index: usize` and `total_items: usize` : Stream progression (step_index as f64 / total_items as f64).\n\
- `open_bins_count: usize` : Number of currently active open bins.\n\
- `mean_residual: f64` : Mean residual across all currently open bins.\n\
- `next_item_size: u32` : Lookahead item size (k=2), or 0 if this is the last item.\n\
- `fitting_bins_count: usize` : Number of open bins that can fit item_size (if == 1, this bin is the last chance before opening a new bin).\n\
CRITICAL OPTIMIZATION OBJECTIVE:\n\
- THE ORACLE MINIMIZES THE RETURNED SCORE: LOWER score = HIGHER priority to select this bin.\n\
- SUBTRACT from score to favor / reward desirable placements.\n\
- ADD to score to penalize undesirable placements (e.g. creating unusable dead space).\n\
- Invent your own functional relationships, non-linear couplings, and calibrated numerical constants.\n\
- Use ONLY variables declared in the function signature. Do NOT declare pseudo-random seeds or inject ungrounded constant scales.\n\
Return ONLY the executable Rust function inside a ```rust ... ``` code block without markdown prose.".to_string() + &Self::bin_packing_profile_note(),
                format!("Synthesize candidate heuristic #{}. {}", index, archetype_desc)
            )
        };

        let raw_response = self.call_inference(&system_prompt, &user_prompt, seed)?;
        let raw_code = Self::extract_code_block(&raw_response);
        let (ast_code, item_fn) = if self.domain == ProblemDomain::MultiResourceScheduling {
            Self::normalize_to_ast_score_node(&raw_code, seed, index)
        } else {
            Self::normalize_to_ast_score_bin(&raw_code, seed, index)
        };
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
            is_network_call: true,
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

    fn mutate_with_lineage(
        &self,
        parent: &SkillCapsule,
        generation: usize,
        seed: u64,
        lineage: Option<&LineageEvolutionState>,
    ) -> Result<(SkillCapsule, ReproducibilityAudit), OperatorError> {
        let is_sched = parent.logic_code.contains("score_node")
            || self.domain == ProblemDomain::MultiResourceScheduling;

        let mut guidance_section = String::new();
        if let Some(lin) = lineage {
            guidance_section.push_str("\n\n--- AGENT-PSO LINEAGE GUIDANCE ---");
            if let Some(v) = &lin.v_previous {
                match v.momentum_action {
                    MomentumAction::ContinueDirection => {
                        guidance_section.push_str(
                            "\nMOMENTUM ACTION: ContinueDirection\n\
The previous mutation/graft succeeded and improved performance. Reinforce and refine the current mathematical logic, fine-tuning thresholds or adding mild complementary terms in the same direction.",
                        );
                    }
                    MomentumAction::Pivot => {
                        guidance_section.push_str(
                            "\nMOMENTUM ACTION: Pivot\n\
The previous modification stagnated or degraded performance. PIVOT to an alternate algorithmic strategy: explore a different trade-off, alter conditional branches, or introduce a novel non-linear weighting.",
                        );
                    }
                    MomentumAction::Combine => {
                        guidance_section.push_str(
                            "\nMOMENTUM ACTION: Combine\n\
Synthesize multiple complementary features: combine fine-grained residual fitting with lookahead anticipation.",
                        );
                    }
                }
                if !v.weakness_diagnosed.is_empty() {
                    guidance_section.push_str(&format!("\nDiagnosed Weakness: {}", v.weakness_diagnosed));
                }
            }
            if let Some(ref partner) = lin.g_best_partner {
                guidance_section.push_str(&format!("\nBest Cross-Niche Partner: {}", partner));
            }
        }

        let mut profile_lines = Vec::new();
        if let Some(eff) = parent.metadata.get("mean_efficiency") {
            profile_lines.push(format!("Mean Packing Efficiency: {}", eff));
        }
        if let Some(tldr) = parent.metadata.get("tldr_score") {
            profile_lines.push(format!("Lyapunov tLDR Score: {}", tldr));
        }
        if let Some(beta1) = parent.metadata.get("beta1_slope") {
            profile_lines.push(format!("Beta_1 Trajectory Slope: {}", beta1));
        }
        if let Some(sigma) = parent.metadata.get("sigma_res") {
            profile_lines.push(format!("Residual Variance Sigma_res: {}", sigma));
        }

        if !profile_lines.is_empty() {
            guidance_section.push_str("\n\n--- PARENT BEHAVIORAL PROFILE ---");
            for line in profile_lines {
                guidance_section.push_str(&format!("\n- {}", line));
            }
        }

        let (system_prompt, user_prompt) = if is_sched {
            (
                "You are an evolutionary code optimizer for Multi-Resource Cloud Job Scheduling heuristics in Rust.\n\
Apply an algorithmic mutation to improve compaction, reduce total nodes used, and balance CPU and RAM usage.\n\
`node_residual_cpu`/`node_residual_ram` are free capacities BEFORE placing the job (the job always fits): reason on the POST-PLACEMENT state `post_cpu = (node_residual_cpu - job_cpu) as f64 / node_capacity_cpu as f64` and `post_ram = (node_residual_ram - job_ram) as f64 / node_capacity_ram as f64`.\n\
Combine multiple input variables: post-placement residuals, capacities, post-placement normalized imbalance `(post_cpu - post_ram).abs()`, time horizon `step_index / total_jobs`, and lookahead `next_job_cpu`/`next_job_ram`.\n\
Introduce non-linear conditionals, lookahead packing bonuses, imbalance penalties, or horizon-aware placement logic.\n\
The candidate node with the MINIMUM score is selected.\n\
Use ONLY signature variables. Do NOT declare pseudo-random seeds or mock constants.\n\
Return ONLY the mutated Rust function inside a ```rust ... ``` code block without markdown prose.".to_string(),
                format!(
                    "PARENT HEURISTIC (ID: {}):\n```rust\n{}\n```\nINTENT: {}\nMutate this heuristic for Generation {}.{}",
                    parent.id, parent.logic_code, parent.semantic_interface, generation, guidance_section
                )
            )
        } else {
            (
                "You are an evolutionary code optimizer for Online 1D Bin Packing heuristics in Rust.\n\
Apply an algorithmic mutation to improve compaction and reduce total bins used.\n\
CRITICAL: THE ORACLE MINIMIZES THE SCORE. LOWER SCORE = HIGHER PRIORITY TO SELECT THIS BIN. SUBTRACT TO REWARD, ADD TO PENALIZE.\n\
Combine available inputs: `item_size`, `bin_residual`, `post_residual`, `bin_capacity`, `step_index`, `total_items`, `open_bins_count`, `mean_residual`, `next_item_size`, and `fitting_bins_count`.\n\
Search for effective variable couplings, non-linear conditionals, fine-tuned thresholds, or lookahead trade-offs.\n\
Invent your own logic and calibrated constants. Do NOT declare pseudo-random seeds or mock constants. Do NOT copy preset templates.\n\
Return ONLY the mutated Rust function inside a ```rust ... ``` code block without markdown prose.".to_string() + &Self::bin_packing_profile_note(),
                format!(
                    "PARENT HEURISTIC (ID: {}):\n```rust\n{}\n```\nINTENT: {}\nMutate this heuristic for Generation {}.{}",
                    parent.id, parent.logic_code, parent.semantic_interface, generation, guidance_section
                )
            )
        };

        let raw_response = self.call_inference(&system_prompt, &user_prompt, seed)?;
        let raw_code = Self::extract_code_block(&raw_response);
        let (ast_code, item_fn) = if is_sched {
            Self::normalize_to_ast_score_node(&raw_code, seed, generation)
        } else {
            Self::normalize_to_ast_score_bin(&raw_code, seed, generation)
        };
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
            is_network_call: true,
        };

        let mut metadata = HashMap::new();
        metadata.insert("operator".to_string(), "HttpLlamaOperator".to_string());
        metadata.insert("parent_hash".to_string(), parent.content_hash.clone());
        if let Some(lin) = lineage {
            if let Some(v) = &lin.v_previous {
                metadata.insert("lineage_momentum".to_string(), format!("{:?}", v.momentum_action));
            }
        }

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
        let name = format!(
            "SwarmCross-g{}-{}-{}",
            generation,
            &parent_a.id.0[..4],
            &parent_b.id.0[..4]
        );
        let desc = format!(
            "Horizontal Swarm Crossover of [{}] and [{}]",
            parent_a.name, parent_b.name
        );

        let mut metadata = HashMap::new();
        metadata.insert("operator".to_string(), "HttpLlamaOperator".to_string());
        metadata.insert("parent_a".to_string(), parent_a.id.to_string());
        metadata.insert("parent_b".to_string(), parent_b.id.to_string());

        let is_sched = parent_a.logic_code.contains("score_node")
            || self.domain == ProblemDomain::MultiResourceScheduling;

        // Attempt deterministic multi-loci or single-locus AST crossover first
        let ast_res = if seed.is_multiple_of(2) {
            multi_locus_ast_crossover(parent_a, parent_b, seed, generation)
                .or_else(|_| deterministic_ast_crossover(parent_a, parent_b, seed, generation))
        } else {
            deterministic_ast_crossover(parent_a, parent_b, seed, generation)
                .or_else(|_| multi_locus_ast_crossover(parent_a, parent_b, seed, generation))
        };

        if let Ok((mut child, manifest)) = ast_res {
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
                full_prompt: format!(
                    "Deterministic AST crossover between {} and {}",
                    parent_a.id, parent_b.id
                ),
                raw_response: child.logic_code.clone(),
                timestamp: Utc::now(),
                is_network_call: false,
            };

            return Ok((child, audit));
        }

        // Fallback: LLM synthesis
        let system_prompt = if is_sched {
            "You are an evolutionary synthesizer performing SWARM CROSSOVER between two distinct algorithmic lineages for Multi-Resource Cloud Job Scheduling.\n\
Recombine their complementary heuristic mechanisms into a single, superior hybrid Rust function score_node.\n\
The node with the MINIMUM score is selected. Residual inputs are measured BEFORE placement: prefer terms computed on the post-placement state (node_residual_cpu - job_cpu, node_residual_ram - job_ram).\n\
Return ONLY the hybrid Rust function in a ```rust ... ``` code block."
        } else {
            "You are an evolutionary synthesizer performing SWARM CROSSOVER between two distinct algorithmic lineages for Online 1D Bin Packing (C=100).\n\
Recombine their complementary heuristic mechanisms into a single, superior hybrid Rust function score_bin.\n\
Return ONLY the hybrid Rust function in a ```rust ... ``` code block."
        };

        let user_prompt = format!(
            "PARENT A (ID: {}):\n```rust\n{}\n```\n\nPARENT B (ID: {}):\n```rust\n{}\n```\n\nPerform horizontal crossover for Generation {}.",
            parent_a.id, parent_a.logic_code,
            parent_b.id, parent_b.logic_code,
            generation
        );

        let raw_response = self.call_inference(system_prompt, &user_prompt, seed)?;
        let raw_code = Self::extract_code_block(&raw_response);
        let (ast_code, item_fn) = if is_sched {
            Self::normalize_to_ast_score_node(&raw_code, seed, generation)
        } else {
            Self::normalize_to_ast_score_bin(&raw_code, seed, generation)
        };
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
            is_network_call: true,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_llama_operator_ast_normalization() {
        let op_bin = HttpLlamaOperator::default().with_domain(ProblemDomain::BinPacking1D);
        assert_eq!(op_bin.domain, ProblemDomain::BinPacking1D);

        // Test bin packing normalization
        let raw_bin = "```rust\npub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {\n    let mut score = bin_residual as f64;\n    score\n}\n```";
        let (code_bin, _fn_bin) = HttpLlamaOperator::normalize_to_ast_score_bin(
            &HttpLlamaOperator::extract_code_block(raw_bin),
            42,
            0,
        );
        assert!(code_bin.contains("score_bin"));

        // Test scheduling normalization
        let op_sched =
            HttpLlamaOperator::default().with_domain(ProblemDomain::MultiResourceScheduling);
        assert_eq!(op_sched.domain, ProblemDomain::MultiResourceScheduling);

        let raw_sched = "```rust\npub fn score_node(job_cpu: u32, job_ram: u32, node_residual_cpu: u32, node_residual_ram: u32, node_capacity_cpu: u32, node_capacity_ram: u32, step_index: usize, total_jobs: usize, active_nodes_count: usize, mean_cpu_residual: f64, mean_ram_residual: f64, next_job_cpu: u32, next_job_ram: u32) -> f64 {\n    let mut score = node_residual_cpu as f64 + node_residual_ram as f64;\n    score\n}\n```";
        let (code_sched, _fn_sched) = HttpLlamaOperator::normalize_to_ast_score_node(
            &HttpLlamaOperator::extract_code_block(raw_sched),
            42,
            0,
        );
        assert!(code_sched.contains("score_node"));

        // Test fallback templates (Rule 0: verify all 4 variants parse cleanly without expert lookahead injection)
        for variant in 0..4 {
            let (code_fallback_bin, _) =
                HttpLlamaOperator::normalize_to_ast_score_bin("invalid code", 42, variant);
            assert!(code_fallback_bin.contains("score_bin"));
            assert!(!code_fallback_bin.contains("next_item_size =="));
            assert!(!code_fallback_bin.contains("fitting_bins_count =="));

            let (code_fallback_sched, _) =
                HttpLlamaOperator::normalize_to_ast_score_node("invalid code", 42, variant);
            assert!(code_fallback_sched.contains("score_node"));
            assert!(!code_fallback_sched.contains("imbalance *"));
            assert!(!code_fallback_sched.contains("horizon *"));
        }
    }
}
