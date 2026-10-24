const fs = require('node:fs');
const path = require('node:path');
const { StringDecoder } = require('node:string_decoder');
const { spawnSync } = require('node:child_process');
const { stripVTControlCharacters } = require('node:util');

// An I/O chunk size, not an output limit. Complete logs remain on disk.
const CHUNK_BYTES = 64 * 1024;

function copyLog(source, destination, writeOutput) {
  const input = fs.openSync(source, 'r');
  const buffer = Buffer.allocUnsafe(CHUNK_BYTES);
  const decoder = new StringDecoder('utf8');
  try {
    let length;
    while ((length = fs.readSync(input, buffer)) > 0) {
      if (destination !== undefined) fs.writeFileSync(destination, buffer.subarray(0, length));
      if (writeOutput) writeOutput(decoder.write(buffer.subarray(0, length)));
    }
    if (writeOutput) writeOutput(decoder.end());
  } finally {
    fs.closeSync(input);
  }
}

function runGateCommand({ commandSpec, env, logPath, spawnSyncImpl = spawnSync,
  writeStdout = (text) => fs.writeSync(process.stdout.fd, text),
  writeStderr = (text) => fs.writeSync(process.stderr.fd, text) }) {
  const temporary = fs.mkdtempSync(path.join(path.dirname(logPath), '.gate-stderr-'));
  const stderrPath = path.join(temporary, 'stderr.log');
  const output = fs.openSync(logPath, 'w');
  const errorOutput = fs.openSync(stderrPath, 'w');
  try {
    const result = spawnSyncImpl(commandSpec.command, commandSpec.args, {
      cwd: commandSpec.cwd,
      env,
      stdio: ['inherit', output, errorOutput],
    });
    // Injected executors may return captured output; native descriptor execution
    // returns null here and never retains a second copy of the child log.
    if (result.stdout) fs.writeFileSync(output, result.stdout);
    if (result.stderr) fs.writeFileSync(errorOutput, result.stderr);
    copyLog(logPath, undefined, writeStdout);
    copyLog(stderrPath, output, writeStderr);
    if (result.error || result.signal) {
      const diagnostic = `${result.error?.message || `Command terminated by ${result.signal}`}\n`;
      fs.writeFileSync(output, diagnostic);
      writeStderr(diagnostic);
    }
    return result.error ? 1 : (result.status ?? 1);
  } finally {
    fs.closeSync(output);
    fs.closeSync(errorOutput);
    fs.rmSync(temporary, { recursive: true, force: true });
  }
}

function combineGateLogs(logPath, artifacts) {
  const output = fs.openSync(logPath, 'w');
  try {
    for (const [index, artifact] of artifacts.entries()) {
      if (index > 0) fs.writeFileSync(output, '\n\n');
      fs.writeFileSync(output, `===== ${artifact.scope} (${artifact.artifactName}) =====\n`);
      const componentLogPath = path.join(artifact.artifactPath, 'quality-gate.latest.log');
      if (fs.existsSync(componentLogPath)) copyLog(componentLogPath, output);
    }
  } finally {
    fs.closeSync(output);
  }
}

function* readLogLines(logPath) {
  const input = fs.openSync(logPath, 'r');
  const buffer = Buffer.allocUnsafe(CHUNK_BYTES);
  const decoder = new StringDecoder('utf8');
  let fragments = [];
  try {
    let length;
    do {
      length = fs.readSync(input, buffer);
      const text = length ? decoder.write(buffer.subarray(0, length)) : decoder.end();
      let start = 0;
      for (let end = text.indexOf('\n'); end >= 0; end = text.indexOf('\n', start)) {
        fragments.push(text.slice(start, end));
        yield fragments.join('').replace(/\r$/u, '');
        fragments = [];
        start = end + 1;
      }
      if (start < text.length) fragments.push(text.slice(start));
    } while (length > 0);
    if (fragments.length > 0) yield fragments.join('');
  } finally {
    fs.closeSync(input);
  }
}

function readFailureExcerpt(logPath) {
  if (!fs.existsSync(logPath)) return '';
  // Retain only the existing 80-line presentation windows, rather than every
  // line in all component logs. The artifact itself is never truncated.
  const lineBudget = 80;
  let tail = [];
  let blanks = [];
  let previous = [];
  let failure;
  let panic;
  let summary;
  let index = 0;
  for (const raw of readLogLines(logPath)) {
    const line = stripVTControlCharacters(raw);
    if (!failure && line.trim() === 'failures:') failure = { index, lines: [] };
    if (!panic && /\bpanicked at\b/u.test(line)) {
      panic = { index: index - previous.length, lines: [...previous] };
    }
    if (!summary && /test result: FAILED/u.test(line)) summary = { index, line };
    for (const window of [failure, panic]) {
      if (window && window.lines.length < lineBudget) window.lines.push(line);
    }
    if (line.trim()) {
      tail = [...tail, ...blanks, line].slice(-lineBudget);
      blanks = [];
    } else {
      blanks.push(line);
      if (blanks.length > lineBudget) blanks.shift();
    }
    previous = [...previous, line].slice(-2);
    index += 1;
  }
  const selected = failure || panic;
  if (!selected) return tail.join('\n').trim();
  if (summary && summary.index >= selected.index + lineBudget) {
    return [...selected.lines.slice(0, lineBudget - 2), '...', summary.line].join('\n').trim();
  }
  return selected.lines.join('\n').trim();
}

module.exports = { runGateCommand, combineGateLogs, readFailureExcerpt };
