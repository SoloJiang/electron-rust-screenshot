const { spawn, execSync } = require('child_process');
const fs = require('fs');
const tmpFile = '/tmp/smoke-result.json';

const child = spawn('node', ['-e', `
const fs = require('fs');
const { start } = require('./dist/lib');
const result = start({ savePath: '/tmp/smoke.png' });
fs.writeFileSync('${tmpFile}', JSON.stringify(result));
`]);

setTimeout(() => {
  try {
    execSync('/opt/homebrew/bin/cliclick kp:esc');
  } catch (e) {}
}, 1500);

child.on('close', () => {
  if (fs.existsSync(tmpFile)) {
    const result = JSON.parse(fs.readFileSync(tmpFile, 'utf8'));
    console.log('result', JSON.stringify(result));
    fs.unlinkSync(tmpFile);
    process.exit(0);
  } else {
    console.error('no result file');
    process.exit(1);
  }
});
