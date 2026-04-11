import { runScreenshot } from '../helpers/runner';
import { drag } from '../helpers/mouse';

describe('free select', () => {
  test('drag creates regionSelected and saved', async () => {
    const result = await runScreenshot(
      { savePath: '/tmp/e2e-drag.png' },
      () => {
        drag(300, 300, 500, 500);
      },
      15000
    );
    expect(result.type).toBe('saved');
  }, 20000);
});
