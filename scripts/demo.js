const { start } = require('../dist/lib.js');

const args = process.argv.slice(2);
const mode = args[0] || 'interactive';

const configs = {
  interactive: {
    savePath: '/tmp/screenshot-demo.png',
    format: 'png',
    quality: 90,
    defaultColor: '#ff0000',
    defaultSize: 3,
  },
  jpg: {
    savePath: '/tmp/screenshot-demo.jpg',
    format: 'jpg',
    quality: 85,
    defaultColor: '#00ff00',
    defaultSize: 5,
  },
  clipboard: {
    savePath: '/tmp/screenshot-demo.png',
    format: 'png',
    quality: 90,
    defaultColor: '#0000ff',
    defaultSize: 4,
  },
};

const config = configs[mode] || configs.interactive;

console.log('\n=== Screenshot Demo ===');
console.log('Mode:', mode);
console.log('Config:', JSON.stringify(config, null, 2));
console.log('\nControls:');
console.log('  Mouse drag     - Select region / draw shape');
console.log('  Mouse move     - Window hover highlight (blue border)');
console.log('  Mouse double-click - Select hovered window directly');
console.log('  1-6            - Switch tool (Rect/Ellipse/Arrow/Brush/Mosaic/Text)');
console.log('  Cmd+Z          - Undo');
console.log('  Cmd+Shift+Z    - Redo');
console.log('  C              - Copy to clipboard (in editing mode)');
console.log('  Enter          - Save');
console.log('  Esc            - Cancel');
console.log('\nStarting overlay...\n');

try {
  const result = start(config);
  console.log('\nResult:', result);
} catch (err) {
  console.error('\nError:', err.message);
  process.exit(1);
}
