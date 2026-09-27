// OS notifications with a per-id rate limit.
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { inTauri } from "./ipc";

const RATE_MS = 10_000;
const lastSent = new Map<string, number>();
let permission: Promise<boolean> | null = null;

function ensurePermission(): Promise<boolean> {
  if (!inTauri()) return Promise.resolve(false);
  permission ??= (async () => {
    if (await isPermissionGranted()) return true;
    return (await requestPermission()) === "granted";
  })();
  return permission;
}

/** Returns true when a notification was actually sent. */
export async function notify(id: string, title: string, body: string, now = Date.now()): Promise<boolean> {
  const last = lastSent.get(id) ?? 0;
  if (now - last < RATE_MS) return false;
  lastSent.set(id, now);
  if (!(await ensurePermission())) return false;
  sendNotification({ title, body: body.slice(0, 200) });
  return true;
}

/** Test seam: reset the rate limiter. */
export function resetNotifyState(): void {
  lastSent.clear();
}
