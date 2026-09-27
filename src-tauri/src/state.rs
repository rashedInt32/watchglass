//! The latest picture of everything, and the verdicts file other tools
//! read: `~/.local/state/watchglass/verdicts.json`.

use crate::bus::now_ms;
use crate::claude::ClaudeSession;
use crate::jev::{Level, VerdictMsg};
use crate::tmux::PaneInfo;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub session_id: String,
    pub name: String,
    pub cwd: String,
    pub status: String,
    pub tmux: Option<String>,
    pub pane_id: Option<String>,
    pub level: Level,
    pub confidence: f64,
    pub source: String,
    pub snippet: String,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaneRow {
    pub id: String,
    pub session: String,
    pub window_index: u32,
    pub window_name: String,
    pub pane_index: u32,
    pub command: String,
    pub title: String,
    pub cwd: String,
    pub level: Level,
    pub confidence: f64,
    pub source: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub updated_at: u64,
    /// Loudest level across sessions and panes.
    pub top: Level,
    /// Items per level; a Claude session and its own pane count once.
    pub counts: HashMap<String, usize>,
    pub sessions: Vec<SessionRow>,
    pub panes: Vec<PaneRow>,
}

#[derive(Default)]
struct Inner {
    panes: Vec<PaneInfo>,
    sessions: Vec<ClaudeSession>,
    verdicts: HashMap<String, VerdictMsg>,
    tails: HashMap<String, String>,
    last_written: Option<Summary>,
}

/// Holds the latest state and writes the summary file when it changes.
pub struct Store {
    inner: Mutex<Inner>,
    path: PathBuf,
}

pub fn default_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state")))?;
    Some(base.join("watchglass").join("verdicts.json"))
}

impl Store {
    pub fn new(path: PathBuf) -> Self {
        Self { inner: Mutex::new(Inner::default()), path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn set_panes(&self, panes: Vec<PaneInfo>) -> Option<Summary> {
        let mut g = self.inner.lock().expect("store");
        g.panes = panes;
        let alive: HashSet<String> = g.panes.iter().map(|p| p.id.clone()).collect();
        g.verdicts.retain(|id, v| v.kind != "pane" || alive.contains(id));
        g.tails.retain(|id, _| alive.contains(id));
        self.flush(&mut g)
    }

    pub fn set_sessions(&self, sessions: Vec<ClaudeSession>) -> Option<Summary> {
        let mut g = self.inner.lock().expect("store");
        g.sessions = sessions;
        let alive: HashSet<String> = g.sessions.iter().map(|s| s.session_id.clone()).collect();
        g.verdicts.retain(|id, v| v.kind != "claude" || alive.contains(id));
        self.flush(&mut g)
    }

    pub fn set_verdict(&self, v: VerdictMsg) -> Option<Summary> {
        let mut g = self.inner.lock().expect("store");
        g.verdicts.insert(v.id.clone(), v);
        self.flush(&mut g)
    }

    /// Last meaningful line of a pane, for the snippet.
    pub fn set_tail(&self, id: &str, last_line: String) {
        let mut g = self.inner.lock().expect("store");
        g.tails.insert(id.to_string(), last_line);
    }

    pub fn previous_level(&self, id: &str) -> Option<Level> {
        self.inner.lock().expect("store").verdicts.get(id).map(|v| v.level)
    }

    pub fn sessions(&self) -> Vec<ClaudeSession> {
        self.inner.lock().expect("store").sessions.clone()
    }

    pub fn verdicts(&self) -> Vec<VerdictMsg> {
        self.inner.lock().expect("store").verdicts.values().cloned().collect()
    }

    pub fn summary(&self) -> Summary {
        build(&self.inner.lock().expect("store"))
    }

    /// Writes when the summary changed. Returns the new summary in that case.
    fn flush(&self, g: &mut Inner) -> Option<Summary> {
        let s = build(g);
        if g.last_written.as_ref().is_some_and(|last| same_apart_from_time(last, &s)) {
            return None;
        }
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let tmp = self.path.with_extension("json.tmp");
        if let Ok(text) = serde_json::to_string_pretty(&s) {
            if fs::write(&tmp, text).is_ok() {
                let _ = fs::rename(&tmp, &self.path);
            }
        }
        g.last_written = Some(s.clone());
        Some(s)
    }
}

fn same_apart_from_time(a: &Summary, b: &Summary) -> bool {
    a.top == b.top && a.counts == b.counts && a.sessions == b.sessions && a.panes == b.panes
}

fn truncate(s: &str, cap: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= cap {
        return s.to_string();
    }
    format!("{}…", s.chars().take(cap).collect::<String>())
}

fn build(g: &Inner) -> Summary {
    let verdict = |id: &str| g.verdicts.get(id);
    let mut sessions: Vec<SessionRow> = g
        .sessions
        .iter()
        .map(|s| {
            let v = verdict(&s.session_id);
            SessionRow {
                session_id: s.session_id.clone(),
                name: s.name.clone(),
                cwd: s.cwd.clone(),
                status: s.status.clone(),
                tmux: s.tmux.clone(),
                pane_id: s.pane_id.clone(),
                level: v.map(|v| v.level).unwrap_or(Level::Idle),
                confidence: v.map(|v| v.confidence).unwrap_or(0.0),
                source: v.map(|v| v.source.clone()).unwrap_or_else(|| "none".into()),
                snippet: truncate(s.last_text.as_deref().unwrap_or(""), 160),
                updated_at: s.updated_at,
            }
        })
        .collect();
    let session_panes: HashSet<&str> = g.sessions.iter().filter_map(|s| s.pane_id.as_deref()).collect();
    let mut panes: Vec<PaneRow> = g
        .panes
        .iter()
        .map(|p| {
            // A Claude session's own verdict speaks for its pane.
            let own = g
                .sessions
                .iter()
                .find(|s| s.pane_id.as_deref() == Some(&p.id))
                .and_then(|s| verdict(&s.session_id));
            let v = own.or_else(|| verdict(&p.id));
            PaneRow {
                id: p.id.clone(),
                session: p.session.clone(),
                window_index: p.window_index,
                window_name: p.window_name.clone(),
                pane_index: p.pane_index,
                command: p.command.clone(),
                title: p.title.clone(),
                cwd: p.cwd.clone(),
                level: v.map(|v| v.level).unwrap_or(Level::Idle),
                confidence: v.map(|v| v.confidence).unwrap_or(0.0),
                source: v.map(|v| v.source.clone()).unwrap_or_else(|| "none".into()),
                snippet: truncate(g.tails.get(&p.id).map(String::as_str).unwrap_or(""), 160),
            }
        })
        .collect();
    sessions.sort_by(|a, b| b.level.cmp(&a.level).then(b.updated_at.cmp(&a.updated_at)));
    panes.sort_by(|a, b| b.level.cmp(&a.level).then(a.session.cmp(&b.session)).then(a.window_index.cmp(&b.window_index)));
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut top = Level::Idle;
    let levels = sessions
        .iter()
        .map(|s| s.level)
        .chain(panes.iter().filter(|p| !session_panes.contains(p.id.as_str())).map(|p| p.level));
    for level in levels {
        *counts.entry(format!("{level:?}").to_lowercase()).or_insert(0) += 1;
        top = top.max(level);
    }
    Summary { updated_at: now_ms(), top, counts, sessions, panes }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(id: &str, kind: &str, level: Level) -> VerdictMsg {
        VerdictMsg { id: id.into(), kind: kind.into(), level, confidence: 0.9, probabilities: HashMap::new(), source: "jev".into(), at: 1 }
    }

    fn session(id: &str, pane: Option<&str>, status: &str, text: &str) -> ClaudeSession {
        ClaudeSession {
            session_id: id.into(),
            pid: 1,
            name: id.into(),
            cwd: format!("/x/{id}"),
            status: status.into(),
            status_updated_at: 0,
            updated_at: 5,
            tmux: None,
            pane_id: pane.map(str::to_string),
            last_text: Some(text.into()),
            last_text_at: None,
        }
    }

    fn pane(id: &str, session: &str, window: &str, command: &str) -> PaneInfo {
        let fields = [id, session, "1", window, "0", "1", command, "", "80", "24", "/x", "1", "1", "1", "0"];
        crate::tmux::parse_line(&fields.join(crate::tmux::SEP)).expect("pane line")
    }

    #[test]
    fn summary_sorts_by_priority_counts_once_and_writes_on_change() {
        let dir = std::env::temp_dir().join(format!("wg-state-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::new(dir.join("verdicts.json"));
        assert!(store.set_panes(vec![pane("%1", "shop", "web", "node"), pane("%2", "main", "claude", "claude")]).is_some(), "first write");
        assert!(store.set_sessions(vec![session("s1", Some("%2"), "idle", "Keep the tests?")]).is_some());
        store.set_tail("%1", "FAIL src/Table.test.tsx".into());
        assert!(store.set_verdict(verdict("%1", "pane", Level::Failing)).is_some());
        assert!(store.set_verdict(verdict("s1", "claude", Level::Attention)).is_some());
        assert!(store.set_verdict(verdict("s1", "claude", Level::Attention)).is_none(), "unchanged summary is not rewritten");

        let s = store.summary();
        assert_eq!(s.top, Level::Attention);
        assert_eq!(s.panes[0].id, "%2", "the Claude pane inherits the session's attention and sorts first");
        assert_eq!(s.panes[0].level, Level::Attention);
        assert_eq!(s.panes[1].snippet, "FAIL src/Table.test.tsx");
        assert_eq!(s.sessions[0].snippet, "Keep the tests?");
        assert_eq!(s.counts.get("attention"), Some(&1), "session and its pane count once");
        assert_eq!(s.counts.get("failing"), Some(&1));

        let text = fs::read_to_string(store.path()).unwrap();
        assert!(text.contains("\"top\": \"attention\""), "{text}");
        assert!(text.contains("\"snippet\": \"Keep the tests?\""));

        // A vanished pane takes its verdict with it.
        assert!(store.set_panes(vec![pane("%2", "main", "claude", "claude")]).is_some());
        assert_eq!(store.summary().counts.get("failing"), None);
        assert_eq!(store.verdicts().len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn truncate_keeps_short_text_and_marks_cut_text() {
        assert_eq!(truncate("  hi  ", 10), "hi");
        assert_eq!(truncate("abcdefghij", 4), "abcd…");
    }
}
