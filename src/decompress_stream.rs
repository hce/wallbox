use crate::*;
use std::fs::File;
use std::io::{Read, Result, Write};
pub fn decompress_stream(dsp: DecompressStreamParams) -> Result<()> {
    let f = File::open(dsp.file_name)?;
    let mut s = flate2::read::GzDecoder::new(f);
    let mut buf = vec![0u8; 64];
    let mut stdout = std::io::stdout();
    while s.read(&mut buf)? > 0 {
        stdout.write_all(&buf)?;
    }
    Ok(())
}
