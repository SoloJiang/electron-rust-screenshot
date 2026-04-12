import { runScreenshot } from '../helpers/runner';
import * as mouse from '../helpers/mouse';
import * as keyboard from '../helpers/keyboard';

describe('selection resize and move', () => {
  jest.setTimeout(30000);

  it('can drag selection body to move it', async () => {
    const result = await runScreenshot(
      { savePath: '/tmp/e2e-move-test.png' },
      async () => {
        // Wait for overlay to fully appear and gain focus
        await new Promise((r) => setTimeout(r, 1500));
        // Free-select a region in the center of the main display
        await mouse.drag(500, 500, 700, 600);
        await new Promise((r) => setTimeout(r, 800));
        // Drag the selection body by 100px right
        await mouse.drag(600, 550, 700, 550);
        await new Promise((r) => setTimeout(r, 500));
        // Save
        await keyboard.keyPress('return');
        await new Promise((r) => setTimeout(r, 800));
      },
      25000
    );
    expect(result.type).toBe('saved');
  });

  it('can drag SE corner to enlarge selection', async () => {
    const result = await runScreenshot(
      { savePath: '/tmp/e2e-resize-test.png' },
      async () => {
        await new Promise((r) => setTimeout(r, 1500));
        // Free-select a small region
        await mouse.drag(500, 500, 600, 600);
        await new Promise((r) => setTimeout(r, 800));
        // Drag the SE corner handle outward by 100px
        await mouse.drag(600, 600, 700, 700);
        await new Promise((r) => setTimeout(r, 500));
        // Save
        await keyboard.keyPress('return');
        await new Promise((r) => setTimeout(r, 800));
      },
      25000
    );
    expect(result.type).toBe('saved');
  });
});
