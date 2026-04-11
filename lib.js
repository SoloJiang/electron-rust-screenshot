const native = require('./native.js');

function start(config = {}) {
  const raw = native.start(config);
  try {
    return JSON.parse(raw);
  } catch (e) {
    throw new Error('Failed to parse screenshot result: ' + raw);
  }
}

module.exports = { start };
