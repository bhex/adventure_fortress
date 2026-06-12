use rand::distr::weighted::WeightedIndex;
use rand::distr::Distribution;

pub type GameRng = rand_chacha::ChaCha8Rng;

pub fn weighted_index(rng: &mut GameRng, weights: &[f64]) -> Option<usize> {
    let dist = WeightedIndex::new(weights).ok()?;
    Some(dist.sample(rng))
}
