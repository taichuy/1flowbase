const path = require('node:path');
const { createHash } = require('node:crypto');
const { parseUnit, unwrap, memberName, immutable, importedCall } = require('./source-unit.js');

const TOOL = 'react-doctor@0.2.16';
const PROVENANCE = {
  toolPackage: TOOL,
  packageIntegrity: 'sha512-o108QscqM/TlC1QmKgDORf/P53PeZ69nZbRsvy53rXkKvWdKZC1Y0u1JRPmThbTh6v+QpMispBwEnFHhlpZQ7w==',
  sourceCommit: '2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc',
};
const RULES = {
  'async-defer-await': {
    ruleSourceSha256: '144e708975fff242fadb84c0ac18a2fe8fc2f5617925ccbd28ef5e8629894c5e',
    references: ['https://github.com/millionco/react-doctor/pull/1272', 'https://github.com/millionco/react-doctor/pull/1896'],
  },
  'async-parallel': {
    ruleSourceSha256: '80e720fab08e07949dd2a08672937c7292b97a728b9759303520dd6942799550',
    references: ['https://github.com/millionco/react-doctor/pull/1288'],
  },
  'async-await-in-loop': {
    ruleSourceSha256: 'de85fed2c25b0d4bb26b1e9ef167c6e314e42bdc9000a146889659d31f0f0bc6',
    references: ['https://github.com/millionco/react-doctor/blob/2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc/packages/oxlint-plugin-react-doctor/src/plugin/rules/js-performance/async-await-in-loop.ts'],
  },
};
const DEFER_MESSAGE = "This await blocks the function before an early-return that doesn't use the awaited value, so the skip path waits for nothing. Move the await below the guard so it only runs when you need the data";
const FUNCTIONS = new Set(['FunctionDeclaration', 'FunctionExpression', 'ArrowFunctionExpression']);
const LOOPS = new Set(['ForStatement', 'ForInStatement', 'ForOfStatement', 'WhileStatement', 'DoWhileStatement']);

function walk(unit, root, context, visit, skipFunctions = false) {
  const stack = [root];
  while (stack.length) {
    if (++context.steps > 20000) throw new Error('semantic proof budget exceeded');
    const node = stack.pop();
    if (!node) continue;
    if (skipFunctions && node !== root && FUNCTIONS.has(node.type)) continue;
    if (visit(node) === false) continue;
    for (const key of unit.visitorKeys[node.type] || []) {
      const value = node[key];
      for (const child of Array.isArray(value) ? value : [value]) if (child?.type) stack.push(child);
    }
  }
}

function addProof(context, node) {
  context.evidence.set(node.range.join(':'), { start: node.range[0], end: node.range[1] });
}
function functionOwner(unit, node) {
  for (let parent = unit.parents.get(node); parent; parent = unit.parents.get(parent)) {
    if (FUNCTIONS.has(parent.type)) return parent;
  }
  return null;
}
function variableDeclaration(unit, definition) {
  const parent = unit.parents.get(definition.node);
  return parent?.type === 'VariableDeclaration' ? parent : definition.node;
}
function isRead(unit, identifier, variable) {
  return variable?.references.some((reference) => reference.identifier === identifier && reference.isRead());
}
function reads(unit, expression, variables, context) {
  let found = false;
  walk(unit, expression, context, (node) => {
    if (node.type !== 'Identifier') return;
    const variable = unit.binding(node);
    if (variables.has(variable) && isRead(unit, node, variable)) found = true;
  });
  return found;
}
function patternBindings(unit, pattern, context) {
  const bindings = new Set();
  walk(unit, pattern, context, (node) => {
    if (node.type === 'Identifier' && unit.binding(node)) bindings.add(unit.binding(node));
  });
  return bindings;
}
function refVariable(unit, expression, context, seen = new Set()) {
  expression = unwrap(expression);
  if (expression?.type !== 'Identifier') return null;
  const variable = unit.binding(expression);
  if (!immutable(variable) || seen.has(variable)) return null;
  seen.add(variable);
  const definition = variable.defs[0];
  if (definition.type !== 'Variable' || definition.node.id.type !== 'Identifier') return null;
  if (importedCall(unit, definition.node.init, 'react', 'useRef')) {
    addProof(context, definition.node.init);
    return variable;
  }
  return refVariable(unit, definition.node.init, context, seen);
}
function refRead(unit, expression, context) {
  expression = unwrap(expression);
  return memberName(expression) === 'current' ? refVariable(unit, expression.object, context) : null;
}

function nativeAbortOwner(unit, expression, context, seen = new Set(), depth = 0) {
  if (depth > 12) return false;
  expression = unwrap(expression);
  if (expression?.type === 'NewExpression') {
    const callee = unwrap(expression.callee);
    if (callee?.type !== 'Identifier' || callee.name !== 'AbortController' || unit.binding(callee)) return false;
    addProof(context, expression);
    return true;
  }
  if (expression?.type === 'Identifier') {
    const variable = unit.binding(expression);
    if (!immutable(variable) || seen.has(variable)) return false;
    seen.add(variable);
    const definition = variable.defs[0];
    return definition.type === 'Variable' && definition.node.id.type === 'Identifier'
      && nativeAbortOwner(unit, definition.node.init, context, seen, depth + 1);
  }
  const ref = refRead(unit, expression, context);
  if (!ref || seen.has(ref)) return false;
  seen.add(ref);
  const assignments = unit.nodes.filter((node) => node.type === 'AssignmentExpression'
    && node.operator === '=' && memberName(node.left) === 'current'
    && refVariable(unit, unwrap(node.left).object, context) === ref);
  if (!assignments.length || assignments.length > 8) return false;
  for (const assignment of assignments) {
    if (!nativeAbortOwner(unit, assignment.right, context, new Set(seen), depth + 1)) return false;
    addProof(context, assignment);
  }
  return true;
}
function liveRead(unit, expression, owner, context) {
  expression = unwrap(expression);
  if (refRead(unit, expression, context)) return true;
  if (memberName(expression) === 'aborted' && memberName(expression.object) === 'signal') {
    return nativeAbortOwner(unit, unwrap(expression.object).object, context);
  }
  if (expression?.type !== 'Identifier') return false;
  const variable = unit.binding(expression);
  if (!variable) return false;
  const writes = variable.references.filter((reference) => reference.isWrite() && !reference.init
    && functionOwner(unit, reference.identifier) !== owner);
  if (!writes.length) return false;
  for (const write of writes) {
    const parent = unit.parents.get(write.identifier);
    if (!['AssignmentExpression', 'UpdateExpression'].includes(parent?.type)) return false;
    addProof(context, parent);
  }
  return true;
}

// Resolve predicate code and binding ownership, never cancellation-like names.
function guardExpression(unit, expression, owner, context, seen = new Set(), depth = 0) {
  if (depth > 12) return null;
  expression = unwrap(expression);
  if (!expression) return null;
  if (expression.type === 'Literal') return { fresh: false };
  if (expression.type === 'Identifier' || expression.type === 'MemberExpression') {
    if (expression.type === 'MemberExpression') {
      const object = guardExpression(unit, expression.object, owner, context, seen, depth + 1);
      const key = !expression.computed || guardExpression(unit, expression.property, owner, context, seen, depth + 1);
      if (!object || !key) return null;
    }
    return { fresh: liveRead(unit, expression, owner, context) };
  }
  if (expression.type === 'UnaryExpression' && expression.operator === '!') {
    return guardExpression(unit, expression.argument, owner, context, seen, depth + 1);
  }
  if (expression.type === 'LogicalExpression' || expression.type === 'BinaryExpression') {
    if (expression.type === 'BinaryExpression' && !['===', '!==', '==', '!='].includes(expression.operator)) return null;
    const left = guardExpression(unit, expression.left, owner, context, seen, depth + 1);
    const right = guardExpression(unit, expression.right, owner, context, seen, depth + 1);
    return left && right ? { fresh: left.fresh || right.fresh } : null;
  }
  if (expression.type !== 'CallExpression' || expression.arguments.length) return null;
  const callee = unwrap(expression.callee);
  if (callee?.type !== 'Identifier') return null;
  const variable = unit.binding(callee);
  if (!immutable(variable) || seen.has(variable)) return null;
  const definition = variable.defs[0];
  const fn = definition.type === 'FunctionName' ? definition.node
    : definition.type === 'Variable' ? unwrap(definition.node.init) : null;
  if (!FUNCTIONS.has(fn?.type) || fn.async || fn.generator || fn.params.length) return null;
  const body = fn.body.type === 'BlockStatement'
    ? fn.body.body.length === 1 && fn.body.body[0].type === 'ReturnStatement' ? fn.body.body[0].argument : null
    : fn.body;
  const result = guardExpression(unit, body, owner, context, new Set([...seen, variable]), depth + 1);
  if (result?.fresh) addProof(context, variableDeclaration(unit, definition));
  return result;
}

function awaitedArgument(statement) {
  const expression = statement?.type === 'VariableDeclaration' && statement.declarations.length === 1
    ? unwrap(statement.declarations[0].init)
    : statement?.type === 'ExpressionStatement' ? unwrap(statement.expression) : null;
  return expression?.type === 'AwaitExpression' ? expression.argument : null;
}
function directStatement(unit, node) {
  for (let current = node; current; current = unit.parents.get(current)) {
    const parent = unit.parents.get(current);
    if (parent?.type === 'BlockStatement' && parent.body.includes(current)) return { statement: current, block: parent };
    if (FUNCTIONS.has(current.type)) return null;
  }
  return null;
}
function onlyReturns(statement) {
  if (statement?.type === 'ReturnStatement') return true;
  return statement?.type === 'BlockStatement' && statement.body.length === 1 && statement.body[0].type === 'ReturnStatement';
}
function completionGuard(unit, node, context) {
  const direct = directStatement(unit, node);
  if (!direct || !awaitedArgument(direct.statement)) return null;
  let index = direct.block.body.indexOf(direct.statement) + 1;
  while (awaitedArgument(direct.block.body[index])) index++;
  const guard = direct.block.body[index];
  if (guard?.type !== 'IfStatement' || guard.alternate || !onlyReturns(guard.consequent)) return null;
  const owner = functionOwner(unit, direct.statement);
  if (!owner?.async) return null;
  const proof = guardExpression(unit, guard.test, owner, context);
  if (!proof?.fresh) return null;
  addProof(context, direct.statement);
  addProof(context, guard);
  return { predicate: 'completion-time-guard', reason: 'The guard reads a proven mutable lifetime owner after suspension; moving it before the await would permit stale completion.' };
}
function dataDependency(unit, node, diagnostic, context) {
  const direct = directStatement(unit, node);
  if (!direct || direct.statement !== node) return null;
  const sequence = [];
  for (let index = direct.block.body.indexOf(node); awaitedArgument(direct.block.body[index]); index++) sequence.push(direct.block.body[index]);
  const message = `These ${sequence.length} sequential await statements run one after another even though they look independent, so the page waits longer than it needs to. Run them together with Promise.all() instead`;
  if (sequence.length < 3 || diagnostic.message !== message) return null;
  const earlier = new Map();
  for (const statement of sequence) {
    const argument = awaitedArgument(statement);
    if (reads(unit, argument, new Set(earlier.keys()), context)) {
      for (const [variable, declaration] of earlier) if (reads(unit, argument, new Set([variable]), context)) addProof(context, declaration);
      addProof(context, statement);
      return { predicate: 'await-data-dependency', reason: 'A later awaited operation reads the resolved binding from an earlier await, including a bare-await argument; the full sequence is not independent.' };
    }
    if (statement.type === 'VariableDeclaration') {
      for (const variable of patternBindings(unit, statement.declarations[0].id, context)) earlier.set(variable, statement);
    }
  }
  return null;
}
function containingLoop(unit, node) {
  for (let parent = unit.parents.get(node); parent; parent = unit.parents.get(parent)) {
    if (FUNCTIONS.has(parent.type)) return null;
    if (LOOPS.has(parent.type)) return parent;
  }
  return null;
}
function loopControl(unit, node, diagnostic, context) {
  if (node.type !== 'AwaitExpression') return null;
  const loop = containingLoop(unit, node);
  const labels = { ForStatement: 'for-loop', ForInStatement: 'for…in loop', ForOfStatement: 'for…of loop', WhileStatement: 'while-loop', DoWhileStatement: 'do-while loop' };
  if (!loop || diagnostic.message !== `This makes the ${labels[loop.type]} slow because each await runs one after another, so collect the independent calls & run them together with `await Promise.all(items.map(...))``) return null;
  const declaration = unit.parents.get(node);
  if (declaration?.type !== 'VariableDeclarator' || declaration.init !== node) return null;
  const dependencies = patternBindings(unit, declaration.id, context);
  if (!dependencies.size) return null;
  const candidates = [];
  walk(unit, loop.body, context, (candidate) => { candidates.push(candidate); }, true);
  for (let pass = 0; pass < 8; pass++) {
    const size = dependencies.size;
    for (const candidate of candidates) {
      if (candidate.type !== 'VariableDeclarator' || !candidate.init || !reads(unit, candidate.init, dependencies, context)) continue;
      for (const variable of patternBindings(unit, candidate.id, context)) dependencies.add(variable);
    }
    if (size === dependencies.size) break;
  }
  for (const exit of candidates) {
    if (!['ReturnStatement', 'BreakStatement'].includes(exit.type) || exit.label) continue;
    if (exit.type === 'BreakStatement') {
      let target = unit.parents.get(exit);
      while (target && !LOOPS.has(target.type) && target.type !== 'SwitchStatement') target = unit.parents.get(target);
      if (target !== loop) continue;
    }
    for (let guard = unit.parents.get(exit); guard && guard !== loop; guard = unit.parents.get(guard)) {
      if (guard.type !== 'IfStatement' || !reads(unit, guard.test, dependencies, context)) continue;
      addProof(context, node);
      addProof(context, guard);
      for (const candidate of candidates) {
        if (candidate.type === 'VariableDeclarator' && candidate.init && reads(unit, candidate.init, dependencies, context)) addProof(context, variableDeclaration(unit, { node: candidate }));
      }
      return { predicate: 'await-loop-control-dependency', reason: 'The awaited result controls termination of this loop or its function; scheduling future iterations concurrently would issue operations that the result may prohibit.' };
    }
  }
  return null;
}
function atDiagnostic(unit, diagnostic) {
  if (!Number.isInteger(diagnostic.line) || !Number.isInteger(diagnostic.column) || diagnostic.line < 1 || diagnostic.column < 1) return null;
  const lines = unit.source.split('\n');
  if (diagnostic.line > lines.length || diagnostic.column > lines[diagnostic.line - 1].length) return null;
  const offset = lines.slice(0, diagnostic.line - 1).reduce((sum, line) => sum + line.length + 1, 0) + diagnostic.column - 1;
  const wanted = diagnostic.rule === 'async-await-in-loop' ? 'AwaitExpression' : null;
  return unit.nodes.find((node) => node.range[0] === offset && (wanted ? node.type === wanted : ['VariableDeclaration', 'ExpressionStatement', 'AwaitExpression'].includes(node.type))) ?? null;
}
function createAsyncSemanticCorrections(repoRoot) {
  const units = new Map();
  return (diagnostic, toolPackage) => {
    if (toolPackage !== TOOL || diagnostic.plugin !== 'react-doctor' || !RULES[diagnostic.rule]) return null;
    if (diagnostic.rule === 'async-defer-await' && diagnostic.message !== DEFER_MESSAGE) return null;
    const file = path.posix.normalize(`web/app/${diagnostic.filePath}`);
    if (!file.startsWith('web/app/') || diagnostic.filePath.startsWith('/') || diagnostic.filePath.includes('\\')) return null;
    try {
      if (!units.has(file)) units.set(file, parseUnit(repoRoot, file));
      const unit = units.get(file);
      const node = unit && atDiagnostic(unit, diagnostic);
      if (!node) return null;
      const context = { steps: 0, evidence: new Map() };
      const proof = diagnostic.rule === 'async-defer-await' ? completionGuard(unit, node, context)
        : diagnostic.rule === 'async-parallel' ? dataDependency(unit, node, diagnostic, context)
          : loopControl(unit, node, diagnostic, context);
      if (!proof) return null;
      return { ...proof, provenance: { ...PROVENANCE, ...RULES[diagnostic.rule] },
        evidence: [{ file, sha256: createHash('sha256').update(unit.source).digest('hex'), spans: [...context.evidence.values()] }] };
    } catch { return null; }
  };
}

module.exports = { createAsyncSemanticCorrections };
