const LEGACY_TYPESCRIPT_PEER_CONSUMERS = new Set([
  'typescript-eslint',
  '@typescript-eslint/eslint-plugin',
  '@typescript-eslint/parser',
  '@typescript-eslint/project-service',
  '@typescript-eslint/tsconfig-utils',
  '@typescript-eslint/type-utils',
  '@typescript-eslint/typescript-estree',
  '@typescript-eslint/utils',
  'ts-api-utils'
]);

// Pin TypeScript JS Compiler API peer consumers to 5.9.3 while workspace `tsc` runs native TypeScript 7.
function readPackage(pkg) {
  if (
    LEGACY_TYPESCRIPT_PEER_CONSUMERS.has(pkg.name) &&
    pkg.peerDependencies?.typescript
  ) {
    delete pkg.peerDependencies.typescript;
    if (pkg.peerDependenciesMeta) {
      delete pkg.peerDependenciesMeta.typescript;
    }
    pkg.dependencies = {
      ...pkg.dependencies,
      typescript: '5.9.3'
    };
  }
  return pkg;
}

module.exports = {
  hooks: {
    readPackage
  }
};
