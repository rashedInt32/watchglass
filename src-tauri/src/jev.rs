//! Jev decides what deserves the user's attention. One Choice per pane or
//! session, debounced, cached by content, with deterministic overrides
//! where the answer is already known.

use crate::bus::{now_ms, BusSlot};
use crate::claude::ClaudeSession;
use crate::tmux::PaneInfo;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub const MODEL: &str = "jev-latest";
pub const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
const DEBOUNCE: Duration = Duration::from_millis(2500);
/// A pane that never goes quiet still gets judged this often.
const MAX_WAIT: Duration = Duration::from_secs(10);
const TAIL_LINES: usize = 40;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Idle,
    Working,
    Warning,
    Failing,
    Attention,
}

impl Level {
    pub fn parse(s: &str) -> Option<Level> {
        match s {
            "idle" => Some(Level::Idle),
            "working" => Some(Level::Working),
            "warning" => Some(Level::Warning),
            "failing" => Some(Level::Failing),
            "attention" => Some(Level::Attention),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VerdictMsg {
    pub id: String,
    /// `pane` or `claude`
    pub kind: String,
    pub level: Level,
    pub confidence: f64,
    pub probabilities: HashMap<String, f64>,
    /// `jev` or `rule`
    pub source: String,
    pub at: u64,
}

pub struct JevClient {
    key: String,
    endpoint: String,
}

pub fn key_from_disk() -> Option<String> {
    if let Ok(k) = std::env::var("TYPESAFE_API_KEY") {
        if !k.trim().is_empty() {
            return Some(k.trim().to_string());
        }
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let key = std::fs::read_to_string(home.join(".config").join("typesafe").join("key")).ok()?;
    let key = key.trim();
    (!key.is_empty()).then(|| key.to_string())
}

impl JevClient {
    pub fn from_disk() -> Option<Self> {
        key_from_disk().map(|key| Self {
            key,
            endpoint: std::env::var("WATCHGLASS_JEV_ENDPOINT").unwrap_or_else(|_| ENDPOINT.into()),
        })
    }

    pub fn ask(&self, state: Value, questions: Value) -> Result<Value, String> {
        let body = json!({ "model": MODEL, "state": state, "questions": questions });
        let resp = ureq::post(&self.endpoint)
            .set("Authorization", &format!("Bearer {}", self.key))
            .set("Content-Type", "application/json")
            .timeout(Duration::from_secs(20))
            .send_json(body)
            .map_err(|e| format!("jev: {e}"))?;
        resp.into_json::<Value>().map_err(|e| format!("jev response: {e}"))
    }
}

fn level_question(instructions: &str, criteria: [(&str, &str); 5]) -> Value {
    let mut c = serde_json::Map::new();
    for (k, v) in criteria {
        c.insert(k.to_string(), Value::String(v.to_string()));
    }
    json!({ "level": { "type": "choice", "instructions": instructions, "criteria": c } })
}

pub fn pane_questions() -> Value {
    level_question(
        "This is the most recent output of one process the user keeps running while working elsewhere. Judge what the user should do about it right now.",
        [
            ("attention", "The process is stopped and waiting for a person: a prompt, a question, a confirmation, a login, a menu, or a choice it cannot make itself."),
            ("failing", "It has failed or is broken: a crash, an exception, failed tests, build or type errors, a server that could not start, repeated connection errors."),
            ("warning", "Something is off but it keeps going: warnings, deprecations, retries that then succeed, slowness."),
            ("working", "Healthy and making progress: normal requests, passing tests, compiling, installing, a watcher with no problems."),
            ("idle", "Nothing is happening: a shell prompt with no running command, finished work, or silence."),
        ],
    )
}

pub fn claude_questions() -> Value {
    level_question(
        "This is a Claude Code session the user runs alongside others. Judge whether it needs the user right now, using its status and the last thing it said.",
        [
            ("attention", "Claude is waiting on the user: it asked a question, needs a decision or approval, or stopped short of finishing and says what it needs."),
            ("failing", "Claude reports being blocked or failing: errors it could not fix, tests it could not make pass, missing access or information."),
            ("warning", "Claude finished but flags something to check, a risk it took, or a step it skipped."),
            ("working", "Claude is still working through the task and needs nothing."),
            ("idle", "Claude finished cleanly and nothing is asked of the user."),
        ],
    )
}

pub fn pane_state(pane: &PaneInfo, tail: &str) -> Value {
    json!({
        "process": {
            "tmux_window": format!("{}:{}", pane.session, pane.window_name),
            "foreground_command": pane.command,
            "title": pane.title,
            "working_dir": pane.cwd,
        },
        "recent_output_last_lines": tail,
    })
}

pub fn claude_state(s: &ClaudeSession) -> Value {
    let age_s = now_ms().saturating_sub(s.updated_at) / 1000;
    json!({
        "status": s.status,
        "seconds_since_last_activity": age_s,
        "working_dir": s.cwd.rsplit('/').next().unwrap_or(&s.cwd),
        "last_assistant_message": s.last_text.clone().unwrap_or_default(),
    })
}

/// Deterministic answers that do not need a model.
pub fn claude_rule(s: &ClaudeSession) -> Option<Level> {
    match s.status.as_str() {
        "waiting" => Some(Level::Attention),
        "busy" => Some(Level::Working),
        _ => None,
    }
}

/// Interactive shells. When one is the foreground command, nothing is
/// running in that pane: it sits at its prompt.
pub const SHELLS: &[&str] = &["zsh", "bash", "fish", "sh", "nu", "dash", "ksh", "tcsh"];

/// A pane whose foreground command is a shell is idle by construction. Its
/// scrollback may still show a finished program's last screen, which reads
/// like a prompt to the model, so this rule answers before Jev is asked.
pub fn pane_rule(pane: &PaneInfo) -> Option<Level> {
    SHELLS.contains(&pane.command.as_str()).then_some(Level::Idle)
}

pub fn parse_verdict(answer: &Value) -> Option<(Level, f64, HashMap<String, f64>)> {
    let a = answer.get("answers")?.get("level")?;
    let level = Level::parse(a.get("choice")?.as_str()?)?;
    let confidence = a.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.0);
    let probabilities = a
        .get("probabilities")
        .and_then(|p| p.as_object())
        .map(|m| m.iter().filter_map(|(k, v)| v.as_f64().map(|f| (k.clone(), f))).collect())
        .unwrap_or_default();
    Some((level, confidence, probabilities))
}

pub enum Job {
    Pane { pane: PaneInfo, tail: String },
    Claude(ClaudeSession),
}

fn hash_of(v: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

/// Debounces jobs per id, skips unchanged content, calls Jev off-thread.
pub struct Classifier {
    tx: Sender<Job>,
    pub enabled: bool,
}

impl Classifier {
    pub fn start(bus: Arc<BusSlot>, log_path: Option<PathBuf>) -> Arc<Self> {
        let client = JevClient::from_disk();
        let enabled = client.is_some();
        let (tx, rx) = mpsc::channel::<Job>();
        thread::Builder::new()
            .name("wg-jev".into())
            .spawn(move || run(rx, client, bus, log_path))
            .expect("jev thread");
        Arc::new(Self { tx, enabled })
    }

    pub fn submit(&self, job: Job) {
        let _ = self.tx.send(job);
    }
}

/// Appends one JSON line per verdict so classification can be inspected.
fn log_verdict(path: &Option<PathBuf>, v: &VerdictMsg) {
    let Some(path) = path else {
        return;
    };
    if let Ok(line) = serde_json::to_string(v) {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            use std::io::Write;
            let _ = writeln!(f, "{line}");
        }
    }
}

struct Pending {
    first_at: Instant,
    ready_at: Instant,
    job: Job,
}

fn run(rx: Receiver<Job>, client: Option<JevClient>, bus: Arc<BusSlot>, log_path: Option<PathBuf>) {
    let pending: Mutex<HashMap<String, Pending>> = Mutex::new(HashMap::new());
    let mut seen: HashMap<String, u64> = HashMap::new();
    loop {
        // Gather everything that arrived, keeping the newest job per id. The
        // debounce restarts on each arrival but never past MAX_WAIT from the
        // first one, so a pane that never goes quiet is still judged.
        while let Ok(job) = rx.try_recv() {
            let now = Instant::now();
            let (id, debounce) = match &job {
                Job::Pane { pane, .. } => (format!("pane:{}", pane.id), DEBOUNCE),
                Job::Claude(s) => (format!("claude:{}", s.session_id), Duration::ZERO),
            };
            let mut p = pending.lock().expect("pending");
            let first_at = p.get(&id).map(|e| e.first_at).unwrap_or(now);
            let ready_at = (now + debounce).min(first_at + MAX_WAIT);
            p.insert(id, Pending { first_at, ready_at, job });
        }
        let due: Vec<(String, Job)> = {
            let mut p = pending.lock().expect("pending");
            let now = Instant::now();
            let keys: Vec<String> = p.iter().filter(|(_, e)| e.ready_at <= now).map(|(k, _)| k.clone()).collect();
            keys.into_iter().filter_map(|k| p.remove(&k).map(|e| (k, e.job))).collect()
        };
        for (key, job) in due {
            let (id, kind, rule, state, questions, content_hash) = match &job {
                Job::Pane { pane, tail } => {
                    let rule = pane_rule(pane);
                    // A shell at its prompt hashes on the command alone, so
                    // its verdict is emitted once, not on every keystroke.
                    let hash = if rule.is_some() { hash_of(&pane.command) } else { hash_of(&(tail, &pane.command)) };
                    (pane.id.clone(), "pane", rule, pane_state(pane, tail), pane_questions(), hash)
                }
                Job::Claude(s) => (
                    s.session_id.clone(),
                    "claude",
                    claude_rule(s),
                    claude_state(s),
                    claude_questions(),
                    hash_of(&(&s.status, &s.last_text)),
                ),
            };
            if seen.get(&key) == Some(&content_hash) {
                continue;
            }
            seen.insert(key, content_hash);
            if let Some(level) = rule {
                let v = VerdictMsg {
                    id,
                    kind: kind.into(),
                    level,
                    confidence: 1.0,
                    probabilities: HashMap::new(),
                    source: "rule".into(),
                    at: now_ms(),
                };
                log_verdict(&log_path, &v);
                bus.verdict(v);
                continue;
            }
            let Some(client) = client.as_ref() else {
                continue;
            };
            match client.ask(state, questions).and_then(|v| parse_verdict(&v).ok_or_else(|| "jev: unexpected answer".into())) {
                Ok((level, confidence, probabilities)) => {
                    let v = VerdictMsg {
                        id,
                        kind: kind.into(),
                        level,
                        confidence,
                        probabilities,
                        source: "jev".into(),
                        at: now_ms(),
                    };
                    log_verdict(&log_path, &v);
                    bus.verdict(v);
                }
                Err(e) => {
                    eprintln!("watchglass: {e}");
                    if let Some(p) = &log_path {
                        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
                            use std::io::Write;
                            let _ = writeln!(f, "{{\"error\":{}}}", serde_json::to_string(&e).unwrap_or_default());
                        }
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(300));
    }
}

pub fn tail_lines() -> usize {
    TAIL_LINES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_order_by_priority() {
        assert!(Level::Attention > Level::Failing);
        assert!(Level::Failing > Level::Warning);
        assert!(Level::Warning > Level::Working);
        assert!(Level::Working > Level::Idle);
        assert_eq!(Level::parse("failing"), Some(Level::Failing));
        assert_eq!(Level::parse("nope"), None);
    }

    #[test]
    fn questions_have_five_criteria_and_state_is_compact() {
        for q in [pane_questions(), claude_questions()] {
            let c = q["level"]["criteria"].as_object().unwrap();
            assert_eq!(c.len(), 5);
            assert_eq!(q["level"]["type"], "choice");
        }
        let pane = crate::tmux::parse_line(&["%1", "main", "1", "web", "0", "1", "node", "t", "80", "24", "/x", "1", "1", "1", "0"].join(crate::tmux::SEP)).unwrap();
        let s = pane_state(&pane, "GET / 200\nError: boom");
        assert_eq!(s["process"]["foreground_command"], "node");
        assert_eq!(s["recent_output_last_lines"], "GET / 200\nError: boom");
    }

    #[test]
    fn parses_a_choice_answer() {
        let v = json!({ "model": "jev-1", "answers": { "level": { "type": "choice", "choice": "attention", "confidence": 0.91, "probabilities": { "attention": 0.91, "working": 0.05 } } } });
        let (level, conf, probs) = parse_verdict(&v).unwrap();
        assert_eq!(level, Level::Attention);
        assert!((conf - 0.91).abs() < 1e-9);
        assert_eq!(probs.get("working"), Some(&0.05));
        assert!(parse_verdict(&json!({ "answers": {} })).is_none());
    }

    #[test]
    fn a_shell_at_its_prompt_is_idle_before_jev_is_asked() {
        let line = |cmd: &str| ["%9", "packages", "1", "[tmux]", "0", "1", cmd, "", "115", "55", "/x", "0", "0", "1", "0"].join(crate::tmux::SEP);
        let shell = crate::tmux::parse_line(&line("zsh")).unwrap();
        assert_eq!(pane_rule(&shell), Some(Level::Idle));
        let running = crate::tmux::parse_line(&line("node")).unwrap();
        assert_eq!(pane_rule(&running), None, "a real foreground program goes to the model");
    }

    #[test]
    fn claude_rules_short_circuit() {
        let mut s = ClaudeSession {
            session_id: "s".into(),
            pid: 1,
            name: "n".into(),
            cwd: "/x".into(),
            status: "waiting".into(),
            status_updated_at: 0,
            updated_at: 0,
            tmux: None,
            pane_id: None,
            last_text: None,
            last_text_at: None,
        };
        assert_eq!(claude_rule(&s), Some(Level::Attention));
        s.status = "busy".into();
        assert_eq!(claude_rule(&s), Some(Level::Working));
        s.status = "idle".into();
        assert_eq!(claude_rule(&s), None, "idle needs the model to read the last message");
    }
}
