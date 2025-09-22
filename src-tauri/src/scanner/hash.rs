use anyhow::{Context, Result};
use sha1::{Digest, Sha1};
use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

const BUFFER_SIZE: usize = 8192;

pub fn hash_first_n(path: &Path, n: usize) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut buffer = vec![0u8; n];
    let read = file
        .read(&mut buffer)
        .with_context(|| format!("reading {}", path.display()))?;
    let mut hasher = Sha1::new();
    hasher.update(&buffer[..read]);
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn hash_full(path: &Path) -> Result<String> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; BUFFER_SIZE];
    loop {
        let read = reader
            .read(&mut buffer)
            .with_context(|| format!("reading {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
