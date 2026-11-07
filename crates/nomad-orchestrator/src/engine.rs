use rand::{seq::SliceRandom, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use nomad_core::{
    BenchmarkScore, BudgetTracker, EvaluatedIndividual, ExperimentConfig, ExperimentalGroup,
    SkillCapsule,
};
use nomad_evaluator::OracleEvaluator;
use nomad_genome::PhylogenyGraph;
use nomad_operator::SkillOperator;
use nomad_sandbox::HeuristicSandbox;

use crate::error::OrchestratorError;

/// Recorded statistics for a single generation in the evolutionary run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRecord {
    pub generation: usize,
    pub best_bins: usize,
    pub best_efficiency: f64,
    pub best_fuel: u64,
    pub best_capsule_id: String,
    pub best_capsule_name: String,
    pub llm_calls_consumed: usize,
    pub executions_consumed: usize,
}

/// The complete final output of an experimental run for scientific analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentRunReport {
    pub group: ExperimentalGroup,
    pub rng_seed: u64,
    pub total_generations: usize,
    pub best_in_distribution_score: BenchmarkScore,
    pub best_ood1_score: BenchmarkScore,
    pub best_ood2_score: BenchmarkScore,
    pub best_capsule: SkillCapsule,
    pub generations_history: Vec<GenerationRecord>,
    pub budget_consumed: BudgetTracker,
    pub total_lineage_nodes: usize,
    pub total_lineage_edges: usize,
}

/// The core NOMAD Evolutionary Engine.
pub struct EvolutionEngine<'a> {
    pub config: ExperimentConfig,
    pub oracle: &'a OracleEvaluator,
    pub sandbox: &'a dyn HeuristicSandbox,
    pub operator: &'a dyn SkillOperator,
    pub phylogeny: PhylogenyGraph,
    pub budget: BudgetTracker,
}

impl<'a> EvolutionEngine<'a> {
    pub fn new(
        config: ExperimentConfig,
        oracle: &'a OracleEvaluator,
        sandbox: &'a dyn HeuristicSandbox,
        operator: &'a dyn SkillOperator,
    ) -> Self {
        Self {
            config,
            oracle,
            sandbox,
            operator,
            phylogeny: PhylogenyGraph::new(),
            budget: BudgetTracker::default(),
        }
    }

    /// Executes the full evolutionary protocol for the configured group.
    pub fn execute_run(&mut self) -> Result<ExperimentRunReport, OrchestratorError> {
        info!(
            "Starting NOMAD Experiment: Group={:?}, Seed={}, Pop={}, MaxGens={}",
            self.config.group, self.config.rng_seed, self.config.population_size, self.config.max_generations
        );

        let mut rng = ChaCha8Rng::seed_from_u64(self.config.rng_seed);
        let mut generation_records = Vec::new();

        // 1. Initialize Seed Population
        let mut current_population = self.initialize_population(&mut rng)?;

        let mut best_overall: Option<EvaluatedIndividual> = None;

        for gen in 1..=self.config.max_generations {
            if !self.budget.can_invoke_llm(&self.config) || !self.budget.can_execute(&self.config) {
                warn!(
                    "Compute budget reached at generation {} (LLM calls: {}, Executions: {})",
                    gen, self.budget.llm_calls_consumed, self.budget.executions_consumed
                );
                break;
            }

            // Sort population: lowest bins, then lowest fuel
            current_population.sort_by(|a, b| a.in_distribution_score.cmp(&b.in_distribution_score));

            for (rank, ind) in current_population.iter_mut().enumerate() {
                ind.rank = rank + 1;
            }

            let top_individual = current_population[0].clone();
            let is_new_global_best = match &best_overall {
                None => true,
                Some(prev) => top_individual.in_distribution_score < prev.in_distribution_score,
            };

            if is_new_global_best {
                best_overall = Some(top_individual.clone());
            }

            let record = GenerationRecord {
                generation: gen,
                best_bins: top_individual.in_distribution_score.total_bins,
                best_efficiency: top_individual.in_distribution_score.mean_efficiency,
                best_fuel: top_individual.in_distribution_score.total_fuel,
                best_capsule_id: top_individual.capsule.id.0.clone(),
                best_capsule_name: top_individual.capsule.name.clone(),
                llm_calls_consumed: self.budget.llm_calls_consumed,
                executions_consumed: self.budget.executions_consumed,
            };
            generation_records.push(record);

            if gen == self.config.max_generations {
                break;
            }

            // 2. Selection & Reproduction based on Group Strategy
            current_population = self.produce_next_generation(&current_population, gen, &mut rng)?;
        }

        let best_ind = best_overall.unwrap_or_else(|| current_population[0].clone());

        // 3. Independent Out-Of-Distribution Evaluation of the Best Heuristic
        info!("Evaluating winning capsule {} on OOD benchmarks...", best_ind.capsule.id);
        let ood1_score = self.oracle.evaluate_ood1(
            &best_ind.capsule,
            self.sandbox,
            self.config.fuel_limit_per_call,
        );
        let ood2_score = self.oracle.evaluate_ood2(
            &best_ind.capsule,
            self.sandbox,
            self.config.fuel_limit_per_call,
        );

        Ok(ExperimentRunReport {
            group: self.config.group,
            rng_seed: self.config.rng_seed,
            total_generations: generation_records.len(),
            best_in_distribution_score: best_ind.in_distribution_score,
            best_ood1_score: ood1_score,
            best_ood2_score: ood2_score,
            best_capsule: best_ind.capsule,
            generations_history: generation_records,
            budget_consumed: self.budget.clone(),
            total_lineage_nodes: self.phylogeny.node_count(),
            total_lineage_edges: self.phylogeny.edge_count(),
        })
    }

    fn initialize_population(
        &mut self,
        rng: &mut ChaCha8Rng,
    ) -> Result<Vec<EvaluatedIndividual>, OrchestratorError> {
        let mut population = Vec::with_capacity(self.config.population_size);

        for i in 0..self.config.population_size {
            let seed = rand::Rng::gen(rng);
            let (capsule, _audit) = self
                .operator
                .generate_seed(i, seed)
                .map_err(|e| OrchestratorError::OperatorError(e.to_string()))?;

            self.budget.record_llm_call();
            self.phylogeny.insert_capsule(&capsule);

            let score = self.oracle.evaluate_in_distribution(
                &capsule,
                self.sandbox,
                self.config.fuel_limit_per_call,
            );
            self.budget.record_executions(self.oracle.in_distribution.instances.len());

            population.push(EvaluatedIndividual::new(capsule, score));
        }

        Ok(population)
    }

    fn produce_next_generation(
        &mut self,
        current_pop: &[EvaluatedIndividual],
        next_gen_idx: usize,
        rng: &mut ChaCha8Rng,
    ) -> Result<Vec<EvaluatedIndividual>, OrchestratorError> {
        let pop_size = self.config.population_size;
        let mut next_population = Vec::with_capacity(pop_size);

        match self.config.group {
            ExperimentalGroup::GroupA => {
                // Group A: 1 individual with sequential self-correction
                let current_best = &current_pop[0];
                next_population.push(current_best.clone());

                while next_population.len() < pop_size && self.budget.can_invoke_llm(&self.config) {
                    let seed = rand::Rng::gen(rng);
                    let (mutated, _) = self
                        .operator
                        .mutate(&current_best.capsule, next_gen_idx, seed)
                        .map_err(|e| OrchestratorError::OperatorError(e.to_string()))?;

                    self.budget.record_llm_call();
                    self.phylogeny.insert_capsule(&mutated);

                    let score = self.oracle.evaluate_in_distribution(
                        &mutated,
                        self.sandbox,
                        self.config.fuel_limit_per_call,
                    );
                    self.budget.record_executions(self.oracle.in_distribution.instances.len());

                    next_population.push(EvaluatedIndividual::new(mutated, score));
                }
            }

            ExperimentalGroup::GroupB => {
                // Group B: Independent parallel agents without horizontal transmission
                for ind in current_pop {
                    if next_population.len() >= pop_size || !self.budget.can_invoke_llm(&self.config) {
                        break;
                    }
                    let seed = rand::Rng::gen(rng);
                    let (mutated, _) = self
                        .operator
                        .mutate(&ind.capsule, next_gen_idx, seed)
                        .map_err(|e| OrchestratorError::OperatorError(e.to_string()))?;

                    self.budget.record_llm_call();
                    self.phylogeny.insert_capsule(&mutated);

                    let score = self.oracle.evaluate_in_distribution(
                        &mutated,
                        self.sandbox,
                        self.config.fuel_limit_per_call,
                    );
                    self.budget.record_executions(self.oracle.in_distribution.instances.len());

                    next_population.push(EvaluatedIndividual::new(mutated, score));
                }
            }

            ExperimentalGroup::GroupD => {
                // Group D: Evolutionary selection top 20%, reproduction by MUTATION ONLY
                let elite_count = ((pop_size as f64) * self.config.elite_selection_ratio).ceil() as usize;
                let elite_pool = &current_pop[..elite_count.max(1)];

                // Elitism: carry over the best directly
                next_population.push(elite_pool[0].clone());

                while next_population.len() < pop_size && self.budget.can_invoke_llm(&self.config) {
                    let parent = elite_pool.choose(rng).unwrap();
                    let seed = rand::Rng::gen(rng);

                    let (mutated, _) = self
                        .operator
                        .mutate(&parent.capsule, next_gen_idx, seed)
                        .map_err(|e| OrchestratorError::OperatorError(e.to_string()))?;

                    self.budget.record_llm_call();
                    self.phylogeny.insert_capsule(&mutated);

                    let score = self.oracle.evaluate_in_distribution(
                        &mutated,
                        self.sandbox,
                        self.config.fuel_limit_per_call,
                    );
                    self.budget.record_executions(self.oracle.in_distribution.instances.len());

                    next_population.push(EvaluatedIndividual::new(mutated, score));
                }
            }

            ExperimentalGroup::GroupC => {
                // Group C (NOMAD Swarm): Selection top 20%, reproduction via Mutation + Swarm Crossover
                let elite_count = ((pop_size as f64) * self.config.elite_selection_ratio).ceil() as usize;
                let elite_pool = &current_pop[..elite_count.max(2)];

                // Elitism: preserve top individual
                next_population.push(elite_pool[0].clone());

                while next_population.len() < pop_size && self.budget.can_invoke_llm(&self.config) {
                    let seed = rand::Rng::gen(rng);
                    let should_crossover = rand::Rng::gen_bool(rng, 0.50) && elite_pool.len() >= 2;

                    let (child_capsule, _) = if should_crossover {
                        // Pick two distinct parents from the elite pool
                        let parent_a = elite_pool.choose(rng).unwrap();
                        let mut parent_b = elite_pool.choose(rng).unwrap();
                        while parent_a.capsule.id == parent_b.capsule.id && elite_pool.len() > 1 {
                            parent_b = elite_pool.choose(rng).unwrap();
                        }

                        self.operator
                            .crossover(&parent_a.capsule, &parent_b.capsule, next_gen_idx, seed)
                            .map_err(|e| OrchestratorError::OperatorError(e.to_string()))?
                    } else {
                        // Mutation
                        let parent = elite_pool.choose(rng).unwrap();
                        self.operator
                            .mutate(&parent.capsule, next_gen_idx, seed)
                            .map_err(|e| OrchestratorError::OperatorError(e.to_string()))?
                    };

                    self.budget.record_llm_call();
                    self.phylogeny.insert_capsule(&child_capsule);

                    let score = self.oracle.evaluate_in_distribution(
                        &child_capsule,
                        self.sandbox,
                        self.config.fuel_limit_per_call,
                    );
                    self.budget.record_executions(self.oracle.in_distribution.instances.len());

                    next_population.push(EvaluatedIndividual::new(child_capsule, score));
                }
            }
        }

        // Fill remaining slots if budget prevented full creation
        while next_population.len() < pop_size && !current_pop.is_empty() {
            next_population.push(current_pop[next_population.len() % current_pop.len()].clone());
        }

        Ok(next_population)
    }
}
