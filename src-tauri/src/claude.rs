//! Claude Code sessions on this machine: the state files Claude writes under
//! `~/.claude/sessions`, plus the last thing each session said, read from
//! its transcript.

use serde::Serialize;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSession {
    pub session_id: String,
    pub pid: u32,
    pub name: String,
    pub cwd: String,
    /// `busy`, `idle`, `waiting`, or whatever Claude wrote.
    pub status: String,
    pub status_updated_at: u64,
    pub updated_at: u64,
    /// `session:@window.%pane` as Claude records it.
    pub tmux: Option<String>,
    /// The `%pane` part, for linking to a tile.
    pub pane_id: Option<String>,
    /// Last assistant text, trimmed, for the list and for Jev.
    pub last_text: Option<String>,
    pub last_text_at: Option<String>,
}

const TRANSCRIPT_TAIL_BYTES: u64 = 512 * 1024;
const TEXT_CAP: usize = 1200;

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub fn sessions_dir() -> Option<PathBuf> {
    home().map(|h| h.join(".claude").join("sessions"))
}

fn alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(unix)]
    {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Claude names the transcript folder after the cwd with `/` and `.` as `-`.
pub fn project_slug(cwd: &str) -> String {
    cwd.chars().map(|c| if c == '/' || c == '.' { '-' } else { c }).collect()
}

pub fn transcript_path(cwd: &str, session_id: &str) -> Option<PathBuf> {
    home().map(|h| {
        h.join(".claude")
            .join("projects")
            .join(project_slug(cwd))
            .join(format!("{session_id}.jsonl"))
    })
}

pub fn sessions() -> Vec<ClaudeSession> {
    let Some(dir) = sessions_dir() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<ClaudeSession> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| parse_state(&p))
        .filter(|s| alive(s.pid))
        .map(|mut s| {
            if let Some(path) = transcript_path(&s.cwd, &s.session_id) {
                if let Some((text, at)) = last_assistant_text(&path) {
                    s.last_text = Some(text);
                    s.last_text_at = Some(at);
                }
            }
            s
        })
        .collect();
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    out
}

fn parse_state(path: &Path) -> Option<ClaudeSession> {
    let text = fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let tmux = v.get("tmux").and_then(|t| t.as_str()).map(str::to_string);
    let pane_id = tmux.as_deref().and_then(pane_from_tmux);
    Some(ClaudeSession {
        session_id: v.get("sessionId")?.as_str()?.to_string(),
        pid: v.get("pid").and_then(|p| p.as_u64()).unwrap_or(0) as u32,
        name: v.get("name").and_then(|n| n.as_str()).unwrap_or("claude").to_string(),
        cwd: v.get("cwd").and_then(|c| c.as_str()).unwrap_or("").to_string(),
        status: v.get("status").and_then(|s| s.as_str()).unwrap_or("unknown").to_string(),
        status_updated_at: v.get("statusUpdatedAt").and_then(|n| n.as_u64()).unwrap_or(0),
        updated_at: v.get("updatedAt").and_then(|n| n.as_u64()).unwrap_or(0),
        tmux,
        pane_id,
        last_text: None,
        last_text_at: None,
    })
}

/// `main:@0.%0` → `%0`.
pub fn pane_from_tmux(value: &str) -> Option<String> {
    value.rsplit('.').next().filter(|p| p.starts_with('%')).map(str::to_string)
}

/// Last assistant message that carried text, from the transcript's tail.
pub fn last_assistant_text(path: &Path) -> Option<(String, String)> {
    let mut f = File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let start = len.saturating_sub(TRANSCRIPT_TAIL_BYTES);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    for line in text.lines().rev() {
        if !line.contains("\"assistant\"") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("assistant") {
            continue;
        }
        let Some(content) = v.pointer("/message/content").and_then(|c| c.as_array()) else {
            continue;
        };
        let joined: String = content
            .iter()
            .filter(|item| item.get("type").and_then(|t| t.as_str()) == Some("text"))
            .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("\n");
        let joined = joined.trim();
        if joined.is_empty() {
            continue;
        }
        let at = v.get("timestamp").and_then(|t| t.as_str()).unwrap_or("").to_string();
        return Some((truncate(joined, TEXT_CAP), at));
    }
    None
}

fn truncate(s: &str, cap: usize) -> String {
    if s.len() <= cap {
        return s.to_string();
    }
    let mut end = cap;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_and_pane_parsing() {
        assert_eq!(project_slug("/Users/rashed/.claude"), "-Users-rashed--claude");
        assert_eq!(project_slug("/Users/me/code/app.v2"), "-Users-me-code-app-v2");
        assert_eq!(pane_from_tmux("main:@0.%0").as_deref(), Some("%0"));
        assert_eq!(pane_from_tmux("main:@3.%17").as_deref(), Some("%17"));
        assert_eq!(pane_from_tmux("nonsense"), None);
    }

    #[test]
    fn finds_last_assistant_text_skipping_tool_only_turns() {
        let dir = std::env::temp_dir().join(format!("wg-claude-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.jsonl");
        let lines = [
            r#"{"type":"user","message":{"role":"user","content":"fix it"}}"#,
            r#"{"type":"assistant","timestamp":"t1","message":{"role":"assistant","content":[{"type":"text","text":"Looking now."}]}}"#,
            r#"{"type":"assistant","timestamp":"t2","message":{"role":"assistant","content":[{"type":"text","text":"Done. Want me to keep the tests?"}]}}"#,
            r#"{"type":"assistant","timestamp":"t3","message":{"role":"assistant","content":[{"type":"tool_use","name":"Bash","input":{}}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"ok"}]}}"#,
        ];
        fs::write(&path, lines.join("\n")).unwrap();
        let (text, at) = last_assistant_text(&path).unwrap();
        assert_eq!(text, "Done. Want me to keep the tests?");
        assert_eq!(at, "t2");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn truncates_on_char_boundaries() {
        let s = "ééééééééé";
        let t = truncate(s, 5);
        assert!(t.ends_with('…'));
        assert!(t.len() < s.len());
    }
}
