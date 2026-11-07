use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a SkillCapsule, derived from SHA-256 hash or deterministic UUID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct CapsuleId(pub String);

impl CapsuleId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CapsuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The decision produced by an online 1D bin packing heuristic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackingDecision {
    /// Place the incoming item in an existing bin at the specified index.
    PlaceInExisting(usize),
    /// Open a new bin for the incoming item.
    OpenNewBin,
}

/// Experimental group designation in the NOMAD pre-registered study.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExperimentalGroup {
    /// Group A: 1 agent with sequential self-correction (baseline)
    GroupA,
    /// Group B: N independent agents with no cross-lineage transmission
    GroupB,
    /// Group D: Evolutionary selection (top 20%) with mutation only (no crossover)
    GroupD,
    /// Group C: Full NOMAD swarm with mutation and horizontal Swarm Crossover
    GroupC,
}

impl ExperimentalGroup {
    pub fn name(&self) -> &'static str {
        match self {
            Self::GroupA => "Group A (Sequential Baseline)",
            Self::GroupB => "Group B (Parallel Independent)",
            Self::GroupD => "Group D (Mutation-Only Selection)",
            Self::GroupC => "Group C (NOMAD Swarm Crossover)",
        }
    }

    pub fn allows_crossover(&self) -> bool {
        matches!(self, Self::GroupC)
    }
}

impl fmt::Display for ExperimentalGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}
