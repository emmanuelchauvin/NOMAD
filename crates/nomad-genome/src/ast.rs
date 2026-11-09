use sha2::{Digest, Sha256};
use syn::visit::Visit;
use syn::{Expr, File, Item, ItemFn, Stmt};
use thiserror::Error;

use nomad_core::StructuralFingerprint;

#[derive(Debug, Error)]
pub enum AstError {
    #[error("Failed to parse Rust syntax: {0}")]
    ParseError(#[from] syn::Error),
    #[error("Function `score_bin` not found in source code")]
    ScoreBinNotFound,
    #[error("Invalid signature for `score_bin`: {0}")]
    InvalidSignature(String),
}

/// ============================================================================
/// CANONICAL_SCORE_BIN_SIGNATURE : Signature Canonique de Référence
/// ============================================================================
/// Signature Rust stricte exigée pour toute heuristique de type `ScorePolicy`
/// dans NOMAD V2. Permet l'analyse et l'évaluation standardisée.
pub const CANONICAL_SCORE_BIN_SIGNATURE: &str = "\
pub fn score_bin(\
    item_size: u32, \
    bin_residual: u32, \
    bin_capacity: u32, \
    step_index: usize, \
    total_items: usize, \
    open_bins_count: usize, \
    mean_residual: f64, \
    next_item_size: u32, \
    post_residual: u32, \
    fitting_bins_count: usize\
) -> f64";

/// ============================================================================
/// CANONICAL_SCORE_NODE_SIGNATURE : Signature Canonique Scheduling Multi-Ressource
/// ============================================================================
pub const CANONICAL_SCORE_NODE_SIGNATURE: &str = "\
pub fn score_node(\
    job_cpu: u32, \
    job_ram: u32, \
    node_residual_cpu: u32, \
    node_residual_ram: u32, \
    node_capacity_cpu: u32, \
    node_capacity_ram: u32, \
    step_index: usize, \
    total_jobs: usize, \
    active_nodes_count: usize, \
    mean_cpu_residual: f64, \
    mean_ram_residual: f64\
) -> f64";

/// ============================================================================
/// parse_heuristic_fn : Analyse Syntaxique et Extraction AST d'une Heuristique
/// ============================================================================
/// Analyse une chaîne de code source Rust via `syn` et extrait la fonction de scoring
/// (`score_bin`, `score_node`, ou toute fonction d'heuristique préfixée par `score_`).
pub fn parse_heuristic_fn(source: &str) -> Result<ItemFn, AstError> {
    // Tentative de parsing direct en fonction unique
    if let Ok(item_fn) = syn::parse_str::<ItemFn>(source) {
        let ident_str = item_fn.sig.ident.to_string();
        if ident_str.starts_with("score_") {
            return Ok(item_fn);
        }
    }

    // Repli : parsing du fichier complet et parcours des items
    let file: File = syn::parse_file(source)?;
    for item in file.items {
        if let Item::Fn(item_fn) = item {
            let ident_str = item_fn.sig.ident.to_string();
            if ident_str.starts_with("score_") {
                return Ok(item_fn);
            }
        }
    }

    Err(AstError::ScoreBinNotFound)
}

/// ============================================================================
/// parse_score_bin : Algorithme d'Analyse Syntaxique et d'Extraction AST
/// ============================================================================
/// Rétrocompatible avec toutes les suites de tests V2.
pub fn parse_score_bin(source: &str) -> Result<ItemFn, AstError> {
    parse_heuristic_fn(source)
}

/// ============================================================================
/// AstAnalyzer : Visiteur d'Analyse de Complexité et de Topologie AST
/// ============================================================================
/// Implémente le pattern Visiteur de `syn` pour mesurer la profondeur maximale,
/// le nombre d'instructions, de branchements et d'opérateurs dans l'arbre syntaxique.
#[derive(Default)]
struct AstAnalyzer {
    current_depth: usize,
    max_depth: usize,
    branch_count: usize,
    op_count: usize,
    statement_count: usize,
}

impl<'ast> Visit<'ast> for AstAnalyzer {
    fn visit_stmt(&mut self, node: &'ast Stmt) {
        self.statement_count += 1;
        syn::visit::visit_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &'ast Expr) {
        self.current_depth += 1;
        if self.current_depth > self.max_depth {
            self.max_depth = self.current_depth;
        }

        match node {
            Expr::If(_) | Expr::Match(_) => {
                self.branch_count += 1;
            }
            Expr::Binary(_) | Expr::Unary(_) => {
                self.op_count += 1;
            }
            _ => {}
        }

        syn::visit::visit_expr(self, node);
        self.current_depth -= 1;
    }
}

/// ============================================================================
/// compute_structural_fingerprint : Algorithme de Calcul d'Empreinte Structurelle
/// ============================================================================
/// Calcule l'empreinte topologique canonique d'une fonction `score_bin` :
/// 1. Parcourt l'AST avec `AstAnalyzer` pour mesurer profondeur, branches et statements.
/// 2. Génère la représentation tokenisée standardisée via `quote`.
/// 3. Calcule le hash cryptographique SHA-256 des tokens normalisés.
pub fn compute_structural_fingerprint(item_fn: &ItemFn) -> StructuralFingerprint {
    let mut analyzer = AstAnalyzer::default();
    analyzer.visit_item_fn(item_fn);

    // Hachage cryptographique des tokens normalisés
    let tokens = quote::quote!(#item_fn).to_string();
    let mut hasher = Sha256::new();
    hasher.update(tokens.as_bytes());
    let ast_hash = hex::encode(hasher.finalize());

    StructuralFingerprint {
        max_depth: analyzer.max_depth,
        branch_count: analyzer.branch_count,
        op_count: analyzer.op_count,
        statement_count: analyzer.statement_count,
        ast_hash,
    }
}

/// ============================================================================
/// normalize_source : Algorithme de Normalisation Syntaxique
/// ============================================================================
/// Réécrit le code source de l'AST sous une forme canonique, débarrassée des
/// variations de formatage ou d'espacement triviales via `quote`.
pub fn normalize_source(item_fn: &ItemFn) -> String {
    quote::quote!(#item_fn).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_fingerprint_valid_score_bin() {
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
                score += 50.0;
            }
            score
        }
        "#;

        let item_fn = parse_score_bin(code).expect("Should parse successfully");
        assert_eq!(item_fn.sig.ident, "score_bin");

        let fingerprint = compute_structural_fingerprint(&item_fn);
        assert!(fingerprint.max_depth > 0);
        assert_eq!(fingerprint.branch_count, 1); // one `if`
        assert!(fingerprint.statement_count >= 2);
        assert!(!fingerprint.ast_hash.is_empty());
    }
}
