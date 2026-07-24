#!/usr/bin/env node
'use strict';

const fs = require('node:fs');
const { appendTimelineEvent } = require('./timeline');

const [timelinePath, secretsPath, donePath] = process.argv.slice(2);
if (!timelinePath || !secretsPath || !donePath) throw new Error('timeline, secrets, and done paths are required');
const secrets = JSON.parse(fs.readFileSync(secretsPath, 'utf8')).filter(Boolean);
const retainedLength = Math.max(0, ...secrets.map((value) => value.length));
let retained = '';

function redact(value) {
  return secrets.reduce((text, secret) => text.split(secret).join('<redacted-application-key>'), value);
}

process.stdin.setEncoding('utf8');
process.stdin.on('data', (chunk) => {
  retained += chunk;
  let visible = '';
  while (retained.length > retainedLength) {
    const secret = secrets.find((value) => retained.startsWith(value));
    if (secret) {
      visible += '<redacted-application-key>';
      retained = retained.slice(secret.length);
    } else {
      visible += retained[0];
      retained = retained.slice(1);
    }
  }
  if (visible) appendTimelineEvent(timelinePath, 'tmux_output', {
    source: 'tmux-pipe-pane',
    text: visible,
  });
});
process.stdin.on('end', () => {
  if (retained) appendTimelineEvent(timelinePath, 'tmux_output', {
    source: 'tmux-pipe-pane',
    text: redact(retained),
  });
  fs.writeFileSync(donePath, 'done\n', { mode: 0o600 });
});
