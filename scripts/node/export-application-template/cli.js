const fs = require('node:fs');
const path = require('node:path');
const { exportApplicationTemplate } = require('./core.js');
const USAGE = `首次：node scripts/node/export-application-template/cli.js --target /absolute/applications-demo/@org/name --selection /path/selection.json --name "模板名称" [--api-base-url http://127.0.0.1:7800]
更新：node scripts/node/export-application-template/cli.js --target /absolute/applications-demo/@org/name
选择字段：page_ids、application_ids、data_model_ids、mcp_instance_ids。凭据读取当前仓库 api/apps/api-server/.env，不保存在导出记录。`;
function argumentsOf(argv) {
  if (argv.includes('--help') || argv.includes('-h')) return { help: true };
  const allowed = new Set(['--target', '--selection', '--name', '--description', '--api-base-url', '--repo-root']);
  const values = {};
  for (let i = 0; i < argv.length; i += 2) {
    if (!allowed.has(argv[i]) || argv[i + 1] === undefined) throw new Error(`invalid argument ${argv[i]}\n${USAGE}`);
    values[argv[i]] = argv[i + 1];
  }
  if (!values['--target'] || !path.isAbsolute(values['--target'])) throw new Error(`--target must be absolute\n${USAGE}`);
  return { target: values['--target'], name: values['--name'], description: values['--description'],
    apiBaseUrl: values['--api-base-url'], repoRoot: values['--repo-root'],
    selection: values['--selection'] ? JSON.parse(fs.readFileSync(values['--selection'], 'utf8')) : undefined };
}
async function main(argv = process.argv.slice(2), dependencies) {
  const options = argumentsOf(argv);
  if (options.help) return process.stdout.write(`${USAGE}\n`);
  process.stdout.write(`${JSON.stringify(await exportApplicationTemplate(options, dependencies), null, 2)}\n`);
}
if (require.main === module) main().catch((error) => { process.stderr.write(`${error.message}\n`); process.exitCode = 1; });
module.exports = { argumentsOf, main };
