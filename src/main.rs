mod ibm370;
mod xpt;

use clap::{Parser, Subcommand};
use anyhow::Result;
use std::path::PathBuf;
use csv::Writer;

#[derive(Parser)]
#[command(name="xpttools", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd
}

#[derive(Subcommand)]
enum Cmd {
    /// Print datasets and variables
    XptCat { file: PathBuf },
    /// Convert first dataset (or named) to CSV
    Xpt2Csv { file: PathBuf, #[arg(short, long)] dataset: Option<String>, #[arg(short, long)] out: Option<PathBuf> }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::XptCat { file } => cmd_cat(file),
        Cmd::Xpt2Csv { file, dataset, out } => cmd_to_csv(file, dataset, out),
    }
}

fn cmd_cat(file: PathBuf) -> Result<()> {
    let members = xpt::read_xpt_v5(&file)?;
    for (i, ds) in members.iter().enumerate() {
        println!("#{}: {}", i+1, ds.name);
        println!("  Variables ({}):", ds.vars.len());
        for v in &ds.vars {
            println!("    {:>3}. {:8}  {:>4} bytes @{:>4}  {:5}  label={}",
                v.varnum, v.name, v.length, v.position, if v.is_char { "CHAR" } else { "NUM" }, v.label);
        }
        println!("  Rows: {}", ds.rows.len());
        println!();
    }
    Ok(())
}

fn cmd_to_csv(file: PathBuf, dataset: Option<String>, out: Option<PathBuf>) -> Result<()> {
    let members = xpt::read_xpt_v5(&file)?;
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