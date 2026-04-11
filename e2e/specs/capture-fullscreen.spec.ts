import { start } from '../../index';
import { keyPress } from '../helpers/keyboard';

describe('capture fullscreen', () => {
  test('start and cancel with ESC', (done) => {
    const session = start({ savePath: '/tmp/e2e-test.png' });
    session.on('started', () => {
      keyPress('esc');
    });
    session.on('cancelled', () => {
      done();
    });
    session.on('error', (e) => done(e));
  }, 10000);
});
