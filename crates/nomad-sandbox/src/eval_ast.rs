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
        mut vars: HashMap<String, f64>,
        default_fallback: f64,
    ) -> Result<AstEvalOutcome, EvalError> {
        if vars.len() > self.max_vars {
            return Err(EvalError::MemoryLimitExceeded(format!(
                "Initial variable count {} exceeds maximum {}",
                vars.len(),
                self.max_vars
            )));
        }

        let mut fuel_consumed = 0u64;
        let mut branches_taken = 0usize;
        let mut score_mutations = 0usize;
        let mut return_val = None;

        for stmt in &self.item_fn.block.stmts {
            fuel_consumed += 5;
            if fuel_consumed > self.fuel_limit {
                return Err(EvalError::FuelExceeded);
            }

            match stmt {
                Stmt::Local(Local {
                    pat,
                    init: Some(local_init),
                    ..
                }) => {
                    let ident = match pat {
                        Pat::Ident(PatIdent { ident, .. }) => ident.to_string(),
                        _ => "local_var".to_string(),
                    };
                    let val = self.eval_expr(
                        &local_init.expr,
                        &mut vars,
                        &mut fuel_consumed,
                        &mut branches_taken,
                        &mut score_mutations,
                        0,
                    )?;
                    if vars.len() >= self.max_vars && !vars.contains_key(&ident) {
                        return Err(EvalError::MemoryLimitExceeded(format!(
                            "Variable count exceeded maximum limit of {}",
                            self.max_vars
                        )));
                    }
                    vars.insert(ident, val);
                }
                Stmt::Expr(expr, semi) => {
                    let val = self.eval_expr(
                        expr,
                        &mut vars,
                        &mut fuel_consumed,
                        &mut branches_taken,
                        &mut score_mutations,
                        0,
                    )?;
                    if semi.is_none() {
                        return_val = Some(val);
                    }
                }
                _ => {}
            }
        }

        let final_score = return_val
            .or_else(|| vars.get("score").copied())
            .unwrap_or(default_fallback);

        check_finite(final_score)?;

        Ok(AstEvalOutcome {
            score: final_score,
            fuel_consumed,
            branches_taken,
            score_mutations,
        })
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

    fn eval_expr(
        &self,
        expr: &Expr,
        vars: &mut HashMap<String, f64>,
        fuel: &mut u64,
        branches: &mut usize,
        mutations: &mut usize,
        depth: usize,
    ) -> Result<f64, EvalError> {
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
                    .map_err(|e| EvalError::ParseError(e.to_string())),
                Lit::Int(i) => i
                    .base10_parse::<f64>()
                    .map_err(|e| EvalError::ParseError(e.to_string())),
                Lit::Bool(b) => Ok(if b.value { 1.0 } else { 0.0 }),
                _ => Err(EvalError::UnsupportedExpr(format!(
                    "Unsupported literal: {:?}",
                    lit
                ))),
            },
            Expr::Path(ExprPath { path, .. }) => {
                if let Some(ident) = path.get_ident() {
                    let name = ident.to_string();
                    if let Some(&v) = vars.get(&name) {
                        Ok(v)
                    } else if name == "true" {
                        Ok(1.0)
                    } else if name == "false" {
                        Ok(0.0)
                    } else {
                        // Default undefined helper variables to 0.0
                        Ok(0.0)
                    }
                } else {
                    Ok(0.0)
                }
            }
            Expr::Cast(ExprCast { expr, .. }) => {
                self.eval_expr(expr, vars, fuel, branches, mutations, depth + 1)
            }
            Expr::Paren(ExprParen { expr, .. }) => {
                self.eval_expr(expr, vars, fuel, branches, mutations, depth + 1)
            }
            Expr::Unary(ExprUnary { op, expr, .. }) => {
                let val = self.eval_expr(expr, vars, fuel, branches, mutations, depth + 1)?;
                match op {
                    UnOp::Neg(_) => Ok(-val),
                    UnOp::Not(_) => Ok(if val == 0.0 { 1.0 } else { 0.0 }),
                    _ => Err(EvalError::UnsupportedExpr(format!(
                        "Unsupported unary operator: {:?}",
                        op
                    ))),
                }
            }
            Expr::Binary(ExprBinary {
                left, op, right, ..
            }) => {
                let l = self.eval_expr(left, vars, fuel, branches, mutations, depth + 1)?;
                let r = self.eval_expr(right, vars, fuel, branches, mutations, depth + 1)?;
                match op {
                    BinOp::Add(_) => check_finite(l + r),
                    BinOp::Sub(_) => check_finite(l - r),
                    BinOp::Mul(_) => check_finite(l * r),
                    BinOp::Div(_) => {
                        if r == 0.0 {
                            Err(EvalError::PathologicalNumericalValue)
                        } else {
                            check_finite(l / r)
                        }
                    }
                    BinOp::Rem(_) => {
                        if r == 0.0 {
                            Err(EvalError::PathologicalNumericalValue)
                        } else {
                            check_finite(l % r)
                        }
                    }
                    BinOp::Lt(_) => Ok(if l < r { 1.0 } else { 0.0 }),
                    BinOp::Le(_) => Ok(if l <= r { 1.0 } else { 0.0 }),
                    BinOp::Gt(_) => Ok(if l > r { 1.0 } else { 0.0 }),
                    BinOp::Ge(_) => Ok(if l >= r { 1.0 } else { 0.0 }),
                    BinOp::Eq(_) => Ok(if (l - r).abs() < 1e-6 { 1.0 } else { 0.0 }),
                    BinOp::Ne(_) => Ok(if (l - r).abs() >= 1e-6 { 1.0 } else { 0.0 }),
                    BinOp::And(_) => Ok(if l != 0.0 && r != 0.0 { 1.0 } else { 0.0 }),
                    BinOp::Or(_) => Ok(if l != 0.0 || r != 0.0 { 1.0 } else { 0.0 }),
                    BinOp::AddAssign(_)
                    | BinOp::SubAssign(_)
                    | BinOp::MulAssign(_)
                    | BinOp::DivAssign(_)
                    | BinOp::RemAssign(_) => {
                        if let Expr::Path(ExprPath { path, .. }) = &**left {
                            if let Some(ident) = path.get_ident() {
                                let name = ident.to_string();
                                let current = vars.get(&name).copied().unwrap_or(0.0);
                                let updated = match op {
                                    BinOp::AddAssign(_) => check_finite(current + r)?,
                                    BinOp::SubAssign(_) => check_finite(current - r)?,
                                    BinOp::MulAssign(_) => check_finite(current * r)?,
                                    BinOp::DivAssign(_) => {
                                        if r == 0.0 {
                                            return Err(EvalError::PathologicalNumericalValue);
                                        }
                                        check_finite(current / r)?
                                    }
                                    BinOp::RemAssign(_) => {
                                        if r == 0.0 {
                                            return Err(EvalError::PathologicalNumericalValue);
                                        }
                                        check_finite(current % r)?
                                    }
                                    _ => unreachable!(),
                                };
                                if name == "score" {
                                    *mutations += 1;
                                }
                                if vars.len() >= self.max_vars && !vars.contains_key(&name) {
                                    return Err(EvalError::MemoryLimitExceeded(format!(
                                        "Variable count exceeded maximum limit of {}",
                                        self.max_vars
                                    )));
                                }
                                vars.insert(name, updated);
                                Ok(updated)
                            } else {
                                Ok(r)
                            }
                        } else {
                            Ok(r)
                        }
                    }
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
                    self.eval_expr(cond, vars, fuel, branches, mutations, depth + 1)? != 0.0;
                if condition_met {
                    *branches += 1;
                    let mut last_val = 0.0;
                    for stmt in &then_branch.stmts {
                        match stmt {
                            Stmt::Local(Local {
                                pat,
                                init: Some(init),
                                ..
                            }) => {
                                let ident = match pat {
                                    Pat::Ident(PatIdent { ident, .. }) => ident.to_string(),
                                    _ => "local".to_string(),
                                };
                                let v = self.eval_expr(
                                    &init.expr,
                                    vars,
                                    fuel,
                                    branches,
                                    mutations,
                                    depth + 1,
                                )?;
                                if vars.len() >= self.max_vars && !vars.contains_key(&ident) {
                                    return Err(EvalError::MemoryLimitExceeded(format!(
                                        "Variable count exceeded maximum limit of {}",
                                        self.max_vars
                                    )));
                                }
                                vars.insert(ident, v);
                            }
                            Stmt::Expr(e, _) => {
                                last_val =
                                    self.eval_expr(e, vars, fuel, branches, mutations, depth + 1)?;
                            }
                            _ => {}
                        }
                    }
                    Ok(last_val)
                } else if let Some((_, else_expr)) = else_branch {
                    self.eval_expr(else_expr, vars, fuel, branches, mutations, depth + 1)
                } else {
                    Ok(0.0)
                }
            }
            Expr::Assign(assign) => {
                let r =
                    self.eval_expr(&assign.right, vars, fuel, branches, mutations, depth + 1)?;
                if let Expr::Path(ExprPath { path, .. }) = &*assign.left {
                    if let Some(ident) = path.get_ident() {
                        let name = ident.to_string();
                        if name == "score" {
                            *mutations += 1;
                        }
                        if vars.len() >= self.max_vars && !vars.contains_key(&name) {
                            return Err(EvalError::MemoryLimitExceeded(format!(
                                "Variable count exceeded maximum limit of {}",
                                self.max_vars
                            )));
                        }
                        vars.insert(name, r);
                    }
                }
                Ok(r)
            }
            Expr::Block(block) => {
                let mut last = 0.0;
                for stmt in &block.block.stmts {
                    if let Stmt::Expr(e, _) = stmt {
                        last = self.eval_expr(e, vars, fuel, branches, mutations, depth + 1)?;
                    }
                }
                Ok(last)
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
                let method_name = method.to_string();
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
                match method_name.as_str() {
                    "powi" => {
                        let exp = eval_args.first().copied().unwrap_or(1.0) as i32;
                        check_finite(recv_val.powi(exp))
                    }
                    "powf" => {
                        let exp = eval_args.first().copied().unwrap_or(1.0);
                        check_finite(recv_val.powf(exp))
                    }
                    "sqrt" => {
                        if recv_val < 0.0 {
                            return Err(EvalError::PathologicalNumericalValue);
                        }
                        check_finite(recv_val.sqrt())
                    }
                    "abs" => check_finite(recv_val.abs()),
                    "floor" => check_finite(recv_val.floor()),
                    "ceil" => check_finite(recv_val.ceil()),
                    "round" => check_finite(recv_val.round()),
                    "min" => {
                        let other = eval_args.first().copied().unwrap_or(recv_val);
                        check_finite(recv_val.min(other))
                    }
                    "max" => {
                        let other = eval_args.first().copied().unwrap_or(recv_val);
                        check_finite(recv_val.max(other))
                    }
                    "clamp" => {
                        let min_val = eval_args.first().copied().unwrap_or(recv_val);
                        let max_val = eval_args.get(1).copied().unwrap_or(recv_val);
                        let clamped = if min_val <= max_val {
                            recv_val.clamp(min_val, max_val)
                        } else {
                            recv_val.clamp(max_val, min_val)
                        };
                        check_finite(clamped)
                    }
                    "ln" => {
                        if recv_val <= 0.0 {
                            return Err(EvalError::PathologicalNumericalValue);
                        }
                        check_finite(recv_val.ln())
                    }
                    "exp" => {
                        check_finite(recv_val.exp())
                    }
                    _ => Err(EvalError::UnsupportedExpr(format!(
                        "Unsupported method call: {}",
                        method_name
                    ))),
                }
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
                let fn_name = if let Expr::Path(ExprPath { path, .. }) = &**func {
                    path.segments
                        .last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                match fn_name.as_str() {
                    "min" => {
                        let a = eval_args.first().copied().unwrap_or(0.0);
                        let b = eval_args.get(1).copied().unwrap_or(0.0);
                        check_finite(a.min(b))
                    }
                    "max" => {
                        let a = eval_args.first().copied().unwrap_or(0.0);
                        let b = eval_args.get(1).copied().unwrap_or(0.0);
                        check_finite(a.max(b))
                    }
                    "sqrt" => {
                        let a = eval_args.first().copied().unwrap_or(0.0);
                        if a < 0.0 {
                            return Err(EvalError::PathologicalNumericalValue);
                        }
                        check_finite(a.sqrt())
                    }
                    "abs" => {
                        let a = eval_args.first().copied().unwrap_or(0.0);
                        check_finite(a.abs())
                    }
                    "powi" => {
                        let a = eval_args.first().copied().unwrap_or(0.0);
                        let b = eval_args.get(1).copied().unwrap_or(1.0) as i32;
                        check_finite(a.powi(b))
                    }
                    "powf" => {
                        let a = eval_args.first().copied().unwrap_or(0.0);
                        let b = eval_args.get(1).copied().unwrap_or(1.0);
                        check_finite(a.powf(b))
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
