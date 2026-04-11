const { start } = require('./index');

const session = start({ savePath: '/tmp/smoke.png' });
session.on('started', (e) => {
  console.log('started', JSON.stringify(e));
  session.cancel();
});
session.on('cancelled', () => {
  console.log('cancelled');
  process.exit(0);
});
session.on('error', (e) => {
  console.error('error', e);
  process.exit(1);
});
setTimeout(() => process.exit(0), 3000);
