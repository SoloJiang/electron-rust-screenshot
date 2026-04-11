import { spawn } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';

export async function runScreenshot(
  config: any,
  actions: () => void | Promise<void>,
  timeoutMs = 15000
): Promise<any> {
  const tmpFile = `/tmp/screenshot-result-${Date.now()}.json`;
  const libPath = path.resolve(__dirname, '../../lib');
  const child = spawn(
    'node',
    [
      '-e',
      `const fs = require('fs');
const { start } = require('${libPath}');
const result = start(${JSON.stringify(config)});
fs.writeFileSync('${tmpFile}', JSON.stringify(result));
`,
    ],
    {
      env: {
        ...process.env,
        SCREENSHOT_TEST_TIMEOUT_MS: String(timeoutMs),
      },
    }
  );

  // Wait for overlay to appear
  await new Promise((r) => setTimeout(r, 800));

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
    await new Promise((r) => setTimeout(r, 200));
  }

  child.kill();
  throw new Error('Screenshot did not complete in time');
}
