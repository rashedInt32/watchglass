//! Messages from the core to every subscriber: the full window, the menu
//! bar panel, and anything else that asks. Subscribers are keyed by window
//! label so a reloaded web view replaces its stale channels.

use crate::claude::ClaudeSession;
use crate::jev::VerdictMsg;
use crate::tmux::PaneInfo;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize)]
pub struct ChunkMsg {
    pub id: String,
    pub seq: u64,
    pub t: u64,
    /// base64 of the raw terminal bytes
    pub data: String,
}

/// Each method returns false when the subscriber is gone, which drops it.
pub trait Bus: Send + Sync + 'static {
    fn chunk(&self, msg: &ChunkMsg) -> bool;
    fn panes(&self, panes: &[PaneInfo]) -> bool;
    fn claude(&self, sessions: &[ClaudeSession]) -> bool;
    fn verdict(&self, verdict: &VerdictMsg) -> bool;
}

#[derive(Default)]
pub struct BusSlot(RwLock<HashMap<String, Arc<dyn Bus>>>);

impl BusSlot {
    pub fn set(&self, label: &str, bus: Arc<dyn Bus>) {
        self.0.write().expect("bus lock").insert(label.to_string(), bus);
    }

    fn each(&self, f: impl Fn(&dyn Bus) -> bool) {
        let subscribers: Vec<(String, Arc<dyn Bus>)> = self
            .0
            .read()
            .expect("bus lock")
            .iter()
            .map(|(k, v)| (k.clone(), Arc::clone(v)))
            .collect();
        let dead: Vec<String> = subscribers.iter().filter(|(_, b)| !f(b.as_ref())).map(|(k, _)| k.clone()).collect();
        if !dead.is_empty() {
            let mut map = self.0.write().expect("bus lock");
            for k in dead {
                map.remove(&k);
            }
        }
    }

    pub fn chunk(&self, msg: ChunkMsg) {
        self.each(|b| b.chunk(&msg));
    }
    pub fn panes(&self, panes: Vec<PaneInfo>) {
        self.each(|b| b.panes(&panes));
    }
    pub fn claude(&self, sessions: Vec<ClaudeSession>) {
        self.each(|b| b.claude(&sessions));
    }
    pub fn verdict(&self, verdict: VerdictMsg) {
        self.each(|b| b.verdict(&verdict));
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
