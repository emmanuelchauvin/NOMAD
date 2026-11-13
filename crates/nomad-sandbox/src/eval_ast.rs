use nomad_core::SchedulingInputs;
use std::collections::HashMap;
use syn::{
    BinOp, Expr, ExprBinary, ExprCall, ExprCast, ExprIf, ExprLit, ExprMethodCall, ExprParen,
    ExprPath, ExprUnary, ItemFn, Lit, Local, Pat, PatIdent, Stmt, UnOp,
};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum EvalError {
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),
    #[error("Fuel limit exceeded during AST interpretation")]
    FuelExceeded,
    #[error("Maximum AST expression recursion depth exceeded ({current} > {max})")]
    RecursionLimitExceeded { current: usize, max: usize },
    #[error("Memory limit or collection capacity exceeded: {0}")]
    MemoryLimitExceeded(String),
    #[error("Pathological input rejected: {0}")]
    InvalidInput(String),
    #[error("Pathological numerical value encountered (NaN, Inf, or division by zero)")]
    PathologicalNumericalValue,
    #[error("Unsupported AST expression: {0}")]
    UnsupportedExpr(String),
}

#[inline]
fn check_finite(val: f64) -> Result<f64, EvalError> {
    if val.is_finite() {
        Ok(val)
    } else {
        Err(EvalError::PathologicalNumericalValue)
    }
}

/// ============================================================================
/// ScorePolicyContext : Contexte d'Évaluation d'un Placement de Bac
/// ============================================================================
/// Données d'état instantanées fournies à l'AST `score_bin` pour scorer un bac spécifique.
#[derive(Debug, Clone, Copy)]
pub struct ScorePolicyContext {
    /// Taille de l'objet courant en cours de placement.
    pub item_size: u32,
    /// Taille du prochain objet dans le flux (Lookahead borné k=2, ou 0 si dernier objet).
    pub next_item_size: u32,
    /// Espace résiduel actuel du bac candidat.
    pub bin_residual: u32,
    /// Capacité maximale théorique du bac (ex: 100).
    pub bin_capacity: u32,
    /// Indice séquentiel de l'objet dans la séquence.
    pub step_index: usize,
    /// Nombre total d'objets dans l'instance.
    pub total_items: usize,
    /// Nombre de bacs ouverts à cet instant.
    pub open_bins_count: usize,
    /// Moyenne d'espace résiduel des bacs ouverts.
    pub mean_residual: f64,
    /// Espace résiduel après insertion de l'objet (bin_residual - item_size).
    pub post_residual: u32,
    /// Nombre total de bacs ouverts pouvant accueillir l'objet courant (si == 1, dernière chance).
    pub fitting_bins_count: usize,
}

/// ============================================================================
/// AstEvalOutcome : Résultat d'Exécution et Profil d'Interprétation
/// ============================================================================
/// Bilan d'une exécution de `score_bin` renvoyant le score, le fuel consommé
/// et les statistiques d'activation des branches.
#[derive(Debug, Clone, PartialEq)]
pub struct AstEvalOutcome {
    /// Score numérique attribué au bac (le bac ayant le score minimal ou maximal est choisi).
    pub score: f64,
    /// Unités de fuel logique consommées lors de l'évaluation (protection anti-boucle infinie).
    pub fuel_consumed: u64,
    /// Nombre de branches conditionnelles (`if`) empruntées.
    pub branches_taken: usize,
    /// Nombre de modifications apportées à la variable de score.
    pub score_mutations: usize,
}

/// ============================================================================
/// AstInterpreter : Interpréteur Déterministe d'AST Rust avec Limitation de Ressources
/// ============================================================================
/// Interpréteur déterministe avec limitation de ressources (fuel, profondeur, mémoire),
/// sans isolation de processus au sens OS.
pub struct AstInterpreter<'a> {
    item_fn: &'a ItemFn,
    fuel_limit: u64,
    max_depth: usize,
    max_vars: usize,
}

impl<'a> AstInterpreter<'a> {
    /// Initialise l'interpréteur avec la fonction AST, un plafond de fuel,
    /// une profondeur maximale de récursion de 48 et une limite de 128 variables.
    pub fn new(item_fn: &'a ItemFn, fuel_limit: u64) -> Self {
        Self {
            item_fn,
            fuel_limit,
            max_depth: 48,
            max_vars: 128,
        }
    }

    /// Initialise l'interpréteur avec des limites de ressources configurables.
    pub fn with_limits(
        item_fn: &'a ItemFn,
        fuel_limit: u64,
        max_depth: usize,
        max_vars: usize,
    ) -> Self {
        Self {
            item_fn,
            fuel_limit,
            max_depth,
            max_vars,
        }
    }

    /// ========================================================================
    /// eval_with_vars : Interprétation Séquentielle Générique avec Variables
    /// ========================================================================
    pub fn eval_with_vars(
        &self,
        vars: HashMap<String, f64>,
        default_fallback: f64,
    ) -> Result<AstEvalOutcome, EvalError> {
        if vars.len() > self.max_vars {
            return Err(EvalError::MemoryLimitExceeded(format!(
                "Initial variable count {} exceeds maximum {}",
                vars.len(),
                self.max_vars
            )));
        }

        // Les valeurs d'entree sont typees d'apres la signature de la fonction, sinon
        // d'apres le nom canonique du parametre.
        let signature_types = self.signature_types();
        let mut env: HashMap<String, Val> = vars
            .into_iter()
            .map(|(name, v)| {
                let ty = signature_types
                    .get(&name)
                    .copied()
                    .unwrap_or_else(|| canonical_input_type(&name));
                (name, Val { v, ty })
            })
            .collect();

        let mut fuel_consumed = 0u64;
        let mut branches_taken = 0usize;
        let mut score_mutations = 0usize;
        let mut return_val = None;

        let stmts = &self.item_fn.block.stmts;
        for (i, stmt) in stmts.iter().enumerate() {
            fuel_consumed += 5;
            if fuel_consumed > self.fuel_limit {
                return Err(EvalError::FuelExceeded);
            }
            // Portee de la fonction : aucune restauration a la sortie.
            let mut shadowed = Vec::new();
            let val = self.eval_stmt(
                stmt,
                &mut env,
                &mut shadowed,
                &mut fuel_consumed,
                &mut branches_taken,
                &mut score_mutations,
                0,
            )?;
            // Seule l'expression finale sans point-virgule est la valeur de la fonction.
            return_val = if i + 1 == stmts.len() { val } else { None };
        }

        let final_score = return_val
            .map(|v| v.v)
            .or_else(|| env.get("score").map(|v| v.v))
            .unwrap_or(default_fallback);

        check_finite(final_score)?;

        Ok(AstEvalOutcome {
            score: final_score,
            fuel_consumed,
            branches_taken,
            score_mutations,
        })
    }

    /// Types des parametres declares dans la signature de la fonction.
    fn signature_types(&self) -> HashMap<String, Ty> {
        let mut types = HashMap::new();
        for input in &self.item_fn.sig.inputs {
            if let syn::FnArg::Typed(pat_type) = input {
                if let Pat::Ident(PatIdent { ident, .. }) = &*pat_type.pat {
                    if let Some(ty) = ty_from_type(&pat_type.ty) {
                        types.insert(ident.to_string(), ty);
                    }
                }
            }
        }
        types
    }

    /// ========================================================================
    /// eval : Algorithme d'Interprétation Séquentielle de `score_bin`
    /// ========================================================================
    pub fn eval(&self, ctx: &ScorePolicyContext) -> Result<AstEvalOutcome, EvalError> {
        // Validation stricte des entrées pour rejeter les cas pathologiques
        if ctx.bin_capacity == 0 {
            return Err(EvalError::InvalidInput(
                "bin_capacity must be > 0".to_string(),
            ));
        }
        if ctx.item_size == 0 {
            return Err(EvalError::InvalidInput("item_size must be > 0".to_string()));
        }
        if ctx.item_size > ctx.bin_capacity {
            return Err(EvalError::InvalidInput(format!(
                "item_size ({}) exceeds bin_capacity ({})",
                ctx.item_size, ctx.bin_capacity
            )));
        }
        if !ctx.mean_residual.is_finite() {
            return Err(EvalError::InvalidInput(
                "mean_residual must be a finite number".to_string(),
            ));
        }

        let mut vars = HashMap::new();
        vars.insert("item_size".to_string(), ctx.item_size as f64);
        vars.insert("next_item_size".to_string(), ctx.next_item_size as f64);
        vars.insert("bin_residual".to_string(), ctx.bin_residual as f64);
        vars.insert("bin_capacity".to_string(), ctx.bin_capacity as f64);
        vars.insert("step_index".to_string(), ctx.step_index as f64);
        vars.insert("total_items".to_string(), ctx.total_items as f64);
        vars.insert("open_bins_count".to_string(), ctx.open_bins_count as f64);
        vars.insert("mean_residual".to_string(), ctx.mean_residual);
        vars.insert("post_residual".to_string(), ctx.post_residual as f64);
        vars.insert(
            "fitting_bins_count".to_string(),
            ctx.fitting_bins_count as f64,
        );

        self.eval_with_vars(vars, ctx.post_residual as f64)
    }

    /// ========================================================================
    /// eval_scheduling : Interprétation Séquentielle de `score_node`
    /// ========================================================================
    pub fn eval_scheduling(&self, ctx: &SchedulingInputs) -> Result<AstEvalOutcome, EvalError> {
        // Validation stricte des entrées pour l'ordonnancement
        if ctx.node_capacity_cpu == 0 || ctx.node_capacity_ram == 0 {
            return Err(EvalError::InvalidInput(
                "node capacity CPU and RAM must be > 0".to_string(),
            ));
        }
        if ctx.job_cpu == 0 && ctx.job_ram == 0 {
            return Err(EvalError::InvalidInput(
                "job CPU and RAM cannot both be 0".to_string(),
            ));
        }
        if ctx.job_cpu > ctx.node_capacity_cpu || ctx.job_ram > ctx.node_capacity_ram {
            return Err(EvalError::InvalidInput(
                "job requirements exceed node capacity".to_string(),
            ));
        }
        if !ctx.mean_cpu_residual.is_finite() || !ctx.mean_ram_residual.is_finite() {
            return Err(EvalError::InvalidInput(
                "mean CPU and RAM residuals must be finite numbers".to_string(),
            ));
        }

        let mut vars = HashMap::new();
        vars.insert("job_cpu".to_string(), ctx.job_cpu as f64);
        vars.insert("job_ram".to_string(), ctx.job_ram as f64);
        vars.insert("next_job_cpu".to_string(), ctx.next_job_cpu as f64);
        vars.insert("next_job_ram".to_string(), ctx.next_job_ram as f64);
        vars.insert(
            "node_residual_cpu".to_string(),
            ctx.node_residual_cpu as f64,
        );
        vars.insert(
            "node_residual_ram".to_string(),
            ctx.node_residual_ram as f64,
        );
        vars.insert(
            "node_capacity_cpu".to_string(),
            ctx.node_capacity_cpu as f64,
        );
        vars.insert(
            "node_capacity_ram".to_string(),
            ctx.node_capacity_ram as f64,
        );
        vars.insert("step_index".to_string(), ctx.step_index as f64);
        vars.insert("total_jobs".to_string(), ctx.total_jobs as f64);
        vars.insert(
            "active_nodes_count".to_string(),
            ctx.active_nodes_count as f64,
        );
        vars.insert("mean_cpu_residual".to_string(), ctx.mean_cpu_residual);
        vars.insert("mean_ram_residual".to_string(), ctx.mean_ram_residual);

        let fallback = (ctx.node_residual_cpu + ctx.node_residual_ram) as f64;
        self.eval_with_vars(vars, fallback)
    }

    /// Evalue une instruction. Renvoie `Some(valeur)` pour une expression sans
    /// point-virgule, `None` sinon. Les variables declarees sont notees dans `shadowed`
    /// avec leur valeur precedente, pour etre restaurees a la sortie du bloc.
    #[allow(clippy::too_many_arguments)]
    fn eval_stmt(
        &self,
        stmt: &Stmt,
        vars: &mut HashMap<String, Val>,
        shadowed: &mut Vec<(String, Option<Val>)>,
        fuel: &mut u64,
        branches: &mut usize,
        mutations: &mut usize,
        depth: usize,
    ) -> Result<Option<Val>, EvalError> {
        match stmt {
            Stmt::Local(Local { pat, init, .. }) => {
                let (name, annotation) = binding_of(pat)?;
                let value = match init {
                    Some(local_init) => {
                        if local_init.diverge.is_some() {
                            return Err(EvalError::UnsupportedExpr("let ... else".to_string()));
                        }
                        let v = self.eval_expr(
                            &local_init.expr,
                            vars,
                            fuel,
                            branches,
                            mutations,
                            depth + 1,
                        )?;
                        Some(coerce(v, annotation)?)
                    }
                    None => None,
                };
                self.declare(vars, shadowed, name, value)?;
                Ok(None)
            }
            Stmt::Item(syn::Item::Const(item_const)) => {
                let annotation = ty_from_type(&item_const.ty);
                let v = self.eval_expr(&item_const.expr, vars, fuel, branches, mutations, depth + 1)?;
                self.declare(
                    vars,
                    shadowed,
                    Some(item_const.ident.to_string()),
                    Some(coerce(v, annotation)?),
                )?;
                Ok(None)
            }
            Stmt::Expr(expr, semi) => {
                let v = self.eval_expr(expr, vars, fuel, branches, mutations, depth + 1)?;
                Ok(if semi.is_none() { Some(v) } else { None })
            }
            _ => Err(EvalError::UnsupportedExpr(
                "Unsupported statement".to_string(),
            )),
        }
    }

    /// Declare une variable dans la portee courante. `name == None` : motif `_`.
    /// `value == None` : declaration sans valeur (`let x;`), l'ancienne valeur est masquee.
    fn declare(
        &self,
        vars: &mut HashMap<String, Val>,
        shadowed: &mut Vec<(String, Option<Val>)>,
        name: Option<String>,
        value: Option<Val>,
    ) -> Result<(), EvalError> {
        let Some(name) = name else { return Ok(()) };
        if vars.len() >= self.max_vars && !vars.contains_key(&name) {
            return Err(EvalError::MemoryLimitExceeded(format!(
                "Variable count exceeded maximum limit of {}",
                self.max_vars
            )));
        }
        let previous = match value {
            Some(v) => vars.insert(name.clone(), v),
            None => vars.insert(name.clone(), Val { v: 0.0, ty: Ty::Uninit }),
        };
        shadowed.push((name, previous));
        Ok(())
    }

    /// Evalue un bloc avec sa propre portee : les `let` du bloc ne survivent pas au bloc,
    /// les affectations a des variables exterieures sont conservees.
    fn eval_block(
        &self,
        block: &syn::Block,
        vars: &mut HashMap<String, Val>,
        fuel: &mut u64,
        branches: &mut usize,
        mutations: &mut usize,
        depth: usize,
    ) -> Result<Val, EvalError> {
        let mut shadowed: Vec<(String, Option<Val>)> = Vec::new();
        let mut last = None;
        for (i, stmt) in block.stmts.iter().enumerate() {
            let v = self.eval_stmt(stmt, vars, &mut shadowed, fuel, branches, mutations, depth)?;
            last = if i + 1 == block.stmts.len() { v } else { None };
        }
        for (name, previous) in shadowed.into_iter().rev() {
            match previous {
                Some(v) => {
                    vars.insert(name, v);
                }
                None => {
                    vars.remove(&name);
                }
            }
        }
        Ok(last.unwrap_or(UNIT))
    }

    fn eval_expr(
        &self,
        expr: &Expr,
        vars: &mut HashMap<String, Val>,
        fuel: &mut u64,
        branches: &mut usize,
        mutations: &mut usize,
        depth: usize,
    ) -> Result<Val, EvalError> {
        if depth > self.max_depth {
            return Err(EvalError::RecursionLimitExceeded {
                current: depth,
                max: self.max_depth,
            });
        }

        *fuel += 2;
        if *fuel > self.fuel_limit {
            return Err(EvalError::FuelExceeded);
        }

        match expr {
            Expr::Lit(ExprLit { lit, .. }) => match lit {
                Lit::Float(f) => f
                    .base10_parse::<f64>()
                    .map(float)
                    .map_err(|e| EvalError::ParseError(e.to_string())),
                Lit::Int(i) => {
                    let v = i
                        .base10_parse::<u64>()
                        .map_err(|e| EvalError::ParseError(e.to_string()))?
                        as f64;
                    match i.suffix() {
                        "" => int_result(v, Ty::IntLit),
                        "f32" | "f64" => Ok(float(v)),
                        suffix => match ty_from_name(suffix) {
                            Some(ty) => int_result(v, ty),
                            None => Err(EvalError::UnsupportedExpr(format!(
                                "Unsupported literal suffix: {}",
                                suffix
                            ))),
                        },
                    }
                }
                Lit::Bool(b) => Ok(boolean(b.value)),
                _ => Err(EvalError::UnsupportedExpr(format!(
                    "Unsupported literal: {:?}",
                    lit
                ))),
            },
            Expr::Path(ExprPath { path, .. }) => {
                if let Some(ident) = path.get_ident() {
                    let name = ident.to_string();
                    match vars.get(&name) {
                        Some(v) if v.ty != Ty::Uninit => Ok(*v),
                        _ => Err(EvalError::UndefinedVariable(name)),
                    }
                } else {
                    path_constant(path)
                }
            }
            Expr::Cast(ExprCast { expr, ty, .. }) => {
                let v = self.eval_expr(expr, vars, fuel, branches, mutations, depth + 1)?;
                let target = ty_from_type(ty).ok_or_else(|| {
                    EvalError::UnsupportedExpr("Unsupported cast target type".to_string())
                })?;
                cast(v, target)
            }
            Expr::Paren(ExprParen { expr, .. }) => {
                self.eval_expr(expr, vars, fuel, branches, mutations, depth + 1)
            }
            Expr::Group(group) => {
                self.eval_expr(&group.expr, vars, fuel, branches, mutations, depth + 1)
            }
            Expr::Unary(ExprUnary { op, expr, .. }) => {
                let val = self.eval_expr(expr, vars, fuel, branches, mutations, depth + 1)?;
                match op {
                    UnOp::Neg(_) => match val.ty {
                        Ty::Float => Ok(float(-val.v)),
                        // Rust refuse la negation d'un entier non signe ; le resultat est
                        // traite comme un entier signe.
                        Ty::Int { signed: false, .. } | Ty::IntLit => int_result(-val.v, Ty::IntLit),
                        ty => int_result(-val.v, ty),
                    },
                    UnOp::Not(_) => match val.ty {
                        Ty::Float => Ok(boolean(val.v == 0.0)),
                        _ => Err(EvalError::UnsupportedExpr(
                            "Bitwise not on an integer".to_string(),
                        )),
                    },
                    _ => Err(EvalError::UnsupportedExpr(format!(
                        "Unsupported unary operator: {:?}",
                        op
                    ))),
                }
            }
            Expr::Binary(ExprBinary {
                left, op, right, ..
            }) => {
                // `&&` et `||` n'evaluent l'operande droit que si necessaire, comme en Rust.
                if let BinOp::And(_) | BinOp::Or(_) = op {
                    let l = self.eval_expr(left, vars, fuel, branches, mutations, depth + 1)?;
                    let decided = if matches!(op, BinOp::And(_)) { l.v == 0.0 } else { l.v != 0.0 };
                    if decided {
                        return Ok(boolean(l.v != 0.0));
                    }
                    let r = self.eval_expr(right, vars, fuel, branches, mutations, depth + 1)?;
                    return Ok(boolean(r.v != 0.0));
                }

                if let BinOp::AddAssign(_)
                | BinOp::SubAssign(_)
                | BinOp::MulAssign(_)
                | BinOp::DivAssign(_)
                | BinOp::RemAssign(_) = op
                {
                    let r = self.eval_expr(right, vars, fuel, branches, mutations, depth + 1)?;
                    let name = assigned_name(left)?;
                    let current = match vars.get(&name) {
                        Some(v) if v.ty != Ty::Uninit => *v,
                        _ => return Err(EvalError::UndefinedVariable(name)),
                    };
                    let plain_op = match op {
                        BinOp::AddAssign(_) => Arith::Add,
                        BinOp::SubAssign(_) => Arith::Sub,
                        BinOp::MulAssign(_) => Arith::Mul,
                        BinOp::DivAssign(_) => Arith::Div,
                        _ => Arith::Rem,
                    };
                    let updated = arith(plain_op, current, r)?;
                    if name == "score" {
                        *mutations += 1;
                    }
                    vars.insert(name, updated);
                    return Ok(UNIT);
                }

                let l = self.eval_expr(left, vars, fuel, branches, mutations, depth + 1)?;
                let r = self.eval_expr(right, vars, fuel, branches, mutations, depth + 1)?;
                match op {
                    BinOp::Add(_) => arith(Arith::Add, l, r),
                    BinOp::Sub(_) => arith(Arith::Sub, l, r),
                    BinOp::Mul(_) => arith(Arith::Mul, l, r),
                    BinOp::Div(_) => arith(Arith::Div, l, r),
                    BinOp::Rem(_) => arith(Arith::Rem, l, r),
                    BinOp::Lt(_) => Ok(boolean(l.v < r.v)),
                    BinOp::Le(_) => Ok(boolean(l.v <= r.v)),
                    BinOp::Gt(_) => Ok(boolean(l.v > r.v)),
                    BinOp::Ge(_) => Ok(boolean(l.v >= r.v)),
                    BinOp::Eq(_) => Ok(boolean(l.v == r.v)),
                    BinOp::Ne(_) => Ok(boolean(l.v != r.v)),
                    _ => Err(EvalError::UnsupportedExpr(format!(
                        "Unsupported binary operator: {:?}",
                        op
                    ))),
                }
            }
            Expr::If(ExprIf {
                cond,
                then_branch,
                else_branch,
                ..
            }) => {
                let condition_met =
                    self.eval_expr(cond, vars, fuel, branches, mutations, depth + 1)?.v != 0.0;
                if condition_met {
                    *branches += 1;
                    self.eval_block(then_branch, vars, fuel, branches, mutations, depth + 1)
                } else if let Some((_, else_expr)) = else_branch {
                    self.eval_expr(else_expr, vars, fuel, branches, mutations, depth + 1)
                } else {
                    Ok(UNIT)
                }
            }
            Expr::Assign(assign) => {
                let r =
                    self.eval_expr(&assign.right, vars, fuel, branches, mutations, depth + 1)?;
                let name = assigned_name(&assign.left)?;
                // Une affectation exige une variable declaree (eventuellement sans valeur).
                if !vars.contains_key(&name) {
                    return Err(EvalError::UndefinedVariable(name));
                }
                if name == "score" {
                    *mutations += 1;
                }
                vars.insert(name, r);
                Ok(UNIT)
            }
            Expr::Block(block) => {
                self.eval_block(&block.block, vars, fuel, branches, mutations, depth + 1)
            }
            Expr::MethodCall(ExprMethodCall {
                receiver,
                method,
                args,
                ..
            }) => {
                if args.len() > 8 {
                    return Err(EvalError::MemoryLimitExceeded(format!(
                        "Method argument count {} exceeds limit of 8",
                        args.len()
                    )));
                }
                let recv_val =
                    self.eval_expr(receiver, vars, fuel, branches, mutations, depth + 1)?;
                let mut eval_args = Vec::with_capacity(args.len());
                for arg in args {
                    eval_args.push(self.eval_expr(
                        arg,
                        vars,
                        fuel,
                        branches,
                        mutations,
                        depth + 1,
                    )?);
                }
                call_method(&method.to_string(), recv_val, &eval_args)
            }
            Expr::Call(ExprCall { func, args, .. }) => {
                if args.len() > 8 {
                    return Err(EvalError::MemoryLimitExceeded(format!(
                        "Function argument count {} exceeds limit of 8",
                        args.len()
                    )));
                }
                let mut eval_args = Vec::with_capacity(args.len());
                for arg in args {
                    eval_args.push(self.eval_expr(
                        arg,
                        vars,
                        fuel,
                        branches,
                        mutations,
                        depth + 1,
                    )?);
                }
                let segments: Vec<String> = if let Expr::Path(ExprPath { path, .. }) = &**func {
                    path.segments.iter().map(|s| s.ident.to_string()).collect()
                } else {
                    Vec::new()
                };
                let fn_name = segments.last().cloned().unwrap_or_default();
                let Some((first, rest)) = eval_args.split_first() else {
                    return Err(EvalError::UnsupportedExpr(format!(
                        "Unsupported function call without argument: {}",
                        fn_name
                    )));
                };
                match fn_name.as_str() {
                    // `f64::from(x)` : conversion sans perte vers un flottant.
                    "from" if segments.first().map(String::as_str) == Some("f64") && rest.is_empty() => {
                        Ok(float(first.v))
                    }
                    // Fonctions appelees par chemin (`f64::max(a, b)`, `std::cmp::min(a, b)`)
                    // ou par leur nom seul : meme calcul que la methode correspondante.
                    "min" | "max" | "sqrt" | "abs" | "powi" | "powf" => {
                        call_method(&fn_name, *first, rest)
                    }
                    _ => Err(EvalError::UnsupportedExpr(format!(
                        "Unsupported function call: {}",
                        fn_name
                    ))),
                }
            }
            _ => Err(EvalError::UnsupportedExpr(
                "Unsupported expression construct".to_string(),
            )),
        }
    }
}

/// Type d'une valeur de l'interpreteur. Les booleens sont des flottants 1.0 / 0.0.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Ty {
    Float,
    Int { signed: bool, bits: u8 },
    /// Litteral entier sans suffixe : prend le type de l'autre operande, sinon `i32`.
    IntLit,
    /// Variable declaree sans valeur (`let x;`).
    Uninit,
}

/// Valeur typee. Les entiers sont stockes dans un `f64`, exact jusqu'a 2^53.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Val {
    v: f64,
    ty: Ty,
}

/// Valeur d'une instruction ou d'un bloc sans expression finale.
const UNIT: Val = Val { v: 0.0, ty: Ty::Float };

/// Au-dela de 2^53, un `f64` ne represente plus exactement les entiers.
const EXACT_INT_LIMIT: f64 = 9_007_199_254_740_992.0;

#[derive(Debug, Clone, Copy)]
enum Arith {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

fn float(v: f64) -> Val {
    Val { v, ty: Ty::Float }
}

fn boolean(b: bool) -> Val {
    float(if b { 1.0 } else { 0.0 })
}

fn ty_from_name(name: &str) -> Option<Ty> {
    let int = |signed, bits| Some(Ty::Int { signed, bits });
    match name {
        "f64" | "f32" | "bool" => Some(Ty::Float),
        "u8" => int(false, 8),
        "u16" => int(false, 16),
        "u32" => int(false, 32),
        "u64" | "usize" => int(false, 64),
        "i8" => int(true, 8),
        "i16" => int(true, 16),
        "i32" => int(true, 32),
        "i64" | "isize" => int(true, 64),
        _ => None,
    }
}

fn ty_from_type(ty: &syn::Type) -> Option<Ty> {
    match ty {
        syn::Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .and_then(|s| ty_from_name(&s.ident.to_string())),
        syn::Type::Paren(inner) => ty_from_type(&inner.elem),
        _ => None,
    }
}

/// Type d'une entree fournie par le moteur quand la signature ne la declare pas.
fn canonical_input_type(name: &str) -> Ty {
    match name {
        "item_size" | "next_item_size" | "bin_residual" | "bin_capacity" | "post_residual" => {
            Ty::Int { signed: false, bits: 32 }
        }
        "step_index" | "total_items" | "open_bins_count" | "fitting_bins_count" | "total_jobs" => {
            Ty::Int { signed: false, bits: 64 }
        }
        _ => Ty::Float,
    }
}

/// Bornes d'un type entier, limitees a la plage exacte d'un `f64`.
fn int_bounds(ty: Ty) -> (f64, f64) {
    match ty {
        Ty::Int { signed: false, bits } => (0.0, (2f64.powi(bits as i32) - 1.0).min(EXACT_INT_LIMIT)),
        Ty::Int { signed: true, bits } => {
            let half = 2f64.powi(bits as i32 - 1);
            ((-half).max(-EXACT_INT_LIMIT), (half - 1.0).min(EXACT_INT_LIMIT))
        }
        // Litteral sans suffixe : `i32` en Rust.
        _ => (-2_147_483_648.0, 2_147_483_647.0),
    }
}

/// Resultat d'un calcul entier. Un depassement (ou un non signe negatif) est une erreur,
/// comme la panique de Rust en mode debug.
fn int_result(v: f64, ty: Ty) -> Result<Val, EvalError> {
    let (min, max) = int_bounds(ty);
    if v.is_finite() && v >= min && v <= max {
        Ok(Val { v, ty })
    } else {
        Err(EvalError::PathologicalNumericalValue)
    }
}

/// Type du resultat d'une operation entre deux valeurs.
fn unify(l: Ty, r: Ty) -> Ty {
    match (l, r) {
        // Rust refuse de melanger entier et flottant ; le calcul est fait en flottant.
        (Ty::Float, _) | (_, Ty::Float) | (Ty::Uninit, _) | (_, Ty::Uninit) => Ty::Float,
        (Ty::IntLit, other) => other,
        (typed, _) => typed,
    }
}

fn arith(op: Arith, l: Val, r: Val) -> Result<Val, EvalError> {
    let ty = unify(l.ty, r.ty);
    if ty == Ty::Float {
        let v = match op {
            Arith::Add => l.v + r.v,
            Arith::Sub => l.v - r.v,
            Arith::Mul => l.v * r.v,
            Arith::Div | Arith::Rem if r.v == 0.0 => {
                return Err(EvalError::PathologicalNumericalValue)
            }
            Arith::Div => l.v / r.v,
            Arith::Rem => l.v % r.v,
        };
        return check_finite(v).map(float);
    }
    let v = match op {
        Arith::Add => l.v + r.v,
        Arith::Sub => l.v - r.v,
        Arith::Mul => l.v * r.v,
        Arith::Div | Arith::Rem if r.v == 0.0 => return Err(EvalError::PathologicalNumericalValue),
        // Division entiere : troncature vers zero. Reste : signe du dividende.
        Arith::Div => (l.v / r.v).trunc(),
        Arith::Rem => l.v % r.v,
    };
    int_result(v, ty)
}

/// Conversion `as`, selon les regles de Rust.
fn cast(v: Val, target: Ty) -> Result<Val, EvalError> {
    match (v.ty, target) {
        (_, Ty::Float) => Ok(float(v.v)),
        (Ty::Float, Ty::Int { .. }) => {
            // Flottant vers entier : troncature vers zero, saturation, NaN vers 0.
            let (min, max) = int_bounds(target);
            let t = if v.v.is_nan() { 0.0 } else { v.v.trunc().clamp(min, max) };
            int_result(t, target)
        }
        (_, Ty::Int { signed, bits }) => {
            // Entier vers entier : repliement en complement a deux.
            let modulus = 1i128 << bits;
            let mut w = (v.v as i128).rem_euclid(modulus);
            if signed && w >= modulus / 2 {
                w -= modulus;
            }
            int_result(w as f64, target)
        }
        _ => Err(EvalError::UnsupportedExpr("Unsupported cast".to_string())),
    }
}

/// Applique l'annotation de type d'un `let` ou d'un `const` a un litteral entier.
fn coerce(v: Val, annotation: Option<Ty>) -> Result<Val, EvalError> {
    match (annotation, v.ty) {
        (Some(target @ Ty::Int { .. }), Ty::IntLit) => int_result(v.v, target),
        (Some(Ty::Float), Ty::IntLit | Ty::Int { .. }) => Ok(float(v.v)),
        _ => Ok(v),
    }
}

/// Nom et annotation de type d'un motif de `let`. `None` pour le motif `_`.
fn binding_of(pat: &Pat) -> Result<(Option<String>, Option<Ty>), EvalError> {
    match pat {
        Pat::Ident(PatIdent { ident, subpat: None, .. }) => Ok((Some(ident.to_string()), None)),
        Pat::Wild(_) => Ok((None, None)),
        Pat::Type(pat_type) => {
            let (name, _) = binding_of(&pat_type.pat)?;
            let ty = ty_from_type(&pat_type.ty).ok_or_else(|| {
                EvalError::UnsupportedExpr("Unsupported type annotation".to_string())
            })?;
            Ok((name, Some(ty)))
        }
        _ => Err(EvalError::UnsupportedExpr(
            "Unsupported let pattern".to_string(),
        )),
    }
}

/// Nom de la variable a gauche d'une affectation.
fn assigned_name(left: &Expr) -> Result<String, EvalError> {
    if let Expr::Path(ExprPath { path, .. }) = left {
        if let Some(ident) = path.get_ident() {
            return Ok(ident.to_string());
        }
    }
    Err(EvalError::UnsupportedExpr(
        "Unsupported assignment target".to_string(),
    ))
}

/// Constantes a chemin : `f64::MAX`, `u32::MAX`, `std::f64::consts::PI`, etc.
fn path_constant(path: &syn::Path) -> Result<Val, EvalError> {
    let segments: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    let name = segments.last().map(String::as_str).unwrap_or("");
    let owner = if segments.len() >= 2 {
        segments[segments.len() - 2].as_str()
    } else {
        ""
    };
    let value = match (owner, name) {
        ("f64", "MAX") => Some(float(f64::MAX)),
        ("f64", "MIN") => Some(float(f64::MIN)),
        ("f64", "INFINITY") => Some(float(f64::INFINITY)),
        ("f64", "NEG_INFINITY") => Some(float(f64::NEG_INFINITY)),
        ("f64", "EPSILON") => Some(float(f64::EPSILON)),
        ("f64", "MIN_POSITIVE") => Some(float(f64::MIN_POSITIVE)),
        ("f64", "NAN") => Some(float(f64::NAN)),
        ("consts", "PI") => Some(float(std::f64::consts::PI)),
        ("consts", "E") => Some(float(std::f64::consts::E)),
        ("consts", "TAU") => Some(float(std::f64::consts::TAU)),
        ("consts", "SQRT_2") => Some(float(std::f64::consts::SQRT_2)),
        ("consts", "LN_2") => Some(float(std::f64::consts::LN_2)),
        ("consts", "LN_10") => Some(float(std::f64::consts::LN_10)),
        (int_ty, "MAX" | "MIN") => ty_from_name(int_ty)
            .filter(|ty| matches!(ty, Ty::Int { .. }))
            .map(|ty| {
                let (min, max) = int_bounds(ty);
                Val { v: if name == "MAX" { max } else { min }, ty }
            }),
        _ => None,
    };
    // Les bornes des entiers 64 bits ne sont pas representables exactement : refusees.
    match value {
        Some(Val { ty: Ty::Int { bits: 64, .. }, .. }) | None => Err(EvalError::UnsupportedExpr(
            format!("Unsupported path: {}", segments.join("::")),
        )),
        Some(v) => Ok(v),
    }
}

fn arg(args: &[Val], index: usize, method: &str) -> Result<Val, EvalError> {
    args.get(index).copied().ok_or_else(|| {
        EvalError::UnsupportedExpr(format!("Missing argument for: {}", method))
    })
}

/// Methodes numeriques. Le resultat est entier si le receveur et les arguments le sont.
fn call_method(method: &str, recv: Val, args: &[Val]) -> Result<Val, EvalError> {
    let is_int = recv.ty != Ty::Float;
    let f = recv.v;
    match method {
        "min" | "max" => {
            let other = arg(args, 0, method)?;
            let v = if method == "min" { f.min(other.v) } else { f.max(other.v) };
            match unify(recv.ty, other.ty) {
                Ty::Float => check_finite(v).map(float),
                ty => int_result(v, ty),
            }
        }
        "clamp" => {
            let (lo, hi) = (arg(args, 0, method)?, arg(args, 1, method)?);
            // Rust panique si la borne basse depasse la borne haute.
            if lo.v > hi.v || lo.v.is_nan() || hi.v.is_nan() {
                return Err(EvalError::PathologicalNumericalValue);
            }
            let v = f.clamp(lo.v, hi.v);
            match unify(unify(recv.ty, lo.ty), hi.ty) {
                Ty::Float => check_finite(v).map(float),
                ty => int_result(v, ty),
            }
        }
        "abs" if is_int => int_result(f.abs(), recv.ty),
        "signum" if is_int => int_result(f.signum() * if f == 0.0 { 0.0 } else { 1.0 }, recv.ty),
        "pow" if is_int => {
            let exp = arg(args, 0, method)?.v;
            if !(0.0..=64.0).contains(&exp) {
                return Err(EvalError::PathologicalNumericalValue);
            }
            int_result(f.powi(exp as i32), recv.ty)
        }
        "saturating_sub" if is_int => {
            let (min, max) = int_bounds(recv.ty);
            int_result((f - arg(args, 0, method)?.v).clamp(min, max), recv.ty)
        }
        "saturating_add" if is_int => {
            let (min, max) = int_bounds(recv.ty);
            int_result((f + arg(args, 0, method)?.v).clamp(min, max), recv.ty)
        }
        "abs_diff" if is_int => {
            let ty = match recv.ty {
                Ty::Int { bits, .. } => Ty::Int { signed: false, bits },
                _ => Ty::Int { signed: false, bits: 32 },
            };
            int_result((f - arg(args, 0, method)?.v).abs(), ty)
        }
        "abs" => check_finite(f.abs()).map(float),
        "signum" => Ok(float(f.signum())),
        "powi" => check_finite(f.powi(arg(args, 0, method)?.v as i32)).map(float),
        "powf" => check_finite(f.powf(arg(args, 0, method)?.v)).map(float),
        "sqrt" => {
            if f < 0.0 {
                return Err(EvalError::PathologicalNumericalValue);
            }
            check_finite(f.sqrt()).map(float)
        }
        "cbrt" => check_finite(f.cbrt()).map(float),
        "floor" => check_finite(f.floor()).map(float),
        "ceil" => check_finite(f.ceil()).map(float),
        "round" => check_finite(f.round()).map(float),
        "trunc" => check_finite(f.trunc()).map(float),
        "fract" => check_finite(f.fract()).map(float),
        "recip" => {
            if f == 0.0 {
                return Err(EvalError::PathologicalNumericalValue);
            }
            check_finite(f.recip()).map(float)
        }
        "ln" | "log2" | "log10" => {
            if f <= 0.0 {
                return Err(EvalError::PathologicalNumericalValue);
            }
            check_finite(match method {
                "ln" => f.ln(),
                "log2" => f.log2(),
                _ => f.log10(),
            })
            .map(float)
        }
        "exp" => check_finite(f.exp()).map(float),
        "tanh" => check_finite(f.tanh()).map(float),
        "sin" => check_finite(f.sin()).map(float),
        "cos" => check_finite(f.cos()).map(float),
        "atan" => check_finite(f.atan()).map(float),
        _ => Err(EvalError::UnsupportedExpr(format!(
            "Unsupported method call: {}",
            method
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_interpreter_basic() {
        let code = r#"
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
                score = score + 100.0;
            }
            score
        }
        "#;

        let item_fn: ItemFn = syn::parse_str(code).unwrap();
        let interpreter = AstInterpreter::new(&item_fn, 50_000);

        let ctx_small = ScorePolicyContext {
            item_size: 20,
            next_item_size: 30,
            bin_residual: 5,
            bin_capacity: 100,
            step_index: 0,
            total_items: 50,
            open_bins_count: 1,
            mean_residual: 50.0,
            post_residual: 5,
            fitting_bins_count: 1,
        };

        let outcome = interpreter.eval(&ctx_small).unwrap();
        assert_eq!(outcome.score, 105.0); // 5.0 + 100.0
        assert_eq!(outcome.branches_taken, 1);
        assert_eq!(outcome.score_mutations, 1);

        let ctx_large = ScorePolicyContext {
            item_size: 20,
            next_item_size: 0,
            bin_residual: 25,
            bin_capacity: 100,
            step_index: 0,
            total_items: 50,
            open_bins_count: 1,
            mean_residual: 50.0,
            post_residual: 5,
            fitting_bins_count: 1,
        };

        let outcome_large = interpreter.eval(&ctx_large).unwrap();
        assert_eq!(outcome_large.score, 25.0);
        assert_eq!(outcome_large.branches_taken, 0);
    }

    #[test]
    fn test_ast_interpreter_methods_and_powers() {
        let code = r#"
        pub fn score_bin(
            item_size: u32,
            bin_residual: u32,
            bin_capacity: u32,
            step_index: usize,
            total_items: usize,
            open_bins_count: usize,
            mean_residual: f64,
            next_item_size: u32,
            post_residual: u32,
            fitting_bins_count: usize,
        ) -> f64 {
            let stream_progress = (step_index as f64 / total_items as f64).powi(2);
            let residual_ratio = post_residual as f64 / bin_capacity as f64;
            let score = 1.0 - (1.5 * stream_progress + 0.5) * residual_ratio.powi(3);
            let score = score * 0.95 + ((post_residual as f64 / bin_capacity as f64) as f64) * 0.05;
            score
        }
        "#;

        let item_fn: ItemFn = syn::parse_str(code).unwrap();
        let interpreter = AstInterpreter::new(&item_fn, 50_000);

        let ctx = ScorePolicyContext {
            item_size: 20,
            next_item_size: 10,
            bin_residual: 40,
            bin_capacity: 100,
            step_index: 25,
            total_items: 50,
            open_bins_count: 5,
            mean_residual: 30.0,
            post_residual: 20,
            fitting_bins_count: 3,
        };

        let outcome = interpreter.eval(&ctx).unwrap();
        assert!(
            (outcome.score - 0.95335).abs() < 1e-4,
            "Expected ~0.95335, got {}",
            outcome.score
        );
    }

    #[test]
    fn test_ast_interpreter_recursion_depth_limit() {
        // Build an expression nested 55 levels deep: ((((...(1.0 + 1.0)...))))
        let mut expr_str = "1.0".to_string();
        for _ in 0..55 {
            expr_str = format!("({} + 1.0)", expr_str);
        }
        let code = format!(
            "pub fn score_bin() -> f64 {{ let score = {}; score }}",
            expr_str
        );
        let item_fn: ItemFn = syn::parse_str(&code).unwrap();

        let interpreter = AstInterpreter::new(&item_fn, 50_000);
        let ctx = ScorePolicyContext {
            item_size: 10,
            next_item_size: 10,
            bin_residual: 50,
            bin_capacity: 100,
            step_index: 0,
            total_items: 10,
            open_bins_count: 1,
            mean_residual: 50.0,
            post_residual: 40,
            fitting_bins_count: 1,
        };

        let result = interpreter.eval(&ctx);
        assert!(matches!(
            result,
            Err(EvalError::RecursionLimitExceeded {
                current: _,
                max: 48
            })
        ));
    }

    #[test]
    fn test_ast_interpreter_memory_limit_exceeded() {
        // Generate a function declaring 135 local variables, exceeding max_vars = 128
        let mut decls = String::new();
        for i in 0..135 {
            decls.push_str(&format!("let v{} = {} as f64;\n", i, i));
        }
        let code = format!(
            "pub fn score_bin() -> f64 {{\n{} let score = 1.0;\n score }}",
            decls
        );
        let item_fn: ItemFn = syn::parse_str(&code).unwrap();

        let interpreter = AstInterpreter::new(&item_fn, 50_000);
        let ctx = ScorePolicyContext {
            item_size: 10,
            next_item_size: 10,
            bin_residual: 50,
            bin_capacity: 100,
            step_index: 0,
            total_items: 10,
            open_bins_count: 1,
            mean_residual: 50.0,
            post_residual: 40,
            fitting_bins_count: 1,
        };

        let result = interpreter.eval(&ctx);
        assert!(matches!(result, Err(EvalError::MemoryLimitExceeded(_))));
    }

    #[test]
    fn test_ast_interpreter_pathological_inputs_rejected() {
        let code = "pub fn score_bin() -> f64 { 1.0 }";
        let item_fn: ItemFn = syn::parse_str(code).unwrap();
        let interpreter = AstInterpreter::new(&item_fn, 50_000);

        // Case 1: item_size == 0
        let ctx_zero_item = ScorePolicyContext {
            item_size: 0,
            next_item_size: 0,
            bin_residual: 50,
            bin_capacity: 100,
            step_index: 0,
            total_items: 10,
            open_bins_count: 1,
            mean_residual: 50.0,
            post_residual: 50,
            fitting_bins_count: 1,
        };
        assert!(matches!(
            interpreter.eval(&ctx_zero_item),
            Err(EvalError::InvalidInput(_))
        ));

        // Case 2: bin_capacity == 0
        let ctx_zero_cap = ScorePolicyContext {
            item_size: 10,
            next_item_size: 0,
            bin_residual: 50,
            bin_capacity: 0,
            step_index: 0,
            total_items: 10,
            open_bins_count: 1,
            mean_residual: 50.0,
            post_residual: 40,
            fitting_bins_count: 1,
        };
        assert!(matches!(
            interpreter.eval(&ctx_zero_cap),
            Err(EvalError::InvalidInput(_))
        ));

        // Case 3: item_size > bin_capacity
        let ctx_huge_item = ScorePolicyContext {
            item_size: 150,
            next_item_size: 0,
            bin_residual: 50,
            bin_capacity: 100,
            step_index: 0,
            total_items: 10,
            open_bins_count: 1,
            mean_residual: 50.0,
            post_residual: 0,
            fitting_bins_count: 1,
        };
        assert!(matches!(
            interpreter.eval(&ctx_huge_item),
            Err(EvalError::InvalidInput(_))
        ));

        // Case 4: NaN in mean_residual
        let ctx_nan = ScorePolicyContext {
            item_size: 10,
            next_item_size: 0,
            bin_residual: 50,
            bin_capacity: 100,
            step_index: 0,
            total_items: 10,
            open_bins_count: 1,
            mean_residual: f64::NAN,
            post_residual: 40,
            fitting_bins_count: 1,
        };
        assert!(matches!(
            interpreter.eval(&ctx_nan),
            Err(EvalError::InvalidInput(_))
        ));
    }

    #[test]
    fn test_ast_interpreter_fuel_exhaustion_without_panic() {
        let code = r#"
        pub fn score_bin() -> f64 {
            let mut score = 0.0;
            score = score + 1.0;
            score = score + 2.0;
            score = score + 3.0;
            score
        }
        "#;
        let item_fn: ItemFn = syn::parse_str(code).unwrap();
        // Set fuel limit very low (e.g. 5) so it runs out cleanly
        let interpreter = AstInterpreter::new(&item_fn, 5);
        let ctx = ScorePolicyContext {
            item_size: 10,
            next_item_size: 10,
            bin_residual: 50,
            bin_capacity: 100,
            step_index: 0,
            total_items: 10,
            open_bins_count: 1,
            mean_residual: 50.0,
            post_residual: 40,
            fitting_bins_count: 1,
        };

        let result = interpreter.eval(&ctx);
        assert_eq!(result, Err(EvalError::FuelExceeded));
    }

    #[test]
    fn test_ast_interpreter_pathological_numerical_values() {
        let ctx = ScorePolicyContext {
            item_size: 10,
            next_item_size: 10,
            bin_residual: 50,
            bin_capacity: 100,
            step_index: 0,
            total_items: 10,
            open_bins_count: 1,
            mean_residual: 50.0,
            post_residual: 40,
            fitting_bins_count: 1,
        };

        // Division by zero
        let code_div_zero = "pub fn score_bin() -> f64 { 1.0 / 0.0 }";
        let item_fn: ItemFn = syn::parse_str(code_div_zero).unwrap();
        let interp = AstInterpreter::new(&item_fn, 50_000);
        assert_eq!(interp.eval(&ctx), Err(EvalError::PathologicalNumericalValue));

        // Negative sqrt method
        let code_neg_sqrt = "pub fn score_bin() -> f64 { (-4.0).sqrt() }";
        let item_fn: ItemFn = syn::parse_str(code_neg_sqrt).unwrap();
        let interp = AstInterpreter::new(&item_fn, 50_000);
        assert_eq!(interp.eval(&ctx), Err(EvalError::PathologicalNumericalValue));

        // Negative sqrt function call
        let code_neg_sqrt_fn = "pub fn score_bin() -> f64 { sqrt(-4.0) }";
        let item_fn: ItemFn = syn::parse_str(code_neg_sqrt_fn).unwrap();
        let interp = AstInterpreter::new(&item_fn, 50_000);
        assert_eq!(interp.eval(&ctx), Err(EvalError::PathologicalNumericalValue));
    }
}

/// Conformite a la semantique de Rust. Chaque test correspond a un ecart corrige apres la
/// relecture externe du 13 novembre 2026 : avant correction, l'interpreteur renvoyait en
/// silence une autre valeur que Rust.
#[cfg(test)]
mod rust_semantics_tests {
    use super::*;

    fn ctx(bin_residual: u32, post_residual: u32) -> ScorePolicyContext {
        ScorePolicyContext {
            item_size: 20,
            next_item_size: 30,
            bin_residual,
            bin_capacity: 100,
            step_index: 0,
            total_items: 50,
            open_bins_count: 1,
            mean_residual: 50.0,
            post_residual,
            fitting_bins_count: 1,
        }
    }

    fn run(code: &str, c: &ScorePolicyContext) -> Result<f64, EvalError> {
        let item_fn: ItemFn = syn::parse_str(code).unwrap();
        AstInterpreter::new(&item_fn, 50_000).eval(c).map(|o| o.score)
    }

    /// Une variable jamais definie est une erreur (avant : 0.0 en silence).
    #[test]
    fn undefined_variable_is_an_error() {
        let code = "pub fn score_bin(post_residual: u32) -> f64 {
            let score = post_residual as f64 + undefined_bonus * 5.0;
            score
        }";
        assert_eq!(
            run(code, &ctx(25, 5)),
            Err(EvalError::UndefinedVariable("undefined_bonus".to_string()))
        );
        let compound = "pub fn score_bin(post_residual: u32) -> f64 { score += 1.0; score }";
        assert_eq!(
            run(compound, &ctx(25, 5)),
            Err(EvalError::UndefinedVariable("score".to_string()))
        );
    }

    /// Un `let` compte dans un bloc `else` comme dans la branche `then` (avant : ignore).
    #[test]
    fn let_is_honoured_in_else_block() {
        let code = "pub fn score_bin(bin_residual: u32, post_residual: u32) -> f64 {
            let score = if bin_residual > 50 {
                let w = 2.0;
                post_residual as f64 * w
            } else {
                let w = 3.0;
                post_residual as f64 * w
            };
            score
        }";
        assert_eq!(run(code, &ctx(60, 5)).unwrap(), 10.0);
        assert_eq!(run(code, &ctx(25, 5)).unwrap(), 15.0);
    }

    /// Un `let` d'un bloc masque la variable exterieure dans ce bloc seulement.
    #[test]
    fn block_let_shadows_inside_the_block_only() {
        let code = "pub fn score_bin(bin_residual: u32, post_residual: u32) -> f64 {
            let w = 1.0;
            let mut score = 0.0;
            if bin_residual > 50 {
                let w = 2.0;
                score += post_residual as f64 * w;
            } else {
                let w = 3.0;
                score += post_residual as f64 * w;
            }
            score + w
        }";
        assert_eq!(run(code, &ctx(60, 5)).unwrap(), 11.0);
        assert_eq!(run(code, &ctx(25, 5)).unwrap(), 16.0);

        // Une variable declaree dans un bloc n'existe plus apres lui.
        let leak = "pub fn score_bin(bin_residual: u32) -> f64 {
            if bin_residual > 50 { let inner = 2.0; }
            inner
        }";
        assert_eq!(
            run(leak, &ctx(60, 5)),
            Err(EvalError::UndefinedVariable("inner".to_string()))
        );
    }

    /// `let x: T = ...`, `const` et bloc nu `{ ... }` definissent bien leurs variables.
    #[test]
    fn typed_let_const_and_bare_block_define_variables() {
        let code = "pub fn score_bin(post_residual: u32) -> f64 {
            const WEIGHT: f64 = 2.0;
            let base: f64 = post_residual as f64;
            let bonus = { let half = base / 2.0; half + 1.0 };
            base * WEIGHT + bonus
        }";
        assert_eq!(run(code, &ctx(25, 5)).unwrap(), 13.5);
    }

    /// `f64::MAX` vaut `f64::MAX` (avant : 0.0). Un chemin inconnu est une erreur.
    #[test]
    fn path_constants_have_their_rust_value() {
        let code = "pub fn score_bin(bin_residual: u32) -> f64 {
            if bin_residual < 10 { f64::MAX } else { 1.0 }
        }";
        assert_eq!(run(code, &ctx(5, 5)).unwrap(), f64::MAX);
        let unknown = "pub fn score_bin(bin_residual: u32) -> f64 { foo::BAR }";
        assert!(matches!(run(unknown, &ctx(5, 5)), Err(EvalError::UnsupportedExpr(_))));
    }

    /// Les entiers suivent l'arithmetique entiere de Rust.
    #[test]
    fn integers_follow_rust_integer_arithmetic() {
        // Division entiere : 25 / 20 = 1 (avant : 1.25).
        let div = "pub fn score_bin(bin_residual: u32, item_size: u32) -> f64 {
            (bin_residual / item_size) as f64
        }";
        assert_eq!(run(div, &ctx(25, 5)).unwrap(), 1.0);
        // Division flottante apres conversion : 1.25.
        let fdiv = "pub fn score_bin(bin_residual: u32, item_size: u32) -> f64 {
            bin_residual as f64 / item_size as f64
        }";
        assert_eq!(run(fdiv, &ctx(25, 5)).unwrap(), 1.25);
        // Litteraux entiers : 7 / 2 = 3, 7 % 4 = 3.
        let lit = "pub fn score_bin() -> f64 { (7 / 2) as f64 + (7 % 4) as f64 }";
        assert_eq!(run(lit, &ctx(25, 5)).unwrap(), 6.0);
        // Conversion flottant vers entier : troncature vers zero, saturation a 0 pour un non signe.
        let cast = "pub fn score_bin() -> f64 { (2.9 as u32) as f64 + (-3.5 as u32) as f64 + (-3.5 as i32) as f64 }";
        assert_eq!(run(cast, &ctx(25, 5)).unwrap(), -1.0);
        // Soustraction non signee negative : panique en Rust (debug), erreur ici.
        let underflow = "pub fn score_bin(item_size: u32, bin_residual: u32) -> f64 {
            (item_size - bin_residual) as f64
        }";
        assert_eq!(run(underflow, &ctx(25, 5)), Err(EvalError::PathologicalNumericalValue));
        // La version sure donne 0.
        let saturating = "pub fn score_bin(item_size: u32, bin_residual: u32) -> f64 {
            item_size.saturating_sub(bin_residual) as f64 + item_size.abs_diff(bin_residual) as f64
        }";
        assert_eq!(run(saturating, &ctx(25, 5)).unwrap(), 5.0);
    }

    /// `&&` et `||` n'evaluent pas l'operande droit quand le gauche decide (avant : toujours).
    #[test]
    fn logical_operators_short_circuit() {
        let code = "pub fn score_bin(post_residual: u32, bin_capacity: u32) -> f64 {
            if post_residual != 0 && bin_capacity / post_residual > 3 { 1.0 } else { 2.0 }
        }";
        assert_eq!(run(code, &ctx(20, 0)).unwrap(), 2.0);
        assert_eq!(run(code, &ctx(25, 5)).unwrap(), 1.0);
    }

    /// L'egalite est exacte (avant : tolerance de 1e-6).
    #[test]
    fn equality_is_exact() {
        let code = "pub fn score_bin() -> f64 { if 1.0 == 1.0000001 { 1.0 } else { 2.0 } }";
        assert_eq!(run(code, &ctx(25, 5)).unwrap(), 2.0);
    }

    /// Seule l'expression finale donne la valeur de la fonction.
    #[test]
    fn function_value_is_the_tail_expression() {
        let code = "pub fn score_bin(bin_residual: u32) -> f64 {
            let mut score = 1.0;
            if bin_residual > 10 { score += 5.0 }
            score = score * 2.0;
        }";
        // Sans expression finale, la variable `score` sert de valeur (tolerance historique).
        assert_eq!(run(code, &ctx(25, 5)).unwrap(), 12.0);
    }

    /// Outil de comparaison avec le vrai compilateur Rust, pilote par
    /// `scripts/analysis/diff_interpreter_vs_rustc.py`. Pour chaque fichier `*.code.rs` du
    /// dossier `NOMAD_DIFF_DIR`, ecrit dans `*.interp.txt` le score de l'interpreteur sur
    /// chaque contexte de `contexts.txt` (`ERR` en cas d'erreur).
    #[test]
    #[ignore]
    fn differential_dump() {
        let dir = std::env::var("NOMAD_DIFF_DIR").expect("NOMAD_DIFF_DIR");
        let contexts: Vec<ScorePolicyContext> = std::fs::read_to_string(format!("{dir}/contexts.txt"))
            .unwrap()
            .lines()
            .map(|line| {
                let f: Vec<&str> = line.split_whitespace().collect();
                ScorePolicyContext {
                    item_size: f[0].parse().unwrap(),
                    next_item_size: f[1].parse().unwrap(),
                    bin_residual: f[2].parse().unwrap(),
                    bin_capacity: f[3].parse().unwrap(),
                    step_index: f[4].parse().unwrap(),
                    total_items: f[5].parse().unwrap(),
                    open_bins_count: f[6].parse().unwrap(),
                    mean_residual: f[7].parse().unwrap(),
                    post_residual: f[8].parse().unwrap(),
                    fitting_bins_count: f[9].parse().unwrap(),
                }
            })
            .collect();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.to_string_lossy().to_string();
            if !name.ends_with(".code.rs") {
                continue;
            }
            let code = std::fs::read_to_string(&path).unwrap();
            let item_fn: ItemFn = syn::parse_str(&code).unwrap();
            let interpreter = AstInterpreter::new(&item_fn, 10_000_000);
            let out: Vec<String> = contexts
                .iter()
                .map(|c| match interpreter.eval(c) {
                    Ok(o) => format!("{:?}", o.score),
                    Err(e) => format!("ERR {e}"),
                })
                .collect();
            std::fs::write(name.replace(".code.rs", ".interp.txt"), out.join("
")).unwrap();
        }
    }
}
