const path = require('node:path');
const { createHash } = require('node:crypto');
const { parseUnit, unwrap, memberName, immutable, importedCall, finiteValueProof } = require('./source-unit.js');

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

function constantValue(unit, node, seen = new Set(), depth = 0) {
  node = unwrap(node);
  if (!node || depth > 12) return { known: false };
  if (node.type === 'Literal') return { known: true, value: node.value };
  if (node.type === 'UnaryExpression' && ['!', '+', '-'].includes(node.operator)) {
    const argument = constantValue(unit, node.argument, seen, depth + 1);
    if (!argument.known) return argument;
    return { known: true, value: node.operator === '!' ? !argument.value
      : node.operator === '-' ? -argument.value : +argument.value };
  }
  if (node.type !== 'Identifier') return { known: false };
  const variable = unit.binding(node);
  if (!immutable(variable) || seen.has(variable)) return { known: false };
  const definition = variable.defs[0];
  return definition.type === 'Variable'
    ? constantValue(unit, definition.node.init, new Set([...seen, variable]), depth + 1) : { known: false };
}
function reachable(unit, node) {
  for (let parent = unit.parents.get(node), child = node; parent; child = parent, parent = unit.parents.get(parent)) {
    if (['IfStatement', 'ConditionalExpression'].includes(parent.type)) {
      const test = constantValue(unit, parent.test);
      if (test.known && (child === parent.consequent && !test.value || child === parent.alternate && test.value)) return false;
    }
    if (parent.type === 'LogicalExpression' && child === parent.right) {
      const left = constantValue(unit, parent.left);
      if (left.known && (parent.operator === '&&' && !left.value || parent.operator === '||' && left.value)) return false;
    }
    if (['WhileStatement', 'ForStatement'].includes(parent.type) && child === parent.body) {
      const test = constantValue(unit, parent.test);
      if (test.known && !test.value) return false;
    }
  }
  return true;
}

// A memoized callback alone is not a registration. Follow its actual returned,
// rendered, scheduled or React lifecycle use, including local call forwarding.
function lifetimeCapability(unit, fn, context, seen = new Set(), depth = 0) {
  if (!fn || depth > 12 || seen.has(fn) || !reachable(unit, fn)) return false;
  context.capabilities ??= new Map();
  if (context.capabilities.has(fn)) return context.capabilities.get(fn);
  const next = new Set([...seen, fn]);
  function escapes(node, visits = new Set(), level = 0) {
    if (++context.steps > 20000 || level > 12 || !node || visits.has(node) || !reachable(unit, node)) return false;
    visits = new Set([...visits, node]);
    const parent = unit.parents.get(node);
    if (!parent) return false;
    if (['ExportNamedDeclaration', 'ExportDefaultDeclaration'].includes(parent.type)) return true;
    if (parent.type === 'VariableDeclarator' && parent.id.type === 'Identifier') {
      const declaration = unit.parents.get(parent);
      if (unit.parents.get(declaration)?.type === 'ExportNamedDeclaration') return true;
      const variable = unit.binding(parent.id);
      return immutable(variable) && variable.references.some((reference) => reference.isRead()
        && escapes(reference.identifier, visits, level + 1));
    }
    if (parent.type === 'FunctionDeclaration' && parent.id === node) return escapes(parent, visits, level + 1);
    if (parent.type === 'CallExpression') {
      if (parent.arguments[0] === node && ['useCallback', 'useMemo'].some((name) => importedCall(unit, parent, 'react', name))) {
        return escapes(parent, visits, level + 1);
      }
      if (parent.arguments[0] === node && ['useEffect', 'useLayoutEffect'].some((name) => importedCall(unit, parent, 'react', name))) {
        return lifetimeCapability(unit, functionOwner(unit, parent), context, next, depth + 1);
      }
      const callee = unwrap(parent.callee);
      if (parent.arguments[0] === node && callee?.type === 'Identifier' && !unit.binding(callee)
        && ['setTimeout', 'setInterval', 'queueMicrotask', 'requestAnimationFrame'].includes(callee.name)) {
        return lifetimeCapability(unit, functionOwner(unit, parent), context, next, depth + 1);
      }
      if (parent.arguments[0] === node && memberName(callee)
        && ['setTimeout', 'setInterval', 'requestAnimationFrame'].includes(memberName(callee))
        && unwrap(callee.object)?.type === 'Identifier' && unwrap(callee.object).name === 'window'
        && !unit.binding(unwrap(callee.object))) {
        return lifetimeCapability(unit, functionOwner(unit, parent), context, next, depth + 1);
      }
      if (parent.callee === node) return lifetimeCapability(unit, functionOwner(unit, parent), context, next, depth + 1);
      return false;
    }
    if (parent.type === 'ReturnStatement' || parent.type === 'JSXExpressionContainer') {
      return lifetimeCapability(unit, functionOwner(unit, parent), context, next, depth + 1);
    }
    if (parent.type === 'ArrowFunctionExpression' && parent.body === node) {
      return lifetimeCapability(unit, parent, context, next, depth + 1);
    }
    if (['Property', 'ObjectExpression', 'ArrayExpression', 'TSAsExpression', 'TSNonNullExpression', 'TSSatisfiesExpression'].includes(parent.type)) {
      return escapes(parent, visits, level + 1);
    }
    return false;
  }
  let result = escapes(fn);
  if (!result && fn.type === 'FunctionDeclaration' && fn.id) {
    const variable = unit.binding(fn.id);
    result = variable?.references.some((reference) => reference.isRead() && escapes(reference.identifier));
  }
  context.capabilities.set(fn, Boolean(result));
  return Boolean(result);
}

function stateWrites(unit, context) {
  if (context.stateWrites) return context.stateWrites;
  const result = [];
  for (const node of unit.nodes) {
    if (++context.steps > 20000) throw new Error('semantic proof budget exceeded');
    if (!['AssignmentExpression', 'UpdateExpression'].includes(node.type) || !reachable(unit, node)) continue;
    const target = unwrap(node.type === 'AssignmentExpression' ? node.left : node.argument);
    const ref = refRead(unit, target, context);
    const scalar = target?.type === 'Identifier' ? unit.binding(target) : null;
    if (ref || scalar) result.push({ node, variable: ref ?? scalar, ref: Boolean(ref) });
  }
  context.stateWrites = result;
  return result;
}

function controllerOrigins(unit, expression, context, seen = new Set(), depth = 0) {
  if (++context.steps > 20000 || depth > 12) return null;
  expression = unwrap(expression);
  if (expression?.type === 'NewExpression') {
    const callee = unwrap(expression.callee);
    if (callee?.type !== 'Identifier' || callee.name !== 'AbortController' || unit.binding(callee)) return null;
    addProof(context, expression);
    return new Set([expression]);
  }
  if (expression?.type === 'Identifier') {
    const variable = unit.binding(expression);
    if (!immutable(variable) || seen.has(variable)) return null;
    const definition = variable.defs[0];
    return definition.type === 'Variable'
      ? controllerOrigins(unit, definition.node.init, context, new Set([...seen, variable]), depth + 1) : null;
  }
  const ref = refRead(unit, expression, context);
  if (!ref || seen.has(ref)) return null;
  const assignments = stateWrites(unit, context).filter((write) => write.ref && write.variable === ref);
  if (!assignments.length || assignments.length > 8) return null;
  const result = new Set();
  for (const { node } of assignments) {
    if (node.operator !== '=') return null;
    const origins = controllerOrigins(unit, node.right, context, new Set([...seen, ref]), depth + 1);
    if (!origins) return null;
    for (const origin of origins) result.add(origin);
    addProof(context, node);
  }
  return result;
}

function completionDependency(unit, guard, owner, context) {
  const model = finiteValueProof(context);
  const keys = new Map();
  const identities = new Map();
  const symbol = (node) => {
    if (!identities.has(node)) identities.set(node, Symbol('source identity'));
    return identities.get(node);
  };
  const key = (node, seen = new Set()) => {
    node = unwrap(node);
    if (node?.type === 'Identifier') {
      const variable = unit.binding(node);
      if (!variable) return null;
      const definition = variable.defs[0];
      if (immutable(variable) && !seen.has(variable) && definition.type === 'Variable'
        && unwrap(definition.node.init)?.type === 'Identifier') {
        return key(definition.node.init, new Set([...seen, variable]));
      }
      if (!keys.has(variable)) keys.set(variable, `binding:${keys.size}`);
      return keys.get(variable);
    }
    if (node?.type !== 'MemberExpression' || memberName(node) === null) return null;
    const root = key(node.object, seen);
    return root === null ? null : `${root}.${memberName(node)}`;
  };
  const writes = stateWrites(unit, context);
  const cells = new Map();
  const cellFor = (node) => {
    const ref = refRead(unit, node, context);
    const variable = ref ?? (node.type === 'Identifier' ? unit.binding(node) : null);
    if (!variable || !ref && immutable(variable)) return null;
    if (cells.has(variable)) return cells.get(variable);
    const mutations = writes.filter((write) => write.variable === variable && write.ref === Boolean(ref)
      && lifetimeCapability(unit, functionOwner(unit, write.node), context));
    if (!mutations.length || mutations.length > 8) return null;
    const definition = variable.defs[0];
    const initializer = ref ? unwrap(definition.node.init)?.arguments[0] : definition?.node.init;
    const cell = { variable, key: ref ? `${key(node.object)}.current` : key(node), initializer, mutations };
    cells.set(variable, cell);
    return cell;
  };
  function arithmeticStep(node, cell, seen = new Set(), depth = 0) {
    node = unwrap(node);
    if (!node || depth > 12) return null;
    if (node.type === 'Identifier') {
      const variable = unit.binding(node);
      if (!immutable(variable) || seen.has(variable)) return null;
      const definition = variable.defs[0];
      return definition.type === 'Variable'
        ? arithmeticStep(definition.node.init, cell, new Set([...seen, variable]), depth + 1) : null;
    }
    if (node.type !== 'BinaryExpression' || !['+', '-'].includes(node.operator)) return null;
    const same = cellFor(unwrap(node.left));
    const value = constantValue(unit, node.right);
    return same === cell && value.known && typeof value.value === 'number'
      ? node.operator === '+' ? value.value : -value.value : null;
  }
  function sourceValue(node, seen = new Set(), depth = 0) {
    node = unwrap(node);
    if (!node || depth > 12) return { known: false };
    const literal = constantValue(unit, node);
    if (literal.known) return literal;
    if (['ObjectExpression', 'ArrayExpression', 'NewExpression'].includes(node.type)) {
      return { known: true, value: symbol(node) };
    }
    if (node.type !== 'Identifier') return { known: false };
    const variable = unit.binding(node);
    if (!immutable(variable) || seen.has(variable)) return { known: false };
    const definition = variable.defs[0];
    return definition.type === 'Variable'
      ? sourceValue(definition.node.init, new Set([...seen, variable]), depth + 1) : { known: false };
  }
  function domain(cell) {
    const initial = sourceValue(cell.initializer);
    let values = initial.known ? [initial.value] : null;
    if (!values) return undefined;
    const steps = [];
    for (const { node } of cell.mutations) {
      let step;
      if (node.type === 'UpdateExpression') step = node.operator === '++' ? 1 : -1;
      else if (['+=', '-='].includes(node.operator)) {
        const value = sourceValue(node.right);
        step = value.known && typeof value.value === 'number' ? node.operator === '+=' ? value.value : -value.value : null;
      } else if (node.operator === '=') {
        if (cellFor(unwrap(node.right)) === cell) continue;
        const value = sourceValue(node.right);
        if (value.known) { values.push(value.value); continue; }
        step = arithmeticStep(node.right, cell);
      }
      if (step === null || step === undefined || typeof initial.value !== 'number' || !Number.isFinite(step)) return undefined;
      steps.push(step);
    }
    for (const step of steps) values.push(initial.value + step, initial.value + 2 * step);
    // Opposing increments can reach zero, so do not infer a fixed truth value.
    if (steps.some((step) => step > 0) && steps.some((step) => step < 0)) values.push(0);
    return [...new Set(values)];
  }
  const aborts = new Map();
  function abortCell(node) {
    if (memberName(node) !== 'aborted' || memberName(node.object) !== 'signal') return null;
    const origins = controllerOrigins(unit, unwrap(node.object).object, context);
    if (!origins) return null;
    const originKey = [...origins].map((origin) => origin.range[0]).sort().join(':');
    if (aborts.has(originKey)) return aborts.get(originKey);
    let cancel = null;
    for (const call of unit.nodes) {
      if (++context.steps > 20000) throw new Error('semantic proof budget exceeded');
      if (call.type !== 'CallExpression' || memberName(call.callee) !== 'abort' || call.arguments.length
        || !reachable(unit, call)) continue;
      const targets = controllerOrigins(unit, unwrap(call.callee).object, context);
      if (!targets || targets.size !== origins.size || ![...targets].every((origin) => origins.has(origin))) continue;
      if (!lifetimeCapability(unit, functionOwner(unit, call), context)) continue;
      cancel = call;
      break;
    }
    const result = cancel ? { key: `abort:${originKey}`, cancel, abort: true } : null;
    aborts.set(originKey, result);
    return result;
  }
  const candidates = new Map();
  function compile(expression, target = null, mutation = null, after = false, seen = new Set(), depth = 0, writeOwner = null) {
    return model.expression(unit, expression, (node, localDepth, localSeen) => {
      const level = depth + localDepth;
      if (level > 12) return null;
      if (node.type === 'MemberExpression' || node.type === 'Identifier') {
        const abort = abortCell(node);
        const cell = abort ?? cellFor(node);
        if (cell) {
          candidates.set(cell.key, cell);
          const before = model.input(cell.key, cell.abort ? [false] : domain(cell));
          if (!after || target !== cell) return before;
          if (cell.abort) return model.constant(true);
          const change = mutation.node;
          if (change.type === 'UpdateExpression') return model.operation('+', before, model.constant(change.operator === '++' ? 1 : -1));
          if (change.operator === '=') return compile(change.right, null, null, false, localSeen, level + 1, functionOwner(unit, change));
          if (['+=', '-='].includes(change.operator)) return model.operation(change.operator[0], before,
            compile(change.right, null, null, false, localSeen, level + 1, functionOwner(unit, change)));
          return null;
        }
      }
      if (['ObjectExpression', 'ArrayExpression'].includes(node.type)) return model.constant(symbol(node));
      if (node.type === 'Identifier') {
        const variable = unit.binding(node);
        if (!variable || localSeen.has(variable)) return null;
        const definition = variable.defs[0];
        if (definition.type === 'Variable' && immutable(variable)) {
          const init = unwrap(definition.node.init);
          // A ticket produced by ++current, or assigned back to current before
          // suspension, denotes the value installed for this invocation.
          const recaptured = writeOwner && writeOwner === functionOwner(unit, definition.node);
          const installed = recaptured ? null : init?.type === 'UpdateExpression' ? cellFor(unwrap(init.argument))
            : writes.find((write) => write.node.type === 'AssignmentExpression' && write.node.operator === '='
              && unwrap(write.node.right)?.type === 'Identifier' && unit.binding(unwrap(write.node.right)) === variable
              && write.node.range[0] > definition.node.range[0] && write.node.range[0] < guard.range[0]
              && functionOwner(unit, write.node) === functionOwner(unit, definition.node));
          const installedCell = installed?.variable ? installed.key ? installed : cells.get(installed.variable)
            ?? cellFor(unwrap(installed.node.left)) : null;
          if (installedCell) {
            candidates.set(installedCell.key, installedCell);
            return model.input(installedCell.key, domain(installedCell));
          }
          if (['ObjectExpression', 'ArrayExpression', 'NewExpression'].includes(init?.type)) {
            return model.constant(symbol(init));
          }
          if (init?.type === 'CallExpression') {
            return model.input(key(node));
          }
          // An initializer ran before this completion guard: do not re-read a
          // captured snapshot as though it were a completion-time ref access.
          return compile(init, target, mutation, false, new Set([...localSeen, variable]), level + 1, writeOwner);
        }
        if (definition.type === 'Parameter') return model.input(key(node));
        return null;
      }
      if (node.type === 'MemberExpression') {
        const selector = key(node);
        return selector === null ? null : model.input(selector);
      }
      if (node.type !== 'CallExpression') return undefined;
      if (node.arguments.length || unwrap(node.callee)?.type !== 'Identifier') return null;
      const variable = unit.binding(unwrap(node.callee));
      if (!immutable(variable) || localSeen.has(variable)) return null;
      const definition = variable.defs[0];
      const fn = definition.type === 'FunctionName' ? definition.node
        : definition.type === 'Variable' ? unwrap(definition.node.init) : null;
      if (!FUNCTIONS.has(fn?.type) || fn.async || fn.generator || fn.params.length) return null;
      const body = fn.body.type === 'BlockStatement'
        ? fn.body.body.length === 1 && fn.body.body[0].type === 'ReturnStatement' ? fn.body.body[0].argument : null : fn.body;
      const result = compile(body, target, mutation, after, new Set([...localSeen, variable]), level + 1, writeOwner);
      if (result) addProof(context, variableDeclaration(unit, definition));
      return result;
    }, depth, seen);
  }
  const before = compile(guard);
  if (!before || candidates.size > 8) return false;
  for (const cell of candidates.values()) {
    const mutations = cell.abort ? [{ node: cell.cancel }] : cell.mutations;
    for (const mutation of mutations) {
      if (!cell.abort && !lifetimeCapability(unit, functionOwner(unit, mutation.node), context)) continue;
      const after = compile(guard, cell, mutation, true);
      if (!after || !model.witness([before, after], ([a, b]) => Boolean(a) !== Boolean(b))) continue;
      addProof(context, mutation.node);
      return true;
    }
  }
  return false;
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
  if (!completionDependency(unit, guard.test, owner, context)) return null;
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
  if (!loop) return null;
  const message = 'This makes the ' + labels[loop.type] + ' slow because each await runs one after another, so collect the independent calls & run them together with `await Promise.all(items.map(...))`';
  if (diagnostic.message !== message) return null;
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
