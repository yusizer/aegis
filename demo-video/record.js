// Records the aegis-demo.html terminal replay to a webm via Playwright, then
// we convert to mp4 with ffmpeg (see record.sh / package.json).
const { chromium } = require('playwright');
const path = require('path');
const fs = require('fs');

const HTML = 'file://' + path.resolve(__dirname, 'aegis-demo.html').replace(/\\/g, '/');
const OUT_DIR = path.resolve(__dirname, 'videos');
fs.mkdirSync(OUT_DIR, { recursive: true });

const REPLAY_MS = 191000; // replay ~181s + end card

(async () => {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 1280, height: 720 },
    deviceScaleFactor: 1,
    recordVideo: { dir: OUT_DIR, size: { width: 1280, height: 720 } },
  });
  const page = await context.newPage();
  await page.goto(HTML, { waitUntil: 'load' });
  // speed up: let the page run its timeline in real time
  await page.waitForTimeout(REPLAY_MS);
  const vid = page.video();
  await context.close();
  const webm = await vid.path();
  console.log('WEBM=' + webm);
  await browser.close();
})().catch(e => { console.error(e); process.exit(1); });
