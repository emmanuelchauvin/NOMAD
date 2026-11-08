use std::collections::HashSet;
use sha2::{Digest, Sha256};
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

/// ============================================================================
/// extract_logical_units : Algorithme d'Extraction des Unités Logiques
/// ============================================================================
/// Décompose le corps d'une fonction `score_bin` en une liste de fragments transférables,
/// filtre les instructions triviales et calcule le hash de chaque unité candidate.
pub fn extract_logical_units(item_fn: &ItemFn) -> Vec<LogicalUnit> {
    let mut units = Vec::new();

    for stmt in &item_fn.block.stmts {
        let (kind, should_extract) = match stmt {
            Stmt::Local(Local { init: Some(_), .. }) => {
                (LogicalUnitKind::LocalBinding, true)
            }
            Stmt::Expr(Expr::If(_), _) => {
                (LogicalUnitKind::ConditionalBranch, true)
            }
            Stmt::Expr(Expr::Binary(_) | Expr::Assign(_), _) => {
                (LogicalUnitKind::PenaltyOrBonus, true)
            }
            Stmt::Expr(Expr::Block(_), _) => {
                (LogicalUnitKind::GeneralBlock, true)
            }
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
        assert!(units.iter().any(|u| u.kind == LogicalUnitKind::LocalBinding));
        // Should find the conditional branches
        let conditionals: Vec<_> = units
            .iter()
            .filter(|u| u.kind == LogicalUnitKind::ConditionalBranch)
            .collect();
        assert_eq!(conditionals.len(), 2);
        assert!(conditionals[0].dependencies.contains(&"bin_residual".to_string()));
    }
}

