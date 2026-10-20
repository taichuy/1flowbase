const path = require('node:path');
const { createRequire } = require('node:module');
const { maskRustText } = require('../check-rust-backend/source.js');

const PROCESS_METHODS = new Set([
  'exec', 'execSync', 'execFile', 'execFileSync', 'spawn', 'spawnSync', 'fork',
]);
const CHILD_PROCESS_MODULES = new Set(['child_process', 'node:child_process']);
let parser;

function propertyName(node) {
  if (!node.computed && node.property?.type === 'Identifier') return node.property.name;
  return node.property?.type === 'Literal' ? node.property.value : null;
}

function scanJavaScript(source, file) {
  parser ||= createRequire(path.resolve(__dirname, '../../../web/package.json'))('typescript-eslint').parser;
  const { ast, scopeManager, visitorKeys } = parser.parseForESLint(source, {
    filePath: file,
    loc: true,
    range: true,
    ecmaVersion: 'latest',
    sourceType: 'module',
    ecmaFeatures: { jsx: true },
  });
  const references = new Map();
  for (const scope of scopeManager.scopes) {
    for (const reference of scope.references) references.set(reference.identifier, reference.resolved);
  }
  const resolve = (node, seen = new Set()) => {
    if (!node) return null;
    if (['ChainExpression', 'TSAsExpression', 'TSNonNullExpression', 'TSTypeAssertion'].includes(node.type)) {
      return resolve(node.expression, seen);
    }
    if (node.type === 'CallExpression' && node.callee.type === 'Identifier'
      && node.callee.name === 'require' && !references.get(node.callee)
      && CHILD_PROCESS_MODULES.has(node.arguments[0]?.value)) return 'namespace';
    if (node.type === 'MemberExpression') {
      const member = propertyName(node);
      return resolve(node.object, seen) === 'namespace' && PROCESS_METHODS.has(member) ? member : null;
    }
    if (node.type !== 'Identifier') return null;
    const variable = references.get(node);
    if (!variable || seen.has(variable)) return null;
    seen.add(variable);
    // A reassigned binding no longer establishes which API the call targets.
    if (variable.references.some((reference) => reference.isWrite() && !reference.init)) return null;
    if (variable.defs.length !== 1) return null;
    const definition = variable.defs[0];
    if (definition.type === 'ImportBinding') {
      if (definition.node.type === 'TSImportEqualsDeclaration') {
        return CHILD_PROCESS_MODULES.has(definition.node.moduleReference.expression?.value) ? 'namespace' : null;
      }
      if (!CHILD_PROCESS_MODULES.has(definition.parent.source?.value)) return null;
      const specifier = definition.node;
      if (specifier.type !== 'ImportSpecifier') return 'namespace';
      const name = specifier.imported.name || specifier.imported.value;
      return PROCESS_METHODS.has(name) ? name : null;
    }
    if (definition.type !== 'Variable') return null;
    const declaration = definition.node;
    const origin = resolve(declaration.init, seen);
    if (declaration.id.type === 'Identifier') return origin;
    if (origin !== 'namespace' || declaration.id.type !== 'ObjectPattern') return null;
    const entry = declaration.id.properties.find((property) => property.type === 'Property'
      && property.value.type === 'Identifier' && property.value.name === variable.name);
    const name = entry && (entry.computed ? entry.key.value : entry.key.name || entry.key.value);
    return PROCESS_METHODS.has(name) ? name : null;
  };
  const calls = [];
  const visit = (node) => {
    if (!node || typeof node.type !== 'string') return;
    if (node.type === 'CallExpression') {
      const method = resolve(node.callee);
      if (PROCESS_METHODS.has(method)) calls.push({
        startLine: node.loc.start.line, endLine: node.loc.end.line, api: `child_process.${method}`,
      });
    }
    for (const key of visitorKeys[node.type] || []) {
      const child = node[key];
      if (Array.isArray(child)) child.forEach(visit);
      else visit(child);
    }
  };
  visit(ast);
  return calls;
}

// Resolve the finite Rust use-tree syntax needed for std/tokio process imports.
// This is a static name heuristic, not a Rust type checker or macro expansion.
function expandUseTree(tree, prefix = '') {
  const open = tree.indexOf('{');
  if (open < 0) return [`${prefix}${tree.trim()}`];
  const base = `${prefix}${tree.slice(0, open).trim()}`;
  const inner = tree.slice(open + 1, tree.lastIndexOf('}'));
  const entries = [];
  let depth = 0;
  let start = 0;
  for (let index = 0; index <= inner.length; index += 1) {
    if (inner[index] === '{') depth += 1;
    if (inner[index] === '}') depth -= 1;
    if (index === inner.length || (inner[index] === ',' && depth === 0)) {
      const part = inner.slice(start, index).trim();
      if (part) entries.push(...expandUseTree(part, base));
      start = index + 1;
    }
  }
  return entries;
}

function scanRust(source) {
  const code = maskRustText(source);
  const scopes = [{ parent: null, bindings: new Map() }];
  const scopeAt = new Array(code.length + 1);
  const stack = [scopes[0]];
  for (let index = 0; index < code.length; index += 1) {
    scopeAt[index] = stack.at(-1);
    if (code[index] === '{') {
      const scope = { parent: stack.at(-1), bindings: new Map() };
      scopes.push(scope);
      stack.push(scope);
    } else if (code[index] === '}' && stack.length > 1) stack.pop();
  }
  const bind = (offset, name, origin) => scopeAt[offset].bindings.set(name, origin);
  for (const match of code.matchAll(/\buse\s+([^;]+);/gu)) {
    for (const entry of expandUseTree(match[1])) {
      const [imported, alias] = entry.replace(/\s*::\s*/gu, '::').split(/\s+as\s+/u);
      const name = alias || imported.split('::').at(-1);
      bind(match.index, name, /^(?:std|tokio)::process(?:::Command)?$/u.test(imported) ? imported : null);
    }
  }
  // Local declarations win over a process import in the same/child block.
  for (const match of code.matchAll(/\b(?:struct|enum|type|trait|mod|fn|let)\s+(?:mut\s+)?([A-Za-z_][A-Za-z_0-9]*)/gu)) {
    bind(match.index, match[1], null);
  }
  const lookup = (offset, name) => {
    for (let scope = scopeAt[offset]; scope; scope = scope.parent) {
      if (scope.bindings.has(name)) return scope.bindings.get(name);
    }
    return undefined;
  };
  const calls = [];
  const unresolved = [];
  for (const match of code.matchAll(/\b((?:[A-Za-z_][A-Za-z_0-9]*\s*::\s*)+)new\s*\(/gu)) {
    const segments = match[1].replace(/\s/gu, '').split('::').filter(Boolean);
    const binding = lookup(match.index, segments[0]);
    const qualified = binding === undefined ? segments.join('::') : binding && [binding, ...segments.slice(1)].join('::');
    const line = code.slice(0, match.index).split('\n').length;
    if (segments.length === 1 && segments[0] === 'Command' && binding === undefined) {
      unresolved.push({ startLine: line, endLine: line, api: 'Command::new (unresolved import or type)' });
    }
    if (!/^(?:std|tokio)::process::Command$/u.test(qualified || '')) continue;
    calls.push({ startLine: line, endLine: line + match[0].split('\n').length - 1, api: `${qualified}::new` });
  }
  return { calls, unresolved };
}

function scanProcessCalls(source, file) {
  try {
    if (/\.(?:[cm]?[jt]sx?)$/u.test(file)) return { calls: scanJavaScript(source, file) };
    if (/\.rs$/u.test(file)) return scanRust(source);
    return { calls: [] };
  } catch (error) {
    return { calls: [], error: `Process API scan incomplete: ${error.message}` };
  }
}

module.exports = { scanProcessCalls };
