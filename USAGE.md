# xpt.rs Library Usage Guide

This guide explains how to use `xpttools` as a library in your Rust projects.

## Table of Contents

- [Installation](#installation)
- [Basic Usage](#basic-usage)
- [API Reference](#api-reference)
- [Data Structures](#data-structures)
- [Examples](#examples)
- [Error Handling](#error-handling)

## Installation

Add `xpttools` to your `Cargo.toml`:

### As a Local Path Dependency

```toml
[dependencies]
xpttools = { path = "../path/to/xpt.rs" }
anyhow = "1.0"  # Required for error handling
encoding_rs = "0.8"  # Required for character encoding
```

### As a Git Dependency

```toml
[dependencies]
xpttools = { git = "https://github.com/avidys/xpt.rs" }
```

## Basic Usage

### Reading from a File Path

```rust
use xpttools::read_xpt_v5;
use anyhow::Result;

fn main() -> Result<()> {
    let datasets = read_xpt_v5("data.xpt")?;
    
    for dataset in &datasets {
        println!("Dataset: {}", dataset.name);
        println!("Variables: {}", dataset.vars.len());
        println!("Rows: {}", dataset.rows.len());
    }
    
    Ok(())
}
```

### Reading from Byte Slice (Tauri/Web Contexts)

This is useful when you have the file data in memory (e.g., from a file dialog or HTTP request):

```rust
use xpttools::read_xpt_v5_from_bytes;
use anyhow::Result;
use std::fs;

fn main() -> Result<()> {
    // Read file into memory
    let data = fs::read("data.xpt")?;
    
    // Parse from bytes
    let datasets = read_xpt_v5_from_bytes(&data)?;
    
    // Use the first dataset
    let dataset = datasets.first()
        .ok_or_else(|| anyhow::anyhow!("No datasets found"))?;
    
    println!("Dataset: {}", dataset.name);
    Ok(())
}
```

## API Reference

### Functions

#### `read_xpt_v5<P: AsRef<Path>>(path: P) -> Result<Vec<Dataset>>`

Reads an XPT v5 file from the filesystem.

**Parameters:**
- `path`: Path to the XPT file (can be `&str`, `String`, `Path`, `PathBuf`, etc.)

**Returns:**
- `Result<Vec<Dataset>>`: Vector of datasets found in the file (XPT files can contain multiple datasets)

**Example:**
```rust
let datasets = read_xpt_v5("DM.xpt")?;
```

#### `read_xpt_v5_from_bytes(data: &[u8]) -> Result<Vec<Dataset>>`

Reads an XPT v5 file from a byte slice. Ideal for Tauri applications or web contexts where file data is already in memory.

**Parameters:**
- `data`: Byte slice containing the XPT file data

**Returns:**
- `Result<Vec<Dataset>>`: Vector of datasets found in the file

**Example:**
```rust
let data = std::fs::read("PC.xpt")?;
let datasets = read_xpt_v5_from_bytes(&data)?;
```

### Low-Level Functions

#### `ibm64_to_f64(bytes: &[u8]) -> (Option<f64>, IbmMissing)`

Converts IBM 360 floating-point format to IEEE-754 f64. Useful for custom numeric parsing.

**Example:**
```rust
use xpttools::ibm64_to_f64;

let bytes = [0x42, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
let (value, missing) = ibm64_to_f64(&bytes);
match value {
    Some(f) => println!("Value: {}", f),
    None => println!("Missing value: {:?}", missing),
}
```

## Data Structures

### `Dataset`

Represents a complete dataset from an XPT file.

```rust
pub struct Dataset {
    pub name: String,                    // Dataset name (currently "DATASET" placeholder)
    pub vars: Vec<VarMeta>,              // Variable metadata
    pub rows: Vec<Vec<Option<String>>>,  // Data rows (each row is a vector of optional strings)
}
```

**Fields:**
- `name`: Dataset name (TODO: parse from member header)
- `vars`: Vector of variable metadata
- `rows`: Vector of data rows, where each row is a vector of optional string values

### `VarMeta`

Contains metadata for a single variable (column).

```rust
pub struct VarMeta {
    pub name: String,              // Variable name (8 chars max in XPT)
    pub label: String,             // Variable label (40 chars max)
    pub format_name: String,       // SAS format name (e.g., "DATE9.")
    pub format_len: i16,           // Format length
    pub format_decimals: i16,      // Format decimal places
    pub informat_name: String,     // SAS informat name
    pub informat_len: i16,         // Informat length
    pub informat_decimals: i16,    // Informat decimal places
    pub length: usize,             // Storage length in bytes
    pub position: usize,           // Byte position within row
    pub is_char: bool,             // true = character, false = numeric
    pub varnum: i16,               // Variable number (1-based order)
}
```

### `IbmMissing`

Represents different types of missing values in IBM format.

```rust
pub enum IbmMissing {
    Dot,           // Standard missing (.)
    Letter(u8),    // Special missing (.A-.Z)
    None,          // Not missing
}
```

## Examples

### Example 1: Convert XPT to CSV

```rust
use xpttools::read_xpt_v5;
use anyhow::Result;
use std::io::Write;

fn xpt_to_csv(xpt_path: &str, csv_path: &str) -> Result<()> {
    let datasets = read_xpt_v5(xpt_path)?;
    let dataset = datasets.first()
        .ok_or_else(|| anyhow::anyhow!("No datasets found"))?;
    
    let mut file = std::fs::File::create(csv_path)?;
    
    // Write header
    let headers: Vec<String> = dataset.vars.iter()
        .map(|v| v.name.clone())
        .collect();
    writeln!(file, "{}", headers.join(","))?;
    
    // Write rows
    for row in &dataset.rows {
        let values: Vec<String> = row.iter()
            .map(|opt| opt.as_ref().map(|s| s.as_str()).unwrap_or(""))
            .collect();
        writeln!(file, "{}", values.join(","))?;
    }
    
    Ok(())
}
```

### Example 2: Access Variable Labels

```rust
use xpttools::read_xpt_v5;

let datasets = read_xpt_v5("data.xpt")?;
let dataset = &datasets[0];

for var in &dataset.vars {
    println!("Variable: {}", var.name);
    if !var.label.is_empty() {
        println!("  Label: {}", var.label);
    }
    println!("  Type: {}", if var.is_char { "Character" } else { "Numeric" });
    println!("  Format: {}", var.format_name);
}
```

### Example 3: Filter Numeric Variables

```rust
use xpttools::read_xpt_v5;

let datasets = read_xpt_v5("data.xpt")?;
let dataset = &datasets[0];

let numeric_vars: Vec<&VarMeta> = dataset.vars.iter()
    .filter(|v| !v.is_char)
    .collect();

println!("Found {} numeric variables", numeric_vars.len());
```

### Example 4: Tauri Command Handler

```rust
use xpttools::read_xpt_v5_from_bytes;
use serde::Serialize;

#[derive(Serialize)]
struct FileData {
    name: String,
    content: String,
}

#[tauri::command]
fn read_xpt(path: String) -> Result<FileData, String> {
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    let datasets = read_xpt_v5_from_bytes(&data)
        .map_err(|e| e.to_string())?;
    
    let dataset = datasets.first()
        .ok_or_else(|| "No datasets found".to_string())?;
    
    // Convert to CSV
    let mut csv = String::new();
    let headers: Vec<String> = dataset.vars.iter()
        .map(|v| v.name.clone())
        .collect();
    csv.push_str(&headers.join(","));
    csv.push('\n');
    
    for row in &dataset.rows {
        let values: Vec<String> = row.iter()
            .map(|opt| opt.as_ref().map(|s| s.as_str()).unwrap_or(""))
            .collect();
        csv.push_str(&values.join(","));
        csv.push('\n');
    }
    
    let name = std::path::Path::new(&path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    
    Ok(FileData { name, content: csv })
}
```

### Example 5: Handle Multiple Datasets

```rust
use xpttools::read_xpt_v5;

let datasets = read_xpt_v5("multi_dataset.xpt")?;

for (idx, dataset) in datasets.iter().enumerate() {
    println!("Dataset {}: {}", idx + 1, dataset.name);
    println!("  Variables: {}", dataset.vars.len());
    println!("  Rows: {}", dataset.rows.len());
}

// Find a specific dataset by name
let pc_dataset = datasets.iter()
    .find(|ds| ds.name.eq_ignore_ascii_case("PC"));
```

## Error Handling

All functions return `anyhow::Result`, which makes error handling straightforward:

```rust
use xpttools::read_xpt_v5;
use anyhow::{Result, Context};

fn process_xpt(path: &str) -> Result<()> {
    let datasets = read_xpt_v5(path)
        .with_context(|| format!("Failed to read XPT file: {}", path))?;
    
    // Process datasets...
    Ok(())
}
```

Common errors you might encounter:

- **File not found**: The XPT file doesn't exist at the specified path
- **Invalid format**: The file is not a valid XPT v5 file
- **Missing headers**: Required headers (LIBRARY, MEMBER, NAMESTR, OBS) are missing
- **Parse errors**: Data corruption or unexpected format variations

## Notes

- **Character Encoding**: Character variables are decoded using Windows-1252 encoding (as per XPT spec)
- **Numeric Values**: Numeric values are converted from IBM 360 floating-point to IEEE-754 f64, then formatted as strings
- **Missing Values**: Missing numeric values are represented as `None` in the `Option<String>` vectors
- **Multi-Dataset Files**: XPT files can contain multiple datasets (members); the library returns all of them
- **Dataset Names**: Currently, dataset names are set to "DATASET" placeholder. This will be improved in future versions to parse from member headers.

## See Also

- [README.md](README.md) - Command-line tool usage
- [TS-140 Specification](https://www.loc.gov/preservation/digital/formats/fdd/fdd000464.shtml) - Official XPT format documentation
