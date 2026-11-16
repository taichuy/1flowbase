const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');

const TOOL = 'react-doctor@0.2.16';
const PROVENANCE = {
  toolPackage: TOOL,
  packageIntegrity: 'sha512-o108QscqM/TlC1QmKgDORf/P53PeZ69nZbRsvy53rXkKvWdKZC1Y0u1JRPmThbTh6v+QpMispBwEnFHhlpZQ7w==',
  sourceCommit: '2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc',
};
const RULE_PROVENANCE = {
  'no-derived-state': {
    ruleSourceSha256: 'db98ba7989c837bb635b3fed080a937ed425ecce6330126dd58b15f66e9eb91f',
    references: ['https://github.com/millionco/react-doctor/pull/1017'],
  },
  'no-event-handler': {
    ruleSourceSha256: '1f89d763b79829a89a3c00e9d655ff8299e26b98c5f7d46a770f3bb0ccf74062',
    references: [
      'https://react.dev/reference/react/useEffect#controlling-a-non-react-widget',
      'https://github.com/millionco/react-doctor/pull/1219',
    ],
  },
};
const FUNCTION_TYPES = new Set(['FunctionDeclaration', 'FunctionExpression', 'ArrowFunctionExpression']);
const READ_METHODS = new Set(['at', 'slice', 'filter', 'map', 'includes', 'some', 'every']);
const FORM_METHODS = new Map([['setFieldsValue', [1]], ['setFieldValue', [2]], ['resetFields', [0, 1]]]);
const { parseUnit, unwrap, memberName, immutable, importOrigin, importedCall } = require('./source-unit.js');
const digest = (source) => createHash('sha256').update(source).digest('hex');

function functionOwner(unit, node) {
  for (let parent = unit.parents.get(node); parent; parent = unit.parents.get(parent)) {
    if (!FUNCTION_TYPES.has(parent.type)) continue;
    const call = unit.parents.get(parent);
    if (call?.type === 'CallExpression' && call.arguments[0] === parent
      && importedCall(unit, call, 'react', 'useEffect')) continue;
    return parent;
  }
  return null;
}

function atDiagnostic(unit, diagnostic) {
  if (!Number.isInteger(diagnostic.line) || !Number.isInteger(diagnostic.column)
    || diagnostic.line < 1 || diagnostic.column < 1) return null;
  const lines = unit.source.split('\n');
  if (diagnostic.line > lines.length || diagnostic.column > lines[diagnostic.line - 1].length) return null;
  const offset = lines.slice(0, diagnostic.line - 1).reduce((sum, line) => sum + line.length + 1, 0)
    + diagnostic.column - 1;
  return unit.nodes.filter((node) => node.type === 'Identifier' && node.range[0] === offset)[0] ?? null;
}

function addProof(context, unit, node) {
  const spans = context.evidence.get(unit) ?? new Map();
  spans.set(node.range.join(':'), { start: node.range[0], end: node.range[1] });
  context.evidence.set(unit, spans);
}

function parameterBinding(unit, fn, index = 0) {
  const parameter = fn.params[index];
  return parameter?.type === 'Identifier' ? unit.binding(parameter) : null;
}

function pure(unit, node, context, depth = 0) {
  if (++context.steps > 20000 || depth > 32) return false;
  node = unwrap(node);
  if (!node) return false;
  switch (node.type) {
    case 'Identifier':
    case 'Literal':
      return true;
    case 'MemberExpression':
      return pure(unit, node.object, context, depth + 1)
        && (!node.computed || pure(unit, node.property, context, depth + 1));
    case 'UnaryExpression':
      return node.operator !== 'delete' && pure(unit, node.argument, context, depth + 1);
    case 'BinaryExpression':
    case 'LogicalExpression':
      return pure(unit, node.left, context, depth + 1) && pure(unit, node.right, context, depth + 1);
    case 'ConditionalExpression':
      return [node.test, node.consequent, node.alternate].every((child) => pure(unit, child, context, depth + 1));
    case 'ArrayExpression':
      return node.elements.every((element) => element === null || pure(unit, element.type === 'SpreadElement' ? element.argument : element, context, depth + 1));
    case 'ObjectExpression':
      return node.properties.every((property) => property.type === 'SpreadElement'
        ? pure(unit, property.argument, context, depth + 1)
        : property.type === 'Property' && property.kind === 'init' && !property.method
          && (!property.computed || pure(unit, property.key, context, depth + 1))
          && pure(unit, property.value, context, depth + 1));
    case 'TemplateLiteral':
      return node.expressions.every((expression) => pure(unit, expression, context, depth + 1));
    case 'ArrowFunctionExpression':
    case 'FunctionExpression':
      return !node.async && !node.generator && node.body.type !== 'BlockStatement'
        && pure(unit, node.body, context, depth + 1);
    case 'CallExpression': {
      const callee = unwrap(node.callee);
      return READ_METHODS.has(memberName(callee)) && pure(unit, callee.object, context, depth + 1)
        && node.arguments.every((argument) => argument.type !== 'SpreadElement' && pure(unit, argument, context, depth + 1));
    }
    default:
      return false;
  }
}

function stateSetter(unit, node) {
  const variable = unit.binding(node);
  if (!immutable(variable)) return null;
  const definition = variable.defs[0];
  if (definition.type !== 'Variable') return null;
  const declaration = definition.node;
  if (declaration.id.type !== 'ArrayPattern' || declaration.id.elements[1]?.type !== 'Identifier'
    || unit.binding(declaration.id.elements[1]) !== variable
    || !importedCall(unit, declaration.init, 'react', 'useState')) return null;
  const state = declaration.id.elements[0];
  return state?.type === 'Identifier' ? state.name : null;
}

function relativeFunction(unit, identifier, context) {
  const variable = unit.binding(identifier);
  if (!immutable(variable)) return null;
  const definition = variable.defs[0];
  if (definition.type === 'FunctionName') return { unit, fn: definition.node, span: exportedSpan(unit, definition.node) };
  if (definition.type === 'Variable' && FUNCTION_TYPES.has(unwrap(definition.node.init)?.type)) {
    return { unit, fn: unwrap(definition.node.init), span: definition.node };
  }
  const origin = importOrigin(unit, identifier);
  if (!origin?.module.startsWith('.') || origin.name === '*' || origin.name === 'default') return null;
  const base = path.posix.normalize(path.posix.join(path.posix.dirname(unit.file), origin.module));
  if (!base.startsWith('web/app/')) return null;
  const candidates = [base, ...['.ts', '.tsx', '.js', '.jsx'].map((extension) => base + extension)]
    .filter((file) => { try { return fs.statSync(path.join(context.repoRoot, file)).isFile(); } catch { return false; } });
  if (candidates.length !== 1 || context.dependencies.size >= 4) return null;
  const helper = context.read(candidates[0]);
  if (!helper) return null;
  context.dependencies.add(helper.file);
  const exported = helper.ast.body.filter((statement) => statement.type === 'ExportNamedDeclaration'
    && statement.declaration?.type === 'FunctionDeclaration' && statement.declaration.id?.name === origin.name);
  if (exported.length !== 1) return null;
  return { unit: helper, fn: exported[0].declaration, span: exported[0] };
}

function exportedSpan(unit, fn) {
  const parent = unit.parents.get(fn);
  return parent?.type === 'ExportNamedDeclaration' ? parent : fn;
}

// A result must preserve history, not merely read it in a guard or an ignored call.
// An object spread followed by prop-derived field replacement is deliberately absent.
function lineage(unit, node, prior, context, depth = 0, aliases = new Set()) {
  if (++context.steps > 20000 || depth > 12) return null;
  node = unwrap(node);
  if (!node) return null;
  if (node.type === 'Identifier') {
    const variable = unit.binding(node);
    if (variable === prior) return 'prior';
    if (!immutable(variable) || aliases.has(variable)) return null;
    const definition = variable.defs[0];
    if (definition.type !== 'Variable' || definition.node.id.type !== 'Identifier') return null;
    return lineage(unit, definition.node.init, prior, context, depth + 1, new Set([...aliases, variable]));
  }
  if (node.type === 'ConditionalExpression') {
    if (!pure(unit, node.test, context)) return null;
    const a = lineage(unit, node.consequent, prior, context, depth + 1, aliases);
    const b = lineage(unit, node.alternate, prior, context, depth + 1, aliases);
    return a && b ? a === 'history' || b === 'history' ? 'history' : 'prior' : null;
  }
  if (node.type === 'ArrayExpression') {
    let retainsPrior = false;
    for (const element of node.elements) {
      if (!element) continue;
      if (element.type === 'SpreadElement') {
        if (lineage(unit, element.argument, prior, context, depth + 1, aliases)) retainsPrior = true;
        else if (!pure(unit, element.argument, context)) return null;
      } else if (!pure(unit, element, context)) return null;
    }
    return retainsPrior ? 'history' : null;
  }
  if (node.type === 'BinaryExpression' && ['+', '-'].includes(node.operator)) {
    if (!pure(unit, node, context)) return null;
    const left = lineage(unit, node.left, prior, context, depth + 1, aliases);
    const right = lineage(unit, node.right, prior, context, depth + 1, aliases);
    return Boolean(left) !== Boolean(right) ? 'history' : null;
  }
  if (node.type !== 'CallExpression') return null;
  const callee = unwrap(node.callee);
  const method = memberName(callee);
  if (['filter', 'slice'].includes(method)) {
    if (!pure(unit, node, context) || !lineage(unit, callee.object, prior, context, depth + 1, aliases)) return null;
    if (method === 'filter') {
      const callback = node.arguments[0];
      if (!FUNCTION_TYPES.has(callback?.type) || !parameterBinding(unit, callback)) return null;
      const parameter = parameterBinding(unit, callback);
      if (!parameter.references.some((reference) => reference.isRead())) return null;
    }
    if (method === 'slice' && node.arguments[1]?.type === 'Literal' && node.arguments[1].value === 0) return null;
    return 'history';
  }
  if (callee.type !== 'Identifier' || !node.arguments.some((argument) => lineage(unit, argument, prior, context, depth + 1, aliases))) return null;
  const helper = relativeFunction(unit, callee, context);
  if (!helper || context.functions.has(helper.fn) || helper.fn.async || helper.fn.generator) return null;
  const indices = node.arguments.map((argument, index) => lineage(unit, argument, prior, context, depth + 1, aliases) ? index : -1).filter((index) => index >= 0);
  if (indices.length !== 1 || !node.arguments.every((argument) => pure(unit, argument, context))) return null;
  const helperPrior = parameterBinding(helper.unit, helper.fn, indices[0]);
  if (!helperPrior || !immutable(helperPrior)) return null;
  context.functions.add(helper.fn);
  const result = functionReturns(helper.unit, helper.fn, helperPrior, context, depth + 1);
  context.functions.delete(helper.fn);
  if (!result) return null;
  addProof(context, helper.unit, helper.span);
  return result;
}

function returnStatements(unit, statements, prior, context, depth) {
  const kinds = [];
  for (const statement of statements) {
    if (statement.type === 'ReturnStatement') {
      const kind = lineage(unit, statement.argument, prior, context, depth + 1);
      return kind ? { kinds: [...kinds, kind], complete: true } : null;
    }
    if (statement.type === 'IfStatement') {
      if (!pure(unit, statement.test, context)) return null;
      const branch = (node) => node ? returnStatements(unit, node.type === 'BlockStatement' ? node.body : [node], prior, context, depth + 1) : { kinds: [], complete: false };
      const a = branch(statement.consequent), b = branch(statement.alternate);
      if (!a || !b) return null;
      kinds.push(...a.kinds, ...b.kinds);
      if (a.complete && b.complete) return { kinds, complete: true };
    } else if (statement.type === 'VariableDeclaration') {
      if (statement.kind !== 'const' || !statement.declarations.every((declaration) => declaration.id.type === 'Identifier'
        && pure(unit, declaration.init, context))) return null;
    } else if (statement.type !== 'EmptyStatement') return null;
  }
  return { kinds, complete: false };
}

function functionReturns(unit, fn, prior, context, depth = 0) {
  if (!immutable(prior)) return null;
  if (fn.body.type !== 'BlockStatement') return lineage(unit, fn.body, prior, context, depth + 1);
  const result = returnStatements(unit, fn.body.body, prior, context, depth);
  return result?.complete && result.kinds.every(Boolean)
    ? result.kinds.includes('history') ? 'history' : 'prior' : null;
}

function historyCorrection(unit, token, diagnostic, context) {
  const call = unit.parents.get(token);
  if (call?.type !== 'CallExpression' || call.callee !== token) return false;
  const state = stateSetter(unit, token);
  const expected = `Storing "${state}" in state when you can derive it from other values costs an extra render.`;
  const alternate = `"${state}" is only set here from other values, so storing it costs an extra render.`;
  if (!state || ![expected, alternate].includes(diagnostic.message) || call.arguments.length !== 1) return false;
  const updater = unwrap(call.arguments[0]);
  if (!FUNCTION_TYPES.has(updater?.type) || updater.async || updater.generator || updater.params.length !== 1) return false;
  const prior = parameterBinding(unit, updater);
  if (!prior || functionReturns(unit, updater, prior, context) !== 'history') return false;
  addProof(context, unit, call);
  return true;
}

function formVariable(unit, variable, seen = new Set()) {
  if (!immutable(variable) || seen.has(variable)) return null;
  seen.add(variable);
  const definition = variable.defs[0];
  if (definition.type !== 'Variable') return null;
  const declaration = definition.node;
  if (declaration.id.type === 'Identifier' && unwrap(declaration.init)?.type === 'Identifier') {
    return formVariable(unit, unit.binding(unwrap(declaration.init)), seen);
  }
  if (declaration.id.type !== 'ArrayPattern' || unit.binding(declaration.id.elements[0]) !== variable) return null;
  const initializer = unwrap(declaration.init);
  if (initializer?.type !== 'CallExpression' || memberName(initializer.callee) !== 'useForm') return null;
  const origin = importOrigin(unit, unwrap(initializer.callee).object);
  return origin?.module === 'antd' && origin.name === 'Form' ? variable : null;
}

function localRef(unit, node, owner) {
  const variable = unit.binding(node);
  if (!immutable(variable)) return false;
  const definition = variable.defs[0];
  return definition.type === 'Variable' && functionOwner(unit, definition.node) === owner
    && importedCall(unit, definition.node.init, 'react', 'useRef');
}

function hydrationStatements(unit, statements, owner, forms, context) {
  let calls = 0;
  for (const statement of statements) {
    if (statement.type === 'BlockStatement') {
      const count = hydrationStatements(unit, statement.body, owner, forms, context);
      if (count === null) return null;
      calls += count;
    } else if (statement.type === 'IfStatement') {
      if (!pure(unit, statement.test, context)) return null;
      for (const branch of [statement.consequent, statement.alternate]) {
        if (!branch) continue;
        const count = hydrationStatements(unit, [branch], owner, forms, context);
        if (count === null) return null;
        calls += count;
      }
    } else if (statement.type === 'VariableDeclaration') {
      if (statement.kind !== 'const' || !statement.declarations.every((declaration) => declaration.id.type === 'Identifier'
        && pure(unit, declaration.init, context))) return null;
    } else if (statement.type === 'ReturnStatement') {
      if (statement.argument) return null;
    } else if (statement.type === 'ExpressionStatement') {
      const expression = unwrap(statement.expression);
      if (expression.type === 'AssignmentExpression' && expression.operator === '='
        && memberName(expression.left) === 'current' && expression.right.type === 'Literal'
        && expression.right.value === null && localRef(unit, unwrap(expression.left).object, owner)) continue;
      if (expression.type !== 'CallExpression') return null;
      const callee = unwrap(expression.callee);
      const method = memberName(callee);
      if (!FORM_METHODS.get(method)?.includes(expression.arguments.length)
        || !expression.arguments.every((argument) => pure(unit, argument, context))) return null;
      const form = formVariable(unit, unit.binding(callee.object));
      if (!form || !forms.has(form)) return null;
      addProof(context, unit, expression);
      calls += 1;
    } else if (statement.type !== 'EmptyStatement') return null;
  }
  return calls;
}

function formCorrection(unit, token, diagnostic, context) {
  if (![
    'Faking an event handler with a prop plus a useEffect costs an extra render & runs late.',
    'Faking an event handler with state plus a useEffect costs an extra render & runs late.',
  ].includes(diagnostic.message)) return false;
  const owner = functionOwner(unit, token);
  if (!owner) return false;
  const forms = new Set();
  for (const scopeBinding of unit.bindings.values()) {
    const form = formVariable(unit, scopeBinding);
    if (form && functionOwner(unit, form.defs[0].node) === owner) forms.add(form);
  }
  if (!forms.size) return false;
  for (const node of unit.nodes) {
    if (!['AssignmentExpression', 'UpdateExpression'].includes(node.type)) continue;
    let target = unwrap(node.left ?? node.argument);
    while (target?.type === 'MemberExpression') target = unwrap(target.object);
    if (target?.type === 'Identifier' && forms.has(formVariable(unit, unit.binding(target)))) return false;
  }
  const effects = unit.nodes.filter((node) => importedCall(unit, node, 'react', 'useEffect')
    && functionOwner(unit, node) === owner);
  if (!effects.length) return false;
  // Pinned diagnostics can point at shared upstream declarations. Until every
  // effect in this owner has the proof, an ambiguous source location stays open.
  for (const effect of effects) {
    const callback = unwrap(effect.arguments[0]);
    if (!FUNCTION_TYPES.has(callback?.type) || callback.async || callback.body.type !== 'BlockStatement') return false;
    if (!hydrationStatements(unit, callback.body.body, owner, forms, context)) return false;
    addProof(context, unit, effect);
  }
  for (const form of forms) addProof(context, unit, form.defs[0].node.init);
  return true;
}

function createEffectSemanticCorrections(repoRoot) {
  const units = new Map();
  const read = (file) => {
    if (!units.has(file)) units.set(file, parseUnit(repoRoot, file));
    return units.get(file);
  };
  return (diagnostic, toolPackage) => {
    if (toolPackage !== TOOL || diagnostic.plugin !== 'react-doctor' || !RULE_PROVENANCE[diagnostic.rule]) return null;
    const file = path.posix.normalize(`web/app/${diagnostic.filePath}`);
    if (!file.startsWith('web/app/') || diagnostic.filePath.startsWith('/') || diagnostic.filePath.includes('\\')) return null;
    try {
      const unit = read(file);
      const token = unit && atDiagnostic(unit, diagnostic);
      if (!token) return null;
      const context = { repoRoot, read, evidence: new Map(), steps: 0, functions: new Set(), dependencies: new Set([file]) };
      const history = diagnostic.rule === 'no-derived-state';
      if (!(history ? historyCorrection(unit, token, diagnostic, context) : formCorrection(unit, token, diagnostic, context))) return null;
      return {
        predicate: history ? 'history-accumulator' : 'external-form-synchronization',
        reason: history
          ? 'The returned transition preserves and composes prior state; current props alone cannot reconstruct this accumulated history.'
          : 'All effect candidates in this owner synchronize a proven AntD Form store or reset its local session; none dispatch a user event.',
        provenance: { ...PROVENANCE, ...RULE_PROVENANCE[diagnostic.rule] },
        evidence: [...context.evidence].map(([source, spans]) => ({ file: source.file, sha256: digest(source.source), spans: [...spans.values()] })),
      };
    } catch {
      return null;
    }
  };
}

module.exports = { createEffectSemanticCorrections };
