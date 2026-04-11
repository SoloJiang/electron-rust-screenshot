import * as path from 'path';
import { runAppleScript } from './platform';

const SCRIPT_DIR = path.join(__dirname, '../scripts');

export function openFixture(htmlPath: string): void {
  runAppleScript(path.join(SCRIPT_DIR, 'open_fixture_window.scpt'), [htmlPath]);
}

export function closeAllFixtures(): void {
  runAppleScript(path.join(SCRIPT_DIR, 'close_all_fixtures.scpt'));
}
