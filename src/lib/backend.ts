// One seam between the UI and whatever watches the panes: the Tauri core in
// the app, a scripted mock in a plain browser (`pnpm dev` for UI work).
import { invoke } from "@tauri-apps/api/core";
import type { Appearance } from "./appearance";
import * as ipc from "./ipc";
import { createMockBackend } from "./mock";

export type Backend = {
  kind: "tauri" | "mock";
  subscribe: typeof ipc.subscribe;
  tmuxAvailable: typeof ipc.tmuxAvailable;
  tmuxStatus: typeof ipc.tmuxStatus;
  attachPane: typeof ipc.attachPane;
  detachPane: typeof ipc.detachPane;
  focusPane: typeof ipc.focusPane;
  sendKeys: typeof ipc.sendKeys;
  sendLine: typeof ipc.sendLine;
  newWindow: typeof ipc.newWindow;
  jevStatus: typeof ipc.jevStatus;
  ghosttyAppearance(): Promise<Appearance>;
};

const tauriBackend: Backend = {
  kind: "tauri",
  subscribe: ipc.subscribe,
  tmuxAvailable: ipc.tmuxAvailable,
  tmuxStatus: ipc.tmuxStatus,
  attachPane: ipc.attachPane,
  detachPane: ipc.detachPane,
  focusPane: ipc.focusPane,
  sendKeys: ipc.sendKeys,
  sendLine: ipc.sendLine,
  newWindow: ipc.newWindow,
  jevStatus: ipc.jevStatus,
  ghosttyAppearance: () => invoke<Appearance>("ghostty_appearance"),
};

export const backend: Backend = ipc.inTauri() ? tauriBackend : createMockBackend();
