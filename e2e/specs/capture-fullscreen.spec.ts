import { runScreenshot } from '../helpers/runner';
import { keyPress } from '../helpers/keyboard';

describe('capture fullscreen', () => {
  test('start and cancel with ESC returns cancelled', async () => {
    const result = await runScreenshot(
      { savePath: '/tmp/e2e-test.png' },
      () => {
        keyPress('esc');
      },
      10000
    );
    expect(result.type).toBe('cancelled');
  }, 15000);
});
