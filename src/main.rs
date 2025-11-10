use clap::{Parser, Subcommand};
use anyhow::Result;
use std::path::PathBuf;
use csv::Writer;
use xpttools::read_xpt_v5;

#[derive(Parser)]
#[command(name="xpttools", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd
}

#[derive(Subcommand)]
enum Cmd {
    /// Print datasets and variables (columns metadata)
    #[command(name = "xptcols")]
    XptCols { file: PathBuf },
    /// Display the first n rows of a dataset
    #[command(name = "xpthead")]
    XptHead { 
        file: PathBuf, 
        #[arg(short, long, default_value = "10")] 
        n: usize,
        #[arg(short, long)]
        dataset: Option<String>
    },
    /// Convert first dataset (or named) to CSV
    #[command(name = "xpt2csv")]
    Xpt2Csv { file: PathBuf, #[arg(short, long)] dataset: Option<String>, #[arg(short, long)] out: Option<PathBuf> }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::XptCols { file } => cmd_cols(file),
        Cmd::XptHead { file, n, dataset } => cmd_head(file, n, dataset),
        Cmd::Xpt2Csv { file, dataset, out } => cmd_to_csv(file, dataset, out),
    }
}

fn cmd_cols(file: PathBuf) -> Result<()> {
    let members = read_xpt_v5(&file)?;
    for (i, ds) in members.iter().enumerate() {
        println!("#{}: {}", i+1, ds.name);
        println!("  Variables ({}):", ds.vars.len());
        for (idx, v) in ds.vars.iter().enumerate() {
            println!("    {:>3}. {:8}  {:>4} bytes @{:>4}  {:5}  label={}",
                idx + 1, v.name, v.length, v.position, if v.is_char { "CHAR" } else { "NUM" }, v.label);
        }
        println!("  Rows: {}", ds.rows.len());
        println!();
    }
    Ok(())
}

fn cmd_head(file: PathBuf, n: usize, dataset: Option<String>) -> Result<()> {
    let members = read_xpt_v5(&file)?;
    if members.is_empty() {
        anyhow::bail!("No datasets found");
    }
    
    let ds = if let Some(name) = dataset {
        members.into_iter()
            .find(|d| d.name.eq_ignore_ascii_case(&name))
            .ok_or_else(|| anyhow::anyhow!("Dataset '{}' not found", name))?
    } else {
        members.into_iter().next().unwrap()
    };
    
    println!("Dataset: {} (showing first {} rows of {})", ds.name, n.min(ds.rows.len()), ds.rows.len());
    println!();
    
    // Print header
    let headers: Vec<String> = ds.vars.iter().map(|v| v.name.clone()).collect();
    println!("{}", headers.join("\t"));
    
    // Print first n rows
    let rows_to_show = n.min(ds.rows.len());
    for row in ds.rows.iter().take(rows_to_show) {
        let values: Vec<String> = row.iter()
            .map(|opt| opt.as_ref().map(|s| s.clone()).unwrap_or_default())
            .collect();
        println!("{}", values.join("\t"));
    }
    
    Ok(())
}

fn cmd_to_csv(file: PathBuf, dataset: Option<String>, out: Option<PathBuf>) -> Result<()> {
    let members = read_xpt_v5(&file)?;
    if members.is_empty() { anyhow::bail!("No datasets found"); }
    let ds = if let Some(name) = dataset {
        members.into_iter().find(|d| d.name.eq_ignore_ascii_case(&name))
            .ok_or_else(|| anyhow::anyhow!("Dataset '{}' not found", name))?
    } else {
        members.into_iter().next().unwrap()
    };

    let mut wtr: Writer<Box<dyn std::io::Write>> = if let Some(path) = out {
        Writer::from_writer(Box::new(std::fs::File::create(path)?) as Box<dyn std::io::Write>)
    } else {
        Writer::from_writer(Box::new(std::io::stdout()) as Box<dyn std::io::Write>)
    };

    // header
    let headers: Vec<String> = ds.vars.iter().map(|v| v.name.clone()).collect();
    wtr.write_record(&headers)?;

    for row in ds.rows {
        let rec: Vec<String> = row.into_iter().map(|opt| opt.unwrap_or_default()).collect();
        wtr.write_record(rec)?;
    }
    wtr.flush()?;
    Ok(())
}