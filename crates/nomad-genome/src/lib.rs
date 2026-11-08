//! NOMAD Genome: Phylogenetic DAG modeling, AST representation, and genetic crossover operators.

pub mod dag;
pub mod ast;
pub mod unit;
pub mod crossover;
pub mod anti_dead_code;

pub use dag::*;
pub use ast::*;
pub use unit::*;
pub use crossover::*;
pub use anti_dead_code::*;

#[cfg(test)]
mod tests {
    use super::*;
    use nomad_core::SkillCapsule;
    use std::collections::HashMap;

    #[test]
    fn test_phylogeny_dag_lineage_and_crossover() {
        let mut graph = PhylogenyGraph::new();

        let parent_a = SkillCapsule::new("ParentA", "Root A", "FIRST_FIT", vec![], 0, HashMap::new());
        let parent_b = SkillCapsule::new("ParentB", "Root B", "BEST_FIT", vec![], 0, HashMap::new());

        let _idx_a = graph.insert_capsule(&parent_a);
        let _idx_b = graph.insert_capsule(&parent_b);

        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.parents_of(&parent_a.id).len(), 0);

        // Single mutation child
        let mut_child = SkillCapsule::new(
            "MutChild",
            "Mutation of A",
            "SCORE: alpha=1.0",
            vec![parent_a.id.clone()],
            1,
            HashMap::new(),
        );
        graph.insert_capsule(&mut_child);
        assert_eq!(graph.parents_of(&mut_child.id), vec![parent_a.id.clone()]);

        // Crossover child
        let cross_child = SkillCapsule::new(
            "CrossChild",
            "Recombination of A and B",
            "SCORE: alpha=1.1 beta=0.2",
            vec![parent_a.id.clone(), parent_b.id.clone()],
            2,
            HashMap::new(),
        );
        graph.insert_capsule(&cross_child);

        let cross_parents = graph.parents_of(&cross_child.id);
        assert_eq!(cross_parents.len(), 2);
        assert!(cross_parents.contains(&parent_a.id));
        assert!(cross_parents.contains(&parent_b.id));

        let ancestors = graph.ancestors_of(&cross_child.id);
        assert_eq!(ancestors.len(), 2);
        assert!(ancestors.contains(&parent_a.id));
        assert!(ancestors.contains(&parent_b.id));

        assert_eq!(graph.lineage_depth(&cross_child.id), 1);
    }
}
