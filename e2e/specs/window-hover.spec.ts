import * as path from 'path';
import { runScreenshot } from '../helpers/runner';
import { moveTo, clickAt } from '../helpers/mouse';
import { openFixture, closeAllFixtures } from '../helpers/window';
import { keyPress } from '../helpers/keyboard';

const fixturePath = path.resolve(__dirname, '../fixtures/sample-window.html');

describe('window hover', () => {
  afterEach(() => closeAllFixtures());

  test('hovering fixture window leads to cancelled via ESC', async () => {
    openFixture(fixturePath);
    const result = await runScreenshot(
      { savePath: '/tmp/e2e-hover.png' },
      async () => {
        moveTo(400, 350);
        await new Promise((r) => setTimeout(r, 600));
        clickAt(400, 350);
        await new Promise((r) => setTimeout(r, 600));
        keyPress('esc');
      },
      15000
    );
    expect(result.type).toBe('cancelled');
  }, 20000);
});
