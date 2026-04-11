import { start } from '../../index';
import { moveTo } from '../helpers/mouse';

describe('performance', () => {
  test('frame time and hit-test p99 under 16ms', (done) => {
    const session = start({ savePath: '/tmp/e2e-perf.png' });
    const metrics: any[] = [];
    session.on('metrics', (m) => metrics.push(m));

    session.on('started', () => {
      setTimeout(() => moveTo(400, 400), 500);
      setTimeout(() => {
        const bad = metrics.filter((m) => m.frameTimeMs > 16 || m.hitTestP99Ms > 16);
        expect(bad.length).toBeLessThan(metrics.length * 0.05);
        session.cancel();
      }, 2000);
    });

    session.on('cancelled', () => done());
    session.on('error', (e) => done(e));
  }, 15000);
});
