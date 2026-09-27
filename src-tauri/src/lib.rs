mod bus;
mod capture;
mod claude;
mod ghostty;
mod jev;
mod tmux;

use bus::{Bus, BusSlot, ChunkMsg};
use claude::ClaudeSession;
use jev::{Classifier, Job, VerdictMsg};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::{Manager, State};
use tmux::{AttachInfo, PaneInfo, TapManager};

struct AppState {
    bus: Arc<BusSlot>,
    taps: Arc<TapManager>,
    classifier: Arc<Classifier>,
    panes: Arc<Mutex<Vec<PaneInfo>>>,
}

struct ChannelBus {
    chunk: Channel<ChunkMsg>,
    panes: Channel<Vec<PaneInfo>>,
    claude: Channel<Vec<ClaudeSession>>,
    verdict: Channel<VerdictMsg>,
}

impl Bus for ChannelBus {
    fn chunk(&self, msg: ChunkMsg) {
        let _ = self.chunk.send(msg);
    }
    fn panes(&self, panes: Vec<PaneInfo>) {
        let _ = self.panes.send(panes);
    }
    fn claude(&self, sessions: Vec<ClaudeSession>) {
        let _ = self.claude.send(sessions);
    }
    fn verdict(&self, verdict: VerdictMsg) {
        let _ = self.verdict.send(verdict);
    }
}

#[tauri::command]
fn subscribe(
    state: State<AppState>,
    on_chunk: Channel<ChunkMsg>,
    on_panes: Channel<Vec<PaneInfo>>,
    on_claude: Channel<Vec<ClaudeSession>>,
    on_verdict: Channel<VerdictMsg>,
) {
    state.bus.set(Arc::new(ChannelBus {
        chunk: on_chunk,
        panes: on_panes,
        claude: on_claude,
        verdict: on_verdict,
    }));
    // Prime the new subscriber so the UI fills without waiting for a change.
    state.bus.panes(tmux::list_panes().unwrap_or_default());
    state.bus.claude(claude::sessions());
}

#[tauri::command]
fn tmux_available() -> bool {
    tmux::available()
}

#[tauri::command]
fn tmux_status() -> tmux::TmuxStatus {
    tmux::status()
}

#[tauri::command]
fn list_panes() -> Result<Vec<PaneInfo>, String> {
    tmux::list_panes()
}

#[tauri::command]
fn attach_pane(state: State<AppState>, id: String) -> Result<AttachInfo, String> {
    let pane = state
        .panes
        .lock()
        .expect("panes")
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .or_else(|| tmux::list_panes().ok()?.into_iter().find(|p| p.id == id))
        .ok_or_else(|| format!("no tmux pane {id}"))?;
    let info = state.taps.attach(&pane)?;
    schedule_pane(&state, &pane.id);
    Ok(info)
}

#[tauri::command]
fn detach_pane(state: State<AppState>, id: String) -> bool {
    state.taps.detach(&id)
}

#[tauri::command]
fn focus_pane(state: State<AppState>, id: String) -> Result<(), String> {
    let pane = state
        .panes
        .lock()
        .expect("panes")
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| format!("no tmux pane {id}"))?;
    tmux::focus(&pane)
}

#[tauri::command]
fn send_keys(id: String, keys: Vec<String>) -> Result<(), String> {
    tmux::send_keys(&id, &keys)
}

/// Types a line into a pane as if at the keyboard, then presses Enter.
#[tauri::command]
fn send_line(id: String, text: String) -> Result<(), String> {
    tmux::send_line(&id, &text)
}

/// Opens a new tmux window running `command`; the poller picks it up as a tile.
#[tauri::command]
fn new_window(session: String, name: String, command: String) -> Result<String, String> {
    tmux::new_window(&session, &name, &command)
}

#[tauri::command]
fn claude_sessions() -> Vec<ClaudeSession> {
    claude::sessions()
}

#[tauri::command]
fn ghostty_appearance() -> ghostty::Appearance {
    ghostty::load()
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct JevStatus {
    enabled: bool,
    reason: String,
}

#[tauri::command]
fn jev_status(state: State<AppState>) -> JevStatus {
    if state.classifier.enabled {
        JevStatus { enabled: true, reason: "key found".into() }
    } else {
        JevStatus {
            enabled: false,
            reason: "no key in TYPESAFE_API_KEY or ~/.config/typesafe/key".into(),
        }
    }
}

fn schedule_pane(state: &AppState, id: &str) {
    let pane = state.panes.lock().expect("panes").iter().find(|p| p.id == id).cloned();
    if let (Some(pane), Some(tail)) = (pane, state.taps.tail_text(id, jev::tail_lines())) {
        if !tail.trim().is_empty() {
            state.classifier.submit(Job::Pane { pane, tail });
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("watchglass"));
            let bus = Arc::new(BusSlot::default());
            let _ = std::fs::create_dir_all(&data_dir);
            tmux::set_log_path(data_dir.join("watchglass.log"));
            tmux::log(&format!("start {} pid {}", env!("CARGO_PKG_VERSION"), std::process::id()));
            let taps = Arc::new(TapManager::new(Arc::clone(&bus), data_dir.clone()));
            let classifier = Classifier::start(Arc::clone(&bus), Some(data_dir.join("verdicts.log")));
            let panes: Arc<Mutex<Vec<PaneInfo>>> = Arc::new(Mutex::new(Vec::new()));

            // Output → classification, debounced inside the classifier.
            {
                let taps = Arc::clone(&taps);
                let classifier = Arc::clone(&classifier);
                let panes = Arc::clone(&panes);
                tmux::set_output_hook(Arc::new(move |id: &str| {
                    let pane = panes.lock().expect("panes").iter().find(|p| p.id == id).cloned();
                    if let (Some(pane), Some(tail)) = (pane, taps.tail_text(id, jev::tail_lines())) {
                        if !tail.trim().is_empty() {
                            classifier.submit(Job::Pane { pane, tail });
                        }
                    }
                }));
            }

            // Pipes left by a crashed run keep tmux appending forever; close ours.
            if let Ok(list) = tmux::list_panes() {
                let cleaned = taps.cleanup_stale(&list);
                if !cleaned.is_empty() {
                    eprintln!("watchglass: closed stale pipes on {}", cleaned.join(", "));
                }
            }
            taps.start_poller(Arc::clone(&panes));

            // Claude sessions: poll, emit on change, classify on change.
            {
                let bus = Arc::clone(&bus);
                let classifier = Arc::clone(&classifier);
                thread::Builder::new()
                    .name("wg-claude".into())
                    .spawn(move || {
                        let mut last: Option<Vec<ClaudeSession>> = None;
                        loop {
                            let sessions = claude::sessions();
                            if last.as_ref() != Some(&sessions) {
                                for s in &sessions {
                                    classifier.submit(Job::Claude(s.clone()));
                                }
                                bus.claude(sessions.clone());
                                last = Some(sessions);
                            }
                            thread::sleep(Duration::from_millis(2000));
                        }
                    })
                    .expect("claude poller");
            }

            app.manage(AppState {
                bus,
                taps: Arc::clone(&taps),
                classifier,
                panes,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            subscribe,
            tmux_available,
            tmux_status,
            list_panes,
            attach_pane,
            detach_pane,
            focus_pane,
            send_keys,
            send_line,
            new_window,
            claude_sessions,
            ghostty_appearance,
            jev_status
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                if window.label() == "main" {
                    if let Some(state) = window.app_handle().try_state::<AppState>() {
                        state.taps.detach_all();
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
