#!/usr/bin/env node
// Records the demo against the browser mock: drives Chrome through the
// story, captions each beat, and writes docs/demo.webm. `pnpm demo:encode`
// turns that into the MP4 and GIF that the README uses.
//
// Needs: the dev server (`pnpm dev`, or `pnpm tauri dev`) on :1420,
// Google Chrome, and ffmpeg on PATH.

import { existsSync, mkdirSync } from "node:fs";
import puppeteer from "puppeteer-core";

const URL = process.env.WG_URL ?? "http://localhost:1420/";
const OUT = "docs/demo.webm";
const CHROME =
  process.env.CHROME ??
  ["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", "/usr/bin/google-chrome"].find(existsSync);

if (!CHROME) {
  console.error("Chrome not found; set CHROME=/path/to/chrome");
  process.exit(1);
}
mkdirSync("docs", { recursive: true });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const browser = await puppeteer.launch({
  executablePath: CHROME,
  headless: false,
  defaultViewport: { width: 1440, height: 900, deviceScaleFactor: 2 },
  args: ["--window-size=1440,980", "--hide-scrollbars", "--force-device-scale-factor=2"],
});
const page = await browser.newPage();
await page.goto(URL, { waitUntil: "networkidle0" });
await page.waitForSelector(".tile", { timeout: 20_000 });

const caption = (text) => page.evaluate((t) => window.__wg?.caption?.(t), text);
const key = async (k, mods = []) => {
  for (const m of mods) await page.keyboard.down(m);
  await page.keyboard.press(k);
  for (const m of mods.reverse()) await page.keyboard.up(m);
};

// Let the mock fill the panes and Jev "answer" before recording.
await sleep(9_000);
const recorder = await page.screencast({ path: OUT });
const t0 = Date.now();
const beat = (label) => console.log(`${((Date.now() - t0) / 1000).toFixed(1)}s  ${label}`);

await caption("Everything running in your tmux, in one window.");
beat("intro");
await sleep(3_800);

await caption("Every pane is a live tile. Every Claude session is listed.");
beat("panes+sidebar");
await sleep(3_800);

await caption("Jev judges each one: red needs you, green is working.");
beat("jev");
await sleep(3_800);

await caption("A session is waiting for permission. Approve it from here.");
beat("approve");
const approve = await page.$(".claude-actions .btn--primary");
if (approve) {
  await sleep(1_200);
  await approve.click();
}
await sleep(2_600);

await caption("Answer Claude's question without leaving the cockpit.");
beat("reply");
const reply = await page.$(".reply-input");
if (reply) {
  await reply.click();
  await page.keyboard.type("Keep the tests.", { delay: 55 });
  await sleep(500);
  await page.keyboard.press("Enter");
}
await sleep(2_400);

await caption("⌘J jumps to the newest error in any pane.");
beat("jump");
await key("j", ["Meta"]);
await sleep(3_200);

await caption("⌘K searches every pane's scrollback.");
beat("search");
await key("k", ["Meta"]);
await sleep(600);
await page.keyboard.type("ECONNREFUSED", { delay: 60 });
await sleep(1_400);
await page.keyboard.press("Enter");
await sleep(2_400);

await caption("Enter focuses one pane. Esc comes back.");
beat("focus");
await page.keyboard.press("Enter");
await sleep(2_600);
await page.keyboard.press("Escape");
await sleep(1_200);

await caption("Need one more thing running? Open a pane from here.");
beat("newpane");
await key("N", ["Shift"]);
await sleep(900);
await page.keyboard.type("pnpm vitest", { delay: 60 });
await sleep(500);
await page.keyboard.press("Enter");
await sleep(3_000);

await caption("watchglass · tmux + Ghostty engine + Jev · no config");
beat("outro");
await sleep(3_600);
await caption(null);
await sleep(600);

await recorder.stop();
beat("stopped");
await browser.close();
console.log(`wrote ${OUT}`);
