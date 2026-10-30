import { execFileSync, spawnSync } from 'node:child_process';
import {
  chmodSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, expect, test } from 'vitest';
import { buildCodexCommands } from '../codex-commands';

const directories: string[] = [];
function temporaryDirectory() {
  const directory = mkdtempSync(join(tmpdir(), 'codex-command-'));
  directories.push(directory);
  return directory;
}
afterEach(() => {
  for (const directory of directories.splice(0)) {
    rmSync(directory, { recursive: true, force: true });
  }
});

test('all commands persist credentials without invoking add or login; CMD carries the exact PowerShell script', () => {
  const commands = buildCodexCommands(
    '实例😀',
    'https://example.com/mcp?value=%PATH%&x="test"',
    "key'$()\n"
  );
  const decoded = Buffer.from(
    commands[2].command.split(' ').at(-1)!,
    'base64'
  ).toString('utf16le');
  expect(decoded).toBe(commands[1].command);
  for (const command of [commands[0].command, decoded]) {
    expect(command).not.toMatch(/codex mcp (?:add|login)/);
    expect(command).toContain('CODEX_HOME');
    expect(command).toContain('http_headers');
    expect(command).not.toContain('bearer-token-env-var');
  }
});

test.skipIf(process.platform === 'win32')(
  'shell writes literal TOML and stops without touching the config when remove fails',
  () => {
    const root = temporaryDirectory();
    const bin = join(root, 'bin');
    mkdirSync(bin);
    const fakeCodex = join(bin, 'codex');
    writeFileSync(
      fakeCodex,
      '#!/bin/sh\n[ "$1" = mcp ] && [ "$2" = remove ] || exit 99\nexit "${CODEX_TEST_EXIT:-0}"\n'
    );
    chmodSync(fakeCodex, 0o755);
    const configDirectory = join(root, 'custom config');
    const env = {
      ...process.env,
      PATH: `${bin}:${process.env.PATH}`,
      CODEX_HOME: configDirectory
    };
    const endpoint = 'https://example.com/mcp?x=$(touch INJECTED)&q="quote"';
    const key = "key'$(touch INJECTED)\\\n";
    const command = buildCodexCommands('sample', endpoint, key)[0].command;
    execFileSync('bash', ['-c', command], { cwd: root, env });
    const path = join(configDirectory, 'config.toml');
    const content = readFileSync(path, 'utf8');
    expect(content).toBe(
      `\n[mcp_servers."sample"]\nurl = ${JSON.stringify(endpoint)}\n[mcp_servers."sample".http_headers]\nAuthorization = ${JSON.stringify(`Bearer ${key}`)}\n`
    );
    expect(spawnSync('test', ['-e', join(root, 'INJECTED')]).status).toBe(1);
    const failed = spawnSync('bash', ['-c', command], {
      env: { ...env, CODEX_TEST_EXIT: '42' }
    });
    expect(failed.status).toBe(42);
    expect(readFileSync(path, 'utf8')).toBe(content);
  }
);

// Opt in on a host with the real CLI: no requests or user config are involved.
test.skipIf(process.env.ONEFLOWBASE_CODEX_COMMAND_TEST !== '1')(
  'real Codex reads first install and repeated updates while preserving other global settings',
  () => {
    for (const customHome of [false, true]) {
      const root = temporaryDirectory();
      const env: NodeJS.ProcessEnv = { ...process.env, HOME: root };
      delete env.CODEX_HOME;
      const directory = customHome
        ? join(root, '自定义 config')
        : join(root, '.codex');
      if (customHome) env.CODEX_HOME = directory;
      const path = join(directory, 'config.toml');
      const name = '1flowbase';
      const endpoint =
        'http://127.0.0.1:1/api/mcp/1flowbase?x="quoted"&y=%PATH%';
      const key = "test-'literal-$()-\\";
      const command = buildCodexCommands(name, endpoint, key)[0].command;
      const run = () =>
        execFileSync('bash', ['-c', command], {
          cwd: root,
          env,
          timeout: 15000
        });
      const get = (server: string) =>
        JSON.parse(
          execFileSync('codex', ['mcp', 'get', server, '--json'], {
            cwd: root,
            env,
            timeout: 15000,
            encoding: 'utf8'
          })
        );
      run();
      expect(get(name).transport).toMatchObject({
        url: endpoint,
        http_headers: { Authorization: `Bearer ${key}` },
        bearer_token_env_var: null
      });
      writeFileSync(
        path,
        `model = "preserved-model"\n[mcp_servers.other]\nurl = "http://127.0.0.1:1/other"\n[mcp_servers."1flowbase"]\nurl = "http://127.0.0.1:1/old"\n[mcp_servers."1flowbase".http_headers]\nAuthorization = "Bearer old"\n`
      );
      run();
      run();
      expect(get(name).transport).toMatchObject({
        url: endpoint,
        http_headers: { Authorization: `Bearer ${key}` }
      });
      expect(get('other').transport.url).toBe('http://127.0.0.1:1/other');
      expect(readFileSync(path, 'utf8')).toContain('model = "preserved-model"');
      expect(
        readFileSync(path, 'utf8').match(/\[mcp_servers\."1flowbase"\]/g)
      ).toHaveLength(1);
      writeFileSync(path, 'invalid = [');
      expect(() => run()).toThrow();
      expect(readFileSync(path, 'utf8')).toBe('invalid = [');
    }
  },
  60000
);
