use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use nomad_core::{
    BenchmarkScore, BudgetTracker, EvaluatedIndividual, ExperimentConfig, ExperimentalGroup,
    ProblemDomain, SkillCapsule,
};
use nomad_evaluator::{
    validate_anti_dead_code_3levels_with_score, OracleEvaluator, SchedulingOracle,
};
use nomad_genome::{
    deterministic_ast_crossover, multi_locus_ast_crossover, normalize_source, parse_heuristic_fn,
    PhylogenyGraph,
};
use nomad_operator::{MutationContext, SkillOperator};
use nomad_sandbox::HeuristicSandbox;

use crate::error::OrchestratorError;

/// Compteurs des operateurs locaux du groupe D (croisement AST, elagage).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocalOpsStats {
    pub crossover_attempts: usize,
    pub crossover_errors: usize,
    pub crossover_accepted: usize,
    pub prune_attempts: usize,
    pub prune_accepted: usize,
    pub rules_removed: usize,
    /// Evaluations sur l'entrainement faites par ces operateurs.
    pub evaluations: usize,
}
use crate::map_elites::MapElitesArchive;
use crate::pso::LineageManager;

fn default_crossover_rate() -> f64 {
    0.60
}

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
    #[serde(default = "default_crossover_rate")]
    pub crossover_rate: f64,
    #[serde(default)]
    pub crossover_not_evaluable_count: usize,
    #[serde(default)]
    pub crossover_rejected_count: usize,
}

/// Une proposition evaluee pendant la recherche, dans l'ordre ou elle a ete produite.
///
/// La suite complete permet de relire un run a n'importe quel budget intermediaire : le
/// champion apres `k` propositions est la derniere d'entre elles marquee `is_new_best`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalRecord {
    /// Rang de la proposition dans le run, a partir de 1.
    pub index: usize,
    /// Appels au modele consommes une fois cette proposition produite.
    pub llm_calls_consumed: usize,
    pub generation: usize,
    /// `fresh` (proposition independante), `mutation` ou `crossover`.
    pub kind: String,
    pub capsule_id: String,
    pub capsule_name: String,
    pub parent_ids: Vec<String>,
    /// Score a l'entrainement.
    pub is_valid: bool,
    pub total_bins: usize,
    pub total_fuel: u64,
    /// Vrai si cette proposition bat strictement toutes les precedentes a l'entrainement
    /// (ordre de `BenchmarkScore`) : elle devient le champion du run a ce stade.
    pub is_new_best: bool,
    /// Score sur la validation, calcule en fin de run pour les seules propositions
    /// `is_new_best`. N'intervient ni dans la selection ni dans le budget.
    #[serde(default)]
    pub validation_is_valid: Option<bool>,
    #[serde(default)]
    pub validation_bins: Option<usize>,
    pub logic_code: String,
}

/// The complete final output of an experimental run for scientific analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentRunReport {
    pub group: ExperimentalGroup,
    pub rng_seed: u64,
    pub total_generations: usize,
    pub best_in_distribution_score: BenchmarkScore,
    #[serde(default)]
    pub best_score_without_lookahead: Option<BenchmarkScore>,
    #[serde(default)]
    pub best_validation_score: Option<BenchmarkScore>,
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
    #[serde(default)]
    pub crossover_attempts: usize,
    #[serde(default)]
    pub crossover_not_evaluable_count: usize,
    #[serde(default)]
    pub crossover_rejected_count: usize,
    /// Toutes les propositions evaluees, dans l'ordre (absent des rapports anterieurs).
    #[serde(default)]
    pub proposals: Vec<ProposalRecord>,
    /// Compteurs des operateurs locaux du groupe D (absent des rapports anterieurs).
    #[serde(default)]
    pub local_ops: LocalOpsStats,
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
    pub cultural_archive: Option<MapElitesArchive>,
    pub lineages: LineageManager,
    pub adaptive_crossover_rate: f64,
    pub crossover_attempts: usize,
    pub crossover_not_evaluable_count: usize,
    pub crossover_rejected_count: usize,
    /// Generations consecutives sans nouveau meilleur score a l'entrainement.
    pub stagnant_generations: usize,
    /// Operateurs locaux du groupe D (sans appel au modele) : compteurs descriptifs.
    pub local_stats: LocalOpsStats,
    /// Dernier champion deja elague (evite de l'elaguer a nouveau).
    last_pruned_id: Option<String>,
    /// Index de la prochaine proposition fraiche (prolonge le cycle des archetypes).
    pub fresh_seed_counter: usize,
    /// Journal de toutes les propositions evaluees, dans l'ordre.
    pub proposals: Vec<ProposalRecord>,
    /// Capsules des champions successifs, avec leur position dans `proposals`.
    best_trail: Vec<(usize, SkillCapsule)>,
    /// Meilleur score d'entrainement rencontre jusqu'ici.
    best_proposal_score: Option<BenchmarkScore>,
}

impl<'a> EvolutionEngine<'a> {
    /// Initialise le moteur avec la configuration, l'oracle, le sandbox et l'opérateur de code.
    pub fn new(
        config: ExperimentConfig,
        oracle: &'a OracleEvaluator,
        sandbox: &'a dyn HeuristicSandbox,
        operator: &'a dyn SkillOperator,
    ) -> Self {
        let domain = config.domain;
        Self {
            config,
            oracle,
            scheduling_oracle: None,
            sandbox,
            operator,
            phylogeny: PhylogenyGraph::new(),
            budget: BudgetTracker::default(),
            archive: MapElitesArchive::new_with_domain(10, 10, domain),
            cultural_archive: None,
            lineages: LineageManager::new(),
            adaptive_crossover_rate: 0.60,
            crossover_attempts: 0,
            crossover_not_evaluable_count: 0,
            crossover_rejected_count: 0,
            stagnant_generations: 0,
            local_stats: LocalOpsStats::default(),
            last_pruned_id: None,
            fresh_seed_counter: 0,
            proposals: Vec::new(),
            best_trail: Vec::new(),
            best_proposal_score: None,
        }
    }

    /// Ajoute une proposition evaluee au journal. Purement passif : ne modifie ni la
    /// selection, ni l'archive, ni le budget.
    fn record_proposal(&mut self, capsule: &SkillCapsule, score: &BenchmarkScore, kind: &str) {
        self.record_proposal_checked(capsule, score, kind, true);
    }

    /// Comme `record_proposal`, mais une proposition rejetee par le filtre du moteur
    /// (`accepted == false`) est journalisee sans pouvoir devenir championne : le champion
    /// du journal est toujours un programme que le moteur a garde.
    fn record_proposal_checked(
        &mut self,
        capsule: &SkillCapsule,
        score: &BenchmarkScore,
        kind: &str,
        accepted: bool,
    ) {
        let is_new_best = accepted
            && match &self.best_proposal_score {
                None => true,
                Some(best) => score < best,
            };
        if is_new_best {
            self.best_proposal_score = Some(score.clone());
            self.best_trail.push((self.proposals.len(), capsule.clone()));
        }
        self.proposals.push(ProposalRecord {
            index: self.proposals.len() + 1,
            llm_calls_consumed: self.budget.llm_calls_consumed,
            generation: capsule.generation,
            kind: kind.to_string(),
            capsule_id: capsule.id.0.clone(),
            capsule_name: capsule.name.clone(),
            parent_ids: capsule.parent_ids.iter().map(|p| p.0.clone()).collect(),
            is_valid: score.is_valid,
            total_bins: score.total_bins,
            total_fuel: score.total_fuel,
            is_new_best,
            validation_is_valid: None,
            validation_bins: None,
            logic_code: capsule.logic_code.clone(),
        });
    }

    /// Note sur la validation chaque champion successif du journal. Appele une fois, en
    /// fin de run : la validation n'intervient jamais dans la recherche.
    fn score_best_trail_on_validation(&mut self) {
        let trail = std::mem::take(&mut self.best_trail);
        for (position, capsule) in &trail {
            let score = if self.config.domain == ProblemDomain::MultiResourceScheduling {
                match self.scheduling_oracle {
                    Some(sched) => sched.evaluate_validation(
                        capsule,
                        self.sandbox,
                        self.config.fuel_limit_per_call,
                    ),
                    None => continue,
                }
            } else {
                self.oracle.evaluate_validation(
                    capsule,
                    self.sandbox,
                    self.config.fuel_limit_per_call,
                )
            };
            let record = &mut self.proposals[*position];
            record.validation_is_valid = Some(score.is_valid);
            record.validation_bins = Some(score.total_bins);
        }
        self.best_trail = trail;
    }

    /// Associe un oracle de scheduling multi-ressources au moteur d'évolution.
    pub fn with_scheduling_oracle(mut self, sched_oracle: &'a SchedulingOracle) -> Self {
        // L'archive garde le domaine de la configuration : fournir l'oracle d'ordonnancement
        // ne change pas le probleme traite (les campagnes le fournissent dans tous les cas).
        self.scheduling_oracle = Some(sched_oracle);
        self
    }

    /// Associe une archive culturelle en lecture seule (patrimoine des époques précédentes).
    pub fn with_cultural_archive(mut self, cultural_archive: MapElitesArchive) -> Self {
        self.cultural_archive = Some(cultural_archive);
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
            self.config.group,
            self.config.rng_seed,
            self.config.population_size,
            self.config.max_generations
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
            current_population
                .sort_by(|a, b| a.in_distribution_score.cmp(&b.in_distribution_score));

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
                self.stagnant_generations = 0;
            } else {
                self.stagnant_generations += 1;
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
                crossover_rate: if self.config.group == nomad_core::ExperimentalGroup::GroupD {
                    0.0
                } else {
                    self.adaptive_crossover_rate
                },
                crossover_not_evaluable_count: self.crossover_not_evaluable_count,
                crossover_rejected_count: self.crossover_rejected_count,
            };
            generation_records.push(record);

            if gen == self.config.max_generations {
                break;
            }

            // 2. Selection & Reproduction based on Group Strategy
            current_population =
                self.produce_next_generation(&current_population, gen, &mut rng)?;
        }

        let best_ind = best_overall.unwrap_or_else(|| current_population[0].clone());

        // 3. Independent In-Distribution Validation & Out-Of-Distribution Evaluation of the Best Heuristic
        // Note: Neither validation nor OOD evaluations affect selection, MAP-Elites, or the search budget.
        info!(
            "Evaluating winning capsule {} on validation and OOD benchmarks...",
            best_ind.capsule.id
        );
        let (score_no_lookahead, val_score, ood1_score, ood2_score) =
            if self.config.domain == ProblemDomain::MultiResourceScheduling {
                if let Some(sched) = self.scheduling_oracle {
                    (
                        Some(sched.evaluate_dataset_with_lookahead(
                            &sched.in_distribution,
                            &best_ind.capsule,
                            self.sandbox,
                            self.config.fuel_limit_per_call,
                            false,
                        )),
                        Some(sched.evaluate_validation(
                            &best_ind.capsule,
                            self.sandbox,
                            self.config.fuel_limit_per_call,
                        )),
                        sched.evaluate_dataset(
                            &sched.ood1_scale,
                            &best_ind.capsule,
                            self.sandbox,
                            self.config.fuel_limit_per_call,
                        ),
                        sched.evaluate_dataset(
                            &sched.ood2_skewed,
                            &best_ind.capsule,
                            self.sandbox,
                            self.config.fuel_limit_per_call,
                        ),
                    )
                } else {
                    (
                        None,
                        None,
                        BenchmarkScore::fatal_penalty(50),
                        BenchmarkScore::fatal_penalty(50),
                    )
                }
            } else {
                (
                    Some(self.oracle.evaluate_dataset_with_lookahead(
                        &self.oracle.in_distribution,
                        &best_ind.capsule,
                        self.sandbox,
                        self.config.fuel_limit_per_call,
                        false,
                    )),
                    Some(self.oracle.evaluate_validation(
                        &best_ind.capsule,
                        self.sandbox,
                        self.config.fuel_limit_per_call,
                    )),
                    self.oracle.evaluate_ood1(
                        &best_ind.capsule,
                        self.sandbox,
                        self.config.fuel_limit_per_call,
                    ),
                    self.oracle.evaluate_ood2(
                        &best_ind.capsule,
                        self.sandbox,
                        self.config.fuel_limit_per_call,
                    ),
                )
            };

        self.score_best_trail_on_validation();

        Ok(ExperimentRunReport {
            proposals: self.proposals.clone(),
            local_ops: self.local_stats.clone(),
            group: self.config.group,
            rng_seed: self.config.rng_seed,
            total_generations: generation_records.len(),
            best_in_distribution_score: best_ind.in_distribution_score,
            best_score_without_lookahead: score_no_lookahead,
            best_validation_score: val_score,
            best_ood1_score: ood1_score,
            best_ood2_score: ood2_score,
            best_capsule: best_ind.capsule,
            generations_history: generation_records,
            budget_consumed: self.budget.clone(),
            total_lineage_nodes: self.phylogeny.node_count(),
            total_lineage_edges: self.phylogeny.edge_count(),
            phylogeny: self.phylogeny.clone(),
            archive: self.archive.clone(),
            crossover_attempts: self.crossover_attempts,
            crossover_not_evaluable_count: self.crossover_not_evaluable_count,
            crossover_rejected_count: self.crossover_rejected_count,
        })
    }

    /// Resultats mesures du parent et meilleur programme connu, pour la mutation informee.
    fn mutation_context(&self, parent: &SkillCapsule) -> Option<MutationContext> {
        let elites = self.archive.all_elites();
        let entry = elites.iter().find(|e| e.capsule.id == parent.id)?;
        let best_other = self
            .archive
            .best_elite()
            .filter(|b| b.capsule.id != parent.id && b.fitness.is_valid)
            .map(|b| (b.capsule.logic_code.clone(), b.fitness.total_bins));
        Some(MutationContext {
            parent_valid: entry.fitness.is_valid,
            parent_bins: entry.fitness.total_bins,
            lower_bound_bins: entry.fitness.total_theoretical_min_bins,
            best_other,
        })
    }

    fn initialize_population(
        &mut self,
        rng: &mut ChaCha8Rng,
    ) -> Result<Vec<EvaluatedIndividual>, OrchestratorError> {
        let initial_size = if self.config.initial_population_size > 0 {
            self.config.initial_population_size
        } else {
            self.config.population_size
        };
        self.fresh_seed_counter = initial_size;
        let mut population = Vec::with_capacity(initial_size);

        // Anti-Inbreeding : 100% de nouvelles graines LLM-Seed sont générées à chaque époque
        // pour démarrer sur des fondations fraîches (Tabula Rasa), le patrimoine étant conservé
        // en lecture seule dans l'archive culturelle de référence.
        for i in 0..initial_size {
            let seed = rand::Rng::gen(rng);
            let (capsule, _audit) = self
                .operator
                .generate_seed(i, seed)
                .map_err(|e| OrchestratorError::OperatorError(e.to_string()))?;

            self.budget.record_llm_call();
            self.throttle_cooldown();

            let (capsule, score) = self.evaluate_and_validate(capsule);
            let (capsule, score) = self.maybe_optimize_constants(capsule, score, seed);
            self.phylogeny.insert_capsule(&capsule);
            self.record_proposal(&capsule, &score, "fresh");
            self.lineages.register_seed(capsule.id.clone());
            self.archive.insert(capsule.clone(), score.clone());
            population.push(EvaluatedIndividual::new(capsule, score));
        }

        Ok(population)
    }

    /// Evaluates a candidate capsule and runs the full 3-level Anti-Dead-Code verification pipeline.
    /// Mutualizes the initial evaluation to eliminate redundant passes.
    fn evaluate_and_validate(
        &mut self,
        mut capsule: SkillCapsule,
    ) -> (SkillCapsule, BenchmarkScore) {
        if self.config.domain == ProblemDomain::MultiResourceScheduling {
            if let Some(sched) = self.scheduling_oracle {
                let score = match sched.validate_anti_dead_code_scheduling_with_score(
                    &mut capsule,
                    self.sandbox,
                    self.config.fuel_limit_per_call,
                    self.config.lookahead,
                ) {
                    Ok((_, score)) => score,
                    Err(_) => sched.evaluate_dataset_with_lookahead(
                        &sched.in_distribution,
                        &capsule,
                        self.sandbox,
                        self.config.fuel_limit_per_call,
                        self.config.lookahead,
                    ),
                };
                self.budget
                    .record_executions(sched.in_distribution.instances.len());
                return (capsule, score);
            }
        }

        if capsule.graft_manifest.is_some() {
            // Record ablation probe evaluations for C_null (limited to 5 probe instances)
            let probe_count = 5.min(self.oracle.in_distribution.instances.len());
            self.budget.record_executions(probe_count);
        }

        let score = match validate_anti_dead_code_3levels_with_score(
            &mut capsule,
            self.oracle,
            self.sandbox,
            &self.oracle.in_distribution,
            self.config.fuel_limit_per_call,
            self.config.tldr_epsilon,
            self.config.lookahead,
        ) {
            Ok((_, score)) => score,
            Err(_) => self.oracle.evaluate_dataset_with_lookahead(
                &self.oracle.in_distribution,
                &capsule,
                self.sandbox,
                self.config.fuel_limit_per_call,
                self.config.lookahead,
            ),
        };
        self.budget
            .record_executions(self.oracle.in_distribution.instances.len());

        capsule.metadata.insert("mean_efficiency".to_string(), format!("{:.4}", score.mean_efficiency));
        capsule.metadata.insert("tldr_score".to_string(), format!("{:.4}", score.tldr_score));
        if let Some(ref tm) = score.trajectory_metrics {
            capsule.metadata.insert("beta1_slope".to_string(), format!("{:.4}", tm.beta1_slope));
            capsule.metadata.insert("sigma_res".to_string(), format!("{:.4}", tm.sigma_res));
        }

        (capsule, score)
    }

    /// Optional symmetric local hill-climbing optimization of AST constants based on relative theoretical minimum.
    fn maybe_optimize_constants(
        &mut self,
        mut capsule: SkillCapsule,
        mut score: BenchmarkScore,
        seed: u64,
    ) -> (SkillCapsule, BenchmarkScore) {
        if self.config.enable_constant_optimization
            && self.config.domain == ProblemDomain::BinPacking1D
            && score.is_valid
            && score.total_excess_bins <= self.config.constant_opt_excess_threshold
        {
            let (opt_cap, opt_score) = nomad_evaluator::optimize_capsule_constants(
                &capsule,
                self.oracle,
                self.sandbox,
                self.config.fuel_limit_per_call,
                12,
                seed,
            );
            self.budget.record_executions(12);
            capsule = opt_cap;
            score = opt_score;
        }
        (capsule, score)
    }

    /// Note un programme sur l'entrainement, comme une proposition du modele.
    fn train_score(&mut self, capsule: &SkillCapsule) -> BenchmarkScore {
        self.local_stats.evaluations += 1;
        self.oracle.evaluate_dataset_with_lookahead(
            &self.oracle.in_distribution,
            capsule,
            self.sandbox,
            self.config.fuel_limit_per_call,
            self.config.lookahead,
        )
    }

    /// Elites de l'archive, du meilleur au moins bon (ordre deterministe).
    fn ranked_elites(&self) -> Vec<(SkillCapsule, BenchmarkScore)> {
        let mut ranked: Vec<(&String, &crate::map_elites::ArchiveEntry)> =
            self.archive.grid.iter().collect();
        ranked.sort_by(|a, b| a.1.fitness.cmp(&b.1.fitness).then_with(|| a.0.cmp(b.0)));
        ranked
            .into_iter()
            .map(|(_, e)| (e.capsule.clone(), e.fitness.clone()))
            .collect()
    }

    /// Un croisement AST local : hote choisi par la regle de selection des parents, donneur
    /// tire uniformement parmi les autres elites. L'enfant n'est garde que s'il bat son hote
    /// sur l'entrainement : une greffe qui ameliore l'hote n'est pas du code mort, le
    /// controle par ablation est donc inutile ici.
    fn local_crossover(&mut self, generation: usize, rng: &mut ChaCha8Rng) {
        let ranked = self.ranked_elites();
        if ranked.len() < 2 {
            return;
        }
        let Some(host) = self.archive.select_parent(self.config.parent_selection, rng) else {
            return;
        };
        let Some(host_score) = ranked
            .iter()
            .find(|(c, _)| c.id == host.id)
            .map(|(_, s)| s.clone())
        else {
            return;
        };
        let donors: Vec<&SkillCapsule> = ranked
            .iter()
            .map(|(c, _)| c)
            .filter(|c| c.id != host.id && c.logic_code != host.logic_code)
            .collect();
        if donors.is_empty() {
            return;
        }
        let donor = donors[rand::Rng::gen_range(rng, 0..donors.len())].clone();
        let seed: u64 = rand::Rng::gen(rng);
        self.local_stats.crossover_attempts += 1;

        let res = if seed % 2 == 0 {
            multi_locus_ast_crossover(&host, &donor, seed, generation)
                .or_else(|_| deterministic_ast_crossover(&host, &donor, seed, generation))
        } else {
            deterministic_ast_crossover(&host, &donor, seed, generation)
                .or_else(|_| multi_locus_ast_crossover(&host, &donor, seed, generation))
        };
        let Ok((mut child, _)) = res else {
            self.local_stats.crossover_errors += 1;
            return;
        };
        // L'enfant est juge comme les autres propositions, sans controle d'ablation.
        child.graft_manifest = None;
        let score = self.train_score(&child);
        let accepted = score.is_valid && score < host_score;
        self.phylogeny.insert_capsule(&child);
        self.record_proposal_checked(&child, &score, "crossover", accepted);
        if accepted {
            self.local_stats.crossover_accepted += 1;
            self.lineages.record_evolution(
                child.id.clone(),
                &host.id,
                Some(donor.id.clone()),
                child.semantic_interface.clone(),
                true,
            );
            self.archive.insert(child, score);
        }
    }

    /// Elague le meilleur elite s'il est nouveau : chaque instruction (sauf l'expression
    /// rendue) est retiree a son tour, du bas vers le haut, et le retrait est garde si le
    /// programme reste valide sans perdre de bac. La version elaguee est inseree si elle est
    /// strictement meilleure que l'original (meme nombre de bacs : moins de fuel).
    fn prune_best_elite(&mut self, generation: usize) {
        let Some((best, best_score)) = self.ranked_elites().into_iter().next() else {
            return;
        };
        if self.last_pruned_id.as_deref() == Some(best.id.0.as_str()) {
            return;
        }
        self.last_pruned_id = Some(best.id.0.clone());
        let Ok(original) = parse_heuristic_fn(&best.logic_code) else {
            return;
        };
        let n = original.block.stmts.len();
        if n < 2 {
            return;
        }
        self.local_stats.prune_attempts += 1;
        let build = |keep: &[bool]| {
            let mut f = original.clone();
            f.block.stmts = original
                .block
                .stmts
                .iter()
                .zip(keep)
                .filter(|(_, k)| **k)
                .map(|(s, _)| s.clone())
                .collect();
            normalize_source(&f)
        };
        let mut keep = vec![true; n];
        let mut current = best_score.clone();
        let mut removed = 0usize;
        for i in (0..n - 1).rev() {
            keep[i] = false;
            let mut trial = best.clone();
            trial.logic_code = build(&keep);
            let s = self.train_score(&trial);
            if s.is_valid && s.total_bins <= current.total_bins {
                current = s;
                removed += 1;
            } else {
                keep[i] = true;
            }
        }
        if removed == 0 || !(current < best_score) {
            return;
        }
        let mut pruned = SkillCapsule::new(
            &format!("Pruned-G{}-{}", generation, &best.id.0[..best.id.0.len().min(8)]),
            &format!("Elagage de {} : {} instruction(s) retiree(s)", best.id, removed),
            &build(&keep),
            vec![best.id.clone()],
            generation,
            best.metadata.clone(),
        );
        pruned.ast_fingerprint = parse_heuristic_fn(&pruned.logic_code)
            .ok()
            .map(|f| nomad_genome::compute_structural_fingerprint(&f));
        self.local_stats.prune_accepted += 1;
        self.local_stats.rules_removed += removed;
        self.phylogeny.insert_capsule(&pruned);
        self.record_proposal_checked(&pruned, &current, "prune", true);
        self.lineages.record_evolution(
            pruned.id.clone(),
            &best.id,
            None,
            pruned.semantic_interface.clone(),
            true,
        );
        self.last_pruned_id = Some(pruned.id.0.clone());
        self.archive.insert(pruned, current);
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
                    let lineage = self.lineages.get_lineage(&current_best.capsule.id);
                    let (mutated, _) = self
                        .operator
                        .mutate_with_lineage(&current_best.capsule, next_gen_idx, seed, lineage)
                        .map_err(|e| OrchestratorError::OperatorError(e.to_string()))?;

                    self.budget.record_llm_call();
                    self.throttle_cooldown();

                    let (mutated, score) = self.evaluate_and_validate(mutated);
                    let (mutated, score) = self.maybe_optimize_constants(mutated, score, seed);
                    self.phylogeny.insert_capsule(&mutated);
                    self.record_proposal(&mutated, &score, "mutation");
                    next_population.push(EvaluatedIndividual::new(mutated, score));
                }
            }

            ExperimentalGroup::GroupB => {
                // Group B: Independent parallel agents without horizontal transmission
                for ind in current_pop {
                    if next_population.len() >= pop_size
                        || !self.budget.can_invoke_llm(&self.config)
                    {
                        break;
                    }
                    let seed = rand::Rng::gen(rng);
                    let lineage = self.lineages.get_lineage(&ind.capsule.id);
                    let (mutated, _) = self
                        .operator
                        .mutate_with_lineage(&ind.capsule, next_gen_idx, seed, lineage)
                        .map_err(|e| OrchestratorError::OperatorError(e.to_string()))?;

                    self.budget.record_llm_call();
                    self.throttle_cooldown();

                    let (mutated, score) = self.evaluate_and_validate(mutated);
                    let (mutated, score) = self.maybe_optimize_constants(mutated, score, seed);
                    self.phylogeny.insert_capsule(&mutated);
                    self.record_proposal(&mutated, &score, "mutation");
                    next_population.push(EvaluatedIndividual::new(mutated, score));
                }
            }

            ExperimentalGroup::GroupD => {
                // Group D: Evolutionary selection with MAP-Elites niche selection, MUTATION ONLY
                let elite = self
                    .archive
                    .best_elite()
                    .map(|e| e.capsule.clone())
                    .unwrap_or_else(|| current_pop[0].capsule.clone());
                let elite_score = self
                    .archive
                    .best_elite()
                    .map(|e| e.fitness.clone())
                    .unwrap_or_else(|| current_pop[0].in_distribution_score.clone());
                next_population.push(EvaluatedIndividual::new(elite, elite_score));

                // Relance : apres `stagnation_patience` generations sans progres, une
                // proposition sur deux est une proposition fraiche au lieu d'une mutation.
                let inject_fresh = self.config.stagnation_patience > 0
                    && self.stagnant_generations >= self.config.stagnation_patience;
                let mut slot = 0usize;

                while next_population.len() < pop_size && self.budget.can_invoke_llm(&self.config) {
                    slot += 1;
                    if inject_fresh && slot % 2 == 0 {
                        let seed = rand::Rng::gen(rng);
                        let index = self.fresh_seed_counter;
                        self.fresh_seed_counter += 1;
                        let (fresh, _) = self
                            .operator
                            .generate_seed(index, seed)
                            .map_err(|e| OrchestratorError::OperatorError(e.to_string()))?;

                        self.budget.record_llm_call();
                        self.throttle_cooldown();

                        let (fresh, score) = self.evaluate_and_validate(fresh);
                        let (fresh, score) = self.maybe_optimize_constants(fresh, score, seed);
                        self.phylogeny.insert_capsule(&fresh);
                        self.record_proposal(&fresh, &score, "fresh");
                        self.lineages.register_seed(fresh.id.clone());
                        self.archive.insert(fresh.clone(), score.clone());
                        next_population.push(EvaluatedIndividual::new(fresh, score));
                        continue;
                    }

                    let parent = self
                        .archive
                        .select_parent(self.config.parent_selection, rng)
                        .unwrap_or_else(|| current_pop[0].capsule.clone());
                    let seed = rand::Rng::gen(rng);
                    let context = if self.config.informed_mutation {
                        self.mutation_context(&parent)
                    } else {
                        None
                    };
                    let lineage = self.lineages.get_lineage(&parent.id);

                    let (mutated, _) = self
                        .operator
                        .mutate_informed(&parent, next_gen_idx, seed, lineage, context.as_ref())
                        .map_err(|e| OrchestratorError::OperatorError(e.to_string()))?;

                    self.budget.record_llm_call();
                    self.throttle_cooldown();

                    let (mutated, score) = self.evaluate_and_validate(mutated);
                    let (mutated, score) = self.maybe_optimize_constants(mutated, score, seed);
                    self.phylogeny.insert_capsule(&mutated);
                    self.record_proposal(&mutated, &score, "mutation");

                    let is_improved = if let Some(elite) = self.archive.best_elite() {
                        score < elite.fitness
                    } else {
                        true
                    };
                    self.lineages.record_evolution(
                        mutated.id.clone(),
                        &parent.id,
                        None,
                        mutated.semantic_interface.clone(),
                        is_improved && score.is_valid,
                    );

                    self.archive.insert(mutated.clone(), score.clone());
                    next_population.push(EvaluatedIndividual::new(mutated, score));
                }

                // Operateurs locaux, sans appel au modele ni effet sur le budget d'appels.
                for _ in 0..self.config.local_crossovers {
                    self.local_crossover(next_gen_idx, rng);
                }
                if self.config.prune_champion {
                    self.prune_best_elite(next_gen_idx);
                }
            }

            ExperimentalGroup::GroupC => {
                // Group C (NOMAD V2 Swarm): Quality-Diversity MAP-Elites + Broad Pool Elitist Competition
                // Parité stricte du budget LLM avec le Groupe D + Quota de croisements AST gratuits (pop_size / 2)
                let mut candidate_pool: Vec<EvaluatedIndividual> = Vec::with_capacity(pop_size * 2);

                let elite = self
                    .archive
                    .best_elite()
                    .map(|e| EvaluatedIndividual::new(e.capsule.clone(), e.fitness.clone()))
                    .unwrap_or_else(|| current_pop[0].clone());
                candidate_pool.push(elite);

                // 1. Quota de mutations LLM (pop_size - 1 mutations pour parité stricte avec le Groupe D)
                let target_mutations = pop_size.saturating_sub(1);
                let mut mutations_done = 0;
                while mutations_done < target_mutations && self.budget.can_invoke_llm(&self.config) {
                    mutations_done += 1;
                    let parent = self
                        .archive
                        .select_parent(self.config.parent_selection, rng)
                        .unwrap_or_else(|| current_pop[0].capsule.clone());
                    let seed = rand::Rng::gen(rng);
                    let lineage = self.lineages.get_lineage(&parent.id);

                    let (mutated, _) = self
                        .operator
                        .mutate_with_lineage(&parent, next_gen_idx, seed, lineage)
                        .map_err(|e| OrchestratorError::OperatorError(e.to_string()))?;

                    self.budget.record_llm_call();
                    self.throttle_cooldown();

                    let (mutated, score) = self.evaluate_and_validate(mutated);
                    let (mutated, score) = self.maybe_optimize_constants(mutated, score, seed);
                    self.phylogeny.insert_capsule(&mutated);

                    let is_validation_passed = match &mutated.validation {
                        nomad_core::ValidationStatus::PassedLevel3Behavioral
                        | nomad_core::ValidationStatus::PassedLevel2Computational
                        | nomad_core::ValidationStatus::PassedLevel1Structural => true,
                        nomad_core::ValidationStatus::NotEvaluable(r)
                        | nomad_core::ValidationStatus::GraftHarmful(r)
                        | nomad_core::ValidationStatus::Failed(r) => {
                            tracing::warn!("Rejet mutation de {} : {}", mutated.name, r);
                            false
                        }
                        nomad_core::ValidationStatus::Unvalidated => true,
                    };
                    self.record_proposal_checked(
                        &mutated,
                        &score,
                        "mutation",
                        is_validation_passed && score.is_valid,
                    );

                    let is_improved = if let Some(best) = self.archive.best_elite() {
                        score < best.fitness
                    } else {
                        true
                    };

                    self.lineages.record_evolution(
                        mutated.id.clone(),
                        &parent.id,
                        None,
                        mutated.semantic_interface.clone(),
                        is_improved && is_validation_passed && score.is_valid,
                    );

                    if is_validation_passed && score.is_valid {
                        self.archive.insert(mutated.clone(), score.clone());
                        candidate_pool.push(EvaluatedIndividual::new(mutated, score));
                    }
                }

                // 2. Quota de croisements AST déterministes gratuits (pop_size / 2) entre partenaires complémentaires
                let crossover_quota = (pop_size / 2).max(1);
                let mut ancestral_usage_counts: std::collections::HashMap<String, usize> =
                    std::collections::HashMap::new();
                let max_ancestral_usage = ((pop_size as f64) * 0.30).ceil().max(1.0) as usize;

                for _ in 0..crossover_quota {
                    let seed = rand::Rng::gen(rng);
                    // 2.1 Parent A selection via UCB curiosity
                    let parent_a = self
                        .archive
                        .select_parent(self.config.parent_selection, rng)
                        .unwrap_or_else(|| current_pop[0].capsule.clone());

                    let parent_a_score = self
                        .archive
                        .grid
                        .values()
                        .find(|e| e.capsule.id == parent_a.id)
                        .map(|e| e.fitness.clone())
                        .unwrap_or_else(|| current_pop[0].in_distribution_score.clone());

                    let parent_a_hash = parent_a.ast_fingerprint.as_ref().map(|f| &f.ast_hash);

                    // 2.2 Parent B selection
                    let mut chosen_b = None;
                    if let Some(ref mut cultural) = self.cultural_archive {
                        if rand::Rng::gen_bool(rng, 0.40) {
                            if let Some(cand) = cultural.select_complementary_partner(
                                &parent_a,
                                &parent_a_score,
                                rng,
                            ) {
                                let cand_hash = cand.ast_fingerprint.as_ref().map(|f| &f.ast_hash);
                                let usage = ancestral_usage_counts
                                    .get(&cand.id.0)
                                    .copied()
                                    .unwrap_or(0);
                                if usage < max_ancestral_usage
                                    && cand.id != parent_a.id
                                    && parent_a_hash != cand_hash
                                {
                                    chosen_b = Some(cand);
                                }
                            }
                        }
                    }

                    if chosen_b.is_none() && self.archive.occupied_niches_count() >= 2 {
                        if let Some(cand) = self.archive.select_complementary_partner(
                            &parent_a,
                            &parent_a_score,
                            rng,
                        ) {
                            let cand_hash = cand.ast_fingerprint.as_ref().map(|f| &f.ast_hash);
                            let usage = ancestral_usage_counts
                                .get(&cand.id.0)
                                .copied()
                                .unwrap_or(0);
                            if usage < max_ancestral_usage
                                && cand.id != parent_a.id
                                && parent_a_hash != cand_hash
                            {
                                chosen_b = Some(cand);
                            }
                        }
                    }

                    if chosen_b.is_none() {
                        for ind in current_pop {
                            let ind_hash =
                                ind.capsule.ast_fingerprint.as_ref().map(|f| &f.ast_hash);
                            let usage = ancestral_usage_counts
                                .get(&ind.capsule.id.0)
                                .copied()
                                .unwrap_or(0);
                            if usage < max_ancestral_usage
                                && ind.capsule.id != parent_a.id
                                && parent_a_hash != ind_hash
                            {
                                chosen_b = Some(ind.capsule.clone());
                                break;
                            }
                        }
                    }

                    if let Some(parent_b) = chosen_b {
                        let p_b_hash = parent_b.ast_fingerprint.as_ref().map(|f| &f.ast_hash);
                        if parent_a.id != parent_b.id && (parent_a_hash.is_none() || parent_a_hash != p_b_hash) {
                            self.crossover_attempts += 1;
                            *ancestral_usage_counts
                                .entry(parent_b.id.0.clone())
                                .or_insert(0) += 1;

                            let (child_capsule, audit) = self
                                .operator
                                .crossover(&parent_a, &parent_b, next_gen_idx, seed)
                                .map_err(|e| OrchestratorError::OperatorError(e.to_string()))?;

                            let is_ast_crossover = !audit.is_network_call || child_capsule.graft_manifest.is_some();
                            if !is_ast_crossover {
                                if !self.budget.can_invoke_llm(&self.config) {
                                    continue;
                                }
                                self.budget.record_llm_call();
                                self.throttle_cooldown();
                            } else {
                                tracing::debug!(
                                    "Local AST crossover for child {} executed with zero LLM budget consumption",
                                    child_capsule.name
                                );
                            }

                            let (child_capsule, score) = self.evaluate_and_validate(child_capsule);
                            let (child_capsule, score) =
                                self.maybe_optimize_constants(child_capsule, score, seed);
                            self.phylogeny.insert_capsule(&child_capsule);

                            // Validation bloquante Anti-Dead-Code pour le crossover (Niveau 3 C_null obligatoire)
                            let is_crossover_passed = matches!(
                                &child_capsule.validation,
                                nomad_core::ValidationStatus::PassedLevel3Behavioral
                            );
                            self.record_proposal_checked(
                                &child_capsule,
                                &score,
                                "crossover",
                                is_crossover_passed && score.is_valid,
                            );

                            if !is_crossover_passed {
                                match &child_capsule.validation {
                                    nomad_core::ValidationStatus::NotEvaluable(reason) => {
                                        self.crossover_not_evaluable_count += 1;
                                        self.crossover_rejected_count += 1;
                                        tracing::info!(
                                            "Crossover child {} marked NotEvaluable: {}",
                                            child_capsule.name,
                                            reason
                                        );
                                    }
                                    _ => {
                                        self.crossover_rejected_count += 1;
                                    }
                                }
                            }

                            let is_improved = if let Some(elite) = self.archive.best_elite() {
                                score < elite.fitness
                            } else {
                                true
                            };

                            let host_id = child_capsule
                                .parent_ids
                                .first()
                                .cloned()
                                .unwrap_or_else(|| child_capsule.id.clone());
                            let partner_id = child_capsule.parent_ids.get(1).cloned();

                            self.lineages.record_evolution(
                                child_capsule.id.clone(),
                                &host_id,
                                partner_id,
                                child_capsule.semantic_interface.clone(),
                                is_improved && is_crossover_passed,
                            );

                            if is_improved && is_crossover_passed {
                                self.adaptive_crossover_rate =
                                    (self.adaptive_crossover_rate + 0.02).min(0.85);
                            } else {
                                self.adaptive_crossover_rate =
                                    (self.adaptive_crossover_rate - 0.01).max(0.25);
                            }

                            if is_crossover_passed && score.is_valid {
                                self.archive.insert(child_capsule.clone(), score.clone());
                                candidate_pool.push(EvaluatedIndividual::new(child_capsule, score));
                            }
                        }
                    }
                }

                // 3. Compétition Élitiste par Pool Élargi :
                // Trie tout le pool selon l'ordre canonique BenchmarkScore (validité, bins, tldr, efficacité, fuel)
                // puis départage par concision du code source en cas d'égalité absolue.
                candidate_pool.sort_by(|a, b| {
                    a.in_distribution_score
                        .cmp(&b.in_distribution_score)
                        .then_with(|| {
                            a.capsule.logic_code.len().cmp(&b.capsule.logic_code.len())
                        })
                });

                let mut seen_ids = std::collections::HashSet::new();
                for ind in candidate_pool {
                    if seen_ids.insert(ind.capsule.id.clone()) {
                        next_population.push(ind);
                        if next_population.len() >= pop_size {
                            break;
                        }
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use nomad_operator::DeterministicMockOperator;
    use nomad_sandbox::DeterministicSandbox;

    /// Une proposition rejetee par le filtre du moteur est journalisee mais ne devient
    /// jamais championne, meme si son score est le meilleur rencontre.
    #[test]
    fn rejected_proposal_never_becomes_champion() {
        let oracle = OracleEvaluator::new(42);
        let sandbox = DeterministicSandbox::new();
        let operator = DeterministicMockOperator::default();
        let mut engine =
            EvolutionEngine::new(ExperimentConfig::default(), &oracle, &sandbox, &operator);
        let (capsule, _) = operator.generate_seed(0, 1).unwrap();
        let score = oracle.evaluate_in_distribution(&capsule, &sandbox, 400_000);

        engine.record_proposal_checked(&capsule, &score, "crossover", false);
        assert_eq!(engine.proposals.len(), 1);
        assert!(!engine.proposals[0].is_new_best);
        assert!(engine.best_trail.is_empty());
        assert!(engine.best_proposal_score.is_none());

        engine.record_proposal_checked(&capsule, &score, "crossover", true);
        assert!(engine.proposals[1].is_new_best);
        assert_eq!(engine.best_trail.len(), 1);
        assert_eq!(engine.best_trail[0].0, 1);
    }

    /// L'elagage retire une regle sans effet, garde la regle utile, et la version elaguee
    /// devient le meilleur elite.
    #[test]
    fn pruning_removes_a_dead_rule_and_keeps_the_useful_one() {
        let oracle = OracleEvaluator::new(42);
        let sandbox = DeterministicSandbox::new();
        let operator = DeterministicMockOperator::default();
        let config = ExperimentConfig {
            group: nomad_core::ExperimentalGroup::GroupD,
            fuel_limit_per_call: 400_000,
            ..Default::default()
        };
        let mut engine = EvolutionEngine::new(config, &oracle, &sandbox, &operator);
        let code = "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32,             step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {             let mut score = (bin_residual - item_size) as f64;             if bin_residual > 1000 { score += 5.0; }             score }";
        let capsule = SkillCapsule::new("Best", "d", code, vec![], 1, Default::default());
        let score = engine.train_score(&capsule);
        assert!(score.is_valid);
        engine.archive.insert(capsule, score.clone());

        engine.prune_best_elite(2);
        assert_eq!(engine.local_stats.prune_accepted, 1);
        assert_eq!(engine.local_stats.rules_removed, 1);
        let (best, best_score) = engine.ranked_elites().into_iter().next().unwrap();
        assert!(!best.logic_code.contains("1000"), "regle morte non retiree");
        assert!(best.logic_code.contains("let mut score"), "regle utile retiree");
        assert_eq!(best_score.total_bins, score.total_bins);
        assert!(best_score < score);

        // Le meme champion n'est pas elague deux fois.
        engine.prune_best_elite(3);
        assert_eq!(engine.local_stats.prune_attempts, 1);
    }
}
