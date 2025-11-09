pub mod ibm370;
pub mod xpt;

pub use xpt::{Dataset, VarMeta};
pub use ibm370::{ibm64_to_f64, IbmMissing};

use anyhow::Result;
use std::io::{Cursor, BufReader};
use std::path::Path;

/// Read XPT v5 file from a path
pub fn read_xpt_v5<P: AsRef<Path>>(path: P) -> Result<Vec<Dataset>> {
    xpt::read_xpt_v5(path)
}

/// Read XPT v5 from byte slice (for use in Tauri/web contexts)
pub fn read_xpt_v5_from_bytes(data: &[u8]) -> Result<Vec<Dataset>> {
    let cursor = Cursor::new(data);
    let reader = BufReader::new(cursor);
    xpt::read_xpt_v5_from_reader(reader)
}

