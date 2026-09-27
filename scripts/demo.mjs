#!/usr/bin/env node
// Records the demo against the browser mock: drives Chrome through the
// story, captions each beat, and writes docs/demo-panel.webm (the menu
// bar list) and docs/demo.webm (the board). `pnpm demo:encode` turns them
// into the GIFs and the MP4 that the README uses.
//
// Needs: the dev server (`pnpm dev`, or `pnpm tauri dev`) on :1420,
// Google Chrome, and ffmpeg on PATH.

import { existsSync, mkdirSync } from "node:fs";
import puppeteer from "puppeteer-core";

const BASE = process.env.WG_URL ?? "http://localhost:1420/";
const CHROME =
  process.env.CHROME ??
  ["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", "/usr/bin/google-chrome"].find(existsSync);

if (!CHROME) {
  console.error("Chrome not found; set CHROME=/path/to/chrome");
  process.exit(1);
}
mkdirSync("docs", { recursive: true });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const t0 = Date.now();
const beat = (label) => console.log(`${((Date.now() - t0) / 1000).toFixed(1)}s  ${label}`);

const browser = await puppeteer.launch({
  executablePath: CHROME,
  headless: false,
  defaultViewport: { width: 1440, height: 900, deviceScaleFactor: 2 },
  args: ["--window-size=1440,980", "--hide-scrollbars", "--force-device-scale-factor=2"],
});

const drive = (page) => ({
  caption: (text) => page.evaluate((t) => window.__wg?.caption?.(t), text),
  press: async (k, mods = []) => {
    for (const m of mods) await page.keyboard.down(m);
    await page.keyboard.press(k);
    for (const m of mods.reverse()) await page.keyboard.up(m);
  },
});

// ── Part 1: the menu bar list ─────────────────────────────────────────
{
  const page = await browser.newPage();
  await page.setViewport({ width: 720, height: 760, deviceScaleFactor: 2 });
  await page.goto(`${BASE}index.html?view=panel&demo=1`, { waitUntil: "networkidle0" });
  await page.waitForSelector(".prow", { timeout: 20_000 });
  const { caption, press } = drive(page);
  await sleep(1_500);
  const recorder = await page.screencast({ path: "docs/demo-panel.webm" });

  await caption("watchglass lives in your menu bar. Click it, or press ⌃⌥W.");
  beat("panel intro");
  await sleep(3_600);

  await caption("Claude sessions first, then panes. Loudest first.");
  beat("panel order");
  await sleep(1_200);
  await press("j");
  await sleep(700);
  await press("j");
  await sleep(700);
  await press("k");
  await sleep(1_200);

  await caption("This one is waiting for permission. Press a to approve.");
  beat("panel approve");
  await sleep(1_600);
  await press("a");
  await sleep(2_400);

  await caption("Enter jumps your tmux client to any row.");
  beat("panel go");
  await sleep(1_000);
  await press("j");
  await sleep(600);
  await press("j");
  await sleep(1_800);

  await caption("Press o for the board.");
  beat("panel board");
  await sleep(2_400);
  await caption(null);
  await sleep(400);
  await recorder.stop();
  await page.close();
}

// ── Part 2: the board ─────────────────────────────────────────────────
{
  const page = await browser.newPage();
  await page.goto(BASE, { waitUntil: "networkidle0" });
  await page.waitForSelector(".tile", { timeout: 20_000 });
  const { caption, press } = drive(page);

  // Let the mock fill the panes and Jev "answer" before recording.
  await sleep(9_000);
  const recorder = await page.screencast({ path: "docs/demo.webm" });

  await caption("The board: every pane a live tile, every Claude session listed.");
  beat("board intro");
  await sleep(3_800);

  await caption("Jev judges each one: red needs you, orange failing, green working.");
  beat("board jev");
  await sleep(3_800);

  await caption("Approve a waiting session from here too.");
  beat("board approve");
  const approve = await page.$(".claude-actions .btn--primary");
  if (approve) {
    await sleep(1_200);
    await approve.click();
  }
  await sleep(2_400);

  await caption("⌘J jumps to the newest error in any pane.");
  beat("board jump");
  await press("j", ["Meta"]);
  await sleep(3_200);

  await caption("⌘K searches every pane's scrollback.");
  beat("board search");
  await press("k", ["Meta"]);
  await sleep(600);
  await page.keyboard.type("ECONNREFUSED", { delay: 60 });
  await sleep(1_400);
  await page.keyboard.press("Enter");
  await sleep(2_400);

  await caption("Enter focuses one pane. Esc comes back.");
  beat("board focus");
  await page.keyboard.press("Enter");
  await sleep(2_600);
  await page.keyboard.press("Escape");
  await sleep(1_200);

  await caption("Need one more thing running? Open a pane from here.");
  beat("board newpane");
  await press("N", ["Shift"]);
  await sleep(900);
  await page.keyboard.type("pnpm vitest", { delay: 60 });
  await sleep(500);
  await page.keyboard.press("Enter");
  await sleep(3_000);

  await caption("watchglass · menu bar + board · tmux + Ghostty engine + Jev · verdicts.json for your editor");
  beat("board outro");
  await sleep(3_800);
  await caption(null);
  await sleep(600);
  await recorder.stop();
  await page.close();
}

beat("stopped");
await browser.close();
console.log("wrote docs/demo-panel.webm and docs/demo.webm");
