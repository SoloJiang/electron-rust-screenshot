const { EventEmitter } = require('events');
const native = require('./native.js');

function start(config = {}) {
  const session = native.start(config);
  const emitter = new EventEmitter();

  const interval = setInterval(() => {
    const raw = session.pollEvent();
    if (!raw) return;
    try {
      const evt = JSON.parse(raw);
      emitter.emit(evt.type, evt);
      if (evt.type === 'saved' || evt.type === 'cancelled' || evt.type === 'error') {
        clearInterval(interval);
      }
    } catch (e) {
      emitter.emit('error', e);
    }
  }, 16);

  emitter.cancel = () => {
    clearInterval(interval);
    session.cancel();
  };

  return emitter;
}

module.exports = { start };
