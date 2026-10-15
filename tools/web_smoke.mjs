// Browser smoke test: serves dist/web, opens the game with ?autotest,
// presses "Играть" and waits for the bot to report AUTOTEST PASSED.
//   node tools/web_smoke.mjs [dist/web] [screenshot.png]
import { chromium } from 'playwright';
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';

const root = path.resolve(process.argv[2] || 'dist/web');
const shot = process.argv[3] || 'web-smoke.png';
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.png': 'image/png', '.txt': 'text/plain' };

const server = http.createServer((req, res) => {
  const url = new URL(req.url, 'http://x');
  const file = path.join(root, url.pathname === '/' ? 'index.html' : decodeURIComponent(url.pathname));
  if (!file.startsWith(root) || !fs.existsSync(file)) {
    res.writeHead(404).end();
    return;
  }
  const body = fs.readFileSync(file);
  res.writeHead(200, { 'Content-Type': types[path.extname(file)] || 'application/octet-stream', 'Content-Length': body.length });
  res.end(body);
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const port = server.address().port;

const browser = await chromium.launch({
  args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--autoplay-policy=no-user-gesture-required'],
});
const page = await browser.newPage({ viewport: { width: 960, height: 540 } });
let result = null;
const log = [];
page.on('console', (m) => {
  const t = m.text();
  if (/autotest|AUTOTEST|panicked|ERROR/i.test(t)) log.push(t);
  if (t.includes('AUTOTEST PASSED')) result = 'pass';
  if (t.includes('AUTOTEST FAILED') || t.includes('panicked')) result = result || 'fail';
});
page.on('pageerror', (e) => log.push('pageerror: ' + e.message));

await page.goto(`http://127.0.0.1:${port}/?autotest`);
await page.click('#start');
const deadline = Date.now() + 25 * 60 * 1000;
let shotTaken = false;
while (!result && Date.now() < deadline) {
  await page.waitForTimeout(1000);
  if (!shotTaken && log.some((l) => l.includes('night'))) {
    await page.screenshot({ path: shot });
    shotTaken = true;
  }
}
if (!shotTaken) await page.screenshot({ path: shot });
for (const l of log) console.log(l.replace(/%c/g, '').slice(0, 300));
await browser.close();
server.close();
if (result !== 'pass') {
  console.error('web smoke test: ' + (result || 'timeout'));
  process.exit(1);
}
console.log('web smoke test passed');
