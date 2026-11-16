const fs = require('node:fs');
const path = require('node:path');
const { createRequire } = require('node:module');

const TYPE_WRAPPERS = new Set(['ChainExpression', 'TSAsExpression', 'TSNonNullExpression', 'TSTypeAssertion', 'ParenthesizedExpression', 'TSSatisfiesExpression']);
let parser;

const unwrap = (node) => {
  while (node && TYPE_WRAPPERS.has(node.type)) node = node.expression;
  return node;
};
const memberName = (node) => {
  node = unwrap(node);
  if (node?.type !== 'MemberExpression') return null;
  return !node.computed && node.property.type === 'Identifier'
    ? node.property.name : node.property.type === 'Literal' && typeof node.property.value === 'string'
      ? node.property.value : null;
};
const immutable = (variable) => variable?.defs.length === 1
  && variable.references.every((reference) => !reference.isWrite() || reference.init);

function parseUnit(repoRoot, file) {
  const absolute = path.resolve(repoRoot, file);
  const real = fs.realpathSync(absolute);
  if (!real.startsWith(`${fs.realpathSync(repoRoot)}${path.sep}`)) return null;
  const source = fs.readFileSync(absolute, 'utf8');
  if (source.length > 512 * 1024) return null;
  parser ||= createRequire(path.resolve(__dirname, '../../../web/package.json'))('typescript-eslint').parser;
  const { ast, scopeManager, visitorKeys } = parser.parseForESLint(source, {
    filePath: file, loc: true, range: true, ecmaVersion: 'latest',
    sourceType: 'module', ecmaFeatures: { jsx: true },
  });
  const parents = new Map();
  const nodes = [];
  const stack = [[ast, null]];
  while (stack.length) {
    if (nodes.length >= 100000) return null;
    const [node, parent] = stack.pop();
    parents.set(node, parent);
    nodes.push(node);
    for (const key of visitorKeys[node.type] || []) {
      const value = node[key];
      for (const child of Array.isArray(value) ? value : [value]) {
        if (child?.type) stack.push([child, node]);
      }
    }
  }
  const bindings = new Map();
  for (const scope of scopeManager.scopes) {
    for (const variable of scope.variables) {
      for (const identifier of variable.identifiers) bindings.set(identifier, variable);
      for (const reference of variable.references) bindings.set(reference.identifier, reference.resolved);
    }
    for (const reference of scope.references) bindings.set(reference.identifier, reference.resolved);
  }
  const unit = { file, source, ast, nodes, parents, bindings, visitorKeys };
  unit.binding = (node) => bindings.get(unwrap(node));
  return unit;
}

function importOrigin(unit, node, seen = new Set()) {
  node = unwrap(node);
  if (node?.type !== 'Identifier') return null;
  const variable = unit.binding(node);
  if (!immutable(variable) || seen.has(variable)) return null;
  seen.add(variable);
  const definition = variable.defs[0];
  if (definition.type === 'ImportBinding') {
    const specifier = definition.node;
    const declaration = unit.parents.get(specifier);
    if (declaration?.type !== 'ImportDeclaration') return null;
    return { module: declaration.source.value,
      name: specifier.type === 'ImportSpecifier' ? specifier.imported.name ?? specifier.imported.value
        : specifier.type === 'ImportNamespaceSpecifier' ? '*' : 'default' };
  }
  if (definition.type === 'Variable' && definition.node.id.type === 'Identifier') {
    return importOrigin(unit, definition.node.init, seen);
  }
  return null;
}

function importedCall(unit, node, module, name) {
  node = unwrap(node);
  if (node?.type !== 'CallExpression') return false;
  const callee = unwrap(node.callee);
  if (callee.type === 'Identifier') {
    const origin = importOrigin(unit, callee);
    return origin?.module === module && origin.name === name;
  }
  if (memberName(callee) !== name) return false;
  const origin = importOrigin(unit, callee.object);
  return origin?.module === module && ['*', 'default'].includes(origin.name);
}

module.exports = { parseUnit, unwrap, memberName, immutable, importOrigin, importedCall };
