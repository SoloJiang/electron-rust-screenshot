import { runScreenshot } from '../helpers/runner';

describe('free select', () => {
  let prevMockDrag: string | undefined;

  beforeEach(() => {
    prevMockDrag = process.env.SCREENSHOT_TEST_MOCK_DRAG;
    delete process.env.SCREENSHOT_TEST_MOCK_DRAG_NO_SAVE;
  });

  afterEach(() => {
    if (prevMockDrag === undefined) {
      delete process.env.SCREENSHOT_TEST_MOCK_DRAG;
    } else {
      process.env.SCREENSHOT_TEST_MOCK_DRAG = prevMockDrag;
    }
    delete process.env.SCREENSHOT_TEST_MOCK_DRAG_NO_SAVE;
  });

  test('mock drag creates regionSelected and saved', async () => {
    process.env.SCREENSHOT_TEST_MOCK_DRAG = '300,300,500,500';
    const result = await runScreenshot(
      { savePath: '/tmp/e2e-drag.png' },
      async () => {
        // Wait for the mock drag to auto-save (5 s delay in the overlay)
        await new Promise((r) => setTimeout(r, 6500));
      },
      15000
    );
    expect(result.type).toBe('saved');
  }, 20000);
});
