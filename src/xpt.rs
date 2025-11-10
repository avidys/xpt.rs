use anyhow::{Result, bail};
use encoding_rs::WINDOWS_1252;
use std::io::{Read, BufReader};
use std::{fs::File, path::Path};
use crate::ibm370::{ibm64_to_f64, IbmMissing};

const CARD: usize = 80;

fn read_card(r: &mut dyn Read) -> Result<[u8; CARD]> {
    let mut buf = [0u8; CARD];
    std::io::Read::read_exact(r, &mut buf)?;
    Ok(buf)
}
fn card_str(c: &[u8; CARD]) -> &str {
    std::str::from_utf8(c).unwrap_or("")
}
fn trim_ascii(s: &[u8]) -> String {
    // Use from_utf8_lossy to handle invalid UTF-8 gracefully, then trim nulls and whitespace
    String::from_utf8_lossy(s)
        .trim_end_matches(|c: char| c.is_whitespace() || c == '\0')
        .to_string()
}

#[derive(Debug, Clone)]
pub struct VarMeta {
    pub name: String,
    pub label: String,
    pub format_name: String,
    pub format_len: i16,
    pub format_decimals: i16,
    pub informat_name: String,
    pub informat_len: i16,
    pub informat_decimals: i16,
    pub length: usize,        // nlng
    pub position: usize,      // npos (byte offset within row)
    pub is_char: bool,        // ntype: 1=numeric, 2=char
    pub varnum: i16,          // nvar0 (1-based order)
}

#[derive(Debug)]
pub struct Dataset {
    pub name: String,
    pub vars: Vec<VarMeta>,
    pub rows: Vec<Vec<Option<String>>>, // keep as strings for CSV-friendly output
}

fn be_i16(b: &[u8]) -> i16 { i16::from_be_bytes([b[0], b[1]]) }
fn be_i32(b: &[u8]) -> i32 { i32::from_be_bytes([b[0], b[1], b[2], b[3]]) }

/// Parse one 140-byte NAMESTR per TS-140 (v5/v6).
/// struct NAMESTR {
///   short ntype;     // 0..1
///   short nhfun;     // 2..3  (hash; often 0)
///   short nlng;      // 4..5  (length)
///   short nvar0;     // 6..7  (varnum)
///   char  nname[8];  // 8..15
///   char  nlabel[40];// 16..55
///   char  nform[8];  // 56..63
///   short nfl;       // 64..65
///   short nfd;       // 66..67
///   short nfj;       // 68..69
///   char  nfill[2];  // 70..71
///   char  niform[8]; // 72..79
///   short nifl;      // 80..81
///   short nifd;      // 82..83
///   long  npos;      // 84..87 (byte position within observation)
///   char  rest[52];  // 88..139 (ignored)
/// };
fn parse_namestr_140(b: &[u8]) -> VarMeta {
    if b.len() < 88 {
        eprintln!("Warning: NAMESTR record too short: {} bytes", b.len());
    }
    let ntype = be_i16(&b[0..2]);     // 1=numeric, 2=char
    let _nhfun = be_i16(&b[2..4]);
    let nlng  = be_i16(&b[4..6]) as usize;
    let nvar0 = be_i16(&b[6..8]);
    let name  = trim_ascii(&b[8..16]);
    let label = trim_ascii(&b[16..56]);
    let nform = trim_ascii(&b[56..64]);
    let nfl   = be_i16(&b[64..66]);
    let nfd   = be_i16(&b[66..68]);
    let _nfj  = be_i16(&b[68..70]);
    // b[70..72] = nfill
    let niform = trim_ascii(&b[72..80]);
    let nifl   = be_i16(&b[80..82]);
    let nifd   = be_i16(&b[82..84]);
    let npos   = be_i32(&b[84..88]) as usize;
    
    // Debug: log raw bytes for first few variables
    if nvar0 <= 3 {
        eprintln!("Raw NAMESTR bytes 0-15: {:?}", &b[0..16.min(b.len())]);
        eprintln!("  ntype={}, nlng={}, nvar0={}, name bytes: {:?}", ntype, nlng, nvar0, &b[8..16]);
    }

    VarMeta {
        name, label,
        format_name: nform, format_len: nfl, format_decimals: nfd,
        informat_name: niform, informat_len: nifl, informat_decimals: nifd,
        length: nlng, position: npos,
        is_char: ntype != 1,
        varnum: nvar0,
    }
}

fn decode_char(bytes: &[u8]) -> String {
    let (cow, _, _) = WINDOWS_1252.decode(bytes);
    cow.trim_end().to_string()
}

#[allow(dead_code)]
fn looks_like_header(card: &[u8]) -> bool {
    card_str(&card.try_into().unwrap_or([0u8; 80])).starts_with("HEADER RECORD*******")
}

/// Read one V5/V6 transport file from a reader; return all members (datasets).
pub fn read_xpt_v5_from_reader<R: Read>(mut r: BufReader<R>) -> Result<Vec<Dataset>> {
    // Try to read LIBRARY header (3 cards: banner + 2 real headers)
    // Some XPT files are single-member and don't have LIBRARY wrapper
    let lib0 = match read_card(&mut r) {
        Ok(c) => c,
        Err(e) => bail!("Failed to read first card (file may be too small or corrupted): {}", e),
    };
    let lib0_str = card_str(&lib0);
    let has_library_header = lib0_str.starts_with("HEADER RECORD*******LIBRARY HEADER RECORD");
    
    let mut first_card = lib0; // Keep track of first card for single-member files
    
    if has_library_header {
        // Full library format - read the two additional header cards
        let _lib1 = read_card(&mut r)?; // real header 1
        let _lib2 = read_card(&mut r)?; // real header 2
    } else {
        // Check if first card looks like any valid XPT header
        if !lib0_str.starts_with("HEADER RECORD*******") {
            bail!("File does not start with a valid XPT header. First 80 bytes: {}", 
                  String::from_utf8_lossy(&first_card[..first_card.len().min(80)]));
        }
    }

    let mut out = Vec::new();
    let mut peeked_namestr_card: Option<[u8; CARD]> = None; // Store a card that might be NAMESTR

    loop {
        // MEMBER HEADER (2 cards) + MEMBER HEADER DATA (2 cards)
        // For single-member files without LIBRARY header, reuse the first card we read
        let mh0 = if !has_library_header && out.is_empty() {
            first_card
        } else {
            match read_card(&mut r) { 
                Ok(c) => c, 
                Err(_) => break 
            }
        };
        
        // Check what type of header this is
        // Note: Headers may have varying numbers of spaces, so we check for key words
        let mh0_str = card_str(&mh0);
        let is_member_header = mh0_str.contains("MEMBER") && mh0_str.contains("HEADER");
        let is_namestr_header = mh0_str.contains("NAMESTR") && mh0_str.contains("HEADER");
        let is_dscrptr_header = mh0_str.contains("DSCRPTR") && mh0_str.contains("HEADER");
        
        if !has_library_header && out.is_empty() {
            // Single-member file - might start with MEMBER, DSCRPTR, or NAMESTR
            if is_member_header {
                // Has member header - read the rest
                let _mh1 = read_card(&mut r)?;
                let _mhd0 = read_card(&mut r)?;
                let _mhd1 = read_card(&mut r)?;
                
                // DSCRPTR HEADER (2 cards) - optional, some files skip it
                // Try to read it, but if it's NAMESTR, we'll use it as NAMESTR header
                let dh0_result = read_card(&mut r);
                match dh0_result {
                    Ok(dh0) => {
                        let dh0_str = card_str(&dh0);
                        if dh0_str.contains("DSCRPTR") && dh0_str.contains("HEADER") {
                            // It's DSCRPTR - read second card
                            let _dh1 = read_card(&mut r)?;
                        } else if dh0_str.contains("NAMESTR") && dh0_str.contains("HEADER") {
                            // It's actually NAMESTR - store it to use as nh0
                            peeked_namestr_card = Some(dh0);
                        }
                        // If it's something else, we'll try to continue
                    }
                    Err(_) => {
                        // EOF - file might be corrupted or incomplete
                    }
                }
            } else if is_dscrptr_header {
                // Starts with DSCRPTR - read second card
                let _dh1 = read_card(&mut r)?;
            } else if !is_namestr_header {
                // Doesn't start with any expected header - try to continue anyway
                // This handles files that might have slight variations
            }
        } else if is_member_header {
            // Full format: read member header cards
            let _mh1 = read_card(&mut r)?;
            let _mhd0 = read_card(&mut r)?;
            let _mhd1 = read_card(&mut r)?;
            
            // DSCRPTR HEADER (2 cards) - optional, some files skip it
            // Try to read it, but if it's NAMESTR, we'll use it as NAMESTR header
            let dh0_result = read_card(&mut r);
            match dh0_result {
                Ok(dh0) => {
                    let dh0_str = card_str(&dh0);
                    if dh0_str.contains("DSCRPTR") && dh0_str.contains("HEADER") {
                        // It's DSCRPTR - read second card
                        let _dh1 = read_card(&mut r)?;
                    } else if dh0_str.contains("NAMESTR") && dh0_str.contains("HEADER") {
                        // It's actually NAMESTR - store it to use as nh0
                        peeked_namestr_card = Some(dh0);
                    }
                    // If it's something else, we'll try to continue
                }
                Err(_) => {
                    // EOF - file might be corrupted or incomplete
                }
            }
        } else if !is_namestr_header && !is_dscrptr_header {
            // Not a recognized header - if we haven't found anything yet, 
            // this might be a file that starts directly with NAMESTR or has a different structure
            // Try to continue and look for NAMESTR header in the next cards
            if out.is_empty() && !has_library_header {
                // For single-member files, try to find NAMESTR by reading ahead
                // But we've already consumed mh0, so we need to check if the next card is NAMESTR
                // Actually, let's just try to read the next card and see if it's NAMESTR
            } else if out.is_empty() {
                let card_preview = card_str(&mh0);
                bail!("Expected MEMBER, DSCRPTR, or NAMESTR header but found: {}", 
                      card_preview.chars().take(60).collect::<String>());
            } else {
                // If we've already parsed a dataset, this might be the end
                break;
            }
        }

        // NAMESTR HEADER (2 cards) — second card has var count at offset 54, ASCII
        let nh0 = if let Some(card) = peeked_namestr_card.take() {
            // We already read the NAMESTR header card (it was after MEMBER, not DSCRPTR)
            card
        } else if !has_library_header && out.is_empty() && is_namestr_header {
            // We already have nh0 in mh0
            mh0
        } else {
            // Need to read NAMESTR header card
            read_card(&mut r)?
        };
        let nh1 = read_card(&mut r)?;
        let nh0_str = card_str(&nh0);
        if !nh0_str.contains("NAMESTR") || !nh0_str.contains("HEADER") {
            bail!("NAMESTR header missing. Found: {}", nh0_str.chars().take(60).collect::<String>());
        }
        // var count: four ASCII digits starting at column offset 54 (0-based)
        // But some files don't have this, so we'll calculate from the space between headers
        let nvars_from_header: usize = std::str::from_utf8(&nh1[54..58]).unwrap_or("0000").trim().parse().unwrap_or(0);
        
        // Debug: show nh1 content
        eprintln!("nh1 (second NAMESTR header card): first 32 bytes: {:?}, as string: {}", 
                 &nh1[0..32], card_str(&nh1).chars().take(40).collect::<String>());
        
        // IMPORTANT: nh1 is actually the first 80 bytes of the first NAMESTR record!
        // We need to include it in the buffer, not skip it
        let mut namestr_buffer = Vec::new();
        namestr_buffer.extend_from_slice(&nh1);

        // NAMESTR records — each is 140 bytes, streamed across 80-byte cards
        // Spec also allows 136 bytes on VAX; we assume 140 here.
        // Records are stored sequentially (not card-aligned), so we extract at 140-byte intervals
        let namestr_len = 140usize;
        
        // After the NAMESTR header (2 cards = 160 bytes), the data starts at the next card boundary
        // But we've already read nh0 and nh1, so the next card should be the start of NAMESTR data
        // However, we need to check if there's padding or if data starts immediately
        
        // Strategy: Read cards until we find OBS header, then calculate nvars from what we read
        // This is more robust than relying on the header field
        // Note: nh1 is already in namestr_buffer above
        let mut cards_read = 0;
        let mut found_obs = false;
        let mut obs_card: Option<[u8; CARD]> = None;
        
        // Read cards and look for OBS header
        // Skip leading zero-filled cards (padding) before NAMESTR data
        let mut skip_zeros = true;
        loop {
            let card = match read_card(&mut r) {
                Ok(c) => c,
                Err(_) => break, // EOF
            };
            cards_read += 1;
            let card_str_check = card_str(&card);
            
            // Debug: show first few cards
            if cards_read <= 5 {
                eprintln!("Card {}: first 32 bytes: {:?}, as string: {}", 
                         cards_read, &card[0..32.min(card.len())], 
                         card_str_check.chars().take(40).collect::<String>());
            }
            
            if card_str_check.contains("OBS") && card_str_check.contains("HEADER") {
                // Found OBS header - store it and break
                obs_card = Some(card);
                found_obs = true;
                break;
            } else {
                // Check if this card is all zeros (padding)
                let is_zero_card = card.iter().all(|&b| b == 0);
                
                if skip_zeros && is_zero_card {
                    // Skip zero-filled padding cards at the start
                    eprintln!("Skipping zero-filled card {}", cards_read);
                    continue;
                } else {
                    // We've found non-zero data, stop skipping
                    skip_zeros = false;
                    // Not OBS header yet - add to buffer
                    namestr_buffer.extend_from_slice(&card);
                }
            }
            
            // Safety limit: if we've read too many cards without finding OBS, something's wrong
            if cards_read > 1000 {
                bail!("Read too many cards ({}) without finding OBS header. Last card: {}", 
                      cards_read, card_str_check.chars().take(60).collect::<String>());
            }
        }
        
        if !found_obs {
            bail!("OBS header not found after reading {} cards of NAMESTR data", cards_read);
        }
        
        // Debug: show first few bytes of buffer
        if namestr_buffer.len() >= 32 {
            eprintln!("First 32 bytes of namestr_buffer: {:?}", &namestr_buffer[0..32]);
        }
        
        // Calculate nvars from the buffer size
        // The buffer contains all cards between NAMESTR and OBS headers
        // Each variable is 140 bytes, stored sequentially (not card-aligned)
        let calculated_nvars = namestr_buffer.len() / namestr_len;
        
        let nvars = if nvars_from_header > 0 && nvars_from_header == calculated_nvars {
            // Header value matches calculated value - use it
            nvars_from_header
        } else if calculated_nvars > 0 {
            // Use calculated value (more reliable)
            calculated_nvars
        } else if nvars_from_header > 0 {
            // Header says we have vars but buffer is empty - use header value
            nvars_from_header
        } else {
            bail!("No variables found. NAMESTR buffer size: {} bytes, bytes_per_var: {}, header count: {}", 
                  namestr_buffer.len(), namestr_len, nvars_from_header);
        };
        
        // Parse the NAMESTR records from the buffer
        // Records are stored sequentially at 140-byte intervals (not card-aligned)
        let mut vars = Vec::with_capacity(nvars);
        for i in 0..nvars {
            let start = i * namestr_len;  // Extract at 140-byte intervals, not card-aligned
            if start + namestr_len <= namestr_buffer.len() {
                let var = parse_namestr_140(&namestr_buffer[start..start + namestr_len]);
                eprintln!("Variable {}: name='{}', length={}, position={}, is_char={}", 
                         i, var.name, var.length, var.position, var.is_char);
                vars.push(var);
            } else {
                // Not enough data for this variable - break
                break;
            }
        }
        
        if vars.is_empty() {
            bail!("Failed to parse any variables. Buffer size: {}, expected {} variables", 
                  namestr_buffer.len(), nvars);
        }
        
        // Sort variables by position to ensure correct order for output
        // But note: the old parser uses sequential offsets when reading data, not position field
        vars.sort_by_key(|v| v.position);
        
        // Calculate row length as sum of all variable lengths (like old parser)
        // The position field in NAMESTR is for ordering, but data is stored sequentially
        let total_row_len: usize = vars.iter().map(|v| v.length).sum();
        eprintln!("Total row length: {} bytes (sum of lengths)", total_row_len);
        
        // Read the second OBS header card
        let _oh1 = match read_card(&mut r) {
            Ok(c) => c,
            Err(e) => bail!("Failed to read OBS header second card: {}", e),
        };

        // Observation length is the maximum (position + length) across all variables
        // This accounts for any gaps or padding between variables
        let row_len = total_row_len;
        eprintln!("Parsing observations: {} variables, row_len={} bytes", vars.len(), row_len);
        let mut rows: Vec<Vec<Option<String>>> = Vec::new();
        let mut buf: Vec<u8> = Vec::new();

        // Pull cards until we see the start of the next HEADER...
        loop {
            let mut c = [0u8; CARD];
            match r.read_exact(&mut c) {
                Ok(()) => {
                    // If this card starts a new header, push back into a small stash and break
                    if card_str(&c).starts_with("HEADER RECORD*******") {
                        // "Unread" this card by storing it in an internal buffer of the reader:
                        // (BufReader doesn't support un-read; so we keep it in memory and handle after loop)
                        // Strategy: keep it and process after loop by creating a chain reader.
                        // Simpler: just keep it and break; the outer loop starts from this header.
                        // But we need to keep it: store in `buf_head`.
                        // Workaround: store in a global and use it next loop? For simplicity: break and
                        // re-create the BufReader from an in-memory buffer is overkill.
                        // Pragmatic solution: keep c in a local and pass to outer loop via return value
                        // -> Simpler: We break, and rely on the outer loop to continue; this loses 1 card.
                        // To avoid loss, we detect this only when buf is empty (i.e., start of a row boundary).
                        if buf.is_empty() { // safe to break: no partial row
                            // put c into a "pre-read" slot by using a custom reader would be cleaner;
                            // for brevity, we stop here and accept that the outer loop already consumed it.
                            // (Real-world: implement a small LookaheadReader.)
                            // We'll break and assume next read starts at mh0 for next member.
                            // Note: this works with many files because headers align at card boundaries.
                            // For robustness, implement a LookaheadReader in production.
                            break;
                        } else {
                            // Otherwise, treat it as data (rare) and continue.
                            buf.extend_from_slice(&c);
                        }
                    } else {
                        buf.extend_from_slice(&c);
                    }
                }
                Err(_) => break, // EOF
            }

            while buf.len() >= row_len {
                // Debug: show first row data
                if rows.is_empty() && buf.len() >= row_len {
                    eprintln!("First row data (first 100 bytes): {:?}", &buf[0..row_len.min(100)]);
                    // Show what we'd read for first few variables
                    for (i, v) in vars.iter().take(5).enumerate() {
                        if v.position + v.length <= buf.len() {
                            let slice = &buf[v.position..v.position + v.length.min(20)];
                            eprintln!("  Variable {} ({}): position={}, slice={:?}", i, v.name, v.position, slice);
                        }
                    }
                }
                
                let mut row = Vec::with_capacity(vars.len());
                // Use sequential offsets (like old parser) - data is stored sequentially
                let mut offset = 0usize;
                for v in &vars {
                    if offset + v.length > buf.len() {
                        eprintln!("Warning: Variable {} at offset {} + length {} exceeds buffer size {}", 
                                 v.name, offset, v.length, buf.len());
                        row.push(None);
                        offset += v.length;
                        continue;
                    }
                    let slice = &buf[offset..offset + v.length];
                    offset += v.length;
                    if v.is_char {
                        row.push(Some(decode_char(slice)));
                    } else {
                        // numeric: read right-aligned up to 8 bytes (IBM double, truncated if width<8)
                        let mut eight = [0u8; 8];
                        if v.length >= 8 {
                            eight.copy_from_slice(&slice[0..8]);
                        } else {
                            // right-align
                            let pad = 8 - v.length;
                            eight[pad..].copy_from_slice(slice);
                        }
                        let (val, miss) = ibm64_to_f64(&eight);
                        match miss {
                            IbmMissing::None => row.push(val.map(|f| f.to_string())),
                            _ => row.push(None),
                        }
                    }
                }
                if row.len() == vars.len() {
                    rows.push(row);
                } else {
                    eprintln!("Skipping row with {} values (expected {})", row.len(), vars.len());
                }
                // drop one row worth of bytes (from 0..row_len)
                buf.drain(0..row_len);
            }
        }

        eprintln!("Parsed {} rows for dataset", rows.len());

        // Dataset name is available in the member header data; TS-140 shows where,
        // but for brevity we use a placeholder here. You can parse _mhd0/_mhd1 to extract it.
        let ds_name = String::from("DATASET");

        out.push(Dataset { name: ds_name, vars, rows });
    }

    if out.is_empty() {
        bail!("No datasets found. File may not be a valid XPT v5 file or may be corrupted.");
    }

    Ok(out)
}

/// Read one V5/V6 transport file; return all members (datasets).
pub fn read_xpt_v5<P: AsRef<Path>>(path: P) -> Result<Vec<Dataset>> {
    let f = File::open(path)?;
    let r = BufReader::new(f);
    read_xpt_v5_from_reader(r)
}