//! `wgcap` v1: append-only capture of one source run.
//!
//! ```text
//! magic  "WGCAP1\n"
//! header one JSON line (RunHeader)
//! frames [u32 LE len][u64 LE t_ms][len bytes] ...
//! ```
//!
//! Frames are written with one `write` call each so a crash leaves at most
//! one truncated frame, which the reader drops.

use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

pub const MAGIC: &[u8] = b"WGCAP1\n";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RunHeader {
    pub source: String,
    pub cmd: Vec<String>,
    pub cwd: String,
    pub started_ms: u64,
    pub cols: u16,
    pub rows: u16,
}

pub struct CaptureWriter {
    file: File,
    frame: Vec<u8>,
}

impl CaptureWriter {
    pub fn create(path: &Path, header: &RunHeader) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = opts.open(path)?;
        let mut head = Vec::with_capacity(256);
        head.extend_from_slice(MAGIC);
        serde_json::to_writer(&mut head, header)?;
        head.push(b'\n');
        file.write_all(&head)?;
        Ok(Self {
            file,
            frame: Vec::with_capacity(64 * 1024),
        })
    }

    pub fn append(&mut self, t_ms: u64, data: &[u8]) -> io::Result<()> {
        self.frame.clear();
        self.frame
            .extend_from_slice(&(data.len() as u32).to_le_bytes());
        self.frame.extend_from_slice(&t_ms.to_le_bytes());
        self.frame.extend_from_slice(data);
        self.file.write_all(&self.frame)
    }
}

/// Reads a capture, calling `on_frame(t_ms, bytes)` per frame. A truncated
/// trailing frame ends the stream silently. Replay in the UI is still open.
#[allow(dead_code)]
pub fn read_capture(
    path: &Path,
    mut on_frame: impl FnMut(u64, &[u8]),
) -> io::Result<RunHeader> {
    let mut r = BufReader::new(File::open(path)?);
    let mut magic = [0u8; 7];
    r.read_exact(&mut magic)?;
    if magic != MAGIC {
        return Err(io::Error::new(ErrorKind::InvalidData, "not a wgcap file"));
    }
    let mut line = Vec::new();
    r.read_until(b'\n', &mut line)?;
    let header: RunHeader = serde_json::from_slice(&line)
        .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;

    let mut data = Vec::new();
    loop {
        let mut len = [0u8; 4];
        if !read_full(&mut r, &mut len)? {
            break;
        }
        let mut t = [0u8; 8];
        if !read_full(&mut r, &mut t)? {
            break;
        }
        let len = u32::from_le_bytes(len) as usize;
        data.resize(len, 0);
        if !read_full(&mut r, &mut data)? {
            break;
        }
        on_frame(u64::from_le_bytes(t), &data);
    }
    Ok(header)
}

/// Like `read_exact`, but returns `Ok(false)` on a clean or truncated EOF.
fn read_full(r: &mut impl Read, buf: &mut [u8]) -> io::Result<bool> {
    let mut filled = 0;
    while filled < buf.len() {
        match r.read(&mut buf[filled..]) {
            Ok(0) => return Ok(false),
            Ok(n) => filled += n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(true)
}

pub fn run_file_name(started_ms: u64) -> String {
    format!("{started_ms:013}.wgcap")
}

/// Runs in `dir`, oldest first, by file name (zero-padded start time).
pub fn list_run_files(dir: &Path) -> Vec<(u64, PathBuf)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut runs: Vec<(u64, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "wgcap"))
        .filter_map(|p| {
            let stem = p.file_stem()?.to_str()?;
            stem.parse::<u64>().ok().map(|ms| (ms, p.clone()))
        })
        .collect();
    runs.sort();
    runs
}

/// Deletes the oldest runs so that at most `keep` remain.
pub fn enforce_retention(dir: &Path, keep: usize) -> usize {
    let runs = list_run_files(dir);
    if runs.len() <= keep {
        return 0;
    }
    let excess = runs.len() - keep;
    let mut removed = 0;
    for (_, path) in runs.into_iter().take(excess) {
        if fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> RunHeader {
        RunHeader {
            source: "dev".into(),
            cmd: vec!["pnpm".into(), "dev".into()],
            cwd: "/tmp/proj".into(),
            started_ms: 1_700_000_000_000,
            cols: 120,
            rows: 40,
        }
    }

    #[test]
    fn round_trips_header_and_frames() {
        let dir = std::env::temp_dir().join(format!("wgcap-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("a.wgcap");
        let mut w = CaptureWriter::create(&path, &header()).unwrap();
        w.append(1, b"hello ").unwrap();
        w.append(2, b"\x1b[31mred\x1b[0m\n").unwrap();
        w.append(3, b"").unwrap();
        drop(w);

        let mut frames = Vec::new();
        let h = read_capture(&path, |t, d| frames.push((t, d.to_vec()))).unwrap();
        assert_eq!(h, header());
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0], (1, b"hello ".to_vec()));
        assert_eq!(frames[1].1, b"\x1b[31mred\x1b[0m\n".to_vec());
        assert_eq!(frames[2], (3, Vec::new()));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn drops_truncated_trailing_frame() {
        let dir = std::env::temp_dir().join(format!("wgcap-trunc-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("b.wgcap");
        let mut w = CaptureWriter::create(&path, &header()).unwrap();
        w.append(10, b"complete\n").unwrap();
        drop(w);
        // Append a frame header that promises more bytes than exist.
        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(&(100u32).to_le_bytes()).unwrap();
        f.write_all(&(11u64).to_le_bytes()).unwrap();
        f.write_all(b"short").unwrap();
        drop(f);

        let mut frames = Vec::new();
        read_capture(&path, |t, d| frames.push((t, d.to_vec()))).unwrap();
        assert_eq!(frames, vec![(10, b"complete\n".to_vec())]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_foreign_files() {
        let dir = std::env::temp_dir().join(format!("wgcap-bad-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("c.wgcap");
        fs::write(&path, b"not a capture at all\n").unwrap();
        let err = read_capture(&path, |_, _| {}).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn retention_keeps_newest() {
        let dir = std::env::temp_dir().join(format!("wgcap-ret-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for ms in [5u64, 1, 3, 2, 4] {
            CaptureWriter::create(&dir.join(run_file_name(ms)), &header()).unwrap();
        }
        assert_eq!(enforce_retention(&dir, 2), 3);
        let left: Vec<u64> = list_run_files(&dir).into_iter().map(|(ms, _)| ms).collect();
        assert_eq!(left, vec![4, 5]);
        let _ = fs::remove_dir_all(&dir);
    }
}
