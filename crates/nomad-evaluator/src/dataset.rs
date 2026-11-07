use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A single Bin Packing Online 1D instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinPackingInstance {
    pub name: String,
    pub items: Vec<u32>,
    pub capacity: u32,
    pub theoretical_min_bins: usize,
    pub instance_hash: String,
}

impl BinPackingInstance {
    pub fn new(name: impl Into<String>, items: Vec<u32>, capacity: u32) -> Self {
        let name = name.into();
        let total_weight: u64 = items.iter().map(|&w| w as u64).sum();
        let cap = capacity as u64;
        let theoretical_min_bins = ((total_weight + cap - 1) / cap) as usize;

        let mut hasher = Sha256::new();
        hasher.update(name.as_bytes());
        hasher.update(capacity.to_le_bytes());
        for &w in &items {
            hasher.update(w.to_le_bytes());
        }
        let hash = hex::encode(hasher.finalize());

        Self {
            name,
            items,
            capacity,
            theoretical_min_bins,
            instance_hash: hash,
        }
    }
}

/// A suite of bin packing instances with pre-registered cryptographic fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dataset {
    pub name: String,
    pub instances: Vec<BinPackingInstance>,
    pub dataset_hash: String,
}

impl Dataset {
    pub fn new(name: impl Into<String>, instances: Vec<BinPackingInstance>) -> Self {
        let name = name.into();
        let mut hasher = Sha256::new();
        hasher.update(name.as_bytes());
        for inst in &instances {
            hasher.update(inst.instance_hash.as_bytes());
        }
        let dataset_hash = hex::encode(hasher.finalize());

        Self {
            name,
            instances,
            dataset_hash,
        }
    }

    /// Pre-registered In-Distribution Dataset:
    /// 50 instances, N=50 items, Uniform distribution U(10, 80), capacity C=100.
    pub fn generate_in_distribution(seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut instances = Vec::with_capacity(50);
        let capacity = 100u32;

        for i in 0..50 {
            let items: Vec<u32> = (0..50).map(|_| rng.gen_range(10..=80)).collect();
            instances.push(BinPackingInstance::new(
                format!("in_dist_{:02}", i + 1),
                items,
                capacity,
            ));
        }

        Self::new("NOMAD_IN_DISTRIBUTION_N50_50INST", instances)
    }

    /// Pre-registered OOD-1 (Scale) Dataset:
    /// 50 instances, N=500 items, Uniform distribution U(10, 80), capacity C=100.
    pub fn generate_ood1_scale(seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0x00D1_5CA1E);
        let mut instances = Vec::with_capacity(50);
        let capacity = 100u32;

        for i in 0..50 {
            let items: Vec<u32> = (0..500).map(|_| rng.gen_range(10..=80)).collect();
            instances.push(BinPackingInstance::new(
                format!("ood1_scale_{:02}", i + 1),
                items,
                capacity,
            ));
        }

        Self::new("NOMAD_OOD1_SCALE_N500_50INST", instances)
    }

    /// Pre-registered OOD-2 (Distribution Shift / Power-Law) Dataset:
    /// 50 instances, N=500 items, Heavy-tailed / power-law skewed distribution, capacity C=100.
    pub fn generate_ood2_distribution_shift(seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0x00D2_5E1F7);
        let mut instances = Vec::with_capacity(50);
        let capacity = 100u32;

        for i in 0..50 {
            let items: Vec<u32> = (0..500)
                .map(|_| {
                    // Power-law distribution: P(x) ~ x^(-gamma) with gamma = 1.5
                    let u: f64 = rng.gen_range(0.001..1.0);
                    let val = 5.0 / u.powf(0.5);
                    let clamped = (val as u32).clamp(5, 95);
                    clamped
                })
                .collect();
            instances.push(BinPackingInstance::new(
                format!("ood2_powerlaw_{:02}", i + 1),
                items,
                capacity,
            ));
        }

        Self::new("NOMAD_OOD2_POWERLAW_N500_50INST", instances)
    }
}
