use crate::types::{ExperimentalGroup, ProblemDomain};
use serde::{Deserialize, Serialize};

fn default_node_cpu() -> u32 {
    32
}

fn default_node_ram() -> u32 {
    128
}

fn default_anti_dead_code_epsilon() -> f64 {
    1e-4
}

fn default_tldr_epsilon() -> f64 {
    1e-4
}

fn default_true() -> bool {
    true
}

fn default_constant_opt_excess() -> usize {
    50
}

/// Regle de choix du parent d'une mutation parmi les elites de l'archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ParentSelection {
    /// Selection par rang : le meilleur elite a une chance sur deux, le suivant une sur
    /// quatre, etc. La pression de selection est forte et ne depend pas de l'echelle des scores.
    #[default]
    Rank,
    /// Curiosite UCB sur les niches (regle des campagnes Weibull v1 et v2). Le bonus
    /// d'exploration y domine les ecarts de qualite : les niches sont visitees presque
    /// uniformement. Conservee pour reproduire ces campagnes (`NOMAD_PARENT_SELECTION=ucb`).
    Ucb,
}

impl ParentSelection {
    /// Lit `NOMAD_PARENT_SELECTION` (`rank` par defaut, `ucb` pour l'ancienne regle).
    pub fn from_env() -> Self {
        match std::env::var("NOMAD_PARENT_SELECTION") {
            Ok(v) if v.trim().eq_ignore_ascii_case("ucb") => Self::Ucb,
            _ => Self::Rank,
        }
    }
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
    /// Seuil epsilon pour le calcul de décroissance Lyapunov (tLDR) et départage directionnel.
    #[serde(default = "default_tldr_epsilon")]
    pub tldr_epsilon: f64,
    /// Option d'évaluation lookahead (k=2) : si faux, les données d'anticipation sont masquées (défaut: true).
    #[serde(default = "default_true")]
    pub lookahead: bool,
    /// Activation de l'optimisation locale des constantes AST par Hill-Climbing (défaut: false pour pureté comparative).
    #[serde(default)]
    pub enable_constant_optimization: bool,
    /// Seuil maximal d'excès de bacs par rapport au minimum théorique pour déclencher l'optimisation des constantes.
    #[serde(default = "default_constant_opt_excess")]
    pub constant_opt_excess_threshold: usize,
    /// Regle de choix du parent d'une mutation (defaut : selection par rang).
    #[serde(default)]
    pub parent_selection: ParentSelection,
    /// Nombre de propositions fraiches de la population de depart. 0 : egal a
    /// `population_size` (comportement historique). Variable `NOMAD_INIT_POP`.
    #[serde(default)]
    pub initial_population_size: usize,
    /// Nombre de generations sans nouveau meilleur score apres lequel une proposition sur
    /// deux redevient une proposition fraiche (groupe D). 0 : jamais (comportement
    /// historique). Variable `NOMAD_STAGNATION_PATIENCE`.
    #[serde(default)]
    pub stagnation_patience: usize,
    /// Si vrai, le prompt de mutation indique le score mesure du parent et le meilleur
    /// programme connu (groupe D). Variable `NOMAD_INFORMED_MUTATION=1`.
    #[serde(default)]
    pub informed_mutation: bool,
}

fn env_usize(name: &str) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
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
            // Surcharge possible par NOMAD_FUEL_LIMIT (longues instances : beaucoup de bacs ouverts).
            fuel_limit_per_call: std::env::var("NOMAD_FUEL_LIMIT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50_000),
            memory_limit_bytes: 4 * 1024 * 1024, // 4 MB
            bin_capacity: 100,
            node_capacity_cpu: 32,
            node_capacity_ram: 128,
            cooldown_ms: 0,
            anti_dead_code_epsilon: 1e-4,
            tldr_epsilon: 1e-4,
            lookahead: true,
            enable_constant_optimization: false,
            constant_opt_excess_threshold: 50,
            parent_selection: ParentSelection::from_env(),
            initial_population_size: env_usize("NOMAD_INIT_POP"),
            stagnation_patience: env_usize("NOMAD_STAGNATION_PATIENCE"),
            informed_mutation: env_usize("NOMAD_INFORMED_MUTATION") == 1,
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
