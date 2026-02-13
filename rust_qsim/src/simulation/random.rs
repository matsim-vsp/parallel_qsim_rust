use rand::rngs::SmallRng;
use rand::SeedableRng;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Random number generator similar to MATSim's MatsimRandom in Java.
/// Provides node-specific random number generators based on a base seed.
#[derive(Debug)]
pub struct RandomGenerator {
    base_seed: RefCell<u64>,
}

impl Default for RandomGenerator {
    fn default() -> Self {
        RandomGenerator::new(4711)
    }
}

impl RandomGenerator {
    /// Creates a new RandomGenerator with the given base seed.
    pub fn new(base_seed: u64) -> Self {
        RandomGenerator {
            base_seed: RefCell::new(base_seed),
        }
    }

    /// Gets a random number generator for a specific node/entity.
    /// The hash parameter should uniquely identify the node/entity.
    pub fn get_rnd<H: Hash>(&self, hash: H) -> SmallRng {
        let base = *self.base_seed.borrow();
        
        // Combine base seed with the hash to get a unique seed for this entity
        let mut hasher = DefaultHasher::new();
        hash.hash(&mut hasher);
        base.hash(&mut hasher);
        let combined_seed = hasher.finish();
        
        SmallRng::seed_from_u64(combined_seed)
    }

    /// Resets the base seed to a new value.
    pub fn reset(&self, seed: u64) {
        *self.base_seed.borrow_mut() = seed;
    }

    /// Gets the current base seed.
    pub fn base_seed(&self) -> u64 {
        *self.base_seed.borrow()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[test]
    fn test_random_generator_deterministic() {
        let gen = RandomGenerator::new(42);
        
        // Same hash should produce same RNG sequence
        let mut rng1 = gen.get_rnd(123);
        let mut rng2 = gen.get_rnd(123);
        
        // Test with integers for robust comparison
        for _ in 0..10 {
            assert_eq!(rng1.random::<u32>(), rng2.random::<u32>());
        }
    }

    #[test]
    fn test_random_generator_different_hashes() {
        let gen = RandomGenerator::new(42);
        
        // Different hashes should produce different sequences
        let mut rng1 = gen.get_rnd(123);
        let mut rng2 = gen.get_rnd(456);
        
        let val1: f32 = rng1.random();
        let val2: f32 = rng2.random();
        
        assert_ne!(val1, val2);
    }

    #[test]
    fn test_random_generator_reset() {
        let gen = RandomGenerator::new(42);
        let mut rng1 = gen.get_rnd(123);
        let val1: f32 = rng1.random();
        
        gen.reset(42);
        let mut rng2 = gen.get_rnd(123);
        let val2: f32 = rng2.random();
        
        // After reset with same seed, should produce same sequence
        assert_eq!(val1, val2);
    }
}
