extern crate rusqlite;

use crate::pac2200::Pac2200Params;
use crate::*;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Result, Write};

pub fn decompress_stream(dsp: DecompressStreamParams) -> Result<()> {
    if let Some(output_to_sqlite) = dsp.output_to_sqlite.as_ref() {
        let db = rusqlite::Connection::open(output_to_sqlite).expect("sqlite open");
        db.execute("CREATE TABLE IF NOT EXISTS strom (id INTEGER PRIMARY KEY, timestamp INTEGER NOT NULL, frequency REAL NOT NULL, Ul1 REAL NOT NULL, Ul2 REAL NOT NULL, Ul3 REAL NOT NULL)", []).expect("create table");
        let mut entry = db.prepare("INSERT INTO strom (timestamp, frequency, Ul1, Ul2, Ul3) VALUES ($1, $2, $3, $4, $5)").expect("sqlite prepare");
        for file in &dsp.file_name {
            let f = File::open(file)?;
            let s = flate2::read::GzDecoder::new(f);
            let mut buff = BufReader::new(s);
            let mut buf_string = String::new();
            loop {
                buf_string.truncate(0);
                match buff.read_line(&mut buf_string) {
                    Err(_) => break,
                    Ok(0) => break,
                    _otherwise => (),
                }
                match serde_json::from_str::<Pac2200Params>(&buf_string) {
                    Err(e) => eprintln!("Serde error: {}", e.to_string()),
                    Ok(json) => {
                        entry
                            .execute((json.update, json.frequency, json.u_l1, json.u_l2, json.u_l3))
                            .expect("sqlite");
                    }
                }
            }
        }
    } else {
        for file in &dsp.file_name {
            let f = File::open(file)?;
            let mut s = flate2::read::GzDecoder::new(f);
            let mut buf = vec![0u8; 64];
            let mut stdout = std::io::stdout();
            while s.read(&mut buf)? > 0 {
                stdout.write_all(&buf)?;
            }
        }
    }
    Ok(())
}
