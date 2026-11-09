use std::collections::HashMap;
use nomad_core::SchedulingInputs;
use syn::{
    BinOp, Expr, ExprBinary, ExprCall, ExprCast, ExprIf, ExprLit, ExprMethodCall, ExprParen,
    ExprPath, ExprUnary, ItemFn, Lit, Local, Pat, PatIdent, Stmt, UnOp,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvalError {
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),
    #[error("Fuel limit exceeded during AST interpretation")]
    FuelExceeded,
    #[error("Unsupported AST expression: {0}")]
    UnsupportedExpr(String),
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
#[derive(Debug, Clone)]
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
/// AstInterpreter : Interpréteur Déterministe d'AST Rust avec Comptage de Fuel
/// ============================================================================
/// Exécute de façon sécurisée et isolée une fonction `score_bin` représentée
/// sous forme d'AST `syn::ItemFn` sans risque de blocage ou d'accès hôte non autorisé.
pub struct AstInterpreter<'a> {
    item_fn: &'a ItemFn,
    fuel_limit: u64,
}

impl<'a> AstInterpreter<'a> {
    /// Initialise l'interpréteur avec la fonction AST et un plafond strict de fuel.
    pub fn new(item_fn: &'a ItemFn, fuel_limit: u64) -> Self {
        Self {
            item_fn,
            fuel_limit,
        }
    }

    /// ========================================================================
    /// eval_with_vars : Interprétation Séquentielle Générique avec Variables
    /// ========================================================================
    pub fn eval_with_vars(&self, mut vars: HashMap<String, f64>, default_fallback: f64) -> Result<AstEvalOutcome, EvalError> {
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
                    )?;
                    vars.insert(ident, val);
                }
                Stmt::Expr(expr, semi) => {
                    let val = self.eval_expr(
                        expr,
                        &mut vars,
                        &mut fuel_consumed,
                        &mut branches_taken,
                        &mut score_mutations,
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
        vars.insert("fitting_bins_count".to_string(), ctx.fitting_bins_count as f64);

        self.eval_with_vars(vars, ctx.post_residual as f64)
    }

    /// ========================================================================
    /// eval_scheduling : Interprétation Séquentielle de `score_node`
    /// ========================================================================
    pub fn eval_scheduling(&self, ctx: &SchedulingInputs) -> Result<AstEvalOutcome, EvalError> {
        let mut vars = HashMap::new();
        vars.insert("job_cpu".to_string(), ctx.job_cpu as f64);
        vars.insert("job_ram".to_string(), ctx.job_ram as f64);
        vars.insert("next_job_cpu".to_string(), ctx.next_job_cpu as f64);
        vars.insert("next_job_ram".to_string(), ctx.next_job_ram as f64);
        vars.insert("node_residual_cpu".to_string(), ctx.node_residual_cpu as f64);
        vars.insert("node_residual_ram".to_string(), ctx.node_residual_ram as f64);
        vars.insert("node_capacity_cpu".to_string(), ctx.node_capacity_cpu as f64);
        vars.insert("node_capacity_ram".to_string(), ctx.node_capacity_ram as f64);
        vars.insert("step_index".to_string(), ctx.step_index as f64);
        vars.insert("total_jobs".to_string(), ctx.total_jobs as f64);
        vars.insert("active_nodes_count".to_string(), ctx.active_nodes_count as f64);
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
    ) -> Result<f64, EvalError> {
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
                _ => Ok(0.0),
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
                self.eval_expr(expr, vars, fuel, branches, mutations)
            }
            Expr::Paren(ExprParen { expr, .. }) => {
                self.eval_expr(expr, vars, fuel, branches, mutations)
            }
            Expr::Unary(ExprUnary { op, expr, .. }) => {
                let val = self.eval_expr(expr, vars, fuel, branches, mutations)?;
                match op {
                    UnOp::Neg(_) => Ok(-val),
                    UnOp::Not(_) => Ok(if val == 0.0 { 1.0 } else { 0.0 }),
                    _ => Ok(val),
                }
            }
            Expr::Binary(ExprBinary {
                left, op, right, ..
            }) => {
                let l = self.eval_expr(left, vars, fuel, branches, mutations)?;
                let r = self.eval_expr(right, vars, fuel, branches, mutations)?;
                match op {
                    BinOp::Add(_) => Ok(l + r),
                    BinOp::Sub(_) => Ok(l - r),
                    BinOp::Mul(_) => Ok(l * r),
                    BinOp::Div(_) => {
                        if r != 0.0 {
                            Ok(l / r)
                        } else {
                            Ok(0.0)
                        }
                    }
                    BinOp::Rem(_) => {
                        if r != 0.0 {
                            Ok(l % r)
                        } else {
                            Ok(0.0)
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
                    // syn 2.0 compound assignments are handled in Expr::Binary when enabled or Expr::Assign
                    _ => Ok(0.0),
                }
            }
            Expr::If(ExprIf {
                cond,
                then_branch,
                else_branch,
                ..
            }) => {
                let condition_met =
                    self.eval_expr(cond, vars, fuel, branches, mutations)? != 0.0;
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
                                )?;
                                vars.insert(ident, v);
                            }
                            Stmt::Expr(e, _) => {
                                last_val = self.eval_expr(e, vars, fuel, branches, mutations)?;
                            }
                            _ => {}
                        }
                    }
                    Ok(last_val)
                } else if let Some((_, else_expr)) = else_branch {
                    self.eval_expr(else_expr, vars, fuel, branches, mutations)
                } else {
                    Ok(0.0)
                }
            }
            Expr::Assign(assign) => {
                let r = self.eval_expr(&assign.right, vars, fuel, branches, mutations)?;
                if let Expr::Path(ExprPath { path, .. }) = &*assign.left {
                    if let Some(ident) = path.get_ident() {
                        let name = ident.to_string();
                        if name == "score" {
                            *mutations += 1;
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
                        last = self.eval_expr(e, vars, fuel, branches, mutations)?;
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
                let recv_val = self.eval_expr(receiver, vars, fuel, branches, mutations)?;
                let method_name = method.to_string();
                let mut eval_args = Vec::with_capacity(args.len());
                for arg in args {
                    eval_args.push(self.eval_expr(arg, vars, fuel, branches, mutations)?);
                }
                match method_name.as_str() {
                    "powi" => {
                        let exp = eval_args.first().copied().unwrap_or(1.0) as i32;
                        let res = recv_val.powi(exp);
                        Ok(if res.is_finite() { res } else { 0.0 })
                    }
                    "powf" => {
                        let exp = eval_args.first().copied().unwrap_or(1.0);
                        let res = recv_val.powf(exp);
                        Ok(if res.is_finite() { res } else { 0.0 })
                    }
                    "sqrt" => {
                        let res = recv_val.max(0.0).sqrt();
                        Ok(if res.is_finite() { res } else { 0.0 })
                    }
                    "abs" => Ok(recv_val.abs()),
                    "floor" => Ok(recv_val.floor()),
                    "ceil" => Ok(recv_val.ceil()),
                    "round" => Ok(recv_val.round()),
                    "min" => {
                        let other = eval_args.first().copied().unwrap_or(recv_val);
                        Ok(recv_val.min(other))
                    }
                    "max" => {
                        let other = eval_args.first().copied().unwrap_or(recv_val);
                        Ok(recv_val.max(other))
                    }
                    "clamp" => {
                        let min_val = eval_args.first().copied().unwrap_or(recv_val);
                        let max_val = eval_args.get(1).copied().unwrap_or(recv_val);
                        if min_val <= max_val {
                            Ok(recv_val.clamp(min_val, max_val))
                        } else {
                            Ok(recv_val.clamp(max_val, min_val))
                        }
                    }
                    "ln" => {
                        let res = recv_val.max(1e-9).ln();
                        Ok(if res.is_finite() { res } else { 0.0 })
                    }
                    "exp" => {
                        let res = recv_val.clamp(-50.0, 50.0).exp();
                        Ok(if res.is_finite() { res } else { 0.0 })
                    }
                    _ => Ok(recv_val),
                }
            }
            Expr::Call(ExprCall { func, args, .. }) => {
                let mut eval_args = Vec::with_capacity(args.len());
                for arg in args {
                    eval_args.push(self.eval_expr(arg, vars, fuel, branches, mutations)?);
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
                        Ok(a.min(b))
                    }
                    "max" => {
                        let a = eval_args.first().copied().unwrap_or(0.0);
                        let b = eval_args.get(1).copied().unwrap_or(0.0);
                        Ok(a.max(b))
                    }
                    "sqrt" => {
                        let a = eval_args.first().copied().unwrap_or(0.0);
                        let res = a.max(0.0).sqrt();
                        Ok(if res.is_finite() { res } else { 0.0 })
                    }
                    "abs" => {
                        let a = eval_args.first().copied().unwrap_or(0.0);
                        Ok(a.abs())
                    }
                    "powi" => {
                        let a = eval_args.first().copied().unwrap_or(0.0);
                        let b = eval_args.get(1).copied().unwrap_or(1.0) as i32;
                        let res = a.powi(b);
                        Ok(if res.is_finite() { res } else { 0.0 })
                    }
                    "powf" => {
                        let a = eval_args.first().copied().unwrap_or(0.0);
                        let b = eval_args.get(1).copied().unwrap_or(1.0);
                        let res = a.powf(b);
                        Ok(if res.is_finite() { res } else { 0.0 })
                    }
                    _ => Ok(eval_args.first().copied().unwrap_or(0.0)),
                }
            }
            _ => Ok(0.0),
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

        // stream_progress = 0.5^2 = 0.25
        // residual_ratio = 0.2, .powi(3) = 0.008
        // 1.5 * 0.25 + 0.5 = 0.875
        // score_init = 1.0 - 0.875 * 0.008 = 0.993
        // final_score = 0.993 * 0.95 + 0.2 * 0.05 = 0.94335 + 0.010 = 0.95335
        let outcome = interpreter.eval(&ctx).unwrap();
        assert!(
            (outcome.score - 0.95335).abs() < 1e-4,
            "Expected ~0.95335, got {}",
            outcome.score
        );
    }
}

