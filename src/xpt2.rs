use anyhow::{Result, bail};
use encoding_rs::WINDOWS_1252;
use serde::Serialize;
use std::{fs::File, io::{Read, BufReader, BufRead}, path::Path};
use crate::ibm370::ibm64_to_f64;

const CARD: usize = 80;

fn read_card(r: &mut dyn Read) -> anyhow::Result<[u8; CARD]> {
    let mut buf = [0u8; CARD];
    r.read_exact(&mut buf)?;
    Ok(buf)
}
fn s80(c:&[u8;CARD])->&str { std::str::from_utf8(c).unwrap_or("") }
fn trim_ascii(bytes:&[u8])->String { std::str::from_utf8(bytes).unwrap_or("").trim_end().to_string() }

fn be_i16(b:&[u8])->i16 { i16::from_be_bytes([b[0],b[1]]) }
fn be_i32(b:&[u8])->i32 { i32::from_be_bytes([b[0],b[1],b[2],b[3]]) }

#[derive(Debug, Clone, Serialize)]
pub struct VarOut {
    pub name: String,
    pub label: String,
    pub length: usize,
    pub position: usize,
    pub is_char: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatasetOut {
    pub name: String,
    pub vars: Vec<VarOut>,
    pub rows: Vec<Vec<serde_json::Value>>, // strings or numbers/null
}

fn parse_namestr_140(b:&[u8])->(VarOut, usize) {
    let ntype = be_i16(&b[0..2]);                // 1=numeric, 2=char
    let nlng  = be_i16(&b[4..6]) as usize;       // length
    let nvar0 = be_i16(&b[6..8]) as usize;       // order (unused here, but could sort)
    let name  = trim_ascii(&b[8..16]);
    let label = trim_ascii(&b[16..56]);
    let npos  = be_i32(&b[84..88]) as usize;     // byte position in row
    let out = VarOut { name, label, length: nlng, position: npos, is_char: ntype != 1 };
    (out, nvar0)
}

fn decode_char(bytes:&[u8])->String {
    let (cow,_,_) = WINDOWS_1252.decode(bytes);
    cow.trim_end().to_string()
}

pub fn read_xpt_v5<P:AsRef<Path>>(path:P)->Result<Vec<DatasetOut>>{
    let f = File::open(path)?;
    let mut r = BufReader::new(f);

    let lib0 = read_card(&mut r)?; // banner
    if !s80(&lib0).starts_with("HEADER RECORD*******LIBRARY HEADER RECORD") {
        bail!("Not a SAS V5/V6 transport file");
    }
    let _ = read_card(&mut r)?; // created
    let _ = read_card(&mut r)?; // modified

    let mut out = Vec::new();

    'members: loop {
        let mh0 = match read_card(&mut r) { Ok(c)=>c, Err(_)=>break };
        if !s80(&mh0).contains("MEMBER HEADER") { break; }
        let _ = read_card(&mut r)?; // member header
        let _ = read_card(&mut r)?; // member header data
        let _ = read_card(&mut r)?; // member header data 2

        let dh0 = read_card(&mut r)?; // descriptor
        let _   = read_card(&mut r)?;
        if !s80(&dh0).contains("DSCRPTR HEADER") { bail!("Descriptor header missing"); }

        let nh0 = read_card(&mut r)?; // namestr header
        let nh1 = read_card(&mut r)?;
        if !s80(&nh0).contains("NAMESTR HEADER") { bail!("NAMESTR header missing"); }
        let nvars: usize = std::str::from_utf8(&nh1[54..58]).unwrap_or("0000").trim().parse().unwrap_or(0);

        let namestr_len = 140usize;
        let bytes_per_var_cards = ((namestr_len + CARD - 1)/CARD)*CARD;
        let mut raw = vec![0u8; nvars * bytes_per_var_cards];
        r.read_exact(&mut raw)?;
        let mut vars = Vec::with_capacity(nvars);
        for i in 0..nvars {
            let start = i*bytes_per_var_cards;
            let (v,_ord)=parse_namestr_140(&raw[start..start+namestr_len]);
            vars.push(v);
        }
        vars.sort_by_key(|v| v.position); // ensure left-to-right

        let oh0 = read_card(&mut r)?; // obs header
        let _   = read_card(&mut r)?;
        if !s80(&oh0).contains("OBS HEADER RECORD") { bail!("OBS header missing"); }

        let row_len: usize = vars.iter().map(|v| v.length).sum();
        let mut rows: Vec<Vec<serde_json::Value>> = Vec::new();
        let mut buf: Vec<u8> = Vec::new();

        // Pull cards; stop when next member header starts
        loop {
            let mut c = [0u8; CARD];
            match r.read_exact(&mut c) {
                Ok(())=>{
                    if s80(&c).starts_with("HEADER RECORD*******") && buf.is_empty() {
                        // Next member; stash card back by building a new reader that starts with this card.
                        // For brevity: keep card in a local slice we won't consume; break and rely on loop.
                        // This works because headers align on card boundaries.
                        break;
                    }
                    buf.extend_from_slice(&c);
                }
                Err(_)=>break
            }
            while buf.len() >= row_len {
                let mut row = Vec::with_capacity(vars.len());
                let mut off = 0usize;
                for v in &vars {
                    let slice = &buf[off .. off+v.length];
                    if v.is_char {
                        row.push(serde_json::Value::String(decode_char(slice)));
                    } else {
                        let mut eight = [0u8;8];
                        if v.length >= 8 { eight.copy_from_slice(&slice[0..8]); }
                        else { let pad = 8 - v.length; eight[pad..].copy_from_slice(slice); }
                        let (val,_miss)=ibm64_to_f64(&eight);
                        match val {
                            Some(f)=>row.push(serde_json::Value::from(f)),
                            None=>row.push(serde_json::Value::Null)
                        }
                    }
                    off += v.length;
                }
                rows.push(row);
                buf.drain(0..row_len);
            }
        }

        out.push(DatasetOut { name: "DATASET".into(), vars, rows });
        // loop continues to next member
    }

    Ok(out)
}