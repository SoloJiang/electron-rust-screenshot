import { runScreenshot } from '../helpers/runner';
import { moveTo, clickAt } from '../helpers/mouse';
import { keyPress } from '../helpers/keyboard';

describe('performance', () => {
  test('overlay starts and cancels without error', async () => {
    const result = await runScreenshot(
      { savePath: '/tmp/e2e-perf.png' },
      async () => {
        moveTo(400, 400);
        await new Promise((r) => setTimeout(r, 500));
        clickAt(400, 400);
        await new Promise((r) => setTimeout(r, 200));
        keyPress('esc');
      },
      15000
    );
    expect(result.type).toBe('cancelled');
  }, 20000);
});
