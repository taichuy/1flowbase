import { quotePosix, quotePowerShell } from './command-quoting';

export function buildCodexCommands(
  instanceName: string,
  endpoint: string,
  apiKey: string
) {
  const serverTable = `mcp_servers.${JSON.stringify(instanceName)}`;
  const lines = [
    `[${serverTable}]`,
    `url = ${JSON.stringify(endpoint)}`,
    `[${serverTable}.http_headers]`,
    `Authorization = ${JSON.stringify(`Bearer ${apiKey}`)}`
  ];
  // remove parses TOML and preserves unrelated settings; add also attempts OAuth.
  const shellCommand = `( umask 077; codex_config_dir="\${CODEX_HOME:-$HOME/.codex}"; mkdir -p "$codex_config_dir" && codex mcp remove ${quotePosix(instanceName)} && printf '%s\\n' '' ${lines.map(quotePosix).join(' ')} >> "$codex_config_dir/config.toml" )`;
  const powerShellCommand = `& { $ErrorActionPreference = 'Stop'; $codexConfigDir = if ($env:CODEX_HOME) { $env:CODEX_HOME } else { Join-Path $HOME '.codex' }; [IO.Directory]::CreateDirectory($codexConfigDir) | Out-Null; codex mcp remove ${quotePowerShell(instanceName)}; if ($LASTEXITCODE -ne 0) { throw 'codex mcp remove failed' }; $codexConfigPath = Join-Path $codexConfigDir 'config.toml'; $codexConfigText = [Environment]::NewLine + (@(${lines.map(quotePowerShell).join(', ')}) -join [Environment]::NewLine) + [Environment]::NewLine; [IO.File]::AppendAllText($codexConfigPath, $codexConfigText, [Text.UTF8Encoding]::new($false)) }`;
  // CMD expands %, quotes and metacharacters before PowerShell sees them.
  // EncodedCommand transports the same script as UTF-16LE without that expansion.
  const encodedPowerShell = btoa(
    powerShellCommand
      .split('')
      .map((character) => {
        const code = character.charCodeAt(0);
        return String.fromCharCode(code & 0xff, code >> 8);
      })
      .join('')
  );

  return [
    { title: 'macOS / Linux Shell', language: 'bash', command: shellCommand },
    {
      title: 'Windows PowerShell',
      language: 'powershell',
      command: powerShellCommand
    },
    {
      title: 'Windows CMD',
      language: 'bat',
      command: `powershell -NoProfile -EncodedCommand ${encodedPowerShell}`
    }
  ];
}
