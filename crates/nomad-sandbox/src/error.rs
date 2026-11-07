use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SandboxError {
    #[error("Item weight {weight} exceeds absolute bin capacity {capacity}")]
    ItemExceedsCapacity { weight: u32, capacity: u32 },

    #[error("Selected bin index {index} is out of bounds (open bins: {open_bins})")]
    InvalidBinIndex { index: usize, open_bins: usize },

    #[error("Placing item of weight {weight} in bin {index} (level {current_level}) overflows capacity {capacity}")]
    CapacityOverflow {
        index: usize,
        current_level: u32,
        weight: u32,
        capacity: u32,
    },

    #[error("Computational fuel limit exceeded ({fuel_consumed} > {fuel_limit})")]
    FuelLimitExceeded { fuel_consumed: u64, fuel_limit: u64 },

    #[error("Code syntax or execution error: {0}")]
    ExecutionCrash(String),

    #[error("Invalid decision returned: {0}")]
    MalformedDecision(String),
}

