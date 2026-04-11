import { start } from '../../index';
import { drag } from '../helpers/mouse';

describe('free select', () => {
  test('drag creates regionSelected', (done) => {
    const session = start({ savePath: '/tmp/e2e-drag.png' });
    session.on('started', () => {
      setTimeout(() => drag(300, 300, 500, 500), 500);
      setTimeout(() => {
        session.cancel();
      }, 1500);
    });
    session.on('regionSelected', (e) => {
      expect(e.rect.w).toBeGreaterThan(50);
      done();
    });
    session.on('cancelled', () => done());
    session.on('error', (e) => done(e));
  }, 15000);
});
