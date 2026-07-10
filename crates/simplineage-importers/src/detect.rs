//! Shared format-detection helpers.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Lowercase extension without the leading dot, if any.
pub fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
}

/// Whether extension is one of the listed values.
pub fn has_extension(path: &Path, exts: &[&str]) -> bool {
    extension(path)
        .map(|e| exts.iter().any(|x| *x == e))
        .unwrap_or(false)
}

/// Read up to `max` bytes from the start of a file.
pub fn read_prefix(path: &Path, max: usize) -> std::io::Result<Vec<u8>> {
    let mut f = File::open(path)?;
    let mut buf = vec![0u8; max];
    let n = f.read(&mut buf)?;
    buf.truncate(n);
    Ok(buf)
}

/// True if file starts with the Parquet magic `PAR1`.
pub fn looks_like_parquet(path: &Path) -> bool {
    read_prefix(path, 4)
        .map(|b| b.as_slice() == b"PAR1")
        .unwrap_or(false)
}

/// True if the path is an existing regular file.
pub fn is_file(path: &Path) -> bool {
    path.is_file()
}

/// Peek first non-empty line of a text file (lossy UTF-8).
pub fn first_text_line(path: &Path) -> Option<String> {
    let bytes = read_prefix(path, 8 * 1024).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_string)
}

/// ZIP/OLE signatures used by xlsx / xls.
pub fn looks_like_excel(path: &Path) -> bool {
    let Ok(mut f) = File::open(path) else {
        return false;
    };
    let mut magic = [0u8; 8];
    if f.read(&mut magic).ok().unwrap_or(0) < 4 {
        return false;
    }
    // ZIP (xlsx, xlsm)
    if magic[0..2] == [0x50, 0x4B] {
        return true;
    }
    // OLE compound (legacy xls)
    if magic[0..4] == [0xD0, 0xCF, 0x11, 0xE0] {
        return true;
    }
    let _ = f.seek(SeekFrom::Start(0));
    false
}

/// True if content looks like JSON object/array.
pub fn looks_like_json(path: &Path) -> bool {
    let Ok(bytes) = read_prefix(path, 512) else {
        return false;
    };
    let trimmed: Vec<u8> = bytes
        .into_iter()
        .skip_while(|b| b.is_ascii_whitespace())
        .collect();
    matches!(trimmed.first(), Some(b'{') | Some(b'['))
}
