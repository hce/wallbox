use std::io::{Read, Seek, SeekFrom, Write};

pub struct DevNullFile;

impl DevNullFile {
    pub fn new() -> DevNullFile {
        DevNullFile
    }
}

impl Write for DevNullFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Read for DevNullFile {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        Ok(0)
    }
}

impl Seek for DevNullFile {
    fn seek(&mut self, _pos: SeekFrom) -> std::io::Result<u64> {
        Ok(0)
    }
}
