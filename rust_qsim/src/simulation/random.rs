use rand::rngs::SmallRng;
use rand::SeedableRng;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Random number generator utilities similar to MATSim's MatsimRandom in Java.
/// Provides static functions to create node-specific random number generators based on a base seed.

/// Gets a random number generator for a specific node/entity.
/// The hash parameter should uniquely identify the node/entity.
pub fn get_rnd<H: Hash>(base_seed: u64, hash: H) -> SmallRng {
    // Combine base seed with the hash to get a unique seed for this entity
    let mut hasher = DefaultHasher::new();
    hash.hash(&mut hasher);
    base_seed.hash(&mut hasher);
    let combined_seed = hasher.finish();
    
    SmallRng::seed_from_u64(combined_seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[test]
    fn test_random_generator_deterministic() {
        let base_seed = 42;
        
        // Same hash should produce same RNG sequence
        let mut rng1 = get_rnd(base_seed, 123);
        let mut rng2 = get_rnd(base_seed, 123);
        
        // Test with integers for robust comparison
        for _ in 0..10 {
            assert_eq!(rng1.random::<u32>(), rng2.random::<u32>());
        }
    }

    #[test]
    fn test_random_generator_different_hashes() {
        let base_seed = 42;
        
        // Different hashes should produce different sequences
        let mut rng1 = get_rnd(base_seed, 123);
        let mut rng2 = get_rnd(base_seed, 456);
        
        let val1: f32 = rng1.random();
        let val2: f32 = rng2.random();
        
        assert_ne!(val1, val2);
    }

    #[test]
    fn test_random_generator_same_seed_reproducible() {
        let base_seed = 42;
        let mut rng1 = get_rnd(base_seed, 123);
        let val1: f32 = rng1.random();
        
        // Create new RNG with same seed
        let mut rng2 = get_rnd(base_seed, 123);
        let val2: f32 = rng2.random();
        
        // Should produce same sequence
        assert_eq!(val1, val2);
    }
}
