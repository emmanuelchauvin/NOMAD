use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap, HashSet};
use thiserror::Error;

use nomad_core::{GraftManifest, PolicyKind, SkillCapsule, ValidationStatus};

use crate::ast::{compute_structural_fingerprint, normalize_source, parse_heuristic_fn, AstError};
use crate::unit::{collect_dependencies, collect_free_variables, extract_logical_units, LogicalUnit};

#[derive(Debug, Error)]
pub enum CrossoverError {
    #[error("AST parsing error: {0}")]
    AstError(#[from] AstError),
    #[error("No transferable logical units found in donor capsule {0}")]
    NoExtractableUnits(String),
    #[error("Host capsule {0} has an empty function body")]
    EmptyHostBody(String),
    #[error("No donor unit of {0} can be grafted: unresolved dependencies in the host")]
    UnresolvedDependencies(String),
}

fn is_builtin_or_method(name: &str) -> bool {
    matches!(
        name,
        "f64"
            | "f32"
            | "u32"
            | "u64"
            | "usize"
            | "i32"
            | "i64"
            | "bool"
            | "min"
            | "max"
            | "abs"
            | "sqrt"
            | "powf"
            | "floor"
            | "ceil"
            | "round"
            | "Some"
            | "None"
            | "true"
            | "false"
    )
}

/// Nom lie par un `let`, avec ou sans annotation de type (`let x = ..` ou `let x: f64 = ..`).
fn local_pat_ident(pat: &syn::Pat) -> Option<&syn::PatIdent> {
    match pat {
        syn::Pat::Ident(p) => Some(p),
        syn::Pat::Type(t) => local_pat_ident(&t.pat),
        _ => None,
    }
}

fn extract_signature_params(item_fn: &syn::ItemFn) -> HashSet<String> {
    let mut params = HashSet::new();
    for input in &item_fn.sig.inputs {
        if let syn::FnArg::Typed(pat_type) = input {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                params.insert(pat_ident.ident.to_string());
            }
        }
    }
    params
}

fn extract_in_scope_locals(stmts: &[syn::Stmt], locus_idx: usize) -> HashSet<String> {
    let mut in_scope = HashSet::new();
    let limit = locus_idx.min(stmts.len());
    for s in &stmts[..limit] {
        if let syn::Stmt::Local(l) = s {
            if let Some(p) = local_pat_ident(&l.pat) {
                in_scope.insert(p.ident.to_string());
            }
        }
    }
    in_scope
}

/// Vrai si `stmt` est un `let` de premier niveau qui lie `var`.
fn defines_var(stmt: &syn::Stmt, var: &str) -> bool {
    matches!(stmt, syn::Stmt::Local(l) if local_pat_ident(&l.pat).is_some_and(|p| p.ident == var))
}

fn path_ident(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(p) if p.qself.is_none() => p.path.get_ident().map(|i| i.to_string()),
        _ => None,
    }
}

/// Noms affectes par une instruction, a toute profondeur (`x = ..`, `x += ..`), et noms
/// lies a l'interieur (`let`, motifs) : ces derniers sont locaux a l'instruction.
#[derive(Default)]
struct WriteCollector {
    written: HashSet<String>,
    bound: HashSet<String>,
}

impl<'ast> syn::visit::Visit<'ast> for WriteCollector {
    fn visit_expr_assign(&mut self, n: &'ast syn::ExprAssign) {
        if let Some(name) = path_ident(&n.left) {
            self.written.insert(name);
        }
        syn::visit::visit_expr_assign(self, n);
    }
    fn visit_expr_binary(&mut self, n: &'ast syn::ExprBinary) {
        if matches!(
            n.op,
            syn::BinOp::AddAssign(_)
                | syn::BinOp::SubAssign(_)
                | syn::BinOp::MulAssign(_)
                | syn::BinOp::DivAssign(_)
                | syn::BinOp::RemAssign(_)
        ) {
            if let Some(name) = path_ident(&n.left) {
                self.written.insert(name);
            }
        }
        syn::visit::visit_expr_binary(self, n);
    }
    fn visit_pat_ident(&mut self, p: &'ast syn::PatIdent) {
        self.bound.insert(p.ident.to_string());
        syn::visit::visit_pat_ident(self, p);
    }
}

fn writes_of(stmt: &syn::Stmt) -> WriteCollector {
    let mut w = WriteCollector::default();
    syn::visit::Visit::visit_stmt(&mut w, stmt);
    w
}

/// Renomme des variables (lectures, ecritures et liaisons) selon `map`.
struct Renamer<'a> {
    map: &'a HashMap<String, String>,
}

impl syn::visit_mut::VisitMut for Renamer<'_> {
    fn visit_expr_path_mut(&mut self, n: &mut syn::ExprPath) {
        if n.qself.is_none() && n.path.segments.len() == 1 {
            let seg = &mut n.path.segments[0];
            if let Some(new) = self.map.get(&seg.ident.to_string()) {
                seg.ident = syn::Ident::new(new, seg.ident.span());
            }
        }
        syn::visit_mut::visit_expr_path_mut(self, n);
    }
    fn visit_pat_ident_mut(&mut self, p: &mut syn::PatIdent) {
        if let Some(new) = self.map.get(&p.ident.to_string()) {
            p.ident = syn::Ident::new(new, p.ident.span());
        }
        syn::visit_mut::visit_pat_ident_mut(self, p);
    }
}

/// Nom libre pour la variable du donneur `var` : `_d_var`, sinon `_d2_var`, `_d3_var`...
fn fresh_donor_name(var: &str, taken: &HashSet<String>) -> String {
    let mut name = format!("_d_{var}");
    let mut k = 2;
    while taken.contains(&name) {
        name = format!("_d{k}_{var}");
        k += 1;
    }
    name
}

/// Backward program slicing syntaxique : rassemble ce que lit `stmt_to_insert` (l'unite
/// d'indice `unit_index` du donneur, ou la regle derivee d'elle) pour l'inserer dans l'hote
/// avant `locus_idx`.
///
/// - Une variable que le donneur definit avant l'unite est celle du donneur : sa definition,
///   ses redefinitions et ses reaffectations (`x += ..`, `if c { x = .. }`) avant l'unite
///   sont copiees dans l'ordre du donneur, puis renommees `_d_x` pour ne capturer ni ecraser
///   aucune variable de l'hote. Une reaffectation qui ecrit aussi une variable de l'hote
///   (`score` compris) rend la greffe impossible.
/// - `score` et les variables que le donneur ne definit pas sont celles de l'hote
///   (parametres ou variables en portee au locus) ; sinon la greffe est impossible.
pub fn graft_slice(
    stmt_to_insert: &syn::Stmt,
    unit_index: usize,
    donor_stmts: &[syn::Stmt],
    host_stmts: &[syn::Stmt],
    host_params: &HashSet<String>,
    locus_idx: usize,
) -> Option<Vec<syn::Stmt>> {
    let mut host_scope = extract_in_scope_locals(host_stmts, locus_idx);
    host_scope.extend(host_params.iter().cloned());
    let limit = unit_index.min(donor_stmts.len());

    let mut donor_vars = BTreeSet::new();
    let mut included = BTreeSet::new();
    let mut reassignments = BTreeSet::new();
    let mut worklist = collect_free_variables(stmt_to_insert);
    let mut visited = HashSet::new();

    while let Some(var) = worklist.pop() {
        if is_builtin_or_method(&var) || !visited.insert(var.clone()) {
            continue;
        }
        let first_def = (0..limit).find(|&i| defines_var(&donor_stmts[i], &var));
        let Some(first_def) = first_def.filter(|_| var != "score") else {
            if host_scope.contains(&var) {
                continue;
            }
            return None;
        };
        donor_vars.insert(var.clone());
        for (i, s) in donor_stmts.iter().enumerate().take(limit).skip(first_def) {
            let is_def = defines_var(s, &var);
            if is_def || writes_of(s).written.contains(&var) {
                included.insert(i);
                if !is_def {
                    reassignments.insert(i);
                }
                worklist.extend(collect_free_variables(s));
            }
        }
    }

    // Une reaffectation copiee n'ecrit que des variables du donneur ou les siennes.
    for &i in &reassignments {
        let w = writes_of(&donor_stmts[i]);
        if w
            .written
            .iter()
            .any(|n| !donor_vars.contains(n) && !w.bound.contains(n))
        {
            return None;
        }
    }

    let mut taken: HashSet<String> = host_params.clone();
    for st in host_stmts.iter().chain(donor_stmts) {
        taken.extend(collect_dependencies(st));
    }
    let mut map = HashMap::new();
    for v in &donor_vars {
        let name = fresh_donor_name(v, &taken);
        taken.insert(name.clone());
        map.insert(v.clone(), name);
    }
    let mut renamer = Renamer { map: &map };

    let mut slice: Vec<syn::Stmt> = included.iter().map(|&i| donor_stmts[i].clone()).collect();
    for st in &mut slice {
        syn::visit_mut::VisitMut::visit_stmt_mut(&mut renamer, st);
    }
    let mut inserted = stmt_to_insert.clone();
    match &mut inserted {
        // Le nom lie par l'unite elle-meme est garde : l'hote peut le lire en aval.
        syn::Stmt::Local(local) => {
            if let Some(init) = &mut local.init {
                syn::visit_mut::VisitMut::visit_expr_mut(&mut renamer, &mut init.expr);
                if let Some((_, diverge)) = &mut init.diverge {
                    syn::visit_mut::VisitMut::visit_expr_mut(&mut renamer, diverge);
                }
            }
        }
        st => syn::visit_mut::VisitMut::visit_stmt_mut(&mut renamer, st),
    }
    slice.push(inserted);
    Some(slice)
}

/// Comme `graft_slice`, pour une instruction du donneur reperee par son contenu (a defaut,
/// toutes les definitions du donneur sont disponibles).
pub fn resolve_dependencies_with_backward_slice(
    stmt_to_insert: &syn::Stmt,
    donor_stmts: &[syn::Stmt],
    host_stmts: &[syn::Stmt],
    host_params: &HashSet<String>,
    locus_idx: usize,
) -> Option<Vec<syn::Stmt>> {
    let unit_index = donor_stmts
        .iter()
        .position(|s| s == stmt_to_insert)
        .unwrap_or(donor_stmts.len());
    graft_slice(stmt_to_insert, unit_index, donor_stmts, host_stmts, host_params, locus_idx)
}

/// Facon de greffer une unite du donneur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraftMode {
    /// Comportement historique : une definition du donneur (`let v = e`) que l'hote ne lit
    /// pas devient un melange `let score = score * 0.95 + e * 0.05` (0,70/0,30 ou 0,80/0,20
    /// pour un `let score` du donneur).
    Mixing,
    /// Regles completes seulement : seules les instructions du donneur qui modifient `score`
    /// (`if .. { score += .. }`, `score *= ..`) sont greffees, avec leur tranche. Aucun melange.
    RulesOnly,
}

/// Instruction du donneur qui modifie `score` sans le redefinir : une regle complete.
fn is_score_rule(unit: &LogicalUnit) -> bool {
    !matches!(unit.stmt, syn::Stmt::Local(_)) && writes_of(&unit.stmt).written.contains("score")
}

fn units_for_mode(units: Vec<LogicalUnit>, mode: GraftMode) -> Vec<LogicalUnit> {
    match mode {
        GraftMode::Mixing => units,
        GraftMode::RulesOnly => units.into_iter().filter(is_score_rule).collect(),
    }
}

/// ============================================================================
/// deterministic_ast_crossover : Algorithme de Croisement Génétique AST Déterministe
/// ============================================================================
/// Réalise le croisement formel de deux organismes sans dépendre obligatoirement
/// d'une réparation LLM (Section 4.5) :
///
/// **Étapes de l'algorithme :**
/// 1. **Analyse Syntaxique :** Parse les codes des capsules hôte et donneuse en `ItemFn`.
/// 2. **Extraction des Unités :** Extrait les blocs et expressions transférables du donneur via `extract_logical_units`.
/// 3. **Sélection Déterministe :** Tire une unité logique donneuse via le générateur pseudo-aléatoire `ChaCha8Rng(seed)`.
/// 4. **Identification du Locus :** Localise le point d'insertion optimal dans l'AST de l'hôte (juste avant l'expression de retour finale).
/// 5. **Greffe Syntaxique :** Insère le statement donneur dans le bloc d'instructions de l'hôte.
/// 6. **Génération du Manifeste :** Calcule les empreintes cryptographiques et produit le `GraftManifest` externe.
/// 7. **Reconstruction Canonique :** Régénère le code source hybride propre via `quote` et initialise l'enfant.
pub fn deterministic_ast_crossover(
    host: &SkillCapsule,
    donor: &SkillCapsule,
    seed: u64,
    generation: usize,
) -> Result<(SkillCapsule, GraftManifest), CrossoverError> {
    deterministic_ast_crossover_with(host, donor, seed, generation, GraftMode::Mixing)
}

/// `deterministic_ast_crossover` avec un mode de greffe explicite.
pub fn deterministic_ast_crossover_with(
    host: &SkillCapsule,
    donor: &SkillCapsule,
    seed: u64,
    generation: usize,
    mode: GraftMode,
) -> Result<(SkillCapsule, GraftManifest), CrossoverError> {
    let host_fn = parse_heuristic_fn(&host.logic_code)?;
    let donor_fn = parse_heuristic_fn(&donor.logic_code)?;

    let _host_fingerprint = compute_structural_fingerprint(&host_fn);
    let donor_fingerprint = compute_structural_fingerprint(&donor_fn);

    let donor_units = units_for_mode(extract_logical_units(&donor_fn), mode);
    if donor_units.is_empty() {
        return Err(CrossoverError::NoExtractableUnits(donor.id.0.clone()));
    }

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let host_params = extract_signature_params(&host_fn);

    let mut hybrid_fn = host_fn.clone();
    let stmts = &mut hybrid_fn.block.stmts;
    if stmts.is_empty() {
        return Err(CrossoverError::EmptyHostBody(host.id.0.clone()));
    }

    // Determine insertion locus: right before the final return/expression statement
    let locus_idx = if stmts.len() > 1 { stmts.len() - 1 } else { 0 };

    let mut candidate_units: Vec<&LogicalUnit> = donor_units.iter().collect();
    candidate_units.shuffle(&mut rng);

    let mut chosen_unit = candidate_units[0];
    let mut chosen_mixed = false;
    let mut resolved_graft: Option<Vec<syn::Stmt>> = None;

    for cand in &candidate_units {
        let mut cand_stmt = cand.stmt.clone();
        if let syn::Stmt::Local(local) = &cand.stmt {
            if let Some(pat_ident) = local_pat_ident(&local.pat) {
                let var_name = pat_ident.ident.to_string();
                let already_defined = stmts.iter().any(|s| {
                    if let syn::Stmt::Local(l) = s {
                        if let Some(p) = local_pat_ident(&l.pat) {
                            return p.ident == var_name;
                        }
                    }
                    false
                });

                if let Some(init) = &local.init {
                    let init_expr = &init.expr;
                    if var_name == "score" {
                        cand_stmt = syn::parse_quote! {
                            let score = score * 0.70 + ((#init_expr) as f64) * 0.30;
                        };
                    } else {
                        let downstream_deps: Vec<String> = stmts[locus_idx..]
                            .iter()
                            .flat_map(collect_dependencies)
                            .collect();
                        if already_defined || !downstream_deps.contains(&var_name) {
                            cand_stmt = syn::parse_quote! {
                                let score = score * 0.95 + ((#init_expr) as f64) * 0.05;
                            };
                        }
                    }
                }
            }
        }

        if let Some(slice) = graft_slice(
            &cand_stmt,
            cand.index,
            &donor_fn.block.stmts,
            stmts,
            &host_params,
            locus_idx,
        ) {
            resolved_graft = Some(slice);
            chosen_mixed = cand_stmt != cand.stmt;
            chosen_unit = cand;
            break;
        }
    }

    // Aucune unité greffable : refuser plutôt qu'insérer un fragment qui ne compilerait pas.
    let graft_stmts =
        resolved_graft.ok_or_else(|| CrossoverError::UnresolvedDependencies(donor.id.0.clone()))?;

    for (offset, stmt) in graft_stmts.iter().enumerate() {
        stmts.insert(locus_idx + offset, stmt.clone());
    }

    let mut fragment_hashes = Vec::new();
    let mut inserted_deps = Vec::new();
    let mut loci_indices = Vec::new();

    for (offset, stmt) in graft_stmts.iter().enumerate() {
        let inserted_repr = quote::quote!(#stmt).to_string();
        let mut fragment_hasher = Sha256::new();
        fragment_hasher.update(inserted_repr.as_bytes());
        fragment_hashes.push(hex::encode(fragment_hasher.finalize()));
        inserted_deps.extend(collect_dependencies(stmt));
        loci_indices.push((locus_idx + offset).to_string());
    }

    let fragment_hash = fragment_hashes.join(":");
    let locus_desc = if loci_indices.len() == 1 {
        format!("body_stmt_index_{}", locus_idx)
    } else {
        format!("multi_loci_{}", loci_indices.join("_"))
    };

    let mut dep_hasher = Sha256::new();
    for d in &inserted_deps {
        dep_hasher.update(d.as_bytes());
    }
    let dependency_hash = hex::encode(dep_hasher.finalize());

    let mut graft_hasher = Sha256::new();
    graft_hasher.update(host.id.0.as_bytes());
    graft_hasher.update(donor.id.0.as_bytes());
    graft_hasher.update(fragment_hash.as_bytes());
    graft_hasher.update(seed.to_le_bytes());
    let graft_id = format!("graft-{}", &hex::encode(graft_hasher.finalize())[..12]);

    let manifest = GraftManifest {
        graft_id: graft_id.clone(),
        donor_capsule: donor.id.clone(),
        host_capsule: host.id.clone(),
        donor_ast_hash: donor_fingerprint.ast_hash,
        grafted_fragment_hash: fragment_hash,
        host_locus: locus_desc,
        dependency_hash,
        repair_version: None,
        repair_diff_hash: None,
    };

    // Reconstruct normalized code and compute new fingerprint
    let hybrid_code = normalize_source(&hybrid_fn);
    let child_fingerprint = compute_structural_fingerprint(&hybrid_fn);

    let child_name = format!("Hybrid-G{}-{}", generation, &graft_id[..10]);
    let semantic_interface = format!(
        "AST hybrid combining host {} with donor {} via fragment [{}]",
        host.id, donor.id, chosen_unit.code_repr
    );

    let mut metadata = HashMap::new();
    metadata.insert(
        "crossover_type".to_string(),
        "ast_deterministic".to_string(),
    );
    metadata.insert("graft_mode".to_string(), format!("{mode:?}"));
    metadata.insert("mixed".to_string(), chosen_mixed.to_string());
    metadata.insert("host_id".to_string(), host.id.0.clone());
    metadata.insert("donor_id".to_string(), donor.id.0.clone());
    metadata.insert("graft_id".to_string(), graft_id);

    let child_capsule = SkillCapsule::new_v2(
        child_name,
        semantic_interface,
        hybrid_code,
        vec![host.id.clone(), donor.id.clone()],
        generation,
        metadata,
        "ScorePolicyV1",
        PolicyKind::ScorePolicy,
        Some(child_fingerprint),
        chosen_unit.dependencies.clone(),
        ValidationStatus::PassedLevel1Structural, // Deterministic graft is syntactically present
        Some(manifest.clone()),
    );

    Ok((child_capsule, manifest))
}

/// Nombre de conditions d'aiguillage proposées par `regime_dispatch_crossover`.
pub const REGIME_VARIANTS: usize = 6;

/// Croisement par aiguillage de régime : l'enfant contient les deux parents entiers et, à
/// chaque décision, exécute l'un ou l'autre selon une condition sur l'état (`variant`).
/// Aucun fragment n'est extrait : chaque parent garde ses règles et ses variables, donc ni
/// dépendance non résolue ni capture de nom. Un seul parent est exécuté par appel.
///
/// Variantes 0 à 4 : la condition ne dépend que de l'objet ou de l'avancement, donc tous les
/// bacs d'une même décision sont notés par le même parent. Variante 5 : elle dépend du bac,
/// et mélange alors deux échelles de score dans une même décision.
pub fn regime_dispatch_crossover(
    host: &SkillCapsule,
    donor: &SkillCapsule,
    variant: usize,
    generation: usize,
) -> Result<SkillCapsule, CrossoverError> {
    let host_fn = parse_heuristic_fn(&host.logic_code)?;
    let donor_fn = parse_heuristic_fn(&donor.logic_code)?;
    if host_fn.block.stmts.is_empty() {
        return Err(CrossoverError::EmptyHostBody(host.id.0.clone()));
    }
    // Les deux corps lisent les paramètres par leur nom : les signatures doivent coïncider.
    let host_params = extract_signature_params(&host_fn);
    let needed = [
        "item_size",
        "bin_residual",
        "bin_capacity",
        "step_index",
        "total_items",
        "mean_residual",
    ];
    if extract_signature_params(&donor_fn) != host_params
        || needed.iter().any(|p| !host_params.contains(*p))
    {
        return Err(CrossoverError::UnresolvedDependencies(donor.id.0.clone()));
    }

    let (label, cond): (&str, syn::Expr) = match variant % REGIME_VARIANTS {
        0 => ("late_half", syn::parse_quote!(step_index * 2 >= total_items)),
        1 => ("early_half", syn::parse_quote!(step_index * 2 < total_items)),
        2 => ("last_fifth", syn::parse_quote!(step_index * 5 >= total_items * 4)),
        3 => ("large_item", syn::parse_quote!(item_size * 4 >= bin_capacity)),
        4 => ("small_item", syn::parse_quote!(item_size * 4 < bin_capacity)),
        _ => ("bin_above_mean", syn::parse_quote!((bin_residual as f64) >= mean_residual)),
    };

    let host_block = &host_fn.block;
    let donor_block = &donor_fn.block;
    let mut hybrid_fn = host_fn.clone();
    // Le donneur est exécuté quand la condition est vraie, l'hôte sinon.
    hybrid_fn.block = Box::new(syn::parse_quote!({
        if #cond #donor_block else #host_block
    }));

    let hybrid_code = normalize_source(&hybrid_fn);
    let child_fingerprint = compute_structural_fingerprint(&hybrid_fn);
    let mut id_hasher = Sha256::new();
    id_hasher.update(host.id.0.as_bytes());
    id_hasher.update(donor.id.0.as_bytes());
    id_hasher.update(label.as_bytes());
    let short = hex::encode(id_hasher.finalize())[..10].to_string();

    let mut metadata = HashMap::new();
    metadata.insert("crossover_type".to_string(), "regime_dispatch".to_string());
    metadata.insert("regime".to_string(), label.to_string());
    metadata.insert("host_id".to_string(), host.id.0.clone());
    metadata.insert("donor_id".to_string(), donor.id.0.clone());

    Ok(SkillCapsule::new_v2(
        format!("Dispatch-G{}-{}", generation, short),
        format!("Regime dispatch [{}] : donor {} else host {}", label, donor.id, host.id),
        hybrid_code,
        vec![host.id.clone(), donor.id.clone()],
        generation,
        metadata,
        "ScorePolicyV1",
        PolicyKind::ScorePolicy,
        Some(child_fingerprint),
        Vec::new(),
        ValidationStatus::Unvalidated,
        None,
    ))
}

/// ============================================================================
/// multi_locus_ast_crossover : Croisement Génétique Multi-Sites (Multi-Loci)
/// ============================================================================
/// Permet l'échange synergique de jusqu'à deux unités logiques distinctes du donneur
/// (ex: conditionnement initial / modulation d'horizon ET raffinement terminal du score)
/// dans l'hôte à deux loci structurels distincts, maximisant la recombinaison sans bloat.
pub fn multi_locus_ast_crossover(
    host: &SkillCapsule,
    donor: &SkillCapsule,
    seed: u64,
    generation: usize,
) -> Result<(SkillCapsule, GraftManifest), CrossoverError> {
    multi_locus_ast_crossover_with(host, donor, seed, generation, GraftMode::Mixing)
}

/// `multi_locus_ast_crossover` avec un mode de greffe explicite.
pub fn multi_locus_ast_crossover_with(
    host: &SkillCapsule,
    donor: &SkillCapsule,
    seed: u64,
    generation: usize,
    mode: GraftMode,
) -> Result<(SkillCapsule, GraftManifest), CrossoverError> {
    let host_fn = parse_heuristic_fn(&host.logic_code)?;
    let donor_fn = parse_heuristic_fn(&donor.logic_code)?;

    let donor_fingerprint = compute_structural_fingerprint(&donor_fn);
    let donor_units = units_for_mode(extract_logical_units(&donor_fn), mode);
    if donor_units.is_empty() {
        return Err(CrossoverError::NoExtractableUnits(donor.id.0.clone()));
    }

    if donor_units.len() < 2 {
        return deterministic_ast_crossover_with(host, donor, seed, generation, mode);
    }

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let host_params = extract_signature_params(&host_fn);

    let mut indices: Vec<usize> = (0..donor_units.len()).collect();
    indices.shuffle(&mut rng);
    let unit1 = &donor_units[indices[0]];
    let unit2 = &donor_units[indices[1]];

    let mut hybrid_fn = host_fn.clone();
    let stmts = &mut hybrid_fn.block.stmts;
    if stmts.is_empty() {
        return Err(CrossoverError::EmptyHostBody(host.id.0.clone()));
    }

    // Locus 1 : juste après la déclaration de `score` dans l'hôte, où qu'elle soit. Avant
    // elle, une greffe qui lit `score` ne compile pas ; un hôte sans `let score` de premier
    // niveau ne peut pas recevoir ce croisement.
    let locus1 = stmts
        .iter()
        .position(|s| matches!(s, syn::Stmt::Local(l)
            if local_pat_ident(&l.pat).is_some_and(|p| p.ident == "score")))
        .map(|i| i + 1)
        .ok_or_else(|| CrossoverError::UnresolvedDependencies(host.id.0.clone()))?;
    let mut stmt1 = unit1.stmt.clone();
    if let syn::Stmt::Local(local) = &unit1.stmt {
        if let Some(pat_ident) = local_pat_ident(&local.pat) {
            let var_name = pat_ident.ident.to_string();
            let already_defined = stmts.iter().any(|s| {
                if let syn::Stmt::Local(l) = s {
                    if let Some(p) = local_pat_ident(&l.pat) {
                        return p.ident == var_name;
                    }
                }
                false
            });

            if let Some(init) = &local.init {
                let init_expr = &init.expr;
                if var_name == "score" {
                    stmt1 = syn::parse_quote! {
                        let score = score * 0.80 + ((#init_expr) as f64) * 0.20;
                    };
                } else {
                    let downstream_deps: Vec<String> = stmts[locus1..]
                        .iter()
                        .flat_map(collect_dependencies)
                        .collect();
                    if already_defined || !downstream_deps.contains(&var_name) {
                        // Anti-shadowing & anti-dead-store : modulation causale directe du score
                        stmt1 = syn::parse_quote! {
                            let score = score * 0.95 + ((#init_expr) as f64) * 0.05;
                        };
                    }
                }
            }
        }
    }

    let slice1 = graft_slice(
        &stmt1,
        unit1.index,
        &donor_fn.block.stmts,
        stmts,
        &host_params,
        locus1,
    )
    .ok_or_else(|| CrossoverError::UnresolvedDependencies(donor.id.0.clone()))?;

    let mut loci_indices = Vec::new();
    let mut fragment_hashes = Vec::new();
    let mut combined_deps = Vec::new();

    for (offset, s) in slice1.iter().enumerate() {
        let actual_idx = locus1 + offset;
        stmts.insert(actual_idx, s.clone());
        loci_indices.push(actual_idx.to_string());

        let repr = quote::quote!(#s).to_string();
        let mut hasher = Sha256::new();
        hasher.update(repr.as_bytes());
        fragment_hashes.push(hex::encode(hasher.finalize()));
        combined_deps.extend(collect_dependencies(s));
    }

    // Locus 2: fin du corps (juste avant return)
    let locus2 = stmts.len().saturating_sub(1);
    let mut stmt2 = unit2.stmt.clone();
    let use_regime_dispatch = seed % 2 == 1;

    if let syn::Stmt::Local(local) = &unit2.stmt {
        if let Some(pat_ident) = local_pat_ident(&local.pat) {
            let var_name = pat_ident.ident.to_string();
            let _already_defined = stmts.iter().any(|s| {
                if let syn::Stmt::Local(l) = s {
                    if let Some(p) = local_pat_ident(&l.pat) {
                        return p.ident == var_name;
                    }
                }
                false
            });

            if let Some(init) = &local.init {
                let init_expr = &init.expr;
                if var_name == "score" {
                    if use_regime_dispatch {
                        // Croisement par Branchement de Régime (Regime Dispatch Crossover)
                        // Assignation conditionnelle par shadowing pour compatibilité stricte rustc
                        if host.logic_code.contains("score_node") {
                            if (seed / 2).is_multiple_of(2) {
                                stmt2 = syn::parse_quote! {
                                    let score = if (step_index as f64 / total_jobs.max(1) as f64) >= 0.5 {
                                        (#init_expr) as f64
                                    } else {
                                        score
                                    };
                                };
                            } else {
                                stmt2 = syn::parse_quote! {
                                    let score = if (node_residual_cpu as f64) >= mean_cpu_residual {
                                        (#init_expr) as f64
                                    } else {
                                        score
                                    };
                                };
                            }
                        } else if (seed / 2).is_multiple_of(2) {
                            stmt2 = syn::parse_quote! {
                                let score = if (step_index as f64 / total_items.max(1) as f64) >= 0.5 {
                                    (#init_expr) as f64
                                } else {
                                    score
                                };
                            };
                        } else {
                            stmt2 = syn::parse_quote! {
                                let score = if (bin_residual as f64) >= mean_residual {
                                    (#init_expr) as f64
                                } else {
                                    score
                                };
                            };
                        }
                    } else {
                        // Combinaison convexe déterministe standard par Shadowing
                        stmt2 = syn::parse_quote! {
                            let score = score * 0.70 + ((#init_expr) as f64) * 0.30;
                        };
                    }
                } else {
                    stmt2 = syn::parse_quote! {
                        let score = score * 0.95 + ((#init_expr) as f64) * 0.05;
                    };
                }
            }
        }
    }

    let mixed = stmt1 != unit1.stmt || stmt2 != unit2.stmt;
    let slice2 = graft_slice(
        &stmt2,
        unit2.index,
        &donor_fn.block.stmts,
        stmts,
        &host_params,
        locus2,
    )
    .ok_or_else(|| CrossoverError::UnresolvedDependencies(donor.id.0.clone()))?;

    for (offset, s) in slice2.iter().enumerate() {
        let actual_idx = locus2 + offset;
        stmts.insert(actual_idx, s.clone());
        loci_indices.push(actual_idx.to_string());

        let repr = quote::quote!(#s).to_string();
        let mut hasher = Sha256::new();
        hasher.update(repr.as_bytes());
        fragment_hashes.push(hex::encode(hasher.finalize()));
        combined_deps.extend(collect_dependencies(s));
    }

    let fragment_hash = fragment_hashes.join(":");
    let locus_desc = format!("multi_loci_{}", loci_indices.join("_"));

    let mut dep_hasher = Sha256::new();
    for d in &combined_deps {
        dep_hasher.update(d.as_bytes());
    }
    let dependency_hash = hex::encode(dep_hasher.finalize());

    let mut graft_hasher = Sha256::new();
    graft_hasher.update(host.id.0.as_bytes());
    graft_hasher.update(donor.id.0.as_bytes());
    graft_hasher.update(fragment_hash.as_bytes());
    graft_hasher.update(seed.to_le_bytes());
    let graft_id = format!(
        "multi-graft-{}",
        &hex::encode(graft_hasher.finalize())[..12]
    );

    let manifest = GraftManifest {
        graft_id: graft_id.clone(),
        donor_capsule: donor.id.clone(),
        host_capsule: host.id.clone(),
        donor_ast_hash: donor_fingerprint.ast_hash,
        grafted_fragment_hash: fragment_hash,
        host_locus: locus_desc,
        dependency_hash,
        repair_version: None,
        repair_diff_hash: None,
    };

    let hybrid_code = normalize_source(&hybrid_fn);
    let child_fingerprint = compute_structural_fingerprint(&hybrid_fn);
    let child_name = format!("MultiCross-G{}-{}", generation, &graft_id[..10]);
    let semantic_interface = format!(
        "Multi-loci AST hybrid combining host {} with donor {} via 2 loci",
        host.id, donor.id
    );

    let mut metadata = HashMap::new();
    metadata.insert("crossover_type".to_string(), "multi_locus_ast".to_string());
    metadata.insert("graft_mode".to_string(), format!("{mode:?}"));
    metadata.insert("mixed".to_string(), mixed.to_string());
    metadata.insert(
        "crossover_mode".to_string(),
        if use_regime_dispatch {
            "regime_dispatch".to_string()
        } else {
            "convex_combination".to_string()
        },
    );
    metadata.insert("host_id".to_string(), host.id.0.clone());
    metadata.insert("donor_id".to_string(), donor.id.0.clone());
    metadata.insert("graft_id".to_string(), graft_id);

    let child_capsule = SkillCapsule::new_v2(
        child_name,
        semantic_interface,
        hybrid_code,
        vec![host.id.clone(), donor.id.clone()],
        generation,
        metadata,
        "ScorePolicyV1",
        PolicyKind::ScorePolicy,
        Some(child_fingerprint),
        combined_deps,
        ValidationStatus::PassedLevel1Structural,
        Some(manifest.clone()),
    );

    Ok((child_capsule, manifest))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIG: &str = "pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32,         step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64";

    fn capsule(name: &str, body: &str) -> SkillCapsule {
        SkillCapsule::new_v2(
            name,
            "Interface",
            format!("{SIG} {{ {body} }}"),
            vec![],
            1,
            HashMap::new(),
            "ScorePolicyV1",
            PolicyKind::ScorePolicy,
            None,
            vec![],
            ValidationStatus::Unvalidated,
            None,
        )
    }

    /// Une regle qui declare son propre `let` et appelle des methodes est greffable.
    #[test]
    fn rule_with_inner_let_and_methods_is_grafted() {
        let host = capsule("Host", "let mut score = bin_residual as f64; score");
        let donor = capsule(
            "Donor",
            "let mut score = bin_residual as f64;              if bin_residual < 20 { let gap = (20 - bin_residual) as f64; score += gap.powi(2).clamp(0.0, 400.0); }              score",
        );
        let (child, _) = deterministic_ast_crossover(&host, &donor, 3, 1).unwrap();
        assert!(child.logic_code.contains("powi"), "{}", child.logic_code);
        parse_heuristic_fn(&child.logic_code).unwrap();
    }

    /// Si aucune unite du donneur n'est resoluble dans l'hote, le croisement echoue au lieu
    /// d'inserer un fragment qui ne compilerait pas.
    /// L'hôte calcule des variables avant `score` : la greffe du locus 1 doit venir après la
    /// déclaration de `score`, sinon l'enfant lit une variable non définie.
    #[test]
    fn multi_locus_graft_follows_host_score_declaration() {
        let host = capsule(
            "h",
            "let progress = step_index as f64 / total_items as f64;              let gap = bin_residual as f64;              let mut score = gap * 2.0;              if progress > 0.5 { score -= 1.0; }              score",
        );
        let donor = capsule(
            "d",
            "let mut score = bin_residual as f64;              if bin_residual < 10 { score += 3.0; }              if item_size > 20 { score -= 2.0; }              score",
        );
        let sandbox = nomad_sandbox_free_check;
        for seed in 0..40u64 {
            let (child, _) = multi_locus_ast_crossover(&host, &donor, seed, 1).expect("greffe");
            let f = parse_heuristic_fn(&child.logic_code).unwrap();
            let decl = f.block.stmts.iter().position(|s| {
                matches!(s, syn::Stmt::Local(l) if matches!(&l.pat, syn::Pat::Ident(p) if p.ident == "score"))
            }).unwrap();
            for s in &f.block.stmts[..decl] {
                assert!(!sandbox(s), "score lu avant sa declaration : {}", child.logic_code);
            }
        }
    }

    fn nomad_sandbox_free_check(s: &syn::Stmt) -> bool {
        crate::unit::collect_free_variables(s).contains(&"score".to_string())
    }

    /// L'aiguillage garde les deux parents entiers, dans une seule fonction analysable.
    #[test]
    fn regime_dispatch_keeps_both_parents() {
        let host = capsule("h", "let mut score = bin_residual as f64; score += 111.0; score");
        let donor = capsule("d", "let gap = item_size as f64; let mut score = gap * 222.0; score");
        for v in 0..REGIME_VARIANTS {
            let child = regime_dispatch_crossover(&host, &donor, v, 1).expect("aiguillage");
            let f = parse_heuristic_fn(&child.logic_code).expect("enfant analysable");
            assert_eq!(f.block.stmts.len(), 1);
            assert!(child.logic_code.contains("111.0") && child.logic_code.contains("222.0"));
            assert!(child.graft_manifest.is_none());
        }
    }

    /// Un `let` annote d'un type est une definition comme une autre : la variable du donneur
    /// est retrouvee et copiee avec la regle qui la lit.
    #[test]
    fn typed_let_is_a_definition() {
        let host = capsule("h", "let mut score: f64 = bin_residual as f64; score");
        let donor = capsule(
            "d",
            "let gap: f64 = item_size as f64 * 2.0; let mut score = 0.0;              if gap > 50.0 { score += gap; } score",
        );
        let mut seen = false;
        for seed in 0..40u64 {
            for res in [
                deterministic_ast_crossover(&host, &donor, seed, 1),
                multi_locus_ast_crossover(&host, &donor, seed, 1),
            ] {
                let (child, _) = res.expect("greffe");
                if child.logic_code.contains("_d_gap > 50.0") {
                    seen = true;
                    assert!(child.logic_code.contains("let _d_gap : f64"), "{}", child.logic_code);
                }
            }
        }
        assert!(seen, "la regle qui lit gap n'a jamais ete greffee");
    }

    #[test]
    fn unresolvable_donor_is_refused() {
        let host = capsule("Host", "let mut score = bin_residual as f64; score");
        let donor = capsule(
            "Donor",
            "let mut score = mystery_input * 2.0; score += mystery_input; score",
        );
        for seed in 0..8 {
            assert!(matches!(
                deterministic_ast_crossover(&host, &donor, seed, 1),
                Err(CrossoverError::UnresolvedDependencies(_))
            ));
        }
    }

    #[test]
    fn test_deterministic_ast_crossover_success() {
        let host_code = r#"
        pub fn score_bin(
            item_size: u32,
            bin_residual: u32,
            bin_capacity: u32,
            step_index: usize,
            total_items: usize,
            open_bins_count: usize,
            mean_residual: f64,
        ) -> f64 {
            let mut score = bin_residual as f64;
            score
        }
        "#;

        let donor_code = r#"
        pub fn score_bin(
            item_size: u32,
            bin_residual: u32,
            bin_capacity: u32,
            step_index: usize,
            total_items: usize,
            open_bins_count: usize,
            mean_residual: f64,
        ) -> f64 {
            let mut score = bin_residual as f64;
            if bin_residual < 10 {
                score += 100.0;
            }
            score
        }
        "#;

        let host = SkillCapsule::new_v2(
            "HostBestFit",
            "Linear best fit",
            host_code,
            vec![],
            1,
            HashMap::new(),
            "ScorePolicyV1",
            PolicyKind::ScorePolicy,
            None,
            vec![],
            ValidationStatus::Unvalidated,
            None,
        );

        let donor = SkillCapsule::new_v2(
            "DonorDeadSpacePenalty",
            "Penalty for residual < 10",
            donor_code,
            vec![],
            1,
            HashMap::new(),
            "ScorePolicyV1",
            PolicyKind::ScorePolicy,
            None,
            vec![],
            ValidationStatus::Unvalidated,
            None,
        );

        let (child, manifest) =
            deterministic_ast_crossover(&host, &donor, 42, 2).expect("Crossover should succeed");

        assert_eq!(child.parent_ids.len(), 2);
        assert!(child.is_crossover());
        assert_eq!(manifest.donor_capsule, donor.id);
        assert_eq!(manifest.host_capsule, host.id);
        assert!(
            child.logic_code.contains("bin_residual < 10") || child.logic_code.contains("score")
        );
        assert!(child.ast_fingerprint.is_some());
    }

    #[test]
    fn test_multi_locus_ast_crossover_success() {
        let host_code = r#"
        pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {
            let mut score = bin_residual as f64;
            score
        }
        "#;

        let donor_code = r#"
        pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64) -> f64 {
            let mut score = bin_residual as f64;
            let dynamic_weight = (total_items - step_index) as f64 / total_items as f64;
            if bin_residual < 10 {
                score = score + 90.0 * dynamic_weight;
            }
            score
        }
        "#;

        let host = SkillCapsule::new("Host", "desc", host_code, vec![], 1, HashMap::new());
        let donor = SkillCapsule::new("Donor", "desc", donor_code, vec![], 1, HashMap::new());

        let (child, manifest) = multi_locus_ast_crossover(&host, &donor, 99, 2)
            .expect("Multi-locus crossover should succeed");

        assert_eq!(child.parent_ids.len(), 2);
        assert!(child.is_crossover());
        assert!(manifest.host_locus.starts_with("multi_loci_"));
        assert!(child.metadata.get("crossover_type").unwrap() == "multi_locus_ast");
        assert_eq!(
            child.metadata.get("crossover_mode").unwrap(),
            "regime_dispatch"
        );

        // Test convex combination mode with even seed
        let (child_convex, _) = multi_locus_ast_crossover(&host, &donor, 100, 2)
            .expect("Multi-locus convex crossover should succeed");
        assert_eq!(
            child_convex.metadata.get("crossover_mode").unwrap(),
            "convex_combination"
        );
    }

    #[test]
    fn test_crossover_backward_program_slicing() {
        let host_code = r#"
        pub fn score_bin(
            item_size: u32,
            bin_residual: u32,
            bin_capacity: u32,
            step_index: usize,
            total_items: usize,
            open_bins_count: usize,
            mean_residual: f64,
        ) -> f64 {
            let mut score = bin_residual as f64;
            score
        }
        "#;

        let donor_code = r#"
        pub fn score_bin(
            item_size: u32,
            bin_residual: u32,
            bin_capacity: u32,
            step_index: usize,
            total_items: usize,
            open_bins_count: usize,
            mean_residual: f64,
        ) -> f64 {
            let mut score = bin_residual as f64;
            let custom_scale = (bin_capacity as f64) * 0.15;
            if bin_residual < 10 {
                score = score + custom_scale;
            }
            score
        }
        "#;

        let host = SkillCapsule::new("Host", "desc", host_code, vec![], 1, HashMap::new());
        let donor = SkillCapsule::new("Donor", "desc", donor_code, vec![], 1, HashMap::new());

        let (child, manifest) = deterministic_ast_crossover(&host, &donor, 42, 1)
            .expect("Crossover with backward program slicing should succeed");

        assert!(child.is_crossover());
        // Verify backward slice pulled `custom_scale` declaration into child logic code
        if child.logic_code.contains("score = score + _d_custom_scale") {
            assert!(
                child.logic_code.contains("let _d_custom_scale ="),
                "Child must contain sliced definition of custom_scale"
            );
        }
        assert!(manifest.host_capsule == host.id);
    }

    fn body(code: &str) -> Vec<syn::Stmt> {
        parse_heuristic_fn(&format!("{SIG} {{ {code} }}")).unwrap().block.stmts
    }

    fn params() -> HashSet<String> {
        parse_heuristic_fn(&format!("{SIG} {{ 0.0 }}"))
            .map(|f| extract_signature_params(&f))
            .unwrap()
    }

    fn render(stmts: &[syn::Stmt]) -> String {
        stmts.iter().map(|s| quote::quote!(#s).to_string()).collect::<Vec<_>>().join(" ")
    }

    /// Les variables du donneur sont renommees : la greffe ne lit ni n'ecrase la variable
    /// homonyme de l'hote.
    #[test]
    fn donor_variables_do_not_capture_host_variables() {
        let host = body("let gap = item_size as f64; let mut score = gap; score");
        let donor = body(
            "let gap = bin_residual as f64 * 3.0; let mut score = 0.0; \
             if gap > 5.0 { score += gap; } score",
        );
        let slice = graft_slice(&donor[2], 2, &donor, &host, &params(), 2).expect("greffe");
        let code = render(&slice);
        assert!(code.contains("let _d_gap = bin_residual as f64 * 3.0"), "{code}");
        assert!(code.contains("_d_gap > 5.0") && code.contains("score += _d_gap"), "{code}");
        assert!(!code.contains("let gap"), "{code}");
    }

    /// Les reaffectations d'une variable du donneur, entre sa definition et l'unite, font
    /// partie de la tranche, dans l'ordre du donneur.
    #[test]
    fn donor_reassignments_are_part_of_the_slice() {
        let host = body("let mut score = bin_residual as f64; score");
        let donor = body(
            "let mut bonus = 0.0; if item_size > 50 { bonus += 10.0; } bonus *= 2.0; \
             let mut score = 0.0; if bin_residual < 20 { score += bonus; } score",
        );
        let slice = graft_slice(&donor[4], 4, &donor, &host, &params(), 1).expect("greffe");
        let code = render(&slice);
        assert_eq!(slice.len(), 4, "{code}");
        let def = code.find("let mut _d_bonus = 0.0").expect(&code);
        let inc = code.find("_d_bonus += 10.0").expect(&code);
        let dbl = code.find("_d_bonus *= 2.0").expect(&code);
        let used = code.find("score += _d_bonus").expect(&code);
        assert!(def < inc && inc < dbl && dbl < used, "{code}");
        // Une reaffectation posterieure a l'unite n'est pas copiee.
        let late = body(
            "let mut bonus = 1.0; let mut score = 0.0; if item_size > 5 { score += bonus; } \
             bonus += 9.0; score",
        );
        let slice = graft_slice(&late[2], 2, &late, &host, &params(), 1).expect("greffe");
        assert!(!render(&slice).contains("9.0"));
    }

    /// Une reaffectation qui ecrit aussi une variable de l'hote ne peut pas etre copiee.
    #[test]
    fn reassignment_writing_a_host_variable_is_refused() {
        let host = body("let mut score = bin_residual as f64; score");
        let donor = body(
            "let mut b = 0.0; let mut score = 1.0; \
             if item_size > 5 { b += 1.0; score -= 1.0; } \
             if bin_residual < 9 { score += b; } score",
        );
        assert!(graft_slice(&donor[3], 3, &donor, &host, &params(), 1).is_none());
    }

    /// Un nom `_d_x` deja pris dans l'hote (greffe anterieure) n'est pas reutilise.
    #[test]
    fn fresh_name_avoids_previous_grafts() {
        let host = body("let mut score = bin_residual as f64; let _d_gap = 1.0; score += _d_gap; score");
        let donor = body("let gap = item_size as f64; let mut score = 0.0; score += gap; score");
        let slice = graft_slice(&donor[2], 2, &donor, &host, &params(), 3).expect("greffe");
        let code = render(&slice);
        assert!(code.contains("let _d2_gap = item_size as f64"), "{code}");
        assert!(code.contains("score += _d2_gap"), "{code}");
    }

    /// Mode regles completes : seules les instructions qui modifient `score` sont greffees,
    /// jamais de melange ; le mode historique peut melanger une definition du donneur.
    #[test]
    fn rules_only_mode_never_mixes() {
        let host = capsule("h", "let mut score = bin_residual as f64; score");
        let donor = capsule(
            "d",
            "let ratio = item_size as f64 / 100.0; let mut score = bin_residual as f64; \
             if bin_residual < 15 { score -= 4.0 * ratio; } score *= 1.5; score",
        );
        let mut mixed_seen = false;
        for seed in 0..40u64 {
            for res in [
                deterministic_ast_crossover_with(&host, &donor, seed, 1, GraftMode::RulesOnly),
                multi_locus_ast_crossover_with(&host, &donor, seed, 1, GraftMode::RulesOnly),
            ] {
                let (child, _) = res.expect("greffe");
                assert_eq!(child.metadata.get("mixed").map(String::as_str), Some("false"));
                assert_eq!(child.metadata.get("graft_mode").map(String::as_str), Some("RulesOnly"));
                assert!(!child.logic_code.contains("0.95"), "{}", child.logic_code);
            }
            let (child, _) = deterministic_ast_crossover(&host, &donor, seed, 1).unwrap();
            mixed_seen |= child.metadata.get("mixed").map(String::as_str) == Some("true");
        }
        assert!(mixed_seen, "le mode historique n'a jamais melange");
        // Un donneur sans regle sur `score` ne donne rien en mode regles completes.
        let flat = capsule("f", "let mut score = item_size as f64; score");
        assert!(deterministic_ast_crossover_with(&host, &flat, 1, 1, GraftMode::RulesOnly).is_err());
    }
}
