# xpt.rs
Rust library and CLI tool to read XPT v5 - SAS Transport files

## Features

- **Library**: Use as a Rust crate in your projects
- **CLI Tool**: Command-line utilities for inspection and conversion
  - `xpt-cat` — print dataset metadata (variables, types, lengths, positions)
  - `xpt2-csv` — convert an XPT dataset to CSV

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


cargo login <your_api_token>  # from crates.io
cargo publish --dry-run


# Build
cargo build --release

# Convert a dataset (first member) to CSV
./target/release/xpttools xpt2csv PC.xpt -o PC.csv

# Convert a named member (if multiple)
./target/release/xpttools xpt2csv SDTM.xpt -d PC -o PC.csv

# Show column metadata
./target/release/xpttools xptcols DM.xpt

# Show first 10 rows (default)
./target/release/xpttools xpthead DM.xpt

# Show first 20 rows
./target/release/xpttools xpthead DM.xpt -n 20

# Show first 10 rows of a specific dataset
./target/release/xpttools xpthead SDTM.xpt -d PC

# Show first 5 rows of a specific dataset
./target/release/xpttools xpthead SDTM.xpt -d PC -n 5


Check with R haven::read_xpt() or Python xport.v56 


The code implements the 80-byte card stream, NAMESTR (140-byte) layout, and IBM/360 (HFP) → IEEE-754 conversion, matching the SAS spec.  ￼


## TODO

Parse the dataset name/label from the member header data cards (TS-140 section 5) to fill Dataset.name.  ￼
	•	Add v6/v8 handling (different limits; v8 spec document linked in Library of Congress refs).  ￼
	•	Add an option to preserve SAS missing (.A–.Z) as strings instead of blank.


## Sources
	•	TS-140: Record Layout of a SAS Version 5/6 Data Set in SAS Transport (XPORT) Format — official offsets for NAMESTR, headers, and missing rules.  ￼
	•	IBM Hex Floating-Point background (for conversion correctness and exponent bias).  ￼

