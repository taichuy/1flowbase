'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { spawn } = require('node:child_process');

function resources(text, platform) {
  if (platform === 'darwin') {
    const cpu = text.match(/([\d.]+)\s+real\s+([\d.]+)\s+user\s+([\d.]+)\s+sys/);
    const rss = text.match(/(\d+)\s+maximum resident set size/);
    return { cpu_seconds: cpu ? Number(cpu[2]) + Number(cpu[3]) : null, max_rss_bytes: rss ? Number(rss[1]) : null };
  }
  const user = text.match(/User time \(seconds\):\s*([\d.]+)/);
  const system = text.match(/System time \(seconds\):\s*([\d.]+)/);
  const rss = text.match(/Maximum resident set size \(kbytes\):\s*(\d+)/);
  return { cpu_seconds: user && system ? Number(user[1]) + Number(system[1]) : null, max_rss_bytes: rss ? Number(rss[1]) * 1024 : null };
}

async function measure(output, command) {
  if (!command.length) throw new Error('A command is required');
  fs.mkdirSync(path.dirname(output), { recursive: true });
  const log = output + '.log';
  fs.writeFileSync(log, '');
  const timed = process.platform !== 'win32' && fs.existsSync('/usr/bin/time');
  const executable = timed ? '/usr/bin/time' : command[0];
  const args = timed ? [process.platform === 'darwin' ? '-l' : '-v', ...command] : command.slice(1);
  const started = performance.now();
  return new Promise(resolve => {
    const child = spawn(executable, args, { stdio: ['inherit', 'inherit', 'pipe'] });
    child.stderr.on('data', bytes => {
      fs.appendFileSync(log, bytes);
      process.stderr.write(bytes);
    });
    let error;
    child.on('error', value => { error = value.message; });
    child.on('close', (code, signal) => {
      const report = {
        command, platform: process.platform, wall_seconds: (performance.now() - started) / 1000,
        exit_code: code, signal, error: error || null,
        ...resources(fs.readFileSync(log, 'utf8'), process.platform),
        rss_scope: 'platform time maximum RSS; not summed concurrent process-tree RSS',
      };
      fs.writeFileSync(output + '.json', JSON.stringify(report, null, 2) + '\n');
      resolve(code === 0 && !error ? 0 : code || 1);
    });
  });
}

if (require.main === module) {
  if (!process.argv[2]) throw new Error('Usage: measure.js <evidence-prefix> <command> [args...]');
  measure(process.argv[2], process.argv.slice(3)).then(code => { process.exitCode = code; }).catch(error => {
    console.error(error);
    process.exitCode = 1;
  });
}
module.exports = { measure, resources };
