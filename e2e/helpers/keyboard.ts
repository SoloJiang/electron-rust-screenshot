import * as path from 'path';
import { runAppleScript } from './platform';

const SCRIPT_DIR = path.join(__dirname, '../scripts');

export function keyPress(key: string): void {
  runAppleScript(path.join(SCRIPT_DIR, 'keypress.scpt'), [key]);
}
