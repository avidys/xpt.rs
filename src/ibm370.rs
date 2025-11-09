use std::f64;

pub enum IbmMissing { Dot, Letter(u8), None }

pub fn ibm64_to_f64(bytes: &[u8]) -> (Option<f64>, IbmMissing) {
    if bytes.len() < 8 { return (None, IbmMissing::None); }
    let b0 = bytes[0];

    if bytes[1..].iter().all(|&v| v == 0x00) {
        return match b0 {
            0x2E | 0x5F => (None, IbmMissing::Dot),
            0x41..=0x5A => (None, IbmMissing::Letter(b0)),
            _ => (None, IbmMissing::None),
        }
    }

    let sign = (b0 & 0x80) != 0;
    let exp  = (b0 & 0x7F) as i32;
    if exp == 0 && bytes[1..].iter().all(|&v| v == 0) {
        return (Some(0.0), IbmMissing::None);
    }
    let p = exp - 64;

    let mut frac_u: u64 = 0;
    for &bb in &bytes[1..8] { frac_u = (frac_u << 8) | bb as u64; }

    let mut f = 0.0f64;
    let mut denom = 1.0f64;
    let mut tmp = frac_u;
    for _ in 0..14 {
        let nib = (tmp >> 52) & 0xF;
        f += (nib as f64) / denom;
        denom *= 16.0;
        tmp <<= 4;
    }
    let mut val = f * 16f64.powi(p);
    if sign { val = -val; }
    (Some(val), IbmMissing::None)
}