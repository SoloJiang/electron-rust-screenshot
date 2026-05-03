import { spawn } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';

export async function runScreenshot(
  config: any,
  actions: () => void | Promise<void>,
  timeoutMs = 15000
): Promise<any> {
  const tmpFile = `/tmp/screenshot-result-${Date.now()}.json`;
  const indexPath = path.resolve(__dirname, '../../dist/index');

  let childExited = false;
  let childExitCode: number | null = null;
  let stderr = '';

  const child = spawn(
    'node',
    [
      '-e',
      `const fs = require('fs');
const { start } = require('${indexPath}');
const raw = start(${JSON.stringify(config)});
try {
  const result = JSON.parse(raw);
  fs.writeFileSync('${tmpFile}', JSON.stringify(result));
} catch (e) {
  fs.writeFileSync('${tmpFile}', JSON.stringify({ type: 'error', message: String(raw) }));
}
`,
    ],
    {
      env: {
        ...process.env,
        SCREENSHOT_TEST_TIMEOUT_MS: String(timeoutMs),
      },
    }
  );

  child.stderr.on('data', (data) => {
    stderr += data.toString();
  });

  child.on('exit', (code) => {
    childExited = true;
    childExitCode = code;
  });

  // Wait for overlay to appear and gain focus
  await new Promise((r) => setTimeout(r, 1200));

  await actions();

  // Poll for result file
  const startTime = Date.now();
  while (Date.now() - startTime < timeoutMs) {
    if (fs.existsSync(tmpFile)) {
      const raw = fs.readFileSync(tmpFile, 'utf8');
      try {
        fs.unlinkSync(tmpFile);
      } catch {}
      child.kill();
      return JSON.parse(raw);
    }
    if (childExited) {
      child.kill();
      throw new Error(
        `Screenshot child exited early with code ${childExitCode}. stderr: ${stderr.slice(0, 500)}`
      );
    }
    await new Promise((r) => setTimeout(r, 200));
  }

  child.kill();
  throw new Error(
    `Screenshot did not complete in time. stderr: ${stderr.slice(0, 500)}`
  );
}
