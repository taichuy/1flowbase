const fs = require('node:fs');
const path = require('node:path');

const SCANNED_ROOTS = [
  'web/app/src',
  'web/packages/ui',
];

function readLegacyBoundaryUtilityClassNames(repoRoot) {
  const inventoryPath = path.join(
    repoRoot,
    'web/packages/tailwindcss-catalog/src/inventory.ts'
  );
  const source = fs.readFileSync(inventoryPath, 'utf8');
  const match = source.match(
    /const LEGACY_INVENTORY_SOURCE = `([\s\S]*?)`;/u
  );
  if (!match) {
    throw new Error('Legacy Tailwind boundary inventory source is unavailable.');
  }
  return new Set(match[1].trim().split(/\s+/u));
}

function readStyleBoundaryImpactFiles(repoRoot) {
  const manifestPath = path.join(
    repoRoot,
    'web/app/src/style-boundary/scenario-manifest.json'
  );
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  return new Set(manifest.flatMap((scene) => scene.impactFiles));
}

function collectTailwindBoundaryViolations(
  files,
  utilityClassNames,
  styleBoundaryImpactFiles = new Set()
) {
  const violations = [];
  for (const file of files) {
    const normalizedPath = file.path.replaceAll('\\', '/');
    const source = file.content;
    const isTest = normalizedPath.includes('/_tests/') || normalizedPath.includes('.test.');
    const isStyleBoundaryFixture = normalizedPath.includes('/style-boundary/');
    const isLowCodeDiagnostic = normalizedPath.endsWith(
      '/shared/code-block/tailwind-utility-diagnostics.ts'
    );
    const isSourceDrivenCompiler = normalizedPath.endsWith(
      '/shared/code-block/native-react-executable-style.ts'
    );
    const isCssModule = normalizedPath.endsWith('.module.css');

    if (
      normalizedPath.startsWith('web/app/src/styles/') ||
      normalizedPath.startsWith('web/packages/ui/')
    ) {
      if (/tailwindcss|@apply\b/u.test(source)) {
        violations.push({
          path: normalizedPath,
          code: 'global-tailwind-entry',
          message: 'Host global and @1flowbase/ui styles must not load or author Tailwind.'
        });
      }
      continue;
    }

    if (
      normalizedPath.startsWith('web/app/src/') &&
      !isTest &&
      (!isStyleBoundaryFixture || isCssModule) &&
      !isLowCodeDiagnostic &&
      !isSourceDrivenCompiler
    ) {
      if (isCssModule) {
        if (/@import\s+['"]tailwindcss(?:\/[^'"]*)?['"]/u.test(source)) {
          violations.push({
            path: normalizedPath,
            code: 'module-full-tailwind-import',
            message: 'CSS Modules may reference Tailwind theme and use @apply, but must not import global utilities or Preflight.'
          });
        }
        if (
          /tailwindcss|@apply\b/u.test(source) &&
          !styleBoundaryImpactFiles.has(normalizedPath)
        ) {
          violations.push({
            path: normalizedPath,
            code: 'style-boundary-owner-missing',
            message: 'Tailwind CSS Modules must be mapped to an owning component/page style-boundary scene.'
          });
        }
      } else if (/tailwindcss|@apply\b/u.test(source)) {
        violations.push({
          path: normalizedPath,
          code: 'tailwind-owner-required',
          message: 'Main-repository Tailwind authoring is limited to colocated *.module.css files.'
        });
      }

      if (/\.[cm]?[jt]sx?$/u.test(normalizedPath)) {
        for (const token of readStaticClassNameTokens(source)) {
          if (utilityClassNames.has(token)) {
            violations.push({
              path: normalizedPath,
              code: 'direct-tailwind-utility',
              message: `Main-repository TSX must not use global Tailwind utility '${token}'; use colocated CSS Modules + @apply.`
            });
          }
        }
      }
    }
  }
  return violations;
}

function readStaticClassNameTokens(source) {
  const tokens = [];
  const pattern = /\bclassName\s*=\s*(?:\{\s*)?(['"`])([^'"`]*?)\1\s*\}?/gu;
  for (const match of source.matchAll(pattern)) {
    tokens.push(...(match[2] ?? '').trim().split(/\s+/u).filter(Boolean));
  }
  return tokens;
}

function collectRepositorySourceFiles(repoRoot) {
  return SCANNED_ROOTS.flatMap((relativeRoot) => {
    const absoluteRoot = path.join(repoRoot, relativeRoot);
    return walkFiles(absoluteRoot)
      .filter((filePath) => /\.(?:css|ts|tsx)$/u.test(filePath))
      .map((filePath) => ({
        path: path.relative(repoRoot, filePath).replaceAll(path.sep, '/'),
        content: fs.readFileSync(filePath, 'utf8')
      }));
  });
}

function walkFiles(root) {
  return fs.readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const entryPath = path.join(root, entry.name);
    return entry.isDirectory() ? walkFiles(entryPath) : [entryPath];
  });
}

async function main(_argv = [], deps = {}) {
  const repoRoot = deps.repoRoot || path.resolve(__dirname, '..', '..', '..');
  const files = deps.files || collectRepositorySourceFiles(repoRoot);
  const utilityClassNames =
    deps.utilityClassNames || readLegacyBoundaryUtilityClassNames(repoRoot);
  const styleBoundaryImpactFiles =
    deps.styleBoundaryImpactFiles || readStyleBoundaryImpactFiles(repoRoot);
  const violations = collectTailwindBoundaryViolations(
    files,
    utilityClassNames,
    styleBoundaryImpactFiles
  );
  if (violations.length > 0) {
    throw new Error(
      `Tailwind boundary failed:\n${violations
        .map((violation) => `${violation.code} ${violation.path}: ${violation.message}`)
        .join('\n')}`
    );
  }
  (deps.writeStdout || process.stdout.write.bind(process.stdout))(
    '[1flowbase-tailwind-boundary] PASS\n'
  );
  return 0;
}

module.exports = {
  collectTailwindBoundaryViolations,
  main,
  readLegacyBoundaryUtilityClassNames,
  readStyleBoundaryImpactFiles,
  readStaticClassNameTokens
};
