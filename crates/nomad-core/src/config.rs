use serde::{Deserialize, Serialize};
use crate::types::{ExperimentalGroup, ProblemDomain};

fn default_node_cpu() -> u32 {
    32
}

fn default_node_ram() -> u32 {
    128
}

fn default_anti_dead_code_epsilon() -> f64 {
    1e-4
}

/// Configuration for an evolutionary run according to the pre-registered protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentConfig {
    /// Combinatorial domain for the experiment (default: BinPacking1D).
    #[serde(default)]
    pub domain: ProblemDomain,
    /// Experimental group designation (A, B, C, or D).
    pub group: ExperimentalGroup,
    /// Random number generator seed for exact reproducibility.
    pub rng_seed: u64,
    /// Number of individuals in the population (default: 100).
    pub population_size: usize,
    /// Number of generations to evolve (default: 100).
    pub max_generations: usize,
    /// Proportion of top individuals selected as parents (default: 0.20 = top 20%).
    pub elite_selection_ratio: f64,
    /// Strict budget of LLM calls permitted across the run (default: 10,000).
    pub max_llm_calls: usize,
    /// Strict budget of Wasm/Sandbox executions across the run (default: 100,000).
    pub max_executions: usize,
    /// Execution fuel/gas limit per single item decision.
    pub fuel_limit_per_call: u64,
    /// Maximum memory limit in bytes.
    pub memory_limit_bytes: usize,
    /// Bin capacity for the 1D Bin Packing problem (default: 100).
    pub bin_capacity: u32,
    /// CPU capacity for scheduling nodes (default: 32 cores).
    #[serde(default = "default_node_cpu")]
    pub node_capacity_cpu: u32,
    /// RAM capacity for scheduling nodes (default: 128 GB).
    #[serde(default = "default_node_ram")]
    pub node_capacity_ram: u32,
    /// Optional cooldown pause in milliseconds between LLM calls to prevent GPU thermal throttling.
    #[serde(default)]
    pub cooldown_ms: u64,
    /// Seuil epsilon de dégradation mesurable requis pour valider le Niveau 3 Anti-Dead-Code.
    #[serde(default = "default_anti_dead_code_epsilon")]
    pub anti_dead_code_epsilon: f64,
}

impl Default for ExperimentConfig {
    fn default() -> Self {
        Self {
            domain: ProblemDomain::BinPacking1D,
            group: ExperimentalGroup::GroupC,
            rng_seed: 42,
            population_size: 100,
            max_generations: 100,
            elite_selection_ratio: 0.20,
            max_llm_calls: 10_000,
            max_executions: 100_000,
            fuel_limit_per_call: 50_000,
            memory_limit_bytes: 4 * 1024 * 1024, // 4 MB
            bin_capacity: 100,
            node_capacity_cpu: 32,
            node_capacity_ram: 128,
            cooldown_ms: 0,
            anti_dead_code_epsilon: 1e-4,
        }
    }
}

/// Tracks compute budget consumption to ensure fairness across all groups.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BudgetTracker {
    pub llm_calls_consumed: usize,
    pub executions_consumed: usize,
}

impl BudgetTracker {
    pub fn can_invoke_llm(&self, config: &ExperimentConfig) -> bool {
        self.llm_calls_consumed < config.max_llm_calls
    }

    pub fn can_execute(&self, config: &ExperimentConfig) -> bool {
        self.executions_consumed < config.max_executions
    }

    pub fn record_llm_call(&mut self) {
        self.llm_calls_consumed += 1;
    }

    pub fn record_executions(&mut self, count: usize) {
        self.executions_consumed += count;
    }
}

