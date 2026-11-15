//! Portee du croisement AST sur de vrais programmes, sans aucune evaluation : parmi les unites
//! logiques des donneurs, combien l'analyse de dependances accepte-t-elle de greffer, et
//! pourquoi refuse-t-elle les autres ?
//!
//! Usage : graft_reach <dossier des run_seed_*_evolution.json> <seed>...

use std::collections::{BTreeMap, HashSet};

use nomad_core::SkillCapsule;
use nomad_genome::{
    extract_logical_units, parse_heuristic_fn, resolve_dependencies_with_backward_slice,
};

// Copie de la liste blanche privee de crossover.rs.
const BUILTINS: &[&str] = &[
    "f64", "f32", "u32", "u64", "usize", "i32", "i64", "bool", "min", "max", "abs", "sqrt",
    "powf", "floor", "ceil", "round", "Some", "None", "true", "false", "score",
];

fn params(code: &str) -> HashSet<String> {
    let open = code.find('(').unwrap();
    let close = code.find(')').unwrap();
    code[open + 1..close]
        .split(',')
        .filter_map(|p| p.split(':').next())
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

fn let_name(repr: &str) -> Option<String> {
    let rest = repr.strip_prefix("let ")?;
    let rest = rest.strip_prefix("mut ").unwrap_or(rest);
    rest.split_whitespace().next().map(|s| s.trim_end_matches(':').to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = &args[1];
    let mut triples = 0usize;
    let mut resolved = 0usize;
    let mut causes: BTreeMap<&str, usize> = BTreeMap::new();
    let mut names: BTreeMap<String, usize> = BTreeMap::new();
    let mut units_total = 0usize;
    let mut units_with_inner_let = 0usize;
    let mut programs = 0usize;

    for seed in &args[2..] {
        let text = std::fs::read_to_string(format!("{dir}/run_seed_{seed}_evolution.json")).unwrap();
        let report: serde_json::Value = serde_json::from_str(&text).unwrap();
        let mut seen = HashSet::new();
        let elites: Vec<SkillCapsule> = report["archive"]["grid"]
            .as_object()
            .unwrap()
            .values()
            .map(|e| serde_json::from_value::<SkillCapsule>(e["capsule"].clone()).unwrap())
            .filter(|c| seen.insert(c.logic_code.clone()))
            .collect();
        let parsed: Vec<_> = elites
            .iter()
            .filter_map(|c| parse_heuristic_fn(&c.logic_code).ok().map(|f| (c, f)))
            .collect();
        programs += parsed.len();
        for (_, f) in &parsed {
            for u in extract_logical_units(f) {
                units_total += 1;
                if u.code_repr.matches("let ").count() > usize::from(u.code_repr.starts_with("let ")) {
                    units_with_inner_let += 1;
                }
            }
        }

        for (hi, (host, host_fn)) in parsed.iter().enumerate() {
            let host_params = params(&host.logic_code);
            let locus = host_fn.block.stmts.len().saturating_sub(1);
            let host_locals: HashSet<String> = extract_logical_units(host_fn)
                .iter()
                .filter_map(|u| let_name(&u.code_repr))
                .collect();
            for (di, (_, donor_fn)) in parsed.iter().enumerate() {
                if di == hi {
                    continue;
                }
                let donor_units = extract_logical_units(donor_fn);
                let donor_locals: HashSet<String> =
                    donor_units.iter().filter_map(|u| let_name(&u.code_repr)).collect();
                for u in &donor_units {
                    triples += 1;
                    let ok = resolve_dependencies_with_backward_slice(
                        &u.stmt,
                        &donor_fn.block.stmts,
                        &host_fn.block.stmts,
                        &host_params,
                        locus,
                    )
                    .is_some();
                    if ok {
                        resolved += 1;
                        continue;
                    }
                    // Noms que l'analyse ne sait pas resoudre, et ce qu'ils sont vraiment.
                    let mut real_missing = false;
                    let mut inner = false;
                    let mut not_a_variable = false;
                    for n in &u.dependencies {
                        if BUILTINS.contains(&n.as_str())
                            || host_params.contains(n)
                            || host_locals.contains(n)
                            || donor_locals.contains(n)
                        {
                            continue;
                        }
                        let r = &u.code_repr;
                        if r.contains(&format!("let {n} ")) || r.contains(&format!("let mut {n} ")) {
                            inner = true;
                        } else if r.contains(&format!(". {n} (")) || r.contains(&format!(":: {n}")) {
                            not_a_variable = true;
                            *names.entry(n.clone()).or_default() += 1;
                        } else {
                            real_missing = true;
                        }
                    }
                    let cause = if real_missing {
                        "variable reellement absente"
                    } else if inner && not_a_variable {
                        "refus a tort : local interne + methode"
                    } else if inner {
                        "refus a tort : local declare dans l'unite"
                    } else if not_a_variable {
                        "refus a tort : nom de methode ou de constante"
                    } else {
                        "autre (dependance d'une definition du donneur)"
                    };
                    *causes.entry(cause).or_default() += 1;
                }
            }
        }
    }

    println!("programmes : {programs}, unites : {units_total}, dont avec un `let` interne : {units_with_inner_let}");
    println!("triplets (hote, donneur, unite) : {triples}");
    println!("  acceptes : {resolved} ({:.1} %)", 100.0 * resolved as f64 / triples as f64);
    for (c, n) in &causes {
        println!("  {c} : {n} ({:.1} %)", 100.0 * *n as f64 / triples as f64);
    }
    let mut top: Vec<_> = names.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1));
    println!("noms refuses a tort les plus frequents : {:?}", &top[..top.len().min(12)]);
}
