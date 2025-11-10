pub mod ibm370;
pub mod xpt_parser;

pub use ibm370::{ibm64_to_f64, IbmMissing};

use anyhow::Result;
use std::fs;
use std::path::Path;

/// Dataset structure matching the expected API
#[derive(Debug, Clone)]
pub struct Dataset {
    pub name: String,
    pub vars: Vec<VarMeta>,
    pub rows: Vec<Vec<Option<String>>>,
}

/// Variable metadata matching the expected API
#[derive(Debug, Clone)]
pub struct VarMeta {
    pub name: String,
    pub label: String,
    pub length: usize,
    pub position: usize,
    pub is_char: bool,
}

/// Read XPT v5 file from a path
pub fn read_xpt_v5<P: AsRef<Path>>(path: P) -> Result<Vec<Dataset>> {
    let data = fs::read(path)?;
    read_xpt_v5_from_bytes(&data)
}

/// Read XPT v5 from byte slice (for use in Tauri/web contexts)
pub fn read_xpt_v5_from_bytes(data: &[u8]) -> Result<Vec<Dataset>> {
    // xpt_parser returns a single dataset, convert to our API format
    let xpt_dataset = xpt_parser::XPTParser::parse(data, None)?;
    
    // Convert XPTDataset to Dataset format
    let vars: Vec<VarMeta> = xpt_dataset.variables.iter()
        .map(|v| VarMeta {
            name: v.name.clone(),
            label: v.label.clone(),
            length: v.length,
            position: v.position,
            is_char: v.var_type == xpt_parser::VariableType::Character,
        })
        .collect();
    
    let rows: Vec<Vec<Option<String>>> = xpt_dataset.rows.iter()
        .map(|row| row.values.iter()
            .map(|v| if v.is_empty() { None } else { Some(v.clone()) })
            .collect())
        .collect();
    
    Ok(vec![Dataset {
        name: xpt_dataset.title,
        vars,
        rows,
    }])
}

