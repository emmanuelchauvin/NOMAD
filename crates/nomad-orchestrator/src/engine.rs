use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use nomad_core::{
    BenchmarkScore, BudgetTracker, EvaluatedIndividual, ExperimentConfig, ExperimentalGroup,
    ProblemDomain, SkillCapsule,
};
use nomad_evaluator::{validate_anti_dead_code_3levels, OracleEvaluator, SchedulingOracle};
use nomad_genome::PhylogenyGraph;
use nomad_operator::SkillOperator;
use nomad_sandbox::HeuristicSandbox;

use crate::error::OrchestratorError;
use crate::map_elites::MapElitesArchive;
use crate::pso::LineageManager;

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
    #[serde(default)]
    pub qd_score: f64,
    #[serde(default)]
    pub archive_coverage: f64,
    #[serde(default)]
    pub occupied_niches: usize,
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
    #[serde(default)]
    pub phylogeny: PhylogenyGraph,
    #[serde(default)]
    pub archive: MapElitesArchive,
}

/// ============================================================================
/// EvolutionEngine : Moteur d'Évolution et d'Orchestration NOMAD
/// ============================================================================
/// Coordonne le cycle de vie complet de l'essaim d'organismes :
/// - Initialisation des graines par le LLM ou le mock.
/// - Évaluation souveraine par l'Oracle dans le Sandbox isolé.
/// - Insertion dans l'archive Quality-Diversity MAP-Elites et compétition locale.
/// - Sélection des parents par curiosité UCB.
/// - Appariement complémentaire AgentPSO.
/// - Reproduction via Crossover AST ou Mutation avec validation Anti-Dead-Code.
/// - Traçabilité dans le graphe phylogénétique.
pub struct EvolutionEngine<'a> {
    pub config: ExperimentConfig,
    pub oracle: &'a OracleEvaluator,
    pub scheduling_oracle: Option<&'a SchedulingOracle>,
    pub sandbox: &'a dyn HeuristicSandbox,
    pub operator: &'a dyn SkillOperator,
    pub phylogeny: PhylogenyGraph,
    pub budget: BudgetTracker,
    pub archive: MapElitesArchive,
    pub lineages: LineageManager,
}

impl<'a> EvolutionEngine<'a> {
    /// Initialise le moteur avec la configuration, l'oracle, le sandbox et l'opérateur de code.
    pub fn new(
        config: ExperimentConfig,
        oracle: &'a OracleEvaluator,
        sandbox: &'a dyn HeuristicSandbox,
        operator: &'a dyn SkillOperator,
    ) -> Self {
        Self {
            config,
            oracle,
            scheduling_oracle: None,
            sandbox,
            operator,
            phylogeny: PhylogenyGraph::new(),
            budget: BudgetTracker::default(),
            archive: MapElitesArchive::new(10, 10),
            lineages: LineageManager::new(),
        }
    }

    /// Associe un oracle de scheduling multi-ressources au moteur d'évolution.
    pub fn with_scheduling_oracle(mut self, sched_oracle: &'a SchedulingOracle) -> Self {
        self.scheduling_oracle = Some(sched_oracle);
        self
    }

    fn throttle_cooldown(&self) {
        if self.config.cooldown_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(self.config.cooldown_ms));
        }
    }

    /// ========================================================================
    /// execute_run : Algorithme Principal de la Boucle Évolutionnaire
    /// ========================================================================
    /// 1. Initialise la population ancestrale (Gen 0) et peuple l'archive.
    /// 2. Pour chaque génération de 1 à max_generations :
    ///    - Contrôle les budgets CPU et LLM.
    ///    - Classe la population et met à jour le champion global.
    ///    - Calcule le QD-Score et la couverture de niches de l'archive.
    ///    - Produit la génération suivante via `produce_next_generation`.
    /// 3. Évalue indépendamment le grand vainqueur sur les benchmarks OOD-1 et OOD-2.
    /// 4. Compile et renvoie le rapport complet `ExperimentRunReport`.
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

            let qd_score = self.archive.qd_score();
            let archive_coverage = self.archive.coverage();
            let occupied_niches = self.archive.occupied_niches_count();

            let record = GenerationRecord {
                generation: gen,
                best_bins: top_individual.in_distribution_score.total_bins,
                best_efficiency: top_individual.in_distribution_score.mean_efficiency,
                best_fuel: top_individual.in_distribution_score.total_fuel,
                best_capsule_id: top_individual.capsule.id.0.clone(),
                best_capsule_name: top_individual.capsule.name.clone(),
                llm_calls_consumed: self.budget.llm_calls_consumed,
                executions_consumed: self.budget.executions_consumed,
                qd_score,
                archive_coverage,
                occupied_niches,
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
        let (ood1_score, ood2_score) = if self.config.domain == ProblemDomain::MultiResourceScheduling {
            if let Some(sched) = self.scheduling_oracle {
                (
                    sched.evaluate_dataset(&sched.ood1_scale, &best_ind.capsule, self.sandbox, self.config.fuel_limit_per_call),
                    sched.evaluate_dataset(&sched.ood2_skewed, &best_ind.capsule, self.sandbox, self.config.fuel_limit_per_call),
                )
            } else {
                (BenchmarkScore::fatal_penalty(50), BenchmarkScore::fatal_penalty(50))
            }
        } else {
            (
                self.oracle.evaluate_ood1(&best_ind.capsule, self.sandbox, self.config.fuel_limit_per_call),
                self.oracle.evaluate_ood2(&best_ind.capsule, self.sandbox, self.config.fuel_limit_per_call),
            )
        };

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
            phylogeny: self.phylogeny.clone(),
            archive: self.archive.clone(),
        })
    }

    fn initialize_population(
        &mut self,
        rng: &mut ChaCha8Rng,
    ) -> Result<Vec<EvaluatedIndividual>, OrchestratorError> {
        let mut population = Vec::with_capacity(self.config.population_size);

        // 1. Incorporate collective heritage capsules already seeded in the archive (Rebirth Protocol)
        let existing_heritage: Vec<(SkillCapsule, BenchmarkScore)> = self
            .archive
            .grid
            .values()
            .map(|entry| (entry.capsule.clone(), entry.fitness.clone()))
            .collect();

        for (cap, score) in existing_heritage.into_iter().take(self.config.population_size / 2) {
            self.phylogeny.insert_capsule(&cap);
            self.lineages.register_seed(cap.id.clone());
            population.push(EvaluatedIndividual::new(cap, score));
        }

        // 2. Fill the remaining slots with freshly generated seeds
        let remaining_slots = self.config.population_size.saturating_sub(population.len());
        for i in 0..remaining_slots {
            let seed = rand::Rng::gen(rng);
            let (capsule, _audit) = self
                .operator
                .generate_seed(i, seed)
                .map_err(|e| OrchestratorError::OperatorError(e.to_string()))?;

            self.budget.record_llm_call();
            self.throttle_cooldown();

            let (capsule, score) = self.evaluate_and_validate(capsule);
            self.phylogeny.insert_capsule(&capsule);
            self.lineages.register_seed(capsule.id.clone());
            self.archive.insert(capsule.clone(), score.clone());
            population.push(EvaluatedIndividual::new(capsule, score));
        }

        Ok(population)
    }

    /// Evaluates a candidate capsule and runs the full 3-level Anti-Dead-Code verification pipeline.
    fn evaluate_and_validate(&mut self, mut capsule: SkillCapsule) -> (SkillCapsule, BenchmarkScore) {
        if self.config.domain == ProblemDomain::MultiResourceScheduling {
            if let Some(sched) = self.scheduling_oracle {
                let _ = sched.validate_anti_dead_code_scheduling(
                    &mut capsule,
                    self.sandbox,
                    self.config.fuel_limit_per_call,
                );
                let score = sched.evaluate_dataset(
                    &sched.in_distribution,
                    &capsule,
                    self.sandbox,
                    self.config.fuel_limit_per_call,
                );
                self.budget.record_executions(sched.in_distribution.instances.len());
                return (capsule, score);
            }
        }

        let _ = validate_anti_dead_code_3levels(
            &mut capsule,
            self.oracle,
            self.sandbox,
            &self.oracle.in_distribution,
            self.config.fuel_limit_per_call,
        );

        let score = self.oracle.evaluate_in_distribution(
            &capsule,
            self.sandbox,
            self.config.fuel_limit_per_call,
        );
        self.budget.record_executions(self.oracle.in_distribution.instances.len());
        (capsule, score)
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
                    self.throttle_cooldown();

                    let (mutated, score) = self.evaluate_and_validate(mutated);
                    self.phylogeny.insert_capsule(&mutated);
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
                    self.throttle_cooldown();

                    let (mutated, score) = self.evaluate_and_validate(mutated);
                    self.phylogeny.insert_capsule(&mutated);
                    next_population.push(EvaluatedIndividual::new(mutated, score));
                }
            }

            ExperimentalGroup::GroupD => {
                // Group D: Evolutionary selection with MAP-Elites niche selection, MUTATION ONLY
                let elite = self.archive.best_elite().map(|e| e.capsule.clone()).unwrap_or_else(|| current_pop[0].capsule.clone());
                let elite_score = self.archive.best_elite().map(|e| e.fitness.clone()).unwrap_or_else(|| current_pop[0].in_distribution_score.clone());
                next_population.push(EvaluatedIndividual::new(elite, elite_score));

                while next_population.len() < pop_size && self.budget.can_invoke_llm(&self.config) {
                    let parent = self.archive.select_parent_ucb(rng).unwrap_or_else(|| current_pop[0].capsule.clone());
                    let seed = rand::Rng::gen(rng);

                    let (mutated, _) = self
                        .operator
                        .mutate(&parent, next_gen_idx, seed)
                        .map_err(|e| OrchestratorError::OperatorError(e.to_string()))?;

                    self.budget.record_llm_call();
                    self.throttle_cooldown();

                    let (mutated, score) = self.evaluate_and_validate(mutated);
                    self.phylogeny.insert_capsule(&mutated);
                    self.archive.insert(mutated.clone(), score.clone());
                    next_population.push(EvaluatedIndividual::new(mutated, score));
                }
            }

            ExperimentalGroup::GroupC => {
                // Group C (NOMAD V2 Swarm): Quality-Diversity MAP-Elites + AgentPSO Swarm Crossover
                if let Some(elite) = self.archive.best_elite() {
                    next_population.push(EvaluatedIndividual::new(elite.capsule.clone(), elite.fitness.clone()));
                } else {
                    next_population.push(current_pop[0].clone());
                }

                while next_population.len() < pop_size && self.budget.can_invoke_llm(&self.config) {
                    let seed = rand::Rng::gen(rng);
                    let should_crossover = rand::Rng::gen_bool(rng, 0.60) && (self.archive.occupied_niches_count() >= 2 || current_pop.len() >= 2);

                    let (child_capsule, _) = if should_crossover {
                        // 1. Quality-Diversity Parent A Selection via UCB curiosity
                        let parent_a = self.archive.select_parent_ucb(rng)
                            .unwrap_or_else(|| current_pop[0].capsule.clone());

                        let parent_a_score = self.archive.grid.values()
                            .find(|e| e.capsule.id == parent_a.id)
                            .map(|e| e.fitness.clone())
                            .unwrap_or_else(|| current_pop[0].in_distribution_score.clone());

                        // 2. AgentPSO Directed Partner Selection for complementary traits
                        let parent_b = if self.archive.occupied_niches_count() >= 2 {
                            self.archive.select_complementary_partner(&parent_a, &parent_a_score, rng)
                                .unwrap_or_else(|| current_pop[rng.gen_range(0..current_pop.len())].capsule.clone())
                        } else {
                            // Archive has 1 niche: draw partner from current population
                            let candidate = &current_pop[rng.gen_range(0..current_pop.len())].capsule;
                            if candidate.id != parent_a.id {
                                candidate.clone()
                            } else {
                                current_pop[(rng.gen_range(0..current_pop.len()) + 1) % current_pop.len()].capsule.clone()
                            }
                        };

                        self.operator
                            .crossover(&parent_a, &parent_b, next_gen_idx, seed)
                            .map_err(|e| OrchestratorError::OperatorError(e.to_string()))?
                    } else {
                        // Mutation from a UCB-selected parent
                        let parent = self.archive.select_parent_ucb(rng)
                            .unwrap_or_else(|| current_pop[0].capsule.clone());

                        self.operator
                            .mutate(&parent, next_gen_idx, seed)
                            .map_err(|e| OrchestratorError::OperatorError(e.to_string()))?
                    };

                    self.budget.record_llm_call();
                    self.throttle_cooldown();

                    let (child_capsule, score) = self.evaluate_and_validate(child_capsule);
                    self.phylogeny.insert_capsule(&child_capsule);

                    // Record in AgentPSO lineage state
                    let host_id = child_capsule.parent_ids.first().cloned().unwrap_or_else(|| child_capsule.id.clone());
                    let partner_id = child_capsule.parent_ids.get(1).cloned();
                    let is_improved = if let Some(elite) = self.archive.best_elite() {
                        score < elite.fitness
                    } else {
                        true
                    };

                    self.lineages.record_evolution(
                        child_capsule.id.clone(),
                        &host_id,
                        partner_id,
                        child_capsule.semantic_interface.clone(),
                        is_improved,
                    );

                    // Local niche competition in MAP-Elites
                    self.archive.insert(child_capsule.clone(), score.clone());

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

