//! Everything already running in the user's tmux: discovery, a live tap on
//! each pane's output, and a few actions. Nothing here starts processes.
//!
//! Taps use `pipe-pane`, which streams a pane's raw output to a shell
//! command. We point it at a file and tail the file; files survive hiccups
//! and never block tmux.

use crate::bus::{now_ms, BusSlot, ChunkMsg};
use crate::capture::{self, CaptureWriter, RunHeader};
use base64::Engine;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PaneInfo {
    /// tmux pane id such as `%3`; stable for the pane's lifetime.
    pub id: String,
    pub session: String,
    pub window_index: u32,
    pub window_name: String,
    pub pane_index: u32,
    pub pid: u32,
    /// `pane_current_command`: the foreground program, e.g. `zsh`, `node`.
    pub command: String,
    pub title: String,
    pub cols: u16,
    pub rows: u16,
    pub cwd: String,
    /// The pane the user is looking at in an attached client.
    pub active: bool,
    pub attached: bool,
    /// tmux reports an open `pipe-pane` on this pane.
    pub piped: bool,
}

/// Field separator for `list-panes -F`. A printable ASCII sequence: tmux
/// rewrites tabs and non-ASCII as `_` for a client without a UTF-8 locale,
/// which is exactly what a Finder-launched app is.
pub const SEP: &str = "<<wg>>";
const FORMAT: &str = "#{pane_id}<<wg>>#{session_name}<<wg>>#{window_index}<<wg>>#{window_name}<<wg>>#{pane_index}<<wg>>#{pane_pid}<<wg>>#{pane_current_command}<<wg>>#{pane_title}<<wg>>#{pane_width}<<wg>>#{pane_height}<<wg>>#{pane_current_path}<<wg>>#{pane_active}<<wg>>#{window_active}<<wg>>#{session_attached}<<wg>>#{pane_pipe}";
const SCROLLBACK_LINES: u32 = 3000;
const TAIL_POLL: Duration = Duration::from_millis(80);
const POLL: Duration = Duration::from_millis(1500);
const RUNS_TO_KEEP: usize = 10;
/// A tap file is emptied once it has been read past this size.
const TAP_ROTATE_BYTES: u64 = 4 * 1024 * 1024;

/// GUI apps get a bare PATH, so look in the usual places too.
pub fn tmux_path() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        if let Some(path) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&path) {
                let p = dir.join("tmux");
                if p.is_file() {
                    return p;
                }
            }
        }
        for p in ["/opt/homebrew/bin/tmux", "/usr/local/bin/tmux", "/opt/local/bin/tmux", "/usr/bin/tmux"] {
            if Path::new(p).is_file() {
                return PathBuf::from(p);
            }
        }
        PathBuf::from("tmux")
    })
}

/// Where diagnostics go; set once at startup. GUI apps have no stderr.
static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn set_log_path(path: PathBuf) {
    let _ = LOG_PATH.set(path);
}

pub fn log(line: &str) {
    eprintln!("watchglass: {line}");
    if let Some(p) = LOG_PATH.get() {
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(p) {
            use std::io::Write;
            let _ = writeln!(f, "{} {line}", now_ms());
        }
    }
}

/// The tmux socket to talk to. A Finder-launched app has no `TMUX`
/// variable, so the default socket is tried first, then any other live
/// socket in tmux's directory.
static SOCKET: OnceLock<Option<PathBuf>> = OnceLock::new();

fn socket_dir() -> PathBuf {
    let base = std::env::var_os("TMUX_TMPDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
    #[cfg(unix)]
    let uid = unsafe { libc::getuid() };
    #[cfg(not(unix))]
    let uid = 0;
    base.join(format!("tmux-{uid}"))
}

fn raw(args: &[&str], socket: Option<&Path>) -> Result<Vec<u8>, String> {
    let mut cmd = Command::new(tmux_path());
    // Without a UTF-8 locale tmux sanitises its output, turning every
    // non-ASCII character (and tabs) into `_`, in formats and captures alike.
    cmd.env("LC_ALL", "en_US.UTF-8");
    if let Some(s) = socket {
        cmd.arg("-S").arg(s);
    }
    let out = cmd.args(args).output().map_err(|e| format!("tmux ({}): {e}", tmux_path().display()))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() { format!("tmux {} failed", args.join(" ")) } else { err });
    }
    Ok(out.stdout)
}

fn resolve_socket() -> Option<PathBuf> {
    // Inside tmux, the client already knows its socket from `TMUX`.
    if std::env::var_os("TMUX").is_some() && raw(&["list-sessions"], None).is_ok() {
        return None;
    }
    let dir = socket_dir();
    let default = dir.join("default");
    if raw(&["list-sessions"], Some(&default)).is_ok() {
        return Some(default);
    }
    let mut others: Vec<PathBuf> = fs::read_dir(&dir)
        .map(|it| it.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p != &default).collect())
        .unwrap_or_default();
    others.sort();
    others.into_iter().find(|s| raw(&["list-sessions"], Some(s)).is_ok())
}

fn socket() -> Option<&'static Path> {
    SOCKET
        .get_or_init(|| {
            let s = resolve_socket();
            log(&format!(
                "tmux at {} socket {}",
                tmux_path().display(),
                s.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "(from TMUX env)".into())
            ));
            s
        })
        .as_deref()
}

fn tmux(args: &[&str]) -> Result<Vec<u8>, String> {
    raw(args, socket())
}

pub fn available() -> bool {
    tmux(&["list-sessions"]).is_ok()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TmuxStatus {
    pub available: bool,
    pub path: String,
    pub socket: String,
    pub error: Option<String>,
}

pub fn status() -> TmuxStatus {
    let error = tmux(&["list-sessions"]).err();
    TmuxStatus {
        available: error.is_none(),
        path: tmux_path().display().to_string(),
        socket: socket().map(|p| p.display().to_string()).unwrap_or_else(|| "TMUX env".into()),
        error,
    }
}

pub fn list_panes() -> Result<Vec<PaneInfo>, String> {
    let out = tmux(&["list-panes", "-a", "-F", FORMAT]).map_err(|e| {
        log(&format!("list-panes failed: {e}"));
        e
    })?;
    let text = String::from_utf8_lossy(&out);
    let panes: Vec<PaneInfo> = text.lines().filter_map(parse_line).collect();
    if panes.is_empty() && !text.trim().is_empty() {
        log(&format!("list-panes returned {} lines but none parsed; first: {:?}", text.lines().count(), text.lines().next().unwrap_or("")));
    }
    Ok(panes)
}

pub fn parse_line(line: &str) -> Option<PaneInfo> {
    let f: Vec<&str> = line.splitn(15, SEP).collect();
    if f.len() < 14 {
        return None;
    }
    Some(PaneInfo {
        id: f[0].to_string(),
        session: f[1].to_string(),
        window_index: f[2].parse().ok()?,
        window_name: f[3].to_string(),
        pane_index: f[4].parse().ok()?,
        pid: f[5].parse().unwrap_or(0),
        command: f[6].to_string(),
        title: f[7].to_string(),
        cols: f[8].parse().unwrap_or(80),
        rows: f[9].parse().unwrap_or(24),
        cwd: f[10].to_string(),
        active: f[11] == "1" && f[12] == "1",
        attached: f[13] != "0",
        piped: f.get(14).is_some_and(|p| *p == "1"),
    })
}

/// Current scrollback with colours, CRLF-terminated, trailing blanks trimmed.
pub fn capture(id: &str) -> Result<Vec<u8>, String> {
    let start = format!("-{SCROLLBACK_LINES}");
    let out = tmux(&["capture-pane", "-p", "-e", "-J", "-S", &start, "-t", id])?;
    let text = String::from_utf8_lossy(&out);
    let trimmed = text.trim_end_matches('\n');
    let mut bytes = Vec::with_capacity(trimmed.len() + 64);
    for line in trimmed.split('\n') {
        bytes.extend_from_slice(line.as_bytes());
        bytes.extend_from_slice(b"\r\n");
    }
    Ok(bytes)
}

pub fn focus(pane: &PaneInfo) -> Result<(), String> {
    let target = format!("{}:{}", pane.session, pane.window_index);
    tmux(&["select-window", "-t", &target])?;
    tmux(&["select-pane", "-t", &pane.id])?;
    // Move an attached client there; ignore failure when none is attached.
    let _ = tmux(&["switch-client", "-t", &pane.session]);
    // Bring the terminal to the front when it is Ghostty.
    let _ = Command::new("open").args(["-a", "Ghostty"]).status();
    Ok(())
}

pub fn send_keys(id: &str, keys: &[String]) -> Result<(), String> {
    let mut args = vec!["send-keys", "-t", id];
    for k in keys {
        args.push(k.as_str());
    }
    tmux(&args).map(|_| ())
}

/// Arguments for typing `text` literally, then Enter. Two calls, because
/// `-l` makes every following word literal, including "Enter".
pub fn send_line_args(id: &str, text: &str) -> [Vec<String>; 2] {
    [
        vec!["send-keys".into(), "-t".into(), id.into(), "-l".into(), text.into()],
        vec!["send-keys".into(), "-t".into(), id.into(), "Enter".into()],
    ]
}

pub fn send_line(id: &str, text: &str) -> Result<(), String> {
    for args in send_line_args(id, text) {
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        tmux(&refs)?;
    }
    Ok(())
}

/// Arguments for a new window in `session` running `command` through the
/// user's shell, printing the new pane id.
pub fn new_window_args(session: &str, name: &str, command: &str) -> Vec<String> {
    let mut args: Vec<String> = vec!["new-window".into(), "-P".into(), "-F".into(), "#{pane_id}".into()];
    if !session.is_empty() {
        args.push("-t".into());
        args.push(format!("{session}:"));
    }
    if !name.is_empty() {
        args.push("-n".into());
        args.push(name.into());
    }
    if !command.trim().is_empty() {
        // Keep the window open after the command ends so its output stays readable.
        args.push(format!("{}; exec $SHELL", command.trim()));
    }
    args
}

/// Opens a new tmux window and returns its pane id.
pub fn new_window(session: &str, name: &str, command: &str) -> Result<String, String> {
    let args = new_window_args(session, name, command);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = tmux(&refs)?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

fn pipe_open(id: &str, file: &Path) -> Result<(), String> {
    let cmd = format!("cat >> '{}'", file.display());
    tmux(&["pipe-pane", "-O", "-t", id, &cmd]).map(|_| ())
}

fn pipe_close(id: &str) {
    let _ = tmux(&["pipe-pane", "-t", id]);
}

fn safe_id(id: &str) -> String {
    id.trim_start_matches('%').to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachInfo {
    pub id: String,
    pub cols: u16,
    pub rows: u16,
    /// The pane's scrollback as of this call, base64. Returned directly so a
    /// freshly mounted tile always gets its screen, attached before or not.
    pub snapshot: String,
}

/// Plain-text tail of a pane for classification. Bounded ring of bytes.
#[derive(Default)]
pub struct Tail {
    text: String,
}

const TAIL_CAP: usize = 8 * 1024;

impl Tail {
    pub fn push(&mut self, raw: &[u8]) {
        self.text.push_str(&strip_ansi(raw));
        if self.text.len() > TAIL_CAP {
            let cut = self.text.len() - TAIL_CAP;
            let at = self.text.char_indices().map(|(i, _)| i).find(|&i| i >= cut).unwrap_or(cut);
            self.text.drain(..at);
        }
    }

    /// The last `n` non-empty lines.
    pub fn last_lines(&self, n: usize) -> String {
        let lines: Vec<&str> = self.text.lines().filter(|l| !l.trim().is_empty()).collect();
        let start = lines.len().saturating_sub(n);
        lines[start..].join("\n")
    }
}

/// Removes escape sequences and resolves carriage returns per line.
pub fn strip_ansi(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.next() {
                Some('[') => {
                    // CSI: parameters until a final byte 0x40..0x7e
                    for d in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&d) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    // OSC: until BEL or ESC \
                    let mut prev = '\0';
                    for d in chars.by_ref() {
                        if d == '\x07' || (prev == '\x1b' && d == '\\') {
                            break;
                        }
                        prev = d;
                    }
                }
                Some('P') | Some('X') | Some('^') | Some('_') => {
                    let mut prev = '\0';
                    for d in chars.by_ref() {
                        if prev == '\x1b' && d == '\\' {
                            break;
                        }
                        prev = d;
                    }
                }
                Some(_) | None => {}
            }
            continue;
        }
        if c == '\r' {
            // A CR without LF rewrites the line: drop what was written so far.
            if chars.peek() != Some(&'\n') {
                if let Some(pos) = out.rfind('\n') {
                    out.truncate(pos + 1);
                } else {
                    out.clear();
                }
            }
            continue;
        }
        if c == '\x07' || (c.is_control() && c != '\n' && c != '\t') {
            continue;
        }
        out.push(c);
    }
    out
}

struct Tap {
    stop: Arc<AtomicBool>,
    tail: Arc<Mutex<Tail>>,
    file: PathBuf,
}

/// Owns one tap per attached pane and the discovery poller.
pub struct TapManager {
    bus: Arc<BusSlot>,
    taps: Mutex<HashMap<String, Tap>>,
    data_dir: PathBuf,
}

impl TapManager {
    pub fn new(bus: Arc<BusSlot>, data_dir: PathBuf) -> Self {
        Self {
            bus,
            taps: Mutex::new(HashMap::new()),
            data_dir,
        }
    }

    pub fn tail_text(&self, id: &str, lines: usize) -> Option<String> {
        let taps = self.taps.lock().expect("taps lock");
        taps.get(id).map(|t| t.tail.lock().expect("tail lock").last_lines(lines))
    }

    pub fn is_attached(&self, id: &str) -> bool {
        self.taps.lock().expect("taps lock").contains_key(id)
    }

    pub fn attach(&self, pane: &PaneInfo) -> Result<AttachInfo, String> {
        let encode = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
        if self.is_attached(&pane.id) {
            // The live tap keeps running; the caller still needs the screen.
            let snapshot = capture(&pane.id)?;
            eprintln!("watchglass: re-attach {} snapshot {} bytes", pane.id, snapshot.len());
            return Ok(AttachInfo {
                id: pane.id.clone(),
                cols: pane.cols,
                rows: pane.rows,
                snapshot: encode(&snapshot),
            });
        }
        let taps_dir = self.data_dir.join("taps");
        fs::create_dir_all(&taps_dir).map_err(|e| format!("taps dir: {e}"))?;
        let file = taps_dir.join(format!("{}.raw", safe_id(&pane.id)));
        let _ = fs::remove_file(&file);
        File::create(&file).map_err(|e| format!("tap file: {e}"))?;

        // Scrollback first, then the live pipe: a few milliseconds may be missed,
        // which beats showing them twice.
        let snapshot = capture(&pane.id)?;
        pipe_open(&pane.id, &file)?;
        eprintln!("watchglass: attach {} snapshot {} bytes", pane.id, snapshot.len());

        let started_ms = now_ms();
        let cap_dir = self.data_dir.join("captures").join("panes").join(safe_id(&pane.id));
        capture::enforce_retention(&cap_dir, RUNS_TO_KEEP.saturating_sub(1));
        let header = RunHeader {
            source: format!("{}:{}.{} {}", pane.session, pane.window_index, pane.pane_index, pane.command),
            cmd: vec![pane.command.clone()],
            cwd: pane.cwd.clone(),
            started_ms,
            cols: pane.cols,
            rows: pane.rows,
        };
        let mut writer = CaptureWriter::create(&cap_dir.join(capture::run_file_name(started_ms)), &header).ok();

        let tail = Arc::new(Mutex::new(Tail::default()));
        tail.lock().expect("tail lock").push(&snapshot);
        let mut seq = 1u64;
        if let Some(w) = writer.as_mut() {
            let _ = w.append(started_ms, &snapshot);
        }

        let stop = Arc::new(AtomicBool::new(false));
        self.taps.lock().expect("taps lock").insert(
            pane.id.clone(),
            Tap { stop: Arc::clone(&stop), tail: Arc::clone(&tail), file: file.clone() },
        );

        let bus = Arc::clone(&self.bus);
        let id = pane.id.clone();
        let hook_fn: Option<Arc<dyn Fn(&str) + Send + Sync>> = HOOK.get().cloned();
        thread::Builder::new()
            .name(format!("wg-tap-{id}"))
            .spawn(move || {
                let Ok(mut f) = OpenOptions::new().read(true).write(true).open(&file) else {
                    return;
                };
                let mut pos: u64 = 0;
                let mut buf = vec![0u8; 64 * 1024];
                while !stop.load(Ordering::Relaxed) {
                    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
                    if len < pos {
                        pos = 0; // file was truncated
                    }
                    // Everything read and the file is large: empty it. tmux's
                    // `cat >>` appends, so its next write lands at the new end.
                    if len == pos && len > TAP_ROTATE_BYTES && f.set_len(0).is_ok() {
                        pos = 0;
                        continue;
                    }
                    if len > pos {
                        if f.seek(SeekFrom::Start(pos)).is_err() {
                            break;
                        }
                        match f.read(&mut buf) {
                            Ok(0) => {}
                            Ok(n) => {
                                pos += n as u64;
                                seq += 1;
                                let t = now_ms();
                                if let Some(w) = writer.as_mut() {
                                    let _ = w.append(t, &buf[..n]);
                                }
                                tail.lock().expect("tail lock").push(&buf[..n]);
                                bus.chunk(ChunkMsg {
                                    id: id.clone(),
                                    seq,
                                    t,
                                    data: base64::engine::general_purpose::STANDARD.encode(&buf[..n]),
                                });
                                if let Some(h) = hook_fn.as_ref() {
                                    h(&id);
                                }
                                continue;
                            }
                            Err(_) => break,
                        }
                    }
                    thread::sleep(TAIL_POLL);
                }
            })
            .map_err(|e| format!("thread: {e}"))?;
        Ok(AttachInfo {
            id: pane.id.clone(),
            cols: pane.cols,
            rows: pane.rows,
            snapshot: encode(&snapshot),
        })
    }

    pub fn detach(&self, id: &str) -> bool {
        let Some(tap) = self.taps.lock().expect("taps lock").remove(id) else {
            return false;
        };
        tap.stop.store(true, Ordering::Relaxed);
        pipe_close(id);
        let _ = fs::remove_file(&tap.file);
        true
    }

    pub fn detach_all(&self) {
        let ids: Vec<String> = self.taps.lock().expect("taps lock").keys().cloned().collect();
        for id in ids {
            self.detach(&id);
        }
    }

    /// Closes pipes a previous run left behind: panes tmux reports as piped
    /// for which one of our tap files exists. Other people's pipes are kept.
    /// Returns the pane ids that were cleaned.
    pub fn cleanup_stale(&self, panes: &[PaneInfo]) -> Vec<String> {
        let taps_dir = self.data_dir.join("taps");
        let mut cleaned = Vec::new();
        for pane in panes.iter().filter(|p| p.piped) {
            let file = taps_dir.join(format!("{}.raw", safe_id(&pane.id)));
            if file.is_file() && !self.is_attached(&pane.id) {
                pipe_close(&pane.id);
                let _ = fs::remove_file(&file);
                cleaned.push(pane.id.clone());
            }
        }
        // Tap files for panes that no longer exist are just litter.
        if let Ok(entries) = fs::read_dir(&taps_dir) {
            let alive: std::collections::HashSet<String> = panes.iter().map(|p| safe_id(&p.id)).collect();
            for e in entries.filter_map(|e| e.ok()) {
                let name = e.file_name().to_string_lossy().to_string();
                if let Some(stem) = name.strip_suffix(".raw") {
                    if !alive.contains(stem) {
                        let _ = fs::remove_file(e.path());
                    }
                }
            }
        }
        cleaned
    }

    /// Emits the pane list whenever it changes, keeps `cache` current, and
    /// detaches vanished panes.
    pub fn start_poller(self: &Arc<Self>, cache: Arc<Mutex<Vec<PaneInfo>>>) {
        let me = Arc::clone(self);
        thread::Builder::new()
            .name("wg-panes".into())
            .spawn(move || {
                let mut last: Option<Vec<PaneInfo>> = None;
                loop {
                    let panes = list_panes().unwrap_or_default();
                    if last.as_ref() != Some(&panes) {
                        let before: Vec<&str> = last.as_deref().unwrap_or(&[]).iter().map(|p| p.id.as_str()).collect();
                        let now: Vec<&str> = panes.iter().map(|p| p.id.as_str()).collect();
                        if before != now {
                            log(&format!(
                                "panes: {}",
                                panes.iter().map(|p| format!("{}={}:{}/{}", p.id, p.session, p.window_name, p.command)).collect::<Vec<_>>().join(" ")
                            ));
                        }
                        *cache.lock().expect("panes cache") = panes.clone();
                        let alive: std::collections::HashSet<&str> = panes.iter().map(|p| p.id.as_str()).collect();
                        let attached: Vec<String> = me.taps.lock().expect("taps lock").keys().cloned().collect();
                        for id in attached {
                            if !alive.contains(id.as_str()) {
                                me.detach(&id);
                            }
                        }
                        me.bus.panes(panes.clone());
                        last = Some(panes);
                    }
                    thread::sleep(POLL);
                }
            })
            .expect("panes poller");
    }
}

/// Process-wide output hook. Set once at startup by the classifier.
static HOOK: OnceLock<Arc<dyn Fn(&str) + Send + Sync>> = OnceLock::new();

pub fn set_output_hook(hook: Arc<dyn Fn(&str) + Send + Sync>) {
    let _ = HOOK.set(hook);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn join(fields: &[&str]) -> String {
        fields.join(SEP)
    }

    #[test]
    fn parses_list_panes_lines() {
        let line = join(&["%3", "main", "1", "nvim", "0", "17559", "claude-hl", "✳ Jev tooling", "230", "60", "/Users/me/proj", "1", "1", "1", "1"]);
        let p = parse_line(&line).unwrap();
        assert_eq!(p.id, "%3");
        assert_eq!(p.session, "main");
        assert_eq!(p.window_index, 1);
        assert_eq!(p.window_name, "nvim");
        assert_eq!(p.pid, 17559);
        assert_eq!(p.command, "claude-hl");
        assert_eq!(p.title, "✳ Jev tooling");
        assert_eq!((p.cols, p.rows), (230, 60));
        assert!(p.active && p.attached && p.piped);
        let unpiped = parse_line(&join(&["%4", "main", "2", "sh", "0", "1", "zsh", "", "80", "24", "/", "0", "0", "1", "0"])).unwrap();
        assert!(!unpiped.piped);
        assert!(parse_line("garbage").is_none());
        // What a client without a UTF-8 locale used to produce: tabs as underscores.
        assert!(parse_line("%8_effective-tutorial_1_claude-hl_0_28706_claude-hl__ Multiple_115_55_/x_1_1_0_1").is_none());
        assert_eq!(FORMAT.matches(SEP).count(), 14, "fifteen fields");
    }

    #[test]
    fn cleanup_closes_only_our_stale_taps() {
        let dir = std::env::temp_dir().join(format!("wg-taps-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("taps")).unwrap();
        fs::write(dir.join("taps").join("7.raw"), b"old").unwrap();
        fs::write(dir.join("taps").join("99.raw"), b"litter").unwrap();
        let bus = Arc::new(BusSlot::default());
        let m = TapManager::new(bus, dir.clone());
        let mut ours = parse_line(&join(&["%7", "main", "1", "w", "0", "1", "node", "", "80", "24", "/", "0", "0", "1", "1"])).unwrap();
        let theirs = parse_line(&join(&["%8", "main", "2", "w", "0", "1", "node", "", "80", "24", "/", "0", "0", "1", "1"])).unwrap();
        // `pipe_close` shells out to tmux; with no server it fails harmlessly.
        let cleaned = m.cleanup_stale(&[ours.clone(), theirs]);
        assert_eq!(cleaned, vec!["%7"]);
        assert!(!dir.join("taps").join("7.raw").exists(), "our stale tap file is removed");
        assert!(!dir.join("taps").join("99.raw").exists(), "litter for a dead pane is removed");
        ours.piped = false;
        assert!(m.cleanup_stale(&[ours]).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn send_line_types_literally_then_enter() {
        let [first, second] = send_line_args("%3", "keep the tests; -l is not a flag here");
        assert_eq!(first, vec!["send-keys", "-t", "%3", "-l", "keep the tests; -l is not a flag here"]);
        assert_eq!(second, vec!["send-keys", "-t", "%3", "Enter"]);
    }

    #[test]
    fn new_window_args_target_session_and_keep_shell() {
        let a = new_window_args("main", "tests", "pnpm vitest");
        assert_eq!(a, vec!["new-window", "-P", "-F", "#{pane_id}", "-t", "main:", "-n", "tests", "pnpm vitest; exec $SHELL"]);
        let bare = new_window_args("", "", "  ");
        assert_eq!(bare, vec!["new-window", "-P", "-F", "#{pane_id}"]);
    }

    #[test]
    fn strips_escapes_and_resolves_carriage_returns() {
        let raw = b"\x1b[32m\xe2\x9c\x93\x1b[0m tick 1\r\n\x1b]0;title\x07spinner 10%\rspinner 20%\rspinner 30%\r\ndone\n";
        assert_eq!(strip_ansi(raw), "✓ tick 1\nspinner 30%\ndone\n");
    }

    #[test]
    fn tail_keeps_last_lines_and_stays_bounded() {
        let mut t = Tail::default();
        for i in 0..2000 {
            t.push(format!("line {i}\r\n").as_bytes());
        }
        assert!(t.text.len() <= TAIL_CAP);
        let last = t.last_lines(3);
        assert_eq!(last, "line 1997\nline 1998\nline 1999");
    }
}
