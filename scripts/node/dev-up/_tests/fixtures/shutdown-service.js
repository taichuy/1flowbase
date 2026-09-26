const fs = require('node:fs');
const { spawn } = require('node:child_process');

const [role, mode, root] = process.argv.slice(2);
const events = `${root}/events`;
const record = (event) => fs.appendFileSync(events, `${event}\n`);

if (role === 'child') {
  process.on('message', (message) => {
    if (message === 'stop') {
      record('child-graceful');
      process.exit(0);
    }
  });
  process.on('SIGTERM', () => {
    record('child-term');
    process.exit(0);
  });
  fs.writeFileSync(`${root}/child-ready`, String(process.pid));
  setInterval(() => {}, 1000);
} else {
  const child = spawn(process.execPath, [__filename, 'child', mode, root], {
    stdio: ['ignore', 'ignore', 'ignore', 'ipc'],
  });
  fs.writeFileSync(`${root}/ready`, JSON.stringify({ parent: process.pid, child: child.pid }));
  process.on('SIGTERM', () => {
    record('parent-term');
    if (mode === 'graceful') {
      child.send('stop');
      child.once('exit', () => {
        record('parent-exit');
        process.exit(0);
      });
    } else if (mode === 'orphan') {
      process.exit(0);
    }
  });
  setInterval(() => {}, 1000);
}
