#![allow(clippy::field_reassign_with_default)]

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use nomad_core::{ExperimentConfig, ExperimentalGroup, ProblemDomain};
use nomad_evaluator::{Dataset, OracleEvaluator, SchedulingDataset, SchedulingOracle};
use nomad_operator::{DeterministicMockOperator, HttpLlamaOperator, SkillOperator};
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

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
#[value(rename_all = "kebab-case")]
enum DomainArg {
    BinPacking,
    Scheduling,
}

impl From<DomainArg> for ProblemDomain {
    fn from(arg: DomainArg) -> Self {
        match arg {
            DomainArg::BinPacking => ProblemDomain::BinPacking1D,
            DomainArg::Scheduling => ProblemDomain::MultiResourceScheduling,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
#[value(rename_all = "lowercase")]
enum OperatorArg {
    Mock,
    Ollama,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
#[value(rename_all = "lowercase")]
enum GraphFormat {
    Mermaid,
    Dot,
    Html,
}

#[derive(Subcommand)]
enum Commands {
    /// Display system positioning, hypotheses H1/H2, and Ollama connection status.
    Info,

    /// Inspect Quality-Diversity MAP-Elites niches from a run report.
    Archive {
        /// Path to the JSON run report file (defaults to run_report.json).
        #[arg(short, long, default_value = "run_report.json")]
        input: PathBuf,
    },

    /// Export or visualize the phylogenetic DAG from a run report file.
    Graph {
        /// Path to the JSON run report file produced by `nomad run --output <file>`.
        #[arg(
            short,
            long,
            visible_alias = "report",
            default_value = "run_report.json"
        )]
        input: PathBuf,

        /// Format for visualization: mermaid, dot, or html.
        #[arg(short, long, value_enum, default_value_t = GraphFormat::Mermaid)]
        format: GraphFormat,

        /// Optional output file path (if omitted, prints to stdout).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Generate and inspect cryptographic SHA-256 fingerprints of benchmark datasets.
    Dataset {
        #[arg(short, long, default_value_t = 42)]
        seed: u64,

        #[arg(short, long, value_enum, default_value_t = DomainArg::BinPacking)]
        domain: DomainArg,
    },

    /// Evaluate a heuristic source file (bin packing `score_bin`) on all datasets.
    Eval {
        #[arg(short, long, default_value_t = 42)]
        seed: u64,

        /// Path to a Rust source file containing `pub fn score_bin(...) -> f64`.
        #[arg(long)]
        code: PathBuf,

        #[arg(long, default_value_t = 50_000)]
        fuel: u64,
    },

    /// Run classical algorithmic baselines.
    Baseline {
        #[arg(short, long, default_value_t = 42)]
        seed: u64,

        #[arg(short, long, value_enum, default_value_t = DomainArg::BinPacking)]
        domain: DomainArg,
    },

    /// Run the "Mort et Renaissance" protocol: Epoch 1 -> Extinction -> Epoch 2 (Heritage vs Tabula Rasa).
    Rebirth {
        #[arg(short, long, value_enum, default_value_t = DomainArg::BinPacking)]
        domain: DomainArg,

        #[arg(short, long, default_value_t = 42)]
        seed: u64,

        #[arg(short, long, default_value_t = 6)]
        pop_size: usize,

        #[arg(short = 'n', long, default_value_t = 3)]
        generations: usize,

        /// Number of successive civilizational epochs (2 or 3).
        #[arg(short = 'e', long, default_value_t = 2)]
        epochs: usize,

        /// Toggle lookahead (k=2) evaluation (pass --lookahead=false to disable lookahead).
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        lookahead: bool,

        #[arg(long, value_enum, default_value_t = OperatorArg::Mock)]
        operator: OperatorArg,

        #[arg(long, default_value = "http://localhost:11434/v1/chat/completions")]
        ollama_url: String,

        #[arg(long, default_value = "qwen2.5-coder:7b")]
        model: String,

        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run an evolutionary trial for a specific group.
    Run {
        #[arg(short, long, value_enum, default_value_t = DomainArg::BinPacking)]
        domain: DomainArg,

        #[arg(short = 'G', long, value_enum, default_value_t = GroupArg::C)]
        group: GroupArg,

        #[arg(short, long, default_value_t = 42)]
        seed: u64,

        #[arg(short, long, default_value_t = 10)]
        pop_size: usize,

        #[arg(short = 'n', long, default_value_t = 5)]
        generations: usize,

        #[arg(long, value_enum, default_value_t = OperatorArg::Ollama)]
        operator: OperatorArg,

        #[arg(long, default_value = "http://localhost:11434/v1/chat/completions")]
        ollama_url: String,

        #[arg(long, default_value = "qwen2.5-coder:7b")]
        model: String,

        /// Optional cooldown in milliseconds between LLM calls to prevent GPU overheating.
        #[arg(long, default_value_t = 0)]
        cooldown_ms: u64,

        /// Toggle lookahead (k=2) evaluation (pass --lookahead=false to disable lookahead).
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        lookahead: bool,

        /// Enable local AST constant hill-climbing optimization (default: false).
        #[arg(long, default_value_t = false)]
        enable_constant_opt: bool,

        #[arg(short, long, default_value = "run_report.json")]
        output: PathBuf,
    },

    /// Run pre-registered comparative experiment between Group C (Swarm) and Group D (Mutation only).
    Compare {
        #[arg(short, long, value_enum, default_value_t = DomainArg::BinPacking)]
        domain: DomainArg,

        #[arg(short, long, default_value_t = 5)]
        runs: usize,

        #[arg(short, long, default_value_t = 10)]
        pop_size: usize,

        #[arg(short = 'n', long, default_value_t = 5)]
        generations: usize,

        /// Toggle lookahead (k=2) evaluation (pass --lookahead=false to disable lookahead).
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        lookahead: bool,

        /// Enable local AST constant hill-climbing optimization (default: false).
        #[arg(long, default_value_t = false)]
        enable_constant_opt: bool,

        #[arg(long, value_enum, default_value_t = OperatorArg::Ollama)]
        operator: OperatorArg,

        #[arg(long, default_value = "http://localhost:11434/v1/chat/completions")]
        ollama_url: String,

        #[arg(long, default_value = "qwen2.5-coder:7b")]
        model: String,

        /// Optional cooldown in milliseconds between LLM calls to prevent GPU overheating.
        #[arg(long, default_value_t = 0)]
        cooldown_ms: u64,

        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run full 30-seed pre-registered statistical campaign (Group C vs Group D).
    Campaign {
        #[arg(short, long, value_enum, default_value_t = DomainArg::BinPacking)]
        domain: DomainArg,

        /// Number of paired seeds in the campaign (pre-registered protocol: 30).
        #[arg(short, long, default_value_t = 30)]
        seeds: usize,

        #[arg(short, long, default_value_t = 10)]
        pop_size: usize,

        #[arg(short = 'n', long, default_value_t = 5)]
        generations: usize,

        /// Toggle lookahead (k=2) evaluation (pass --lookahead=false to disable lookahead).
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        lookahead: bool,

        /// Enable local AST constant hill-climbing optimization (default: false).
        #[arg(long, default_value_t = false)]
        enable_constant_opt: bool,

        #[arg(long, value_enum, default_value_t = OperatorArg::Ollama)]
        operator: OperatorArg,

        #[arg(long, default_value = "http://localhost:11434/v1/chat/completions")]
        ollama_url: String,

        #[arg(long, default_value = "qwen2.5-coder:7b")]
        model: String,

        /// Optional cooldown in milliseconds between LLM calls to prevent GPU overheating.
        #[arg(long, default_value_t = 0)]
        cooldown_ms: u64,

        /// Output directory for individual runs and aggregate JSON report.
        #[arg(short, long, default_value = "campaign_results")]
        output_dir: PathBuf,
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
            println!(
                "Reproduction  : Mutation + Swarm Crossover (Group C) vs Mutation only (Group D)."
            );
            println!("Oracle        : Independent, strict gas & validity constraints.");
            println!("============================================================");
        }

        Commands::Archive { input } => {
            if !input.exists() {
                eprintln!(
                    "Rapport introuvable: {}. Lancez d'abord une simulation via:\n  cargo run --release --bin nomad -- run --operator ollama\n(ou fournissez le chemin via --input <fichier.json>)",
                    input.display()
                );
                return Ok(());
            }
            let content = std::fs::read_to_string(&input)?;
            let report: nomad_orchestrator::ExperimentRunReport = serde_json::from_str(&content)
                .or_else(|_| {
                    serde_json::from_str::<nomad_orchestrator::RebirthExperimentReport>(&content)
                        .map(|r| r.epoch3_heritage.unwrap_or(r.epoch2_heritage))
                })
                .map_err(|e| {
                    anyhow::anyhow!("Could not parse report from {}: {}", input.display(), e)
                })?;

            println!("============================================================");
            println!(" MAP-Elites Archive Inspection : {}", report.group.name());
            println!(
                " Total Niches Defined : {} x {}",
                report.archive.dim_x, report.archive.dim_y
            );
            println!(
                " Active Occupied Niches: {} ({:.1}% Coverage)",
                report.archive.occupied_niches_count(),
                report.archive.coverage() * 100.0
            );
            println!(" QD-Score : {:.3}", report.archive.qd_score());
            println!("------------------------------------------------------------");
            println!(
                "{:<10} {:<14} {:<14} {:<12} {:<18} Name",
                "Niche", "DeadSpace/Imb", "NewBin/ResVar", "UnitsUsed", "Capsule ID"
            );
            println!("------------------------------------------------------------");

            let mut entries: Vec<_> = report.archive.grid.values().collect();
            entries.sort_by_key(|e| e.niche_coord);

            for e in entries {
                let y_val = if e.behavior.residual_variance > 0.0 {
                    e.behavior.residual_variance
                } else {
                    e.behavior.new_bin_ratio
                };
                println!(
                    "({:>2},{:>2})     {:<14.3} {:<14.3} {:<12} {:<18} {}",
                    e.niche_coord.0,
                    e.niche_coord.1,
                    e.behavior.dead_space_ratio,
                    y_val,
                    e.fitness.total_bins,
                    e.capsule.id.0,
                    e.capsule.name
                );
            }
            println!("============================================================");
        }

        Commands::Graph {
            input,
            format,
            output,
        } => {
            let content = std::fs::read_to_string(&input)?;
            let report: nomad_orchestrator::ExperimentRunReport = serde_json::from_str(&content)
                .or_else(|_| {
                    serde_json::from_str::<nomad_orchestrator::RebirthExperimentReport>(&content)
                        .map(|r| r.epoch3_heritage.unwrap_or(r.epoch2_heritage))
                })
                .map_err(|e| {
                    anyhow::anyhow!("Could not parse report from {}: {}", input.display(), e)
                })?;

            if report.phylogeny.node_count() == 0 {
                eprintln!(
                    "Warning: The report in {} does not contain any recorded phylogenetic nodes.",
                    input.display()
                );
                return Ok(());
            }

            let rendered = match format {
                GraphFormat::Mermaid => report.phylogeny.to_mermaid(),
                GraphFormat::Dot => report.phylogeny.to_dot(),
                GraphFormat::Html => {
                    let mermaid_code = report.phylogeny.to_mermaid();
                    format!(
                        r#"<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>NOMAD Phylogeny Tree - {0}</title>
  <script src="https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.min.js"></script>
  <style>
    * {{
      box-sizing: border-box;
      margin: 0;
      padding: 0;
    }}
    body {{
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      background: #0b0f19;
      color: #f8fafc;
      overflow: hidden;
      height: 100vh;
      display: flex;
      flex-direction: column;
    }}
    header {{
      background: #111827;
      border-bottom: 1px solid #1f2937;
      padding: 12px 24px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      flex-shrink: 0;
      z-index: 10;
    }}
    .title-group h1 {{
      font-size: 1.15rem;
      font-weight: 700;
      color: #38bdf8;
      display: flex;
      align-items: center;
      gap: 8px;
    }}
    .meta-pills {{
      display: flex;
      gap: 10px;
      margin-top: 4px;
      font-size: 0.8rem;
      color: #94a3b8;
    }}
    .meta-pills span {{
      background: #1f2937;
      padding: 2px 8px;
      border-radius: 4px;
      border: 1px solid #374151;
    }}
    .meta-pills strong {{
      color: #e2e8f0;
    }}
    .toolbar {{
      display: flex;
      align-items: center;
      gap: 12px;
      flex-wrap: wrap;
    }}
    .btn-group {{
      display: flex;
      background: #1f2937;
      border-radius: 8px;
      padding: 2px;
      border: 1px solid #374151;
    }}
    button {{
      background: transparent;
      border: none;
      color: #f1f5f9;
      padding: 6px 12px;
      font-size: 0.85rem;
      font-weight: 500;
      border-radius: 6px;
      cursor: pointer;
      display: flex;
      align-items: center;
      gap: 6px;
      transition: background 0.15s, color 0.15s;
    }}
    button:hover {{
      background: #374151;
      color: #38bdf8;
    }}
    .zoom-display {{
      font-family: monospace;
      font-size: 0.85rem;
      color: #94a3b8;
      min-width: 52px;
      text-align: center;
      display: flex;
      align-items: center;
      justify-content: center;
    }}
    .legend {{
      display: flex;
      gap: 12px;
      font-size: 0.8rem;
      align-items: center;
      background: #1e293b;
      padding: 4px 12px;
      border-radius: 6px;
      border: 1px solid #334155;
    }}
    .legend-item {{
      display: flex;
      align-items: center;
      gap: 6px;
    }}
    .legend-dot {{
      width: 10px;
      height: 10px;
      border-radius: 50%;
    }}
    .dot-root {{ background: #3b82f6; border: 1px solid #1d4ed8; }}
    .dot-mutation {{ background: #10b981; border: 1px solid #047857; }}
    .dot-crossover {{ background: #8b5cf6; border: 1px solid #6d28d9; }}
    
    #viewport {{
      flex: 1;
      position: relative;
      overflow: hidden;
      background: radial-gradient(#1e293b 1px, transparent 1px);
      background-size: 24px 24px;
      cursor: grab;
      user-select: none;
    }}
    #viewport:active {{
      cursor: grabbing;
    }}
    #canvas {{
      position: absolute;
      top: 0;
      left: 0;
      transform-origin: 0 0;
      will-change: transform;
    }}
    /* UNCONSTRAINED SVG RENDERING: Explicit natural dimensions ensure no squashing */
    #canvas svg {{
      display: block;
      user-select: none;
    }}
    .hint {{
      position: absolute;
      bottom: 16px;
      right: 16px;
      background: rgba(15, 23, 42, 0.85);
      border: 1px solid #334155;
      padding: 6px 14px;
      border-radius: 20px;
      font-size: 0.75rem;
      color: #94a3b8;
      backdrop-filter: blur(4px);
      pointer-events: none;
    }}
    #loading {{
      position: absolute;
      top: 50%;
      left: 50%;
      transform: translate(-50%, -50%);
      font-size: 1.1rem;
      color: #38bdf8;
      font-weight: 500;
      display: flex;
      align-items: center;
      gap: 10px;
    }}
  </style>
</head>
<body>
  <header>
    <div class="title-group">
      <h1>🧬 NOMAD Phylogenetic Lineage DAG</h1>
      <div class="meta-pills">
        <span>Groupe : <strong>{1}</strong></span>
        <span>Seed : <strong>{2}</strong></span>
        <span>Nœuds : <strong>{3}</strong></span>
        <span>Transitions : <strong>{4}</strong></span>
        <span>Champion : <strong>{0}</strong></span>
      </div>
    </div>

    <div class="toolbar">
      <div class="legend">
        <div class="legend-item"><div class="legend-dot dot-root"></div> Racine (Gen 0)</div>
        <div class="legend-item"><div class="legend-dot dot-mutation"></div> Mutation</div>
        <div class="legend-item"><div class="legend-dot dot-crossover"></div> Swarm Crossover</div>
      </div>

      <div class="btn-group">
        <button id="btn-dir" title="Basculer entre l'orientation de haut en bas (TD) et de gauche à droite (LR)">⇄ <span id="dir-label">Orientation : LR</span></button>
      </div>

      <div class="btn-group">
        <button id="btn-zoom-out" title="Zoom Arrière (-)">−</button>
        <div class="zoom-display" id="zoom-val">100%</div>
        <button id="btn-zoom-in" title="Zoom Avant (+)">+</button>
      </div>

      <div class="btn-group">
        <button id="btn-fit" title="Ajuster à la taille de l'écran">⛶ Ajuster</button>
        <button id="btn-reset" title="Réinitialiser l'échelle à 100%">⟲ 1:1</button>
      </div>
    </div>
  </header>

  <div id="viewport">
    <div id="loading">Chargement et tracé du graphe génétique...</div>
    <div id="canvas"></div>
    <div class="hint">💡 Molette souris pour Zoomer • Cliquer-Glisser pour Déplacer l'arbre</div>
  </div>

  <script>
    const rawMermaidTemplate = `{5}`;
    let currentDirection = "LR"; // Default to Left-to-Right for optimal phylogenetic tree readability

    let scale = 1.0;
    let panX = 40;
    let panY = 40;
    let isDragging = false;
    let startX = 0, startY = 0;

    const viewport = document.getElementById('viewport');
    const canvas = document.getElementById('canvas');
    const loading = document.getElementById('loading');
    const zoomVal = document.getElementById('zoom-val');
    const dirLabel = document.getElementById('dir-label');

    function updateTransform() {{
      canvas.style.transform = `translate(${{panX}}px, ${{panY}}px) scale(${{scale}})`;
      zoomVal.textContent = `${{Math.round(scale * 100)}}%`;
    }}

    viewport.addEventListener('mousedown', (e) => {{
      if (e.button !== 0) return;
      isDragging = true;
      startX = e.clientX - panX;
      startY = e.clientY - panY;
    }});

    window.addEventListener('mousemove', (e) => {{
      if (!isDragging) return;
      panX = e.clientX - startX;
      panY = e.clientY - startY;
      updateTransform();
    }});

    window.addEventListener('mouseup', () => {{
      isDragging = false;
    }});

    viewport.addEventListener('wheel', (e) => {{
      e.preventDefault();
      const rect = viewport.getBoundingClientRect();
      const mouseX = e.clientX - rect.left;
      const mouseY = e.clientY - rect.top;

      const zoomFactor = e.deltaY < 0 ? 1.15 : 0.87;
      const newScale = Math.min(Math.max(scale * zoomFactor, 0.05), 8.0);

      panX = mouseX - (mouseX - panX) * (newScale / scale);
      panY = mouseY - (mouseY - panY) * (newScale / scale);
      scale = newScale;

      updateTransform();
    }}, {{ passive: false }});

    document.getElementById('btn-zoom-in').addEventListener('click', () => {{
      const rect = viewport.getBoundingClientRect();
      const centerX = rect.width / 2;
      const centerY = rect.height / 2;
      const newScale = Math.min(scale * 1.25, 8.0);
      panX = centerX - (centerX - panX) * (newScale / scale);
      panY = centerY - (centerY - panY) * (newScale / scale);
      scale = newScale;
      updateTransform();
    }});

    document.getElementById('btn-zoom-out').addEventListener('click', () => {{
      const rect = viewport.getBoundingClientRect();
      const centerX = rect.width / 2;
      const centerY = rect.height / 2;
      const newScale = Math.max(scale / 1.25, 0.05);
      panX = centerX - (centerX - panX) * (newScale / scale);
      panY = centerY - (centerY - panY) * (newScale / scale);
      scale = newScale;
      updateTransform();
    }});

    document.getElementById('btn-reset').addEventListener('click', () => {{
      scale = 1.0;
      panX = 60;
      panY = 60;
      updateTransform();
    }});

    function fitGraph() {{
      const svg = canvas.querySelector('svg');
      if (!svg) return;
      
      const vb = svg.viewBox ? svg.viewBox.baseVal : null;
      const naturalWidth = (vb && vb.width > 0) ? vb.width : (parseFloat(svg.getAttribute('width')) || 2000);
      const naturalHeight = (vb && vb.height > 0) ? vb.height : (parseFloat(svg.getAttribute('height')) || 1500);

      const vRect = viewport.getBoundingClientRect();
      const pad = 60;
      const availW = Math.max(vRect.width - pad * 2, 200);
      const availH = Math.max(vRect.height - pad * 2, 200);

      const scaleX = availW / naturalWidth;
      const scaleY = availH / naturalHeight;
      scale = Math.min(Math.max(Math.min(scaleX, scaleY), 0.05), 1.0);

      panX = Math.round((vRect.width - naturalWidth * scale) / 2);
      panY = Math.round((vRect.height - naturalHeight * scale) / 2);
      updateTransform();
    }}

    document.getElementById('btn-fit').addEventListener('click', fitGraph);

    document.getElementById('btn-dir').addEventListener('click', () => {{
      currentDirection = (currentDirection === "LR") ? "TD" : "LR";
      dirLabel.textContent = `Orientation : ${{currentDirection}}`;
      renderGraph();
    }});

    async function renderGraph() {{
      loading.style.display = 'flex';
      let code = rawMermaidTemplate;
      if (currentDirection === "LR") {{
        code = code.replace(/^graph TD/m, 'graph LR');
      }} else {{
        code = code.replace(/^graph LR/m, 'graph TD');
      }}

      try {{
        mermaid.initialize({{
          startOnLoad: false,
          theme: 'dark',
          securityLevel: 'loose',
          flowchart: {{
            curve: 'basis',
            nodeSpacing: 45,
            rankSpacing: 60,
            useMaxWidth: false,
            htmlLabels: true
          }}
        }});
        
        canvas.innerHTML = '';
        const id = 'svg-' + Date.now();
        const {{ svg }} = await mermaid.render(id, code);
        canvas.innerHTML = svg;

        // Force SVG to keep its exact unconstrained natural layout dimensions
        const svgEl = canvas.querySelector('svg');
        if (svgEl) {{
          const vb = svgEl.viewBox ? svgEl.viewBox.baseVal : null;
          const naturalW = (vb && vb.width > 0) ? vb.width : (parseFloat(svgEl.getAttribute('width')) || 2500);
          const naturalH = (vb && vb.height > 0) ? vb.height : (parseFloat(svgEl.getAttribute('height')) || 2000);

          svgEl.removeAttribute('style');
          svgEl.setAttribute('width', naturalW);
          svgEl.setAttribute('height', naturalH);
          svgEl.style.width = naturalW + 'px';
          svgEl.style.height = naturalH + 'px';
          svgEl.style.maxWidth = 'none';
          svgEl.style.display = 'block';
        }}

        loading.style.display = 'none';
        fitGraph();
      }} catch (err) {{
        loading.textContent = 'Erreur lors du rendu du graphe: ' + err.message;
        console.error(err);
      }}
    }}

    // Initial render
    renderGraph();
  </script>
</body>
</html>"#,
                        report.best_capsule.name,
                        report.group.name(),
                        report.rng_seed,
                        report.total_lineage_nodes,
                        report.total_lineage_edges,
                        mermaid_code
                    )
                }
            };

            if let Some(out_path) = output {
                std::fs::write(&out_path, rendered.as_bytes())?;
                println!("Phylogeny graph written to {}", out_path.display());
            } else {
                println!("{}", rendered);
            }
        }

        Commands::Dataset { seed, domain } => {
            if domain == DomainArg::Scheduling {
                let in_dist = SchedulingDataset::generate_in_distribution(seed);
                let ood1 = SchedulingDataset::generate_ood_scale(seed);
                let ood2 = SchedulingDataset::generate_ood_skewed(seed);

                println!(
                    "--- Pre-Registered Multi-Resource Scheduling Datasets (Seed: {}) ---",
                    seed
                );
                println!("In-Distribution (50 uniform + 25 moderate-skew instances, N=50 jobs, Node: 32C/128G) :");
                println!("  SHA-256: {}", in_dist.dataset_hash);
                println!("OOD-1 Scale     (50 instances, N=300 jobs, Node: 32C/128G) :");
                println!("  SHA-256: {}", ood1.dataset_hash);
                println!("OOD-2 Skewed    (50 instances, N=100 jobs, Asymmetric)    :");
                println!("  SHA-256: {}", ood2.dataset_hash);
            } else {
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
        }

        Commands::Eval { seed, code, fuel } => {
            let sandbox = DeterministicSandbox::new();
            let oracle = OracleEvaluator::new(seed);
            let source = std::fs::read_to_string(&code)?;
            let capsule = nomad_core::SkillCapsule::new(
                "eval-candidate",
                "Externally supplied heuristic",
                source,
                vec![],
                0,
                std::collections::HashMap::new(),
            );
            println!("--- Heuristic evaluation (Dataset Seed: {}) ---", seed);
            for (label, ds) in [
                ("In-Dist", &oracle.in_distribution),
                ("Valid  ", &oracle.validation),
                ("OOD-1  ", &oracle.ood1_scale),
                ("OOD-2  ", &oracle.ood2_powerlaw),
            ] {
                let sc = oracle.evaluate_dataset(ds, &capsule, &sandbox, fuel);
                let bf = oracle.evaluate_classical_baseline("BEST_FIT", ds);
                println!(
                    "  {} : valid={} {} bins (excess: +{}), Best-Fit: {} (excess: +{}), fuel: {}",
                    label,
                    sc.is_valid,
                    sc.total_bins,
                    sc.total_excess_bins,
                    bf.total_bins,
                    bf.total_excess_bins,
                    sc.total_fuel
                );
            }
        }

        Commands::Baseline { seed, domain } => {
            let sandbox = DeterministicSandbox::new();
            if domain == DomainArg::Scheduling {
                let sched_oracle = SchedulingOracle::new(seed);
                println!(
                    "--- Multi-Resource Scheduling Baselines (Dataset Seed: {}) ---",
                    seed
                );

                for name in &[
                    "FIRST_FIT_NODE",
                    "BEST_FIT_CPU",
                    "BEST_FIT_RAM",
                    "BALANCED_FIT",
                ] {
                    let in_score = sched_oracle.evaluate_classical_baseline(
                        name,
                        &sched_oracle.in_distribution,
                        &sandbox,
                    );
                    let ood1_score = sched_oracle.evaluate_classical_baseline(
                        name,
                        &sched_oracle.ood1_scale,
                        &sandbox,
                    );
                    let ood2_score = sched_oracle.evaluate_classical_baseline(
                        name,
                        &sched_oracle.ood2_skewed,
                        &sandbox,
                    );

                    println!("\n[Heuristic: {}]", name);
                    println!(
                        "  In-Dist : {} nodes (excess: +{}), efficiency: {:.2}%, fuel: {}",
                        in_score.total_bins,
                        in_score.total_excess_bins,
                        in_score.mean_efficiency * 100.0,
                        in_score.total_fuel
                    );
                    println!(
                        "  OOD-1   : {} nodes (excess: +{}), efficiency: {:.2}%, fuel: {}",
                        ood1_score.total_bins,
                        ood1_score.total_excess_bins,
                        ood1_score.mean_efficiency * 100.0,
                        ood1_score.total_fuel
                    );
                    println!(
                        "  OOD-2   : {} nodes (excess: +{}), efficiency: {:.2}%, fuel: {}",
                        ood2_score.total_bins,
                        ood2_score.total_excess_bins,
                        ood2_score.mean_efficiency * 100.0,
                        ood2_score.total_fuel
                    );
                }
            } else {
                let oracle = OracleEvaluator::new(seed);
                println!(
                    "--- Classical Heuristic Benchmarks (Dataset Seed: {}) ---",
                    seed
                );

                for name in &["FIRST_FIT", "BEST_FIT", "BEST_FIT_LOOKAHEAD", "WORST_FIT"] {
                    let in_score =
                        oracle.evaluate_classical_baseline(name, &oracle.in_distribution);
                    let ood1_score = oracle.evaluate_classical_baseline(name, &oracle.ood1_scale);
                    let ood2_score =
                        oracle.evaluate_classical_baseline(name, &oracle.ood2_powerlaw);

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
        }

        Commands::Rebirth {
            domain,
            seed,
            pop_size,
            generations,
            epochs,
            lookahead,
            operator,
            ollama_url,
            model,
            output,
        } => {
            let prob_domain: ProblemDomain = domain.into();
            let mut config = ExperimentConfig::default();
            config.domain = prob_domain;
            config.rng_seed = seed;
            config.population_size = pop_size;
            config.max_generations = generations;
            config.lookahead = lookahead;

            let oracle = OracleEvaluator::new(seed);
            let sched_oracle = SchedulingOracle::new(seed);
            let sandbox = DeterministicSandbox::new();

            let op_instance: Box<dyn SkillOperator> = match operator {
                OperatorArg::Mock => {
                    let mut mock = DeterministicMockOperator::default();
                    mock.domain = prob_domain;
                    Box::new(mock)
                }
                OperatorArg::Ollama => Box::new(
                    HttpLlamaOperator::new(
                        ollama_url,
                        model,
                        "dae161e27b0e90dd1856c8bb3209201fd6736d8eb66298e75ed87571486f4364",
                        "Ollama API (Qwen 2.5 Coder 7B)",
                        0.7,
                        0.95,
                    )
                    .with_domain(prob_domain),
                ),
            };

            let rebirth_report = nomad_orchestrator::execute_rebirth_protocol_extended(
                config,
                &oracle,
                Some(&sched_oracle),
                &sandbox,
                op_instance.as_ref(),
                epochs,
            )?;

            println!("\n============================================================");
            println!(" Protocol: MORT ET RENAISSANCE (Section 7.3 & 8.5)");
            println!("============================================================");
            println!(
                " Époque 1 (Genèse)     : Best Bins: {} (eff: {:.2}%), Niches: {}",
                rebirth_report
                    .epoch1_genesis
                    .best_in_distribution_score
                    .total_bins,
                rebirth_report
                    .epoch1_genesis
                    .best_in_distribution_score
                    .mean_efficiency
                    * 100.0,
                rebirth_report.heritage.distinct_niches_count
            );
            println!(
                " Patrimoine Transmis   : {} capsules validées, {} motifs distincts",
                rebirth_report.heritage.validated_capsules.len(),
                rebirth_report.heritage.distinct_motifs_count
            );
            println!("------------------------------------------------------------");
            println!(
                " Époque 2 (Héritière)  : Best Bins: {} (excess: +{}), Eff: {:.2}%",
                rebirth_report
                    .epoch2_heritage
                    .best_in_distribution_score
                    .total_bins,
                rebirth_report
                    .epoch2_heritage
                    .best_in_distribution_score
                    .total_excess_bins,
                rebirth_report
                    .epoch2_heritage
                    .best_in_distribution_score
                    .mean_efficiency
                    * 100.0
            );
            println!(
                " Époque 2 (Tabula Rasa): Best Bins: {} (excess: +{}), Eff: {:.2}%",
                rebirth_report
                    .epoch2_tabula_rasa
                    .best_in_distribution_score
                    .total_bins,
                rebirth_report
                    .epoch2_tabula_rasa
                    .best_in_distribution_score
                    .total_excess_bins,
                rebirth_report
                    .epoch2_tabula_rasa
                    .best_in_distribution_score
                    .mean_efficiency
                    * 100.0
            );
            if let Some(ref ep3) = rebirth_report.epoch3_heritage {
                println!(
                    " Époque 3 (Héritière 2): Best Bins: {} (excess: +{}), Eff: {:.2}%",
                    ep3.best_in_distribution_score.total_bins,
                    ep3.best_in_distribution_score.total_excess_bins,
                    ep3.best_in_distribution_score.mean_efficiency * 100.0
                );
            }
            println!("------------------------------------------------------------");
            if let Some(cif_p) = rebirth_report.cumulative_metrics.cif_performance {
                println!(" CIF Performance (Excess ratio)       : {:.3}x", cif_p);
            } else {
                println!(
                    " CIF Performance (Excess ratio)       : Undefined (Zero excess achieved)"
                );
            }
            println!(
                " CIF Quality-Diversity (QD ratio)     : {:.3}x",
                rebirth_report.cumulative_metrics.cif_qd
            );
            println!(
                " Functional Heritability Rate (R_herit): {:.1}%",
                rebirth_report
                    .cumulative_metrics
                    .functional_heritability_rate
                    * 100.0
            );
            println!(
                " Innovation Rate (R_innov)             : {:.3} per 1000 evals",
                rebirth_report.cumulative_metrics.innovation_rate
            );
            println!("------------------------------------------------------------");
            println!(" Budget Breakdown (Executions consumed):");
            println!(
                "  - Époque 1 (Genèse)                  : {} execs, {} LLM calls",
                rebirth_report
                    .budget_breakdown
                    .budget_patrimony_epoch1
                    .executions_consumed,
                rebirth_report
                    .budget_breakdown
                    .budget_patrimony_epoch1
                    .llm_calls_consumed
            );
            println!(
                "  - Réévaluation du Patrimoine Hérité  : {} execs, {} LLM calls",
                rebirth_report
                    .budget_breakdown
                    .budget_reevaluation_heritage
                    .executions_consumed,
                rebirth_report
                    .budget_breakdown
                    .budget_reevaluation_heritage
                    .llm_calls_consumed
            );
            println!(
                "  - Recherche Lignée Héritière         : {} execs, {} LLM calls",
                rebirth_report
                    .budget_breakdown
                    .budget_search_heritage
                    .executions_consumed,
                rebirth_report
                    .budget_breakdown
                    .budget_search_heritage
                    .llm_calls_consumed
            );
            println!(
                "  - Recherche Lignée Tabula Rasa       : {} execs, {} LLM calls",
                rebirth_report
                    .budget_breakdown
                    .budget_search_tabula_rasa
                    .executions_consumed,
                rebirth_report
                    .budget_breakdown
                    .budget_search_tabula_rasa
                    .llm_calls_consumed
            );
            println!("------------------------------------------------------------");
            println!(" Marginal Epoch Analysis:");
            for rep in &rebirth_report.epoch_marginal_reports {
                println!(
                    "  Époque {} -> Bins Gain: {:+} | Excess Gain: {:+} | QD Gain: {:+.3}",
                    rep.epoch, rep.bins_gain, rep.excess_gain, rep.qd_gain
                );
            }
            println!("============================================================");

            if let Some(path) = output {
                let serialized = serde_json::to_string_pretty(&rebirth_report)?;
                let mut file = File::create(&path)?;
                file.write_all(serialized.as_bytes())?;
                println!("Rebirth report written to {}", path.display());
            }
        }

        Commands::Run {
            domain,
            group,
            seed,
            pop_size,
            generations,
            operator,
            ollama_url,
            model,
            cooldown_ms,
            lookahead,
            enable_constant_opt,
            output,
        } => {
            let exp_group: ExperimentalGroup = group.into();
            let prob_domain: ProblemDomain = domain.into();
            let mut config = ExperimentConfig::default();
            config.domain = prob_domain;
            config.group = exp_group;
            config.rng_seed = seed;
            config.population_size = pop_size;
            config.max_generations = generations;
            config.cooldown_ms = cooldown_ms;
            config.lookahead = lookahead;
            config.enable_constant_optimization = enable_constant_opt;

            let oracle = OracleEvaluator::new(seed);
            let sched_oracle = SchedulingOracle::new(seed);
            let sandbox = DeterministicSandbox::new();

            let op_instance: Box<dyn SkillOperator> = match operator {
                OperatorArg::Mock => {
                    let mut mock = DeterministicMockOperator::default();
                    mock.domain = prob_domain;
                    Box::new(mock)
                }
                OperatorArg::Ollama => Box::new(
                    HttpLlamaOperator::new(
                        ollama_url,
                        model,
                        "dae161e27b0e90dd1856c8bb3209201fd6736d8eb66298e75ed87571486f4364", // SHA256 digest of qwen2.5-coder:7b
                        "Ollama API (Qwen 2.5 Coder 7B)",
                        0.7,
                        0.95,
                    )
                    .with_domain(prob_domain),
                ),
            };

            let mut engine = EvolutionEngine::new(config, &oracle, &sandbox, op_instance.as_ref())
                .with_scheduling_oracle(&sched_oracle);
            let report = engine.execute_run()?;

            println!("\n============================================================");
            println!(" Run Completed: {}", report.group.name());
            println!(
                " Seed: {} | Generations: {}",
                report.rng_seed, report.total_generations
            );
            println!("------------------------------------------------------------");
            println!(
                " Best In-Dist (w/ Lookahead)  : {} bins (excess: +{}), Efficiency: {:.2}%, Fuel: {}",
                report.best_in_distribution_score.total_bins,
                report.best_in_distribution_score.total_excess_bins,
                report.best_in_distribution_score.mean_efficiency * 100.0,
                report.best_in_distribution_score.total_fuel
            );
            if let Some(ref no_la) = report.best_score_without_lookahead {
                println!(
                    " Best In-Dist (w/o Lookahead) : {} bins (excess: +{}), Efficiency: {:.2}%",
                    no_la.total_bins,
                    no_la.total_excess_bins,
                    no_la.mean_efficiency * 100.0
                );
            }
            println!(
                " Dynamic Trajectory: tLDR = {:.4}",
                report.best_in_distribution_score.tldr_score
            );
            println!(
                " Behavior Profile : DeadSpaceRatio: {:.1}%, NewBinRatio: {:.1}%, ResVar: {:.2}",
                report.best_in_distribution_score.behavior.dead_space_ratio * 100.0,
                report.best_in_distribution_score.behavior.new_bin_ratio * 100.0,
                report.best_in_distribution_score.behavior.residual_variance
            );
            println!(
                " MAP-Elites Niches: {} active ({:.1}% coverage), QD-Score: {:.3}",
                report.archive.occupied_niches_count(),
                report.archive.coverage() * 100.0,
                report.archive.qd_score()
            );
            if let Some(ref val_score) = report.best_validation_score {
                println!(
                    " Validation Score : {} bins (excess: +{}), Efficiency: {:.2}%",
                    val_score.total_bins,
                    val_score.total_excess_bins,
                    val_score.mean_efficiency * 100.0
                );
            }
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
            println!(
                " Lineage Nodes    : {}, Transitions: {}",
                report.total_lineage_nodes, report.total_lineage_edges
            );
            println!(" Best Heuristic   : {}", report.best_capsule.name);
            println!(" Logic Code       : {}", report.best_capsule.logic_code);
            println!("============================================================");

            let serialized = serde_json::to_string_pretty(&report)?;
            let mut file = File::create(&output)?;
            file.write_all(serialized.as_bytes())?;
            println!("Report written to {}", output.display());
        }

        Commands::Compare {
            domain,
            runs,
            pop_size,
            generations,
            lookahead,
            enable_constant_opt,
            operator,
            ollama_url,
            model,
            cooldown_ms,
            output,
        } => {
            let prob_domain: ProblemDomain = domain.into();
            println!("============================================================");
            println!(" Running Paired Study: Group C (Swarm) vs Group D (Mutation)");
            println!(" Target Domain        : {:?}", prob_domain);
            println!(" Lookahead (k=2)      : {}", lookahead);
            println!(" Constant Opt Hill-Cl : {}", enable_constant_opt);
            println!(" Number of Paired Runs: {}", runs);
            if cooldown_ms > 0 {
                println!(" Thermal Cooldown: {} ms between LLM calls", cooldown_ms);
            }
            println!("============================================================");

            let op_instance: Box<dyn SkillOperator> = match operator {
                OperatorArg::Mock => {
                    let mut mock = DeterministicMockOperator::default();
                    mock.domain = prob_domain;
                    Box::new(mock)
                }
                OperatorArg::Ollama => Box::new(
                    HttpLlamaOperator::new(
                        ollama_url,
                        model,
                        "dae161e27b0e90dd1856c8bb3209201fd6736d8eb66298e75ed87571486f4364",
                        "Ollama API (Qwen 2.5 Coder 7B)",
                        0.7,
                        0.95,
                    )
                    .with_domain(prob_domain),
                ),
            };

            let mut paired_results = Vec::new();

            for r in 0..runs {
                let seed = 1000 + (r as u64) * 7;
                let oracle = OracleEvaluator::new(seed);
                let sched_oracle = SchedulingOracle::new(seed);
                let sandbox = DeterministicSandbox::new();

                // Group D run
                let mut config_d = ExperimentConfig::default();
                config_d.domain = prob_domain;
                config_d.group = ExperimentalGroup::GroupD;
                config_d.rng_seed = seed;
                config_d.population_size = pop_size;
                config_d.max_generations = generations;
                config_d.cooldown_ms = cooldown_ms;
                config_d.lookahead = lookahead;
                config_d.enable_constant_optimization = enable_constant_opt;
                let mut engine_d =
                    EvolutionEngine::new(config_d, &oracle, &sandbox, op_instance.as_ref())
                        .with_scheduling_oracle(&sched_oracle);
                let report_d = engine_d.execute_run()?;

                // GPU breathing pause between Group D and Group C
                std::thread::sleep(std::time::Duration::from_secs(3));

                // Group C run
                let mut config_c = ExperimentConfig::default();
                config_c.domain = prob_domain;
                config_c.group = ExperimentalGroup::GroupC;
                config_c.rng_seed = seed;
                config_c.population_size = pop_size;
                config_c.max_generations = generations;
                config_c.cooldown_ms = cooldown_ms;
                config_c.lookahead = lookahead;
                config_c.enable_constant_optimization = enable_constant_opt;
                let mut engine_c =
                    EvolutionEngine::new(config_c, &oracle, &sandbox, op_instance.as_ref())
                        .with_scheduling_oracle(&sched_oracle);
                let report_c = engine_c.execute_run()?;

                let val_d = report_d.best_validation_score.as_ref().map(|s| s.total_bins).unwrap_or(0);
                let val_c = report_c.best_validation_score.as_ref().map(|s| s.total_bins).unwrap_or(0);
                println!(
                    "Pair #{:02} (Seed {}):\n  In-Dist : Group D = {} vs Group C = {} (Δ = {:+})\n  Val     : Group D = {} vs Group C = {} (Δ = {:+})\n  OOD-1   : Group D = {} vs Group C = {} (Δ = {:+})\n  OOD-2   : Group D = {} vs Group C = {} (Δ = {:+})",
                    r + 1,
                    seed,
                    report_d.best_in_distribution_score.total_bins,
                    report_c.best_in_distribution_score.total_bins,
                    (report_c.best_in_distribution_score.total_bins as i64)
                        - (report_d.best_in_distribution_score.total_bins as i64),
                    val_d,
                    val_c,
                    (val_c as i64) - (val_d as i64),
                    report_d.best_ood1_score.total_bins,
                    report_c.best_ood1_score.total_bins,
                    (report_c.best_ood1_score.total_bins as i64)
                        - (report_d.best_ood1_score.total_bins as i64),
                    report_d.best_ood2_score.total_bins,
                    report_c.best_ood2_score.total_bins,
                    (report_c.best_ood2_score.total_bins as i64)
                        - (report_d.best_ood2_score.total_bins as i64),
                );

                paired_results.push((report_c, report_d));

                // Incremental auto-save so partial results are never lost
                if let Some(ref path) = output {
                    if let Ok(serialized) = serde_json::to_string_pretty(&paired_results) {
                        let _ = std::fs::write(path, serialized.as_bytes());
                    }
                }
            }

            if let Some(path) = output {
                let serialized = serde_json::to_string_pretty(&paired_results)?;
                let mut file = File::create(&path)?;
                file.write_all(serialized.as_bytes())?;
                println!("Paired comparison written to {}", path.display());
            }
        }

        Commands::Campaign {
            domain,
            seeds,
            pop_size,
            generations,
            lookahead,
            enable_constant_opt,
            operator,
            ollama_url,
            model,
            cooldown_ms,
            output_dir,
        } => {
            let prob_domain: ProblemDomain = domain.into();
            std::fs::create_dir_all(&output_dir)?;
            println!("============================================================");
            println!(" NOMAD PRE-REGISTERED 30-SEED STATISTICAL CAMPAIGN");
            println!(" Target Domain   : {:?}", prob_domain);
            println!(" Paired Seeds    : {}", seeds);
            println!(" Lookahead (k=2) : {}", lookahead);
            println!(" Constant Opt    : {}", enable_constant_opt);
            println!(" Output Directory: {}", output_dir.display());
            println!("============================================================");

            let op_instance: Box<dyn SkillOperator> = match operator {
                OperatorArg::Mock => {
                    let mut mock = DeterministicMockOperator::default();
                    mock.domain = prob_domain;
                    Box::new(mock)
                }
                OperatorArg::Ollama => Box::new(
                    HttpLlamaOperator::new(
                        ollama_url,
                        model,
                        "dae161e27b0e90dd1856c8bb3209201fd6736d8eb66298e75ed87571486f4364",
                        "Ollama API (Qwen 2.5 Coder 7B)",
                        0.7,
                        0.95,
                    )
                    .with_domain(prob_domain),
                ),
            };

            let mut aggregate_pairs = Vec::new();

            for r in 0..seeds {
                let seed = 1000 + (r as u64) * 17;
                let oracle = OracleEvaluator::new(seed);
                let sched_oracle = SchedulingOracle::new(seed);
                let sandbox = DeterministicSandbox::new();

                // 1. Group D Run (Mutation-only baseline)
                let mut config_d = ExperimentConfig::default();
                config_d.domain = prob_domain;
                config_d.group = ExperimentalGroup::GroupD;
                config_d.rng_seed = seed;
                config_d.population_size = pop_size;
                config_d.max_generations = generations;
                config_d.cooldown_ms = cooldown_ms;
                config_d.lookahead = lookahead;
                config_d.enable_constant_optimization = enable_constant_opt;

                let mut engine_d =
                    EvolutionEngine::new(config_d, &oracle, &sandbox, op_instance.as_ref())
                        .with_scheduling_oracle(&sched_oracle);
                let report_d = engine_d.execute_run()?;

                let d_file = output_dir.join(format!("run_seed_{:04}_group_d.json", seed));
                let mut f_d = File::create(&d_file)?;
                f_d.write_all(serde_json::to_string_pretty(&report_d)?.as_bytes())?;

                if cooldown_ms > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(cooldown_ms));
                }

                // 2. Group C Run (Full Swarm with Crossover)
                let mut config_c = ExperimentConfig::default();
                config_c.domain = prob_domain;
                config_c.group = ExperimentalGroup::GroupC;
                config_c.rng_seed = seed;
                config_c.population_size = pop_size;
                config_c.max_generations = generations;
                config_c.cooldown_ms = cooldown_ms;
                config_c.lookahead = lookahead;
                config_c.enable_constant_optimization = enable_constant_opt;

                let mut engine_c =
                    EvolutionEngine::new(config_c, &oracle, &sandbox, op_instance.as_ref())
                        .with_scheduling_oracle(&sched_oracle);
                let report_c = engine_c.execute_run()?;

                let c_file = output_dir.join(format!("run_seed_{:04}_group_c.json", seed));
                let mut f_c = File::create(&c_file)?;
                f_c.write_all(serde_json::to_string_pretty(&report_c)?.as_bytes())?;

                println!(
                    "Seed {:04} ({:02}/{}): Group D = {} bins | Group C = {} bins (Diff = {:+})",
                    seed,
                    r + 1,
                    seeds,
                    report_d.best_in_distribution_score.total_bins,
                    report_c.best_in_distribution_score.total_bins,
                    (report_c.best_in_distribution_score.total_bins as i64)
                        - (report_d.best_in_distribution_score.total_bins as i64)
                );

                aggregate_pairs.push((report_c, report_d));

                // Incremental auto-save of aggregate file
                let agg_file = output_dir.join("campaign_aggregate.json");
                let mut f_agg = File::create(&agg_file)?;
                f_agg.write_all(serde_json::to_string_pretty(&aggregate_pairs)?.as_bytes())?;
            }

            println!("============================================================");
            println!(
                " Campaign Complete: {} paired runs saved in {}",
                seeds,
                output_dir.display()
            );
            println!(
                " Aggregate data file: {}",
                output_dir.join("campaign_aggregate.json").display()
            );
            println!(" To analyze with statistical significance and Holm correction:");
            println!("   python scripts/analysis/wilcoxon_analysis.py {}/campaign_aggregate.json --output-json {}/stats.json --output-csv {}/stats.csv",
                output_dir.display(), output_dir.display(), output_dir.display());
            println!("============================================================");
        }
    }

    Ok(())
}
