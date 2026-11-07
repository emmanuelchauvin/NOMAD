use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use nomad_core::{ExperimentConfig, ExperimentalGroup};
use nomad_evaluator::{Dataset, OracleEvaluator};
use nomad_operator::DeterministicMockOperator;
use nomad_orchestrator::EvolutionEngine;
use nomad_sandbox::DeterministicSandbox;

#[derive(Parser)]
#[command(name = "nomad")]
#[command(author = "NOMAD Research Team")]
#[command(version = "0.1.0")]
#[command(about = "NOMAD (DARWIN.RS): Swarm Intelligence Evolutionary Framework", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
#[value(rename_all = "UPPERCASE")]
enum GroupArg {
    #[value(alias = "a")]
    A,
    #[value(alias = "b")]
    B,
    #[value(alias = "c")]
    C,
    #[value(alias = "d")]
    D,
}

impl From<GroupArg> for ExperimentalGroup {
    fn from(arg: GroupArg) -> Self {
        match arg {
            GroupArg::A => ExperimentalGroup::GroupA,
            GroupArg::B => ExperimentalGroup::GroupB,
            GroupArg::C => ExperimentalGroup::GroupC,
            GroupArg::D => ExperimentalGroup::GroupD,
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Display system positioning, hypotheses H1/H2, and pre-registered protocol.
    Info,

    /// Generate and inspect cryptographic SHA-256 fingerprints of benchmark datasets.
    Dataset {
        #[arg(short, long, default_value_t = 42)]
        seed: u64,
    },

    /// Run classical algorithmic baselines (First-Fit, Best-Fit, Worst-Fit).
    Baseline {
        #[arg(short, long, default_value_t = 42)]
        seed: u64,
    },

    /// Run an evolutionary trial for a specific group.
    Run {
        #[arg(short = 'G', long, value_enum, default_value_t = GroupArg::C)]
        group: GroupArg,

        #[arg(short, long, default_value_t = 42)]
        seed: u64,

        #[arg(short, long, default_value_t = 20)]
        pop_size: usize,

        #[arg(short = 'n', long, default_value_t = 10)]
        generations: usize,

        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run pre-registered comparative experiment between Group C (Swarm) and Group D (Mutation only).
    Compare {
        #[arg(short, long, default_value_t = 5)]
        runs: usize,

        #[arg(short, long, default_value_t = 20)]
        pop_size: usize,

        #[arg(short = 'n', long, default_value_t = 10)]
        generations: usize,

        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Info => {
            println!("============================================================");
            println!("    NOMAD (DARWIN.RS) : Decentralized Cognitive Swarm");
            println!("============================================================");
            println!("Hypothesis H1 : Swarm Crossover achieves superior in-distribution heuristic packing.");
            println!("Hypothesis H2 : Crossover solutions generalize better Out-Of-Distribution (OOD-1, OOD-2).");
            println!();
            println!("Target Domain : Online 1D Bin Packing (C = 100)");
            println!("Lineage Graph : Directed Acyclic Graph (DAG) with full ancestry tracking.");
            println!("Reproduction  : Mutation + Swarm Crossover (Group C) vs Mutation only (Group D).");
            println!("Oracle        : Independent, strict gas & validity constraints.");
            println!("============================================================");
        }

        Commands::Dataset { seed } => {
            let in_dist = Dataset::generate_in_distribution(seed);
            let ood1 = Dataset::generate_ood1_scale(seed);
            let ood2 = Dataset::generate_ood2_distribution_shift(seed);

            println!("--- Pre-Registered Benchmark Datasets (Seed: {}) ---", seed);
            println!("In-Distribution (50 instances, N=50,  U(10,80)) :");
            println!("  SHA-256: {}", in_dist.dataset_hash);
            println!("OOD-1 Scale     (50 instances, N=500, U(10,80)) :");
            println!("  SHA-256: {}", ood1.dataset_hash);
            println!("OOD-2 Shift     (50 instances, N=500, Power-Law):");
            println!("  SHA-256: {}", ood2.dataset_hash);
        }

        Commands::Baseline { seed } => {
            let oracle = OracleEvaluator::new(seed);
            println!("--- Classical Heuristic Benchmarks (Dataset Seed: {}) ---", seed);

            for name in &["FIRST_FIT", "BEST_FIT", "WORST_FIT"] {
                let in_score = oracle.evaluate_classical_baseline(name, &oracle.in_distribution);
                let ood1_score = oracle.evaluate_classical_baseline(name, &oracle.ood1_scale);
                let ood2_score = oracle.evaluate_classical_baseline(name, &oracle.ood2_powerlaw);

                println!("\n[Heuristic: {}]", name);
                println!(
                    "  In-Dist : {} bins (excess: +{}), efficiency: {:.2}%, fuel: {}",
                    in_score.total_bins,
                    in_score.total_excess_bins,
                    in_score.mean_efficiency * 100.0,
                    in_score.total_fuel
                );
                println!(
                    "  OOD-1   : {} bins (excess: +{}), efficiency: {:.2}%, fuel: {}",
                    ood1_score.total_bins,
                    ood1_score.total_excess_bins,
                    ood1_score.mean_efficiency * 100.0,
                    ood1_score.total_fuel
                );
                println!(
                    "  OOD-2   : {} bins (excess: +{}), efficiency: {:.2}%, fuel: {}",
                    ood2_score.total_bins,
                    ood2_score.total_excess_bins,
                    ood2_score.mean_efficiency * 100.0,
                    ood2_score.total_fuel
                );
            }
        }

        Commands::Run {
            group,
            seed,
            pop_size,
            generations,
            output,
        } => {
            let exp_group: ExperimentalGroup = group.into();
            let mut config = ExperimentConfig::default();
            config.group = exp_group;
            config.rng_seed = seed;
            config.population_size = pop_size;
            config.max_generations = generations;

            let oracle = OracleEvaluator::new(seed);
            let sandbox = DeterministicSandbox::new();
            let operator = DeterministicMockOperator::default();

            let mut engine = EvolutionEngine::new(config, &oracle, &sandbox, &operator);
            let report = engine.execute_run()?;

            println!("\n============================================================");
            println!(" Run Completed: {}", report.group.name());
            println!(" Seed: {} | Generations: {}", report.rng_seed, report.total_generations);
            println!("------------------------------------------------------------");
            println!(
                " Best In-Dist Bins: {} (excess: +{}), Efficiency: {:.2}%, Fuel: {}",
                report.best_in_distribution_score.total_bins,
                report.best_in_distribution_score.total_excess_bins,
                report.best_in_distribution_score.mean_efficiency * 100.0,
                report.best_in_distribution_score.total_fuel
            );
            println!(
                " OOD-1 Score      : {} bins (excess: +{}), Efficiency: {:.2}%",
                report.best_ood1_score.total_bins,
                report.best_ood1_score.total_excess_bins,
                report.best_ood1_score.mean_efficiency * 100.0
            );
            println!(
                " OOD-2 Score      : {} bins (excess: +{}), Efficiency: {:.2}%",
                report.best_ood2_score.total_bins,
                report.best_ood2_score.total_excess_bins,
                report.best_ood2_score.mean_efficiency * 100.0
            );
            println!(" Lineage Nodes    : {}, Transitions: {}", report.total_lineage_nodes, report.total_lineage_edges);
            println!(" Best Heuristic   : {}", report.best_capsule.name);
            println!(" Logic Code       : {}", report.best_capsule.logic_code);
            println!("============================================================");

            if let Some(path) = output {
                let serialized = serde_json::to_string_pretty(&report)?;
                let mut file = File::create(&path)?;
                file.write_all(serialized.as_bytes())?;
                println!("Report written to {}", path.display());
            }
        }

        Commands::Compare {
            runs,
            pop_size,
            generations,
            output,
        } => {
            println!("============================================================");
            println!(" Running Paired Study: Group C (Swarm) vs Group D (Mutation)");
            println!(" Number of Paired Runs: {}", runs);
            println!("============================================================");

            let mut paired_results = Vec::new();

            for r in 0..runs {
                let seed = 1000 + (r as u64) * 7;
                let oracle = OracleEvaluator::new(seed);
                let sandbox = DeterministicSandbox::new();
                let operator = DeterministicMockOperator::default();

                // Group D run
                let mut config_d = ExperimentConfig::default();
                config_d.group = ExperimentalGroup::GroupD;
                config_d.rng_seed = seed;
                config_d.population_size = pop_size;
                config_d.max_generations = generations;
                let mut engine_d = EvolutionEngine::new(config_d, &oracle, &sandbox, &operator);
                let report_d = engine_d.execute_run()?;

                // Group C run
                let mut config_c = ExperimentConfig::default();
                config_c.group = ExperimentalGroup::GroupC;
                config_c.rng_seed = seed;
                config_c.population_size = pop_size;
                config_c.max_generations = generations;
                let mut engine_c = EvolutionEngine::new(config_c, &oracle, &sandbox, &operator);
                let report_c = engine_c.execute_run()?;

                println!(
                    "Pair #{:02} (Seed {}): Group D Bins = {} vs Group C Bins = {} (Δ = {:+})",
                    r + 1,
                    seed,
                    report_d.best_in_distribution_score.total_bins,
                    report_c.best_in_distribution_score.total_bins,
                    (report_c.best_in_distribution_score.total_bins as i64)
                        - (report_d.best_in_distribution_score.total_bins as i64)
                );

                paired_results.push((report_c, report_d));
            }

            if let Some(path) = output {
                let serialized = serde_json::to_string_pretty(&paired_results)?;
                let mut file = File::create(&path)?;
                file.write_all(serialized.as_bytes())?;
                println!("Paired comparison written to {}", path.display());
            }
        }
    }

    Ok(())
}
