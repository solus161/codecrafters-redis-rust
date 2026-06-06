use crate::exceptions::CustomError;

const LAT_MIN: f64 = -85.05112878;
const LAT_MAX: f64 = 85.05112878;

const LONG_MIN: f64 = -180.0;
const LONG_MAX: f64 = 180.0;

const LAT_RANGE: f64 = LAT_MAX - LAT_MIN;
const LONG_RANGE: f64 = LONG_MAX - LONG_MIN;

const STEP: u64 = 26;
const RADIUS: f64 = 6372797.560856; // Earth radius in meter

pub fn encode(long: f64, lat: f64) -> Result<u64, CustomError> {
    if !(LAT_MIN..=LAT_MAX).contains(&lat) || !(LONG_MIN..=LONG_MAX).contains(&long) {
        let lat_str = format!("{:.6}", &lat);
        let long_str = format!("{:.6}", &long);
        return Err(CustomError::UnprocessableError(
                format!("ERR invalid longitude,latitude pair {},{}", long_str, lat_str)
        ))
    };

    // Step 1: Normalization
    let norm_long = (long - LONG_MIN)/LONG_RANGE * (1u64 << STEP) as f64;
    let norm_lat = (lat - LAT_MIN)/LAT_RANGE * (1u64 << STEP) as f64;
    
    // Step 2: Trunctation
    let norm_long = norm_long as u32;
    let norm_lat = norm_lat as u32;

    // Step 3: Interleaving
    Ok(interleave(norm_long, norm_lat))
}

pub fn decode(score: u64) -> Result<(f64, f64), CustomError> {
    let (norm_long, norm_lat) = deinterleave(score);
    
    let long_min = (norm_long as f64) / ((1u64 << STEP) as f64) * LONG_RANGE + LONG_MIN;
    let long_max = ((norm_long +1) as f64) / ((1u64 << STEP) as f64) * LONG_RANGE + LONG_MIN;
    let lat_min = (norm_lat as f64) / ((1u64 << STEP) as f64) * LAT_RANGE + LAT_MIN;
    let lat_max = ((norm_lat +1) as f64) / ((1u64 << STEP) as f64) * LAT_RANGE + LAT_MIN;
    
    let long = (long_min + long_max)/2.0;
    let lat = (lat_min + lat_max)/2.0;
    Ok((long, lat))
}

fn interleave(long: u32, lat: u32) -> u64 {
    // First, spreading i32 to i64
    // E.g.:    0000 1111 -> spreading into
    //          0101 0101
    let long = spread_u32_to_u64(long);
    let lat = spread_u32_to_u64(lat);
    lat | long << 1
}

fn deinterleave(score: u64) -> (u64, u64) {
    let lat = score;
    let long = score >> 1;

    (compact_u64_to_u32(long), compact_u64_to_u32(lat)) 
}

fn spread_u32_to_u64(x: u32) -> u64 {
    let mut x = x as u64;
    let masks = [
        0x5555555555555555_u64, 
        0x3333333333333333_u64,
        0x0F0F0F0F0F0F0F0F_u64,
        0x00FF00FF00FF00FF_u64,
        0x0000FFFF0000FFFF_u64,
    ];

    let shifts = [1, 2, 4, 8, 16];
    
    x = (x | (x << shifts[4])) & masks[4];
    x = (x | (x << shifts[3])) & masks[3];
    x = (x | (x << shifts[2])) & masks[2];
    x = (x | (x << shifts[1])) & masks[1];
    x = (x | (x << shifts[0])) & masks[0];
    x 
}

fn compact_u64_to_u32(x: u64) -> u64 {
    // Keep bits in even positions
    let mut x = x & 0x5555555555555555;
    let masks = [
        0x3333333333333333_u64,
        0x0F0F0F0F0F0F0F0F_u64,
        0x00FF00FF00FF00FF_u64,
        0x0000FFFF0000FFFF_u64,
        0x00000000FFFFFFFF_u64
    ];
    let shifts = [1, 2, 4, 8, 16];

    x = (x | (x >> shifts[0])) & masks[0];
    x = (x | (x >> shifts[1])) & masks[1];
    x = (x | (x >> shifts[2])) & masks[2];
    x = (x | (x >> shifts[3])) & masks[3];
    x = (x | (x >> shifts[4])) & masks[4];
    x
}

pub fn distance(mut locations: Vec<(f64, f64)>) -> Result<f64, CustomError> {
    // Harversine distance
    let msg_err = "No location provided for distance calculation";
    let to_radians = |x: f64, y: f64| {return (x.to_radians(), y.to_radians())};
    let (long1, lat1) = locations.pop()
        .map(|(long, lat)| to_radians(long, lat))
        .ok_or(CustomError::InternalError(msg_err.to_string()))?;
    let (long2, lat2) = locations.pop()
        .map(|(long, lat)| to_radians(long, lat))
        .ok_or(CustomError::InternalError(msg_err.to_string()))?;
    
    let v = ((long2 - long1)/2.0).sin();
    if v == 0.0 {
        return Ok(RADIUS * (lat2 - lat1).abs())
    };

    let u = ((lat2 - lat1)/2.0).sin();
    let a = u*u + lat1.cos()*lat2.cos()*v*v;
    Ok(2.0*RADIUS*(a.sqrt().asin()))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spread_u32_to_u64() {
        let x: u32 = 0b1111; 
        assert_eq!(spread_u32_to_u64(x), 0b01010101_u64);
    }
    
    #[test]
    fn test_encoding() {
        let long: f64 = 100.5252;
        let lat: f64 = 13.7220;
        assert_eq!(encode(long, lat).unwrap_or(0), 3962257306574459);
    }

    #[test]
    fn test_decoding() {
        let score: u64 = 3962257306574459;
        let (long, lat) = decode(score).unwrap_or((0.0_f64, 0.0_f64));
        assert_eq!((long*10000.0).round()/10000.0, 100.5252_f64);
        assert_eq!((lat*10000.0).round()/10000.0, 13.7220_f64);
    }
}
