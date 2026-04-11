import * as path from 'path';
import { runAppleScript } from './platform';

const SCRIPT_DIR = path.join(__dirname, '../scripts');

export function moveTo(x: number, y: number): void {
  runAppleScript(path.join(SCRIPT_DIR, 'move_mouse.scpt'), [String(x), String(y)]);
}

export function clickAt(x: number, y: number): void {
  runAppleScript(path.join(SCRIPT_DIR, 'click.scpt'), [String(x), String(y)]);
}

export function drag(x1: number, y1: number, x2: number, y2: number): void {
  runAppleScript(path.join(SCRIPT_DIR, 'drag.scpt'), [String(x1), String(y1), String(x2), String(y2)]);
}
