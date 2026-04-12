import { runScreenshot } from '../helpers/runner';
import { clickAt } from '../helpers/mouse';
import { keyPress } from '../helpers/keyboard';

describe('capture fullscreen', () => {
  test('start and cancel with ESC returns cancelled', async () => {
    const result = await runScreenshot(
      { savePath: '/tmp/e2e-test.png' },
      async () => {
        clickAt(500, 500);
        await new Promise((r) => setTimeout(r, 200));
        keyPress('esc');
      },
      10000
    );
    expect(result.type).toBe('cancelled');
  }, 15000);
});
