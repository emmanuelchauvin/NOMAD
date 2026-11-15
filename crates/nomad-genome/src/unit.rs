use sha2::{Digest, Sha256};
use std::collections::HashSet;
use syn::visit::Visit;
use syn::{Expr, Ident, ItemFn, Local, Stmt};

/// ============================================================================
/// LogicalUnitKind : Typologie des Fragments Logiques Transférables
/// ============================================================================
/// Catégorie syntaxique d'une unité logique candidate à une greffe génétique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalUnitKind {
    /// Branchement conditionnel (ex: `if bin_residual < 10 { ... }`).
    ConditionalBranch,
    /// Déclaration de variable locale (ex: `let gap = bin_residual;`).
    LocalBinding,
    /// Expression de calcul d'une pénalité ou d'un bonus d'ajustement.
    PenaltyOrBonus,
    /// Bloc ou instruction générale de calcul.
    GeneralBlock,
}

/// ============================================================================
/// LogicalUnit : Unité Logique Algorithmique Autonome
/// ============================================================================
/// Fragment de code autonome identifié dans l'AST, prêt pour le transfert
/// ou la greffe vers un autre organisme (Section 4.4).
#[derive(Debug, Clone)]
pub struct LogicalUnit {
    /// Identifiant synthétique unique de l'unité logique.
    pub unit_id: String,
    /// Nature typologique du fragment (branche, variable, calcul).
    pub kind: LogicalUnitKind,
    /// Nœud d'instruction syntaxique `syn::Stmt` concret.
    pub stmt: Stmt,
    /// Représentation textuelle formatée du code du fragment.
    pub code_repr: String,
    /// Identifiants de variables et symboles requis pour l'exécution du fragment.
    pub dependencies: Vec<String>,
    /// Hash cryptographique SHA-256 du fragment tokenisé.
    pub hash: String,
}

/// Visiteur AST extrayant les identifiants de variables utilisés dans un sous-arbre.
struct DependencyCollector {
    identifiers: HashSet<String>,
}

impl<'ast> Visit<'ast> for DependencyCollector {
    fn visit_ident(&mut self, node: &'ast Ident) {
        self.identifiers.insert(node.to_string());
        syn::visit::visit_ident(self, node);
    }
}

/// ============================================================================
/// collect_dependencies : Algorithme d'Analyse des Dépendances
/// ============================================================================
/// Parcourt récursivement un statement AST pour lister toutes les variables référencées,
/// afin de garantir qu'une greffe dans un organisme hôte ne produira pas de variable non liée.
pub fn collect_dependencies(stmt: &Stmt) -> Vec<String> {
    let mut collector = DependencyCollector {
        identifiers: HashSet::new(),
    };
    collector.visit_stmt(stmt);
    let mut deps: Vec<String> = collector.identifiers.into_iter().collect();
    deps.sort();
    deps
}

/// Visiteur ne retenant que les variables lues : chemins à un seul segment (`x`), et non les
/// noms de méthode (`.powi(..)`), de champ, de type ou de chemin (`f64::MAX`). Note aussi les
/// noms liés à l'intérieur du fragment (`let`, paramètres de fermeture, boucle `for`).
struct FreeVariableCollector {
    used: HashSet<String>,
    bound: HashSet<String>,
}

impl FreeVariableCollector {
    fn bind_pat(&mut self, pat: &syn::Pat) {
        match pat {
            syn::Pat::Ident(p) => {
                self.bound.insert(p.ident.to_string());
            }
            syn::Pat::Type(t) => self.bind_pat(&t.pat),
            syn::Pat::Tuple(t) => t.elems.iter().for_each(|p| self.bind_pat(p)),
            syn::Pat::Reference(r) => self.bind_pat(&r.pat),
            _ => {}
        }
    }
}

impl<'ast> Visit<'ast> for FreeVariableCollector {
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if node.qself.is_none() && node.path.segments.len() == 1 {
            self.used.insert(node.path.segments[0].ident.to_string());
        }
    }
    fn visit_local(&mut self, node: &'ast Local) {
        self.bind_pat(&node.pat);
        syn::visit::visit_local(self, node);
    }
    fn visit_expr_closure(&mut self, node: &'ast syn::ExprClosure) {
        node.inputs.iter().for_each(|p| self.bind_pat(p));
        syn::visit::visit_expr_closure(self, node);
    }
    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.bind_pat(&node.pat);
        syn::visit::visit_expr_for_loop(self, node);
    }
}

/// Variables qu'un fragment lit sans les définir lui-même : ce que l'hôte doit fournir pour
/// que la greffe compile. Le nom défini par un `let` de premier niveau n'est requis que s'il
/// est aussi lu dans son initialisation (`let score = score * 2.0`).
pub fn collect_free_variables(stmt: &Stmt) -> Vec<String> {
    let mut c = FreeVariableCollector {
        used: HashSet::new(),
        bound: HashSet::new(),
    };
    match stmt {
        Stmt::Local(local) => {
            if let Some(init) = &local.init {
                c.visit_expr(&init.expr);
                if let Some((_, diverge)) = &init.diverge {
                    c.visit_expr(diverge);
                }
            }
        }
        _ => c.visit_stmt(stmt),
    }
    let mut free: Vec<String> = c.used.difference(&c.bound).cloned().collect();
    free.sort();
    free
}

/// Parcourt récursivement une expression AST pour lister toutes les variables référencées.
pub fn collect_expr_dependencies(expr: &Expr) -> Vec<String> {
    let mut collector = DependencyCollector {
        identifiers: HashSet::new(),
    };
    collector.visit_expr(expr);
    let mut deps: Vec<String> = collector.identifiers.into_iter().collect();
    deps.sort();
    deps
}

/// ============================================================================
/// extract_logical_units : Algorithme d'Extraction des Unités Logiques
/// ============================================================================
/// Décompose le corps d'une fonction `score_bin` en une liste de fragments transférables,
/// filtre les instructions triviales et calcule le hash de chaque unité candidate.
pub fn extract_logical_units(item_fn: &ItemFn) -> Vec<LogicalUnit> {
    let mut units = Vec::new();

    for stmt in &item_fn.block.stmts {
        let (kind, should_extract) = match stmt {
            Stmt::Local(Local { init: Some(_), .. }) => (LogicalUnitKind::LocalBinding, true),
            Stmt::Expr(Expr::If(_), _) => (LogicalUnitKind::ConditionalBranch, true),
            Stmt::Expr(Expr::Binary(_) | Expr::Assign(_), _) => {
                (LogicalUnitKind::PenaltyOrBonus, true)
            }
            Stmt::Expr(Expr::Block(_), _) => (LogicalUnitKind::GeneralBlock, true),
            _ => (LogicalUnitKind::GeneralBlock, false),
        };

        if should_extract {
            let code_repr = quote::quote!(#stmt).to_string();
            let mut hasher = Sha256::new();
            hasher.update(code_repr.as_bytes());
            let hash = hex::encode(hasher.finalize());
            let unit_id = hash[..12].to_string();

            let dependencies = collect_dependencies(stmt);

            units.push(LogicalUnit {
                unit_id,
                kind,
                stmt: stmt.clone(),
                code_repr,
                dependencies,
                hash,
            });
        }
    }

    units
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::parse_score_bin;

    #[test]
    fn test_extract_logical_units() {
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
                score += 100.0;
            }
            if bin_residual == 0 {
                score -= 50.0;
            }
            score
        }
        "#;

        let item_fn = parse_score_bin(code).expect("Should parse");
        let units = extract_logical_units(&item_fn);

        assert!(units.len() >= 3);
        // Should find local binding
        assert!(units
            .iter()
            .any(|u| u.kind == LogicalUnitKind::LocalBinding));
        // Should find the conditional branches
        let conditionals: Vec<_> = units
            .iter()
            .filter(|u| u.kind == LogicalUnitKind::ConditionalBranch)
            .collect();
        assert_eq!(conditionals.len(), 2);
        assert!(conditionals[0]
            .dependencies
            .contains(&"bin_residual".to_string()));
    }
}

#[cfg(test)]
mod free_variable_tests {
    use super::*;

    fn stmt(code: &str) -> Stmt {
        syn::parse_str(code).unwrap()
    }

    #[test]
    fn methods_paths_and_inner_bindings_are_not_free() {
        let s = stmt(
            "if post_residual > 0 { let gap = (next_item_size - post_residual) as f64; \
             score += gap.powi(2).clamp(0.0, f64::MAX) * k; }",
        );
        assert_eq!(
            collect_free_variables(&s),
            vec!["k", "next_item_size", "post_residual", "score"]
        );
    }

    #[test]
    fn top_level_let_needs_only_its_initialiser() {
        assert_eq!(collect_free_variables(&stmt("let gap = a - b;")), vec!["a", "b"]);
        assert_eq!(
            collect_free_variables(&stmt("let score = score * 2.0;")),
            vec!["score"]
        );
    }
}
