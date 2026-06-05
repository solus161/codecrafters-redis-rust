use crate::exceptions::CustomError;

const LAT_MIN: f64 = -85.05112878;
const LAT_MAX: f64 = 85.05112878;

const LONG_MIN: f64 = -180.0;
const LONG_MAX: f64 = 180.0;

const LAT_RANGE: f64 = LAT_MAX - LAT_MIN;
const LONG_RANGE: f64 = LONG_MAX - LONG_MIN;

const STEP: u32 = 26;

pub fn encoding(lat: f64, long: f64) -> Result<u64, CustomError> {
    // Step 1: Normalization
    let norm_lat = (2.0 as f64).powf(26.0) * (lat - LAT_MIN)/LAT_RANGE;
    let norm_long = (2.0 as f64).powf(26.0) * (lat - LAT_MIN)/LAT_RANGE;
    
    // Step 2: Trunctation
    let norm_lat = norm_lat as u64;
    let norm_long = norm_long as u64;

    // Step 3: Interleaving
    
    Ok(0)

}

fn interleaving(lat: u32, long: u32) -> u32 {
    // First, spreading i32 to i64
    // E.g.:    0000 1111 -> spreading into
    //          0101 0101
    0
}

fn spread_i32_to_i64(x: u32) -> u64 {
    let x = x as u64;
    
    let mut new_x = 0u64;

    for i in 0..32 {
        // Mask get value at bit 2^(i + 1)
        // Left shift by i
        new_x |= (x & (1u64 << i) << i)
    };
    new_x
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spread_i32_to_i64() {
        let x: u32 = 0b1111; 
        println!("{}", spread_i32_to_i64(x));
        assert_eq!(spread_i32_to_i64(x), 0b01010101_u64);
    }
}
