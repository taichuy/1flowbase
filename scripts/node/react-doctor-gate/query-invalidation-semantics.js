const path = require('node:path');
const { createHash } = require('node:crypto');
const { parseUnit, unwrap, memberName, immutable, importedCall } = require('./source-unit.js');

const TOOL = 'react-doctor@0.2.16';
const RULE = 'query-mutation-missing-invalidation';
const MESSAGE = 'useMutation with no cache update leaves your users looking at stale data after it runs.';
const COMMIT = '2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc';
const FUNCTIONS = new Set(['FunctionDeclaration', 'FunctionExpression', 'ArrowFunctionExpression']);
const PROVENANCE = {
  toolPackage: TOOL,
  packageIntegrity: 'sha512-o108QscqM/TlC1QmKgDORf/P53PeZ69nZbRsvy53rXkKvWdKZC1Y0u1JRPmThbTh6v+QpMispBwEnFHhlpZQ7w==',
  sourceCommit: COMMIT,
  ruleSourceSha256: 'e2803ee846be2117441a2edac81120838a0f62b0e3f306b088b262b3f2ffca9c',
  references: [
    `https://github.com/millionco/react-doctor/blob/${COMMIT}/packages/oxlint-plugin-react-doctor/src/plugin/rules/tanstack-query/${RULE}.ts`,
    'https://tanstack.com/query/latest/docs/framework/react/guides/invalidations-from-mutations',
  ],
};

function step(context) {
  if (++context.steps > 20000) throw new Error('query invalidation proof budget exceeded');
}
function evidence(context, node) {
  if (node?.range) context.spans.set(node.range.join(':'), { start: node.range[0], end: node.range[1] });
}
function importEvidence(context, node) {
  node = unwrap(node);
  if (node?.type === 'MemberExpression') node = unwrap(node.object);
  const variable = context.unit.binding(node);
  const definition = variable?.defs[0];
  if (definition?.type === 'ImportBinding') evidence(context, context.unit.parents.get(definition.node));
  else if (definition?.type === 'Variable') {
    evidence(context, definition.node);
    importEvidence(context, definition.node.init);
  }
}

// Resolve bindings, never a cache-like variable or helper name. External helper
// bodies and arbitrary hook wrappers are deliberately opaque in this adapter.
function localFunction(context, expression, seen = new Set(), depth = 0) {
  step(context);
  if (depth > 8) return null;
  expression = unwrap(expression);
  if (!expression) return null;
  if (FUNCTIONS.has(expression.type)) return expression.generator ? null : expression;
  if (importedCall(context.unit, expression, 'react', 'useCallback')) {
    if (expression.arguments.length !== 2 || !FUNCTIONS.has(unwrap(expression.arguments[0])?.type)) return null;
    importEvidence(context, expression.callee);
    evidence(context, expression);
    return localFunction(context, expression.arguments[0], seen, depth + 1);
  }
  if (expression.type !== 'Identifier') return null;
  const variable = context.unit.binding(expression);
  if (!immutable(variable) || seen.has(variable)) return null;
  seen.add(variable);
  const definition = variable.defs[0];
  if (definition.type === 'FunctionName') {
    evidence(context, definition.node);
    return localFunction(context, definition.node, seen, depth + 1);
  }
  if (definition.type !== 'Variable' || definition.node.id.type !== 'Identifier') return null;
  evidence(context, definition.node);
  return localFunction(context, definition.node.init, seen, depth + 1);
}

function clientOrigin(context, expression, seen = new Set(), depth = 0) {
  step(context);
  if (depth > 8) return null;
  expression = unwrap(expression);
  if (expression?.type !== 'Identifier') return null;
  const variable = context.unit.binding(expression);
  if (!immutable(variable) || seen.has(variable)) return null;
  seen.add(variable);
  const definition = variable.defs[0];
  if (definition.type !== 'Variable' || definition.node.id.type !== 'Identifier') return null;
  evidence(context, definition.node);
  if (importedCall(context.unit, definition.node.init, '@tanstack/react-query', 'useQueryClient')) {
    importEvidence(context, unwrap(definition.node.init).callee);
    evidence(context, definition.node.init);
    return variable;
  }
  return clientOrigin(context, definition.node.init, seen, depth + 1);
}

function clientWasPatched(context, origin) {
  for (const node of context.unit.nodes) {
    step(context);
    const target = unwrap(node.type === 'AssignmentExpression' ? node.left
      : node.type === 'UpdateExpression' ? node.argument : null);
    if (target?.type === 'MemberExpression' && clientOrigin(context, target.object) === origin) return true;
  }
  return false;
}

// An expression proves only an eagerly executed cache action. Function values,
// callbacks given to opaque APIs and optional/conditional calls prove nothing.
function expressionAction(context, expression, depth, settled = false) {
  step(context);
  if (depth > 8) return false;
  expression = unwrap(expression);
  if (!expression) return false;
  if (expression.type === 'AwaitExpression') return expressionAction(context, expression.argument, depth, true);
  if (expression.type === 'UnaryExpression' && expression.operator === 'void') {
    return expressionAction(context, expression.argument, depth);
  }
  if (expression.type === 'SequenceExpression') {
    return expression.expressions.map((item, index) => expressionAction(context, item, depth,
      settled && index === expression.expressions.length - 1)).some(Boolean);
  }
  if (expression.type === 'ConditionalExpression') {
    return expressionAction(context, expression.consequent, depth, settled)
      && expressionAction(context, expression.alternate, depth, settled);
  }
  if (expression.type !== 'CallExpression' || expression.optional
      || expression.arguments.some((argument) => argument.type === 'SpreadElement')) return false;
  const callee = unwrap(expression.callee);
  if (callee?.type === 'MemberExpression' && !callee.optional && memberName(callee) === 'invalidateQueries') {
    const origin = clientOrigin(context, callee.object);
    if (origin && !clientWasPatched(context, origin)) {
      evidence(context, expression);
      return true;
    }
    return false;
  }
  if (memberName(callee) === 'all' && unwrap(callee.object)?.type === 'Identifier'
      && callee.object.name === 'Promise' && !context.unit.binding(callee.object)
      && expression.arguments.length === 1 && unwrap(expression.arguments[0])?.type === 'ArrayExpression') {
    // Static items run eagerly. A spread's optional source invalidation cannot
    // stand in for an unconditional target action, so it supplies no proof.
    const actions = unwrap(expression.arguments[0]).elements.map((item) =>
      item?.type !== 'SpreadElement' && expressionAction(context, item, depth, settled));
    if (actions.some(Boolean)) {
      evidence(context, expression);
      return true;
    }
    return false;
  }
  const helper = localFunction(context, callee);
  if (!helper || (helper.async && !settled)) return false;
  if (context.activeFunctions.has(helper)) {
    context.cycle = true;
    return false;
  }
  const action = functionAction(context, helper, depth + 1);
  if (action) evidence(context, expression);
  return action;
}

const unique = (values) => [...new Set(values)];
function statements(context, body, active, depth) {
  const exits = [];
  for (const statement of body) {
    step(context);
    if (!active.length) break;
    if (statement.type === 'ExpressionStatement' || statement.type === 'VariableDeclaration') {
      const expressions = statement.type === 'ExpressionStatement' ? [statement.expression]
        : statement.declarations.map((declaration) => declaration.init);
      const action = expressions.map((expression) => expressionAction(context, expression, depth)).some(Boolean);
      active = active.map((prior) => prior || action);
    } else if (statement.type === 'ReturnStatement' || statement.type === 'ThrowStatement') {
      const action = statement.type === 'ReturnStatement' && expressionAction(context, statement.argument, depth, true);
      exits.push(...active.map((prior) => prior || action));
      active = [];
    } else if (statement.type === 'IfStatement') {
      const branch = (node) => node?.type === 'BlockStatement' ? node.body : node ? [node] : [];
      const yes = statements(context, branch(statement.consequent), [...active], depth);
      const no = statements(context, branch(statement.alternate), [...active], depth);
      if (!yes || !no) return null;
      exits.push(...yes.exits, ...no.exits);
      active = unique([...yes.active, ...no.active]);
    } else if (statement.type === 'BlockStatement') {
      const block = statements(context, statement.body, active, depth);
      if (!block) return null;
      exits.push(...block.exits);
      active = block.active;
    } else if (!['FunctionDeclaration', 'EmptyStatement'].includes(statement.type)) {
      return null;
    }
  }
  return { active: unique(active), exits: unique(exits) };
}

function functionAction(context, fn, depth) {
  step(context);
  if (depth > 8 || fn.generator || context.activeFunctions.has(fn)) return false;
  context.activeFunctions.add(fn);
  let action;
  if (fn.body.type === 'BlockStatement') {
    const result = statements(context, fn.body.body, [false], depth);
    const outcomes = result && [...result.active, ...result.exits];
    action = Boolean(outcomes?.length && outcomes.every(Boolean));
  } else {
    action = expressionAction(context, fn.body, depth, true);
  }
  context.activeFunctions.delete(fn);
  if (action) evidence(context, fn);
  return action;
}

function mutationAt(unit, diagnostic) {
  const { line, column } = diagnostic;
  if (!Number.isInteger(line) || !Number.isInteger(column) || line < 1 || column < 1) return null;
  const lines = unit.source.split('\n');
  if (line > lines.length || column > lines[line - 1].length) return null;
  const offset = lines.slice(0, line - 1).reduce((sum, text) => sum + text.length + 1, 0) + column - 1;
  const candidates = unit.nodes.filter((node) => node.type === 'CallExpression' && node.range[0] === offset
    && importedCall(unit, node, '@tanstack/react-query', 'useMutation'));
  return candidates.length === 1 ? candidates[0] : null;
}

function successCallback(options) {
  if (options?.type !== 'ObjectExpression') return null;
  const properties = new Map();
  for (const property of options.properties) {
    if (property.type !== 'Property' || property.kind !== 'init' || property.method || property.computed) return null;
    const name = property.key.type === 'Identifier' ? property.key.name : property.key.value;
    if (typeof name !== 'string' || properties.has(name)) return null;
    properties.set(name, property.value);
  }
  return properties.has('mutationFn') ? properties.get('onSuccess') : null;
}

function createQueryInvalidationSemanticCorrections(repoRoot) {
  const units = new Map();
  return (diagnostic, toolPackage) => {
    if (toolPackage !== TOOL || diagnostic.plugin !== 'react-doctor'
        || diagnostic.rule !== RULE || diagnostic.message !== MESSAGE) return null;
    if (typeof diagnostic.filePath !== 'string') return null;
    const file = path.posix.normalize(`web/app/${diagnostic.filePath}`);
    if (!file.startsWith('web/app/') || diagnostic.filePath.startsWith('/') || diagnostic.filePath.includes('\\')) return null;
    try {
      if (!units.has(file)) units.set(file, parseUnit(repoRoot, file));
      const unit = units.get(file);
      const mutation = unit && mutationAt(unit, diagnostic);
      if (!mutation || mutation.optional || mutation.arguments.length !== 1) return null;
      const options = unwrap(mutation.arguments[0]);
      const callback = successCallback(options);
      if (!callback) return null;
      const context = { unit, steps: 0, spans: new Map(), activeFunctions: new Set(), cycle: false };
      const fn = localFunction(context, callback);
      if (!fn || !functionAction(context, fn, 0) || context.cycle) return null;
      importEvidence(context, mutation.callee);
      evidence(context, options);
      evidence(context, mutation);
      return {
        predicate: 'mutation-success-invalidation',
        reason: 'The resolved onSuccess callback unconditionally invokes actual invalidateQueries on a TanStack useQueryClient result, through the inspected local helper chain. This proves a cache action exists; mutation-to-query key coverage is a separate contract.',
        provenance: PROVENANCE,
        evidence: [{ file, sha256: createHash('sha256').update(unit.source).digest('hex'), spans: [...context.spans.values()] }],
      };
    } catch {
      return null;
    }
  };
}

module.exports = { createQueryInvalidationSemanticCorrections };
