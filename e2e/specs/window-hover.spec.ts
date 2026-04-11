import * as path from 'path';
import { start } from '../../index';
import { moveTo, clickAt } from '../helpers/mouse';
import { openFixture, closeAllFixtures } from '../helpers/window';

const fixturePath = path.resolve(__dirname, '../fixtures/sample-window.html');

describe('window hover', () => {
  afterEach(() => closeAllFixtures());

  test('hovering fixture window triggers windowHovered', (done) => {
    openFixture(fixturePath);
    const session = start({ savePath: '/tmp/e2e-hover.png' });

    session.on('started', () => {
      setTimeout(() => moveTo(400, 350), 500);
      setTimeout(() => clickAt(400, 350), 1200);
    });

    session.on('windowHovered', (e) => {
      expect(e.window).toBeDefined();
    });

    session.on('regionSelected', (e) => {
      expect(e.rect.w).toBeGreaterThan(0);
      session.cancel();
      closeAllFixtures();
      done();
    });

    session.on('error', (e) => done(e));
  }, 15000);
});
