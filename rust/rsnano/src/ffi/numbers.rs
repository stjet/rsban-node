use crate::numbers::Difficulty;

#[no_mangle]
pub extern "C" fn rsn_difficulty_to_multiplier(difficulty: u64, base_difficulty: u64) -> f64 {
    Difficulty::to_multiplier(difficulty, base_difficulty)
}

#[no_mangle]
pub extern "C" fn rsn_difficulty_from_multiplier(multiplier: f64, base_difficulty: u64) -> u64{
    Difficulty::from_multiplier(multiplier, base_difficulty)
}