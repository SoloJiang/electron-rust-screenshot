import { execSync } from 'child_process';

export function isMac(): boolean {
  return process.platform === 'darwin';
}

export function runAppleScript(scriptPath: string, args: string[] = []): string {
  if (!isMac()) throw new Error('AppleScript only on macOS');
  return execSync(`osascript "${scriptPath}" ${args.map(a => `"${a}"`).join(' ')}`, {
    encoding: 'utf-8',
  }).trim();
}
