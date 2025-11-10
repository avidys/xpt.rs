# xpt.rs
Rust library and CLI tool to read XPT v5 - SAS Transport files

## Features

- **Library**: Use as a Rust crate in your projects

- **CLI Tool**: Command-line utilities for inspection and conversion
  - `xptcols` — Print dataset metadata (variables, types, lengths, positions)
  - `xpthead` — Display the first n rows of a dataset
  - `xpt2csv` — Convert an XPT dataset to CSV

## CLI usage

- Show column metadata
./target/release/xpttools xptcols DM.xpt

- Show first 10 rows (default)
./target/release/xpttools xpthead DM.xpt

- Show first 20 rows
./target/release/xpttools xpthead DM.xpt -n 20

- Convert a dataset (first member) to CSV
./target/release/xpttools xpt2csv PC.xpt -o PC.csv

- Convert a named member (if multiple)
./target/release/xpttools xpt2csv SDTM.xpt -d PC -o PC.csv

- Show first 10 rows of a specific dataset
./target/release/xpttools xpthead SDTM.xpt -d PC

- Show first 5 rows of a specific dataset
./target/release/xpttools xpthead SDTM.xpt -d PC -n 5

## Library Usage

**📖 See [USAGE.md](USAGE.md) for detailed library documentation and examples.**

Quick start:

```rust
use xpttools::read_xpt_v5;

let datasets = read_xpt_v5("data.xpt")?;
let dataset = &datasets[0];
println!("Dataset: {} ({} variables, {} rows)", 
    dataset.name, dataset.vars.len(), dataset.rows.len());
```

For Tauri/web contexts, use `read_xpt_v5_from_bytes()`:

```rust
use xpttools::read_xpt_v5_from_bytes;

let data = std::fs::read("data.xpt")?;
let datasets = read_xpt_v5_from_bytes(&data)?;
```

## Notes

- TS-140: Record Layout of a SAS Version 5/6 Data Set in SAS Transport (XPORT) Format — official offsets for NAMESTR, headers, and missing rules.  ￼
- IBM Hex Floating-Point background (for conversion correctness and exponent bias).  ￼
- The code implements the 80-byte card stream, NAMESTR (140-byte) layout, and IBM/360 (HFP) → IEEE-754 conversion, matching the SAS spec.  ￼
