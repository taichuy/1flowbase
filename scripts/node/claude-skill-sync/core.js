const fs = require('node:fs');
const path = require('node:path');

const DEFAULT_SOURCE = path.join('.agents', 'skills');
const DEFAULT_TARGET = path.join('.claude', 'skills');
const SKILL_FILE_NAME = 'SKILL.md';
const SHARED_DIR_NAME = '_shared';

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function usage() {
  process.stdout.write(`用法：node scripts/node/cli/claude-skill-sync.js [选项]

默认行为：
  将 .agents/skills 下的技能转换到 .claude/skills/<技能名>/SKILL.md
  同时镜像 .agents/skills/_shared 到 .claude/skills/_shared，保证 skill 内 ../_shared 引用可解析

选项：
  --source <dir>  源目录，默认 .agents/skills
  --target <dir>  目标目录，默认 .claude/skills
  -h, --help      查看帮助
`);
}

function log(message) {
  process.stdout.write(`[1flowbase-claude-skill-sync] ${message}\n`);
}

function parseCliArgs(argv) {
  const options = {
    help: false,
    source: DEFAULT_SOURCE,
    target: DEFAULT_TARGET,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === '-h' || arg === '--help') {
      options.help = true;
      continue;
    }

    if (arg === '--source' || arg === '--target') {
      const value = argv[index + 1];
      if (!value || value.startsWith('-')) {
        throw new Error(`${arg} 缺少值`);
      }

      if (arg === '--source') {
        options.source = value;
      } else {
        options.target = value;
      }

      index += 1;
      continue;
    }

    throw new Error(`未知参数：${arg}`);
  }

  return options;
}

function normalizeNewlines(source) {
  return source.replace(/\r\n/gu, '\n');
}

function parseYamlInlineScalar(value, fieldName) {
  const trimmedValue = value.trim();
  if (!trimmedValue) {
    throw new Error(`skill front matter 缺少 ${fieldName}`);
  }

  if (
    (trimmedValue.startsWith('"') && trimmedValue.endsWith('"')) ||
    (trimmedValue.startsWith("'") && trimmedValue.endsWith("'"))
  ) {
    return trimmedValue.slice(1, -1);
  }

  return trimmedValue;
}

function extractFrontMatter(source) {
  const normalizedSource = normalizeNewlines(source);
  const match = normalizedSource.match(/^---\n([\s\S]*?)\n---\n?/u);
  if (!match) {
    throw new Error('SKILL.md 缺少合法的 YAML front matter');
  }

  return {
    frontMatter: match[1],
    body: normalizedSource.slice(match[0].length),
  };
}

function parseSkillMetadata(frontMatter) {
  const lines = frontMatter.split('\n');
  let name = '';
  let description = '';

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];

    if (line.startsWith('name:')) {
      name = parseYamlInlineScalar(line.slice('name:'.length), 'name');
      continue;
    }

    if (line.startsWith('description:')) {
      const rawDescription = line.slice('description:'.length).trim();

      if (!rawDescription) {
        throw new Error('skill front matter 缺少 description');
      }

      if (rawDescription === '|' || rawDescription === '>') {
        const descriptionLines = [];

        for (let blockIndex = index + 1; blockIndex < lines.length; blockIndex += 1) {
          const blockLine = lines[blockIndex];
          if (blockLine.startsWith('  ')) {
            descriptionLines.push(blockLine.slice(2));
            index = blockIndex;
            continue;
          }

          if (!blockLine) {
            descriptionLines.push('');
            index = blockIndex;
            continue;
          }

          break;
        }

        description = descriptionLines.join('\n').trimEnd();
      } else {
        description = parseYamlInlineScalar(rawDescription, 'description');
      }
    }
  }

  if (!name) {
    throw new Error('skill front matter 缺少 name');
  }

  if (!description) {
    throw new Error('skill front matter 缺少 description');
  }

  return { name, description };
}

function parseSkillSource(source) {
  const { frontMatter, body } = extractFrontMatter(source);
  const metadata = parseSkillMetadata(frontMatter);
  return {
    ...metadata,
    body,
  };
}

function formatDescriptionBlockScalar(description) {
  return description.split('\n').map((line) => `  ${line}`).join('\n');
}

function convertSkillSourceToClaudeSkill(source) {
  const { name, description, body } = parseSkillSource(source);
  const header = [
    '---',
    `name: ${name}`,
    'description: |',
    formatDescriptionBlockScalar(description),
    '---',
  ].join('\n');

  return `${header}\n${body}`;
}

function resolveWorkspacePaths(repoRoot, options) {
  return {
    sourceDir: path.resolve(repoRoot, options.source),
    targetDir: path.resolve(repoRoot, options.target),
  };
}

function listSkillFiles(sourceDir) {
  if (!fs.existsSync(sourceDir)) {
    throw new Error(`源目录不存在：${sourceDir}`);
  }

  const entries = fs.readdirSync(sourceDir, { withFileTypes: true });
  const skillFiles = entries
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(sourceDir, entry.name, SKILL_FILE_NAME))
    .filter((filePath) => fs.existsSync(filePath))
    .sort((left, right) => left.localeCompare(right));

  if (skillFiles.length === 0) {
    throw new Error(`未在源目录中找到任何 ${SKILL_FILE_NAME}：${sourceDir}`);
  }

  return skillFiles;
}

function copySkillSupportFiles(sourceSkillDir, targetSkillDir) {
  fs.rmSync(targetSkillDir, { recursive: true, force: true });
  fs.cpSync(sourceSkillDir, targetSkillDir, { recursive: true });
}

function syncSharedDir(sourceDir, targetDir) {
  const sourceSharedDir = path.join(sourceDir, SHARED_DIR_NAME);
  if (!fs.existsSync(sourceSharedDir)) {
    return null;
  }

  const targetSharedDir = path.join(targetDir, SHARED_DIR_NAME);
  copySkillSupportFiles(sourceSharedDir, targetSharedDir);
  return targetSharedDir;
}

function syncClaudeSkills({
  repoRoot = getRepoRoot(),
  source = DEFAULT_SOURCE,
  target = DEFAULT_TARGET,
} = {}) {
  const { sourceDir, targetDir } = resolveWorkspacePaths(repoRoot, { source, target });
  const skillFiles = listSkillFiles(sourceDir);
  const skillNames = [];

  for (const skillFile of skillFiles) {
    const skillSource = fs.readFileSync(skillFile, 'utf8');
    const convertedSkill = convertSkillSourceToClaudeSkill(skillSource);
    const { name } = parseSkillSource(skillSource);
    const sourceSkillDir = path.dirname(skillFile);
    const targetSkillDir = path.join(targetDir, name);
    const targetFile = path.join(targetSkillDir, SKILL_FILE_NAME);

    copySkillSupportFiles(sourceSkillDir, targetSkillDir);
    fs.writeFileSync(targetFile, convertedSkill, 'utf8');

    skillNames.push(name);
  }

  const sharedDir = syncSharedDir(sourceDir, targetDir);

  return {
    count: skillNames.length,
    skillNames,
    sharedDir,
    sourceDir,
    targetDir,
  };
}

async function main(argv = process.argv.slice(2)) {
  const options = parseCliArgs(argv);
  if (options.help) {
    usage();
    return 0;
  }

  const repoRoot = getRepoRoot();
  const result = syncClaudeSkills({
    repoRoot,
    source: options.source,
    target: options.target,
  });

  log(
    `已转换 ${result.count} 个 skill 到 ${path.relative(repoRoot, result.targetDir)}：${result.skillNames.join(
      ', '
    )}`
  );
  if (result.sharedDir) {
    log(`已镜像共享目录到 ${path.relative(repoRoot, result.sharedDir)}`);
  }
  return 0;
}

module.exports = {
  DEFAULT_SOURCE,
  DEFAULT_TARGET,
  SKILL_FILE_NAME,
  SHARED_DIR_NAME,
  convertSkillSourceToClaudeSkill,
  copySkillSupportFiles,
  extractFrontMatter,
  formatDescriptionBlockScalar,
  getRepoRoot,
  listSkillFiles,
  normalizeNewlines,
  parseCliArgs,
  parseSkillMetadata,
  parseSkillSource,
  resolveWorkspacePaths,
  syncClaudeSkills,
  syncSharedDir,
  main,
};
