// Follow mode per tile (SPEC §5.4). Pure so it can be tested without a terminal.

export type FollowMode = "live" | "paused";

export type FollowEvent =
  | { type: "scroll"; atBottom: boolean }
  | { type: "jump" }
  | { type: "live" }
  | { type: "toggle" };

export function reduceFollow(mode: FollowMode, event: FollowEvent): FollowMode {
  switch (event.type) {
    case "scroll":
      return event.atBottom ? "live" : "paused";
    case "jump":
      return "paused";
    case "live":
      return "live";
    case "toggle":
      return mode === "live" ? "paused" : "live";
  }
}

/** Errors seen since the tile was last focused. */
export function reduceUnread(unread: number, event: "error" | "focus"): number {
  return event === "focus" ? 0 : unread + 1;
}
