use std::collections::{HashMap, HashSet, VecDeque};
use chrono::{DateTime, Utc};
use petgraph::dot::{Config, Dot};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use nomad_core::{CapsuleId, SkillCapsule};

#[derive(Debug, Error)]
pub enum GenomeError {
    #[error("Capsule with id {0} not found in phylogeny graph")]
    CapsuleNotFound(String),
    #[error("Graph serialization error: {0}")]
    SerializationError(String),
}

/// Metadata stored on a phylogenetic DAG node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsuleNode {
    pub id: CapsuleId,
    pub name: String,
    pub generation: usize,
    pub content_hash: String,
    pub is_crossover: bool,
    pub created_at: DateTime<Utc>,
}

/// Lineage edge connecting parent to child.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LineageKind {
    InitialSeed,
    Mutation { details: String },
    Crossover { parent_role: String },
}

/// Directed Acyclic Graph (DAG) tracking the phylogenetic evolution of all capsules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhylogenyGraph {
    graph: DiGraph<CapsuleNode, LineageKind>,
    node_indices: HashMap<CapsuleId, NodeIndex>,
}

impl Default for PhylogenyGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl PhylogenyGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
        }
    }

    /// Inserts a new SkillCapsule into the phylogenetic DAG, automatically creating
    /// directed edges from all its parent capsules.
    pub fn insert_capsule(&mut self, capsule: &SkillCapsule) -> NodeIndex {
        if let Some(&idx) = self.node_indices.get(&capsule.id) {
            return idx;
        }

        let node = CapsuleNode {
            id: capsule.id.clone(),
            name: capsule.name.clone(),
            generation: capsule.generation,
            content_hash: capsule.content_hash.clone(),
            is_crossover: capsule.is_crossover(),
            created_at: capsule.created_at,
        };

        let child_idx = self.graph.add_node(node);
        self.node_indices.insert(capsule.id.clone(), child_idx);

        if capsule.parent_ids.is_empty() {
            // Root seed individual
        } else if capsule.parent_ids.len() == 1 {
            let parent_id = &capsule.parent_ids[0];
            if let Some(&parent_idx) = self.node_indices.get(parent_id) {
                self.graph.add_edge(
                    parent_idx,
                    child_idx,
                    LineageKind::Mutation {
                        details: "Single-parent mutation".to_string(),
                    },
                );
            }
        } else {
            // Horizontal Swarm Crossover
            for (i, parent_id) in capsule.parent_ids.iter().enumerate() {
                if let Some(&parent_idx) = self.node_indices.get(parent_id) {
                    self.graph.add_edge(
                        parent_idx,
                        child_idx,
                        LineageKind::Crossover {
                            parent_role: format!("Parent-{}", i + 1),
                        },
                    );
                }
            }
        }

        child_idx
    }

    /// Retrieves immediate parent IDs of a capsule.
    pub fn parents_of(&self, id: &CapsuleId) -> Vec<CapsuleId> {
        let Some(&idx) = self.node_indices.get(id) else {
            return Vec::new();
        };

        self.graph
            .neighbors_directed(idx, Direction::Incoming)
            .map(|p_idx| self.graph[p_idx].id.clone())
            .collect()
    }

    /// Retrieves immediate children IDs of a capsule.
    pub fn children_of(&self, id: &CapsuleId) -> Vec<CapsuleId> {
        let Some(&idx) = self.node_indices.get(id) else {
            return Vec::new();
        };

        self.graph
            .neighbors_directed(idx, Direction::Outgoing)
            .map(|c_idx| self.graph[c_idx].id.clone())
            .collect()
    }

    /// Traverses backward to collect all ancestral capsule IDs in the lineage.
    pub fn ancestors_of(&self, id: &CapsuleId) -> HashSet<CapsuleId> {
        let mut ancestors = HashSet::new();
        let Some(&start_idx) = self.node_indices.get(id) else {
            return ancestors;
        };

        let mut queue = VecDeque::new();
        queue.push_back(start_idx);

        while let Some(current) = queue.pop_front() {
            for parent_idx in self.graph.neighbors_directed(current, Direction::Incoming) {
                let parent_id = self.graph[parent_idx].id.clone();
                if ancestors.insert(parent_id) {
                    queue.push_back(parent_idx);
                }
            }
        }

        ancestors
    }

    /// Computes the longest phylogenetic depth (number of ancestor generations) to a root.
    pub fn lineage_depth(&self, id: &CapsuleId) -> usize {
        let Some(&start_idx) = self.node_indices.get(id) else {
            return 0;
        };

        let mut max_depth = 0;
        let mut queue = VecDeque::new();
        queue.push_back((start_idx, 0));

        while let Some((curr, depth)) = queue.pop_front() {
            if depth > max_depth {
                max_depth = depth;
            }
            for parent_idx in self.graph.neighbors_directed(curr, Direction::Incoming) {
                queue.push_back((parent_idx, depth + 1));
            }
        }

        max_depth
    }

    /// Returns the total count of capsules recorded in the phylogenetic tree.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Returns the total count of lineage transitions (edges).
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Exports the graph in Graphviz DOT format for visual inspection of lineages.
    pub fn to_dot(&self) -> String {
        format!("{:?}", Dot::with_config(&self.graph, &[Config::EdgeNoLabel]))
    }

    /// Exports the graph in Mermaid flowchart syntax (`graph TD`) for markdown visualization.
    pub fn to_mermaid(&self) -> String {
        let mut mermaid = String::from("graph TD\n");

        for idx in self.graph.node_indices() {
            let node = &self.graph[idx];
            let clean_id = format!("node_{}", node.id.0.replace('-', "_"));
            let style_suffix = if node.is_crossover {
                ":::crossover"
            } else if node.generation == 0 {
                ":::root"
            } else {
                ":::mutation"
            };
            let safe_name = node.name.replace('"', "").replace('<', "").replace('>', "");
            mermaid.push_str(&format!(
                "    {}[\"{}<br/>(Gen {})\"]{}\n",
                clean_id, safe_name, node.generation, style_suffix
            ));
        }

        for edge_idx in self.graph.edge_indices() {
            if let Some((source_idx, target_idx)) = self.graph.edge_endpoints(edge_idx) {
                let source = format!("node_{}", self.graph[source_idx].id.0.replace('-', "_"));
                let target = format!("node_{}", self.graph[target_idx].id.0.replace('-', "_"));
                let edge_weight = &self.graph[edge_idx];

                let label = match edge_weight {
                    LineageKind::InitialSeed => "",
                    LineageKind::Mutation { .. } => "Mutation",
                    LineageKind::Crossover { parent_role } => parent_role.as_str(),
                };

                if label.is_empty() {
                    mermaid.push_str(&format!("    {} --> {}\n", source, target));
                } else {
                    mermaid.push_str(&format!("    {} -->|{}| {}\n", source, label, target));
                }
            }
        }

        mermaid.push_str("    classDef root fill:#1e40af,stroke:#3b82f6,stroke-width:2px,color:#fff\n");
        mermaid.push_str("    classDef mutation fill:#065f46,stroke:#10b981,stroke-width:2px,color:#fff\n");
        mermaid.push_str("    classDef crossover fill:#5b21b6,stroke:#8b5cf6,stroke-width:2px,color:#fff\n");

        mermaid
    }
}

