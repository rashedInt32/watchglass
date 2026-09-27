// Chunk fan-out by source id, outside React so output never re-renders the tree.
import type { ChunkMsg } from "./ipc";

type Listener = (msg: ChunkMsg) => void;

const listeners = new Map<string, Set<Listener>>();
const backlog = new Map<string, ChunkMsg[]>();
const BACKLOG_CAP = 2000;

export function publish(msg: ChunkMsg): void {
  const set = listeners.get(msg.id);
  if (set && set.size > 0) {
    for (const l of set) l(msg);
    return;
  }
  let queue = backlog.get(msg.id);
  if (!queue) {
    queue = [];
    backlog.set(msg.id, queue);
  }
  queue.push(msg);
  if (queue.length > BACKLOG_CAP) queue.splice(0, queue.length - BACKLOG_CAP);
}

/** Subscribes and replays anything that arrived before the subscriber existed. */
export function subscribeChunks(id: string, listener: Listener): () => void {
  let set = listeners.get(id);
  if (!set) {
    set = new Set();
    listeners.set(id, set);
  }
  set.add(listener);
  const queued = backlog.get(id);
  if (queued) {
    backlog.delete(id);
    for (const msg of queued) listener(msg);
  }
  return () => {
    set.delete(listener);
    if (set.size === 0) listeners.delete(id);
  };
}

export function countLineFeeds(bytes: Uint8Array): number {
  let n = 0;
  for (let i = 0; i < bytes.length; i++) if (bytes[i] === 10) n++;
  return n;
}
