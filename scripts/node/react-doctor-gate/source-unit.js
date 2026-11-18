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

// A finite witness uses values of shared inputs, never independent truth values
// for comparisons. In particular x === y and x !== y remain correlated.
// Callers supply ownership, temporal identity and admissible input domains.
function finiteValueProof(context) {
  const unknown = Symbol('unproved value');
  const identities = [Symbol('identity A'), Symbol('identity B')];
  const numbers = new Set([-1, 0, 1, 2]);
  const constants = new Set();
  const inputs = new Map();
  const tick = () => {
    if (++context.steps > 20000) throw new Error('semantic proof budget exceeded');
  };
  const constant = (value) => {
    constants.add(value);
    if (typeof value === 'number' && Number.isFinite(value)) {
      numbers.add(value);
      numbers.add(value - 1);
      numbers.add(value + 1);
    }
    return { kind: 'constant', value };
  };
  const input = (key, domain) => {
    if (!inputs.has(key)) inputs.set(key, domain);
    return { kind: 'input', key };
  };
  const calculate = (operator, values) => {
    if (values.includes(unknown)) return unknown;
    const [a, b] = values;
    switch (operator) {
      case '!': return !a;
      case '&&': return a && b;
      case '||': return a || b;
      case '===': return a === b;
      case '!==': return a !== b;
      case '==': return a == b; // Deliberately model the source operator.
      case '!=': return a != b;
      case '<': case '<=': case '>': case '>=':
        if (!((typeof a === 'number' && typeof b === 'number')
          || (typeof a === 'string' && typeof b === 'string'))) return unknown;
        return operator === '<' ? a < b : operator === '<=' ? a <= b : operator === '>' ? a > b : a >= b;
      case '+': case '-': case '*': case '/': case '%':
        if (!values.every((value) => typeof value === 'number' && Number.isFinite(value))) return unknown;
        if (values.length === 1) return operator === '-' ? -a : operator === '+' ? +a : unknown;
        return operator === '+' ? a + b : operator === '-' ? a - b
          : operator === '*' ? a * b : operator === '/' ? a / b : a % b;
      default: return unknown;
    }
  };
  const operation = (operator, ...args) => {
    if (args.some((arg) => !arg)) return null;
    if (args.every((arg) => arg.kind === 'constant')) {
      const value = calculate(operator, args.map((arg) => arg.value));
      return value === unknown ? null : constant(value);
    }
    return { kind: 'operation', operator, args };
  };
  function expression(unit, node, resolve, depth = 0, seen = new Set()) {
    tick();
    if (depth > 12) return null;
    node = unwrap(node);
    if (!node) return null;
    if (node.type === 'Literal') return constant(node.value);
    const resolved = resolve?.(node, depth, seen);
    if (resolved !== undefined) return resolved;
    const child = (value, nextSeen = seen) => expression(unit, value, resolve, depth + 1, nextSeen);
    if (node.type === 'Identifier') {
      const variable = unit.binding(node);
      if (!immutable(variable) || seen.has(variable)) return null;
      const definition = variable.defs[0];
      return definition.type === 'Variable' && definition.node.id.type === 'Identifier'
        ? child(definition.node.init, new Set([...seen, variable])) : null;
    }
    if (node.type === 'UnaryExpression' && ['!', '+', '-'].includes(node.operator)) {
      return operation(node.operator, child(node.argument));
    }
    if (node.type === 'LogicalExpression' && ['&&', '||'].includes(node.operator)) {
      const left = child(node.left);
      // Constants can prove short circuits without assigning an unknown value.
      if (left?.kind === 'constant' && (node.operator === '&&' ? !left.value : left.value)) return left;
      return operation(node.operator, left, child(node.right));
    }
    if (node.type === 'BinaryExpression'
      && ['===', '!==', '==', '!=', '<', '<=', '>', '>=', '+', '-', '*', '/', '%'].includes(node.operator)) {
      return operation(node.operator, child(node.left), child(node.right));
    }
    return null;
  }
  function read(node, values) {
    tick();
    if (node.kind === 'constant') return node.value;
    if (node.kind === 'input') return values.get(node.key);
    const first = read(node.args[0], values);
    if (first === unknown) return unknown;
    if (node.operator === '&&' && !first || node.operator === '||' && first) return first;
    return calculate(node.operator, [first, ...node.args.slice(1).map((arg) => read(arg, values))]);
  }
  function witness(expressions, accepts) {
    if (expressions.some((node) => !node)) return false;
    const keys = new Set();
    const collect = (node, depth = 0) => {
      tick();
      if (depth > 24) return false;
      if (node.kind === 'input') keys.add(node.key);
      return keys.size <= 8 && (node.kind !== 'operation' || node.args.every((arg) => collect(arg, depth + 1)));
    };
    if (!expressions.every((node) => collect(node))) return false;
    const ordered = [...keys];
    const values = new Map();
    const defaults = [...new Set([false, true, null, undefined, ...numbers, ...identities, ...constants])];
    const visit = (index) => {
      tick();
      if (index === ordered.length) {
        const results = expressions.map((node) => read(node, values));
        return !results.includes(unknown) && accepts(results);
      }
      const key = ordered[index];
      for (const value of inputs.get(key) ?? defaults) {
        values.set(key, value);
        if (visit(index + 1)) return true;
      }
      return false;
    };
    return visit(0);
  }
  return { constant, input, operation, expression, witness };
}

module.exports = { parseUnit, unwrap, memberName, immutable, importOrigin, importedCall, finiteValueProof };
