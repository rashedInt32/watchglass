//! Messages from the core to the web view, and the slot that holds the
//! current subscriber. Set once the web view subscribes; swappable.

use crate::claude::ClaudeSession;
use crate::jev::VerdictMsg;
use crate::tmux::PaneInfo;
use serde::Serialize;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize)]
pub struct ChunkMsg {
    pub id: String,
    pub seq: u64,
    pub t: u64,
    /// base64 of the raw terminal bytes
    pub data: String,
}

pub trait Bus: Send + Sync + 'static {
    fn chunk(&self, msg: ChunkMsg);
    fn panes(&self, panes: Vec<PaneInfo>);
    fn claude(&self, sessions: Vec<ClaudeSession>);
    fn verdict(&self, verdict: VerdictMsg);
}

#[derive(Default)]
pub struct BusSlot(RwLock<Option<Arc<dyn Bus>>>);

impl BusSlot {
    pub fn set(&self, bus: Arc<dyn Bus>) {
        *self.0.write().expect("bus lock") = Some(bus);
    }
    fn get(&self) -> Option<Arc<dyn Bus>> {
        self.0.read().expect("bus lock").clone()
    }
    pub fn chunk(&self, msg: ChunkMsg) {
        if let Some(b) = self.get() {
            b.chunk(msg);
        }
    }
    pub fn panes(&self, panes: Vec<PaneInfo>) {
        if let Some(b) = self.get() {
            b.panes(panes);
        }
    }
    pub fn claude(&self, sessions: Vec<ClaudeSession>) {
        if let Some(b) = self.get() {
            b.claude(sessions);
        }
    }
    pub fn verdict(&self, verdict: VerdictMsg) {
        if let Some(b) = self.get() {
            b.verdict(verdict);
        }
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
