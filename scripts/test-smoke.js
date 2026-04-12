const { spawn, execSync } = require('child_process');
const fs = require('fs');
const path = require('path');
const os = require('os');

const tmpFile = path.join(os.tmpdir(), `smoke-result-${Date.now()}.json`);
const indexPath = path.resolve(__dirname, '../dist/index.js');

// Discover cliclick path
function findCliclick() {
  const candidates = [
    '/opt/homebrew/bin/cliclick',
    '/usr/local/bin/cliclick',
  ];
  for (const c of candidates) {
    if (fs.existsSync(c)) return c;
  }
  try {
    return execSync('which cliclick', { encoding: 'utf8' }).trim();
  } catch {
    return null;
  }
}

const cliclick = findCliclick();

const child = spawn(
  process.execPath,
  [
    '-e',
    `
const fs = require('fs');
const { start } = require(${JSON.stringify(indexPath)});
try {
  const result = start({ savePath: '/tmp/smoke.png' });
  fs.writeFileSync(${JSON.stringify(tmpFile)}, JSON.stringify(result));
} catch (err) {
  const msg = (err && err.message) ? err.message : String(err);
  fs.writeFileSync(${JSON.stringify(tmpFile)}, JSON.stringify({ error: msg }));
}
`,
  ],
  {
    env: {
      ...process.env,
      SCREENSHOT_TEST_TIMEOUT_MS: '8000',
    },
  }
);

// Auto-dismiss overlay after it gains focus
setTimeout(() => {
  if (cliclick) {
    try {
      execSync(`${cliclick} kp:esc`);
    } catch (e) {
      console.error('cliclick failed:', e.message);
    }
  } else {
    console.error('cliclick not found, relying on timeout fallback');
  }
}, 1500);

function finish() {
  if (fs.existsSync(tmpFile)) {
    const raw = fs.readFileSync(tmpFile, 'utf8');
    try {
      fs.unlinkSync(tmpFile);
    } catch {}
    try {
      const result = JSON.parse(raw);
      console.log('result', JSON.stringify(result));
      if (result && result.error) {
        process.exit(1);
      }
      process.exit(0);
    } catch (e) {
      console.error('Failed to parse result:', raw);
      process.exit(1);
    }
  } else {
    console.error('no result file');
    process.exit(1);
  }
}

child.on('exit', finish);
child.on('error', (err) => {
  console.error('child process error:', err.message);
  finish();
});
