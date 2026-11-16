const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { parseUnit, unwrap, memberName, immutable, importOrigin, importedCall } = require('./source-unit.js');

const TOOL = 'react-doctor@0.2.16';
const COMMIT = '2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc';
const FUNCTIONS = new Set(['FunctionDeclaration', 'FunctionExpression', 'ArrowFunctionExpression']);
const RULES = {
  'jsx-no-jsx-as-prop': {
    hash: '6c7d5e6da3dd051357f524e31491c08be6ca0be8d128a04564c796d53fcfd24c',
    source: 'react-builtins/jsx-no-jsx-as-prop.ts',
    message: 'This child redraws every render because the prop gets brand new JSX each time.',
    references: ['https://github.com/millionco/react-doctor/pull/1017', 'https://react.dev/reference/react/memo'],
  },
  'no-render-in-render': {
    hash: '9bcda8ae0e0342fdc72780a91481dc8bc757f3e6e5a7e632ad9e6d1a57558720',
    source: 'architecture/no-render-in-render.ts',
    references: ['https://github.com/millionco/react-doctor/pull/1094', 'https://react.dev/learn/preserving-and-resetting-state'],
  },
  'no-polymorphic-children': {
    hash: '986bc09f88e535b9eecffe17b2ca8a5fb118c08cd8d96ea1ff6bc4c1375913ca',
    source: 'correctness/no-polymorphic-children.ts',
    message: 'Your users hit inconsistent behavior because `typeof children === "string"` makes this component switch on what callers pass, so add clear subcomponents like `<Button.Text>` instead.',
    references: ['https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Reference/Attributes/aria-label'],
  },
  'no-react19-deprecated-apis': {
    hash: 'fcb5ff5c9c0859840f869f9afc0550fd9014358732fe67b81c2435b5501dbca2',
    source: 'architecture/no-react-dom-deprecated-apis.ts',
    message: 'forwardRef is dead weight in React 19, since ref is a normal prop now, so drop it & pass ref straight through.',
    references: ['https://react.dev/reference/react/forwardRef'],
  },
};

function step(context) {
  if (++context.steps > 20000) throw new Error('composition proof budget exceeded');
}
function proof(context, unit, node) {
  if (!node?.range) return;
  const spans = context.evidence.get(unit) ?? new Map();
  spans.set(node.range.join(':'), { start: node.range[0], end: node.range[1] });
  context.evidence.set(unit, spans);
}
function walk(unit, node, context, visit, skipFunctions = false) {
  const stack = [node];
  while (stack.length) {
    step(context);
    const current = stack.pop();
    if (!current || (skipFunctions && current !== node && FUNCTIONS.has(current.type))) continue;
    if (visit(current) === false) continue;
    for (const key of unit.visitorKeys[current.type] || []) {
      const value = current[key];
      for (const child of Array.isArray(value) ? value : [value]) if (child?.type) stack.push(child);
    }
  }
}
function importProof(context, unit, node, seen = new Set()) {
  node = unwrap(node);
  if (node?.type === 'MemberExpression') node = unwrap(node.object);
  const variable = unit.binding(node);
  if (!immutable(variable) || seen.has(variable)) return;
  seen.add(variable);
  const definition = variable.defs[0];
  if (definition.type === 'ImportBinding') proof(context, unit, unit.parents.get(definition.node));
  else if (definition.type === 'Variable') {
    proof(context, unit, definition.node);
    importProof(context, unit, definition.node.init, seen);
  }
}
function nearestFunction(unit, node) {
  for (let parent = unit.parents.get(node); parent; parent = unit.parents.get(parent)) {
    if (FUNCTIONS.has(parent.type)) return parent;
  }
  return null;
}
function atLocation(unit, diagnostic, types) {
  const { line, column } = diagnostic;
  if (!Number.isInteger(line) || !Number.isInteger(column) || line < 1 || column < 1) return null;
  const lines = unit.source.split('\n');
  if (line > lines.length || column > lines[line - 1].length) return null;
  const offset = lines.slice(0, line - 1).reduce((sum, text) => sum + text.length + 1, 0) + column - 1;
  const matches = unit.nodes.filter((node) => node.range[0] === offset && types.includes(node.type));
  return matches.length === 1 ? matches[0] : null;
}
function relativeUnit(context, unit, specifier) {
  if (typeof specifier !== 'string' || !specifier.startsWith('.')) return null;
  const base = path.posix.normalize(path.posix.join(path.posix.dirname(unit.file), specifier));
  if (!base.startsWith('web/app/src/')) return null;
  const candidates = [base, ...['.ts', '.tsx', '.js', '.jsx'].flatMap((extension) => [base + extension, `${base}/index${extension}`])]
    .filter((file) => { try { return fs.statSync(path.join(context.repoRoot, file)).isFile(); } catch { return false; } });
  if (candidates.length !== 1) return null;
  return context.read(candidates[0]);
}
function exportedFunction(context, unit, name, seen, depth) {
  step(context);
  if (depth > 8) return null;
  const matches = [];
  for (const statement of unit.ast.body) {
    if (statement.type !== 'ExportNamedDeclaration') continue;
    const declaration = statement.declaration;
    if (declaration?.type === 'FunctionDeclaration' && declaration.id?.name === name) matches.push([statement, declaration.id]);
    if (declaration?.type === 'VariableDeclaration') {
      for (const item of declaration.declarations) if (item.id.type === 'Identifier' && item.id.name === name) matches.push([statement, item.id]);
    }
    for (const specifier of statement.specifiers || []) {
      if ((specifier.exported.name ?? specifier.exported.value) === name) matches.push([statement, specifier.local]);
    }
  }
  if (matches.length !== 1) return null;
  const [statement, local] = matches[0];
  proof(context, unit, statement);
  if (statement.source) {
    const helper = relativeUnit(context, unit, statement.source.value);
    return helper && exportedFunction(context, helper, local.name ?? local.value, seen, depth + 1);
  }
  return callable(context, unit, local, seen, depth + 1);
}

// The only admitted wrapper is the import-resolved React forwardRef factory.
// memo, lazy, classes and opaque HOCs never become ordinary receivers here.
function callable(context, unit, node, seen = new Set(), depth = 0) {
  step(context);
  if (depth > 8) return null;
  node = unwrap(node);
  if (!node) return null;
  if (FUNCTIONS.has(node.type)) {
    if (node.generator) return null;
    proof(context, unit, node);
    return { unit, fn: node };
  }
  if (importedCall(unit, node, 'react', 'forwardRef')) {
    if (node.arguments.length !== 1) return null;
    importProof(context, unit, node.callee);
    proof(context, unit, node);
    const result = callable(context, unit, node.arguments[0], seen, depth + 1);
    return result && { ...result, forwardRef: true };
  }
  if (!['Identifier', 'JSXIdentifier'].includes(node.type)) return null;
  const variable = unit.binding(node);
  if (!immutable(variable) || seen.has(variable)) return null;
  seen.add(variable);
  const definition = variable.defs[0];
  if (definition.type === 'FunctionName') return callable(context, unit, definition.node, seen, depth + 1);
  if (definition.type === 'Variable' && definition.node.id.type === 'Identifier') {
    proof(context, unit, definition.node);
    return callable(context, unit, definition.node.init, seen, depth + 1);
  }
  if (definition.type !== 'ImportBinding' || definition.node.type !== 'ImportSpecifier') return null;
  const declaration = unit.parents.get(definition.node);
  const helper = relativeUnit(context, unit, declaration?.source.value);
  if (!helper) return null;
  proof(context, unit, declaration);
  return exportedFunction(context, helper, definition.node.imported.name ?? definition.node.imported.value, seen, depth + 1);
}
function jsxOrigin(unit, name) {
  const variable = unit.binding(name);
  return variable?.identifiers[0] ? importOrigin(unit, variable.identifiers[0]) : null;
}
const native = (opening) => opening?.name.type === 'JSXIdentifier' && /^[a-z][a-z0-9]*$/.test(opening.name.name);
function fieldBinding(unit, fn, name) {
  const pattern = fn.params[0];
  if (pattern?.type !== 'ObjectPattern') return null;
  const properties = pattern.properties.filter((property) => property.type === 'Property' && !property.computed
    && (property.key.name ?? property.key.value) === name);
  if (properties.length !== 1) return null;
  let value = properties[0].value;
  if (value.type === 'AssignmentPattern') value = value.left;
  return value.type === 'Identifier' ? unit.binding(value) : null;
}
function freshJsx(context, unit, node, seen = new Set()) {
  step(context);
  node = unwrap(node);
  if (!node) return false;
  if (['JSXElement', 'JSXFragment'].includes(node.type)) return true;
  if (node.type === 'LogicalExpression') return freshJsx(context, unit, node.left, seen) || freshJsx(context, unit, node.right, seen);
  if (node.type === 'ConditionalExpression') return freshJsx(context, unit, node.consequent, seen) || freshJsx(context, unit, node.alternate, seen);
  const variable = unit.binding(node);
  if (node.type !== 'Identifier' || !immutable(variable) || seen.has(variable)) return false;
  seen.add(variable);
  const definition = variable.defs[0];
  if (definition.type !== 'Variable' || definition.node.id.type !== 'Identifier') return false;
  proof(context, unit, definition.node);
  return freshJsx(context, unit, definition.node.init, seen);
}
function ordinarySlot(context, unit, attribute) {
  if (attribute.name.type !== 'JSXIdentifier' || attribute.value?.type !== 'JSXExpressionContainer'
      || !freshJsx(context, unit, attribute.value.expression)) return false;
  const opening = unit.parents.get(attribute);
  if (opening?.type !== 'JSXOpeningElement' || native(opening)) return false;
  const receiver = callable(context, unit, opening.name);
  if (!receiver) return false;
  const binding = fieldBinding(receiver.unit, receiver.fn, attribute.name.name);
  if (!immutable(binding) || !binding.references.some((reference) => reference.isRead())) return false;
  for (const reference of binding.references.filter((reference) => reference.isRead())) {
    for (let node = reference.identifier; node && node !== receiver.fn; node = receiver.unit.parents.get(node)) {
      step(context);
      const parent = receiver.unit.parents.get(node);
      if (parent?.type === 'CallExpression' && (
        ['useEffect', 'useLayoutEffect', 'useMemo', 'useCallback'].some((hook) => importedCall(receiver.unit, parent, 'react', hook))
        || parent.arguments.includes(reference.identifier))) return false;
      if (parent?.type === 'BinaryExpression' && ['===', '!==', '==', '!='].includes(parent.operator)) return false;
    }
  }
  proof(context, receiver.unit, receiver.fn);
  proof(context, unit, attribute);
  return true;
}

function stableResult(context, unit, node, depth = 0) {
  step(context);
  if (depth > 8) return false;
  node = unwrap(node);
  if (!node) return false;
  if (node.type === 'Literal' || node.type === 'Identifier' && node.name === 'undefined' && !unit.binding(node)) return true;
  if (node.type === 'ConditionalExpression') return stableResult(context, unit, node.consequent, depth + 1)
    && stableResult(context, unit, node.alternate, depth + 1);
  if (node.type !== 'JSXElement' && node.type !== 'JSXFragment') return false;
  if (node.type === 'JSXElement') {
    const opening = node.openingElement;
    if (opening.attributes.some((attribute) => attribute.type === 'JSXSpreadAttribute'
        || attribute.name.name === 'key' && attribute.value?.type !== 'Literal')) return false;
    if (!native(opening)) {
      const type = callable(context, unit, opening.name);
      if (!type || nearestFunction(type.unit, type.fn)) return false;
      proof(context, type.unit, type.fn);
    }
  }
  return node.children.every((child) => child.type === 'JSXText'
    || child.type === 'JSXExpressionContainer' && (child.expression.type === 'JSXEmptyExpression'
      || stableResult(context, unit, child.expression, depth + 1))
    || stableResult(context, unit, child, depth + 1));
}
function renderResult(context, unit, call, diagnostic) {
  if (call.callee.type !== 'Identifier' || !/^render[A-Z]/.test(call.callee.name)
      || diagnostic.message !== `Your users lose state because "${call.callee.name}()" builds UI from an inline call that React remounts, so pull it into its own component instead.`) return false;
  const helper = callable(context, unit, call.callee);
  if (!helper) return false;
  let hooks = false;
  walk(helper.unit, helper.fn.body, context, (node) => {
    if (node.type === 'CallExpression' && node.callee.type === 'Identifier' && /^use[A-Z]/.test(node.callee.name)) hooks = true;
  }, true);
  if (hooks) return false;
  const body = helper.fn.body;
  const returned = body.type === 'BlockStatement'
    ? body.body.length === 1 && body.body[0].type === 'ReturnStatement' ? body.body[0].argument : null
    : body;
  if (!stableResult(context, helper.unit, returned)) return false;
  proof(context, helper.unit, helper.fn);
  proof(context, unit, call);
  return true;
}

function dependencyVersion(context, name, major) {
  const file = 'web/app/package.json';
  const source = fs.readFileSync(path.join(context.repoRoot, file), 'utf8');
  if (source.length > 65536) return false;
  const parsed = JSON.parse(source);
  const version = parsed.dependencies?.[name] ?? parsed.devDependencies?.[name];
  if (typeof version !== 'string' || !new RegExp(`^[~^]?${major}\\.`).test(version)) return false;
  proof(context, { file, source }, { range: [0, source.length] });
  return true;
}
function librarySink(context, unit, opening, field) {
  const origin = jsxOrigin(unit, opening.name);
  if (origin?.module !== 'antd' || !dependencyVersion(context, 'antd', 6)) return false;
  if (!(origin.name === 'Button' && ['aria-label', 'ref'].includes(field)
      || origin.name === 'Tooltip' && field === 'title')) return false;
  importProof(context, unit, unit.binding(opening.name)?.identifiers[0]);
  proof(context, unit, opening);
  return true;
}
function forwardedSink(context, unit, opening, field, depth = 0) {
  step(context);
  if (depth > 4) return false;
  if (native(opening) || librarySink(context, unit, opening, field)) return true;
  const receiver = callable(context, unit, opening.name);
  if (!receiver) return false;
  if (field === 'ref') return receiver.forwardRef && forwardsRef(context, receiver.unit, receiver.fn, depth + 1);
  const pattern = receiver.fn.params[0];
  const rest = pattern?.type === 'ObjectPattern' && pattern.properties.find((property) => property.type === 'RestElement');
  const variable = rest?.argument.type === 'Identifier' && receiver.unit.binding(rest.argument);
  if (!immutable(variable)) return false;
  const reads = variable.references.filter((reference) => reference.isRead());
  if (!reads.length) return false;
  return reads.every((reference) => {
    const spread = receiver.unit.parents.get(reference.identifier);
    const target = receiver.unit.parents.get(spread);
    return spread?.type === 'JSXSpreadAttribute' && target?.type === 'JSXOpeningElement'
      && forwardedSink(context, receiver.unit, target, field, depth + 1);
  });
}
function attributeUse(context, unit, identifier, field) {
  const container = unit.parents.get(identifier);
  const attribute = unit.parents.get(container);
  const opening = unit.parents.get(attribute);
  if (container?.type !== 'JSXExpressionContainer' || container.expression !== identifier
      || attribute?.type !== 'JSXAttribute' || attribute.name.name !== field
      || opening?.type !== 'JSXOpeningElement') return false;
  const index = opening.attributes.indexOf(attribute);
  if (opening.attributes.slice(index + 1).some((item) => item.type === 'JSXSpreadAttribute'
      || item.name.name === field)) return false;
  if (!forwardedSink(context, unit, opening, field)) return false;
  proof(context, unit, attribute);
  return true;
}
function undefinedValue(unit, node) {
  return node?.type === 'Identifier' && node.name === 'undefined' && !unit.binding(node);
}
function accessibleNarrowing(context, unit, comparison) {
  if (!['===', '=='].includes(comparison.operator)) return false;
  const operands = [unwrap(comparison.left), unwrap(comparison.right)];
  const typed = operands.find((node) => node.type === 'UnaryExpression' && node.operator === 'typeof');
  if (!typed || !operands.some((node) => node.type === 'Literal' && node.value === 'string')
      || typed.argument.type !== 'Identifier') return false;
  const children = unit.binding(typed.argument);
  const fn = nearestFunction(unit, comparison);
  if (!fn || fieldBinding(unit, fn, 'children') !== children) return false;
  const conditional = unit.parents.get(comparison);
  if (conditional?.type !== 'ConditionalExpression' || conditional.test !== comparison
      || unit.binding(unwrap(conditional.consequent)) !== children
      || !undefinedValue(unit, unwrap(conditional.alternate))) return false;
  let initializer = conditional;
  const parent = unit.parents.get(initializer);
  if (parent?.type === 'LogicalExpression' && parent.operator === '??' && parent.right === initializer) {
    const explicit = unwrap(parent.left);
    if (memberName(explicit) !== 'aria-label') return false;
    const rest = fn.params[0].properties.find((property) => property.type === 'RestElement');
    if (!rest || unit.binding(explicit.object) !== unit.binding(rest.argument)) return false;
    initializer = parent;
  }
  const declaration = unit.parents.get(initializer);
  if (declaration?.type !== 'VariableDeclarator' || declaration.init !== initializer || declaration.id.type !== 'Identifier') return false;
  const scalar = unit.binding(declaration.id);
  if (!immutable(scalar)) return false;
  const reads = scalar.references.filter((reference) => reference.isRead());
  if (!reads.length) return false;
  for (const reference of reads) {
    const identifier = reference.identifier;
    if (attributeUse(context, unit, identifier, 'aria-label') || attributeUse(context, unit, identifier, 'title')) continue;
    const fallback = unit.parents.get(identifier);
    const container = unit.parents.get(fallback);
    if (fallback?.type !== 'LogicalExpression' || fallback.operator !== '??' || fallback.right !== identifier
        || unit.binding(unwrap(fallback.left)) !== children || container?.type !== 'JSXExpressionContainer'
        || unit.parents.get(container)?.type === 'JSXAttribute') return false;
    proof(context, unit, fallback);
  }
  proof(context, unit, declaration);
  proof(context, unit, comparison);
  return true;
}
function forwardsRef(context, unit, fn, depth = 0) {
  if (depth > 4 || fn.params.length !== 2 || fn.params[1].type !== 'Identifier') return false;
  const binding = unit.binding(fn.params[1]);
  if (!immutable(binding)) return false;
  const reads = binding.references.filter((reference) => reference.isRead());
  if (!reads.length) return false;
  for (const reference of reads) {
    const container = unit.parents.get(reference.identifier);
    const attribute = unit.parents.get(container);
    const opening = unit.parents.get(attribute);
    if (container?.type !== 'JSXExpressionContainer' || attribute?.type !== 'JSXAttribute'
        || attribute.name.name !== 'ref' || !forwardedSink(context, unit, opening, 'ref', depth + 1)) return false;
    proof(context, unit, attribute);
  }
  proof(context, unit, fn);
  return true;
}
function supportedRef(context, unit, node) {
  if (!dependencyVersion(context, 'react', 19)) return false;
  let variable;
  if (node.type === 'ImportSpecifier') {
    const declaration = unit.parents.get(node);
    if (declaration?.source.value !== 'react' || (node.imported.name ?? node.imported.value) !== 'forwardRef') return false;
    variable = unit.binding(node.local);
    proof(context, unit, declaration);
  } else if (node.type === 'MemberExpression') {
    const origin = importOrigin(unit, node.object);
    if (memberName(node) !== 'forwardRef' || origin?.module !== 'react' || !['*', 'default'].includes(origin.name)) return false;
  } else return false;
  const calls = unit.nodes.filter((candidate) => candidate.type === 'CallExpression'
    && importedCall(unit, candidate, 'react', 'forwardRef')
    && (variable ? unit.binding(candidate.callee) === variable : candidate.callee === node));
  if (!calls.length) return false;
  for (const call of calls) {
    if (call.arguments.length !== 1) return false;
    const render = callable(context, unit, call.arguments[0]);
    if (!render || !forwardsRef(context, render.unit, render.fn)) return false;
    proof(context, unit, call);
  }
  return true;
}

function createCompositionSemanticCorrections(repoRoot) {
  return (diagnostic, toolPackage) => {
    const rule = RULES[diagnostic?.rule];
    if (toolPackage !== TOOL || diagnostic?.plugin !== 'react-doctor' || !rule
        || rule.message && diagnostic.message !== rule.message || typeof diagnostic.filePath !== 'string') return null;
    const file = path.posix.normalize(`web/app/${diagnostic.filePath}`);
    if (!file.startsWith('web/app/src/') || diagnostic.filePath.startsWith('/') || diagnostic.filePath.includes('\\')) return null;
    try {
      const context = { repoRoot, steps: 0, evidence: new Map(), units: new Map() };
      context.read = (target) => {
        if (!context.units.has(target)) {
          if (context.units.size >= 4) return null;
          context.units.set(target, parseUnit(repoRoot, target));
        }
        return context.units.get(target);
      };
      const unit = context.read(file);
      if (!unit) return null;
      const types = diagnostic.rule === 'jsx-no-jsx-as-prop' ? ['JSXAttribute']
        : diagnostic.rule === 'no-render-in-render' ? ['CallExpression']
          : diagnostic.rule === 'no-polymorphic-children' ? ['BinaryExpression'] : ['ImportSpecifier', 'MemberExpression'];
      const node = atLocation(unit, diagnostic, types);
      if (!node) return null;
      let predicate;
      let reason;
      if (diagnostic.rule === 'jsx-no-jsx-as-prop' && ordinarySlot(context, unit, node)) {
        predicate = 'nonmemoized-slot-receiver';
        reason = 'The resolved receiver is an ordinary function or React forwardRef, with no memo bailout for this JSX identity to defeat. Its inspected slot use does not establish the asserted extra receiver render; this is not a performance guarantee for descendants.';
      } else if (diagnostic.rule === 'no-render-in-render' && renderResult(context, unit, node, diagnostic)) {
        predicate = 'stable-render-result';
        reason = 'The invoked hook-free helper returns elements with stable module-level types. Ordinary invocation adds no component boundary, so it does not by itself remount the returned subtree.';
      } else if (diagnostic.rule === 'no-polymorphic-children' && accessibleNarrowing(context, unit, node)) {
        predicate = 'accessible-name-narrowing';
        reason = 'The string check only narrows an accessible-name scalar. Every inspected use is a naming sink or a children-first nullish fallback that preserves the original children; it selects no component type or variant.';
      } else if (diagnostic.rule === 'no-react19-deprecated-apis' && supportedRef(context, unit, node)) {
        predicate = 'supported-forward-ref';
        reason = 'React19 still supports the genuine forwardRef calls inspected here, and their render callbacks actually forward refs. Ref-as-prop is a migration option; the documented future deprecation is not evidence that this current plumbing is broken.';
      } else return null;
      return {
        predicate, reason,
        provenance: {
          toolPackage: TOOL,
          packageIntegrity: 'sha512-o108QscqM/TlC1QmKgDORf/P53PeZ69nZbRsvy53rXkKvWdKZC1Y0u1JRPmThbTh6v+QpMispBwEnFHhlpZQ7w==',
          sourceCommit: COMMIT, ruleSourceSha256: rule.hash,
          references: [`https://github.com/millionco/react-doctor/blob/${COMMIT}/packages/oxlint-plugin-react-doctor/src/plugin/rules/${rule.source}`, ...rule.references],
        },
        evidence: [...context.evidence].map(([source, spans]) => ({ file: source.file,
          sha256: createHash('sha256').update(source.source).digest('hex'), spans: [...spans.values()] })),
      };
    } catch { return null; }
  };
}

module.exports = { createCompositionSemanticCorrections };
