const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { parseUnit, unwrap, memberName, immutable, importOrigin, importedCall } = require('./source-unit.js');

const TOOL = 'react-doctor@0.2.16';
const COMMIT = '2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc';
const FUNCTIONS = new Set(['FunctionDeclaration', 'FunctionExpression', 'ArrowFunctionExpression']);
const RULES = {
  'no-derived-useState': {
    hash: 'cc7730387e948eb92f3bc614156bdc57a9a5ddf50887a74f33718cc0e3c77991',
    source: 'state-and-effects/no-derived-use-state.ts',
    references: ['https://github.com/millionco/react-doctor/pull/2144', 'https://react.dev/learn/preserving-and-resetting-state'],
  },
  'rerender-state-only-in-handlers': {
    hash: '92f15323842cd00786ad17fe24178c448b57c584fd760cfb9207b64f21eff98a',
    source: 'state-and-effects/rerender-state-only-in-handlers.ts',
    references: ['https://github.com/millionco/react-doctor/pull/1973', 'https://react.dev/learn/state-a-components-memory'],
  },
};
function step(context) {
  if (++context.steps > 40000) throw new Error('draft/render proof budget exceeded');
}
function evidence(context, unit, node) {
  if (!node?.range) return;
  const record = context.evidence.get(unit.file) ?? { source: unit.source, spans: new Map() };
  record.spans.set(node.range.join(':'), { start: node.range[0], end: node.range[1] });
  context.evidence.set(unit.file, record);
}
function walk(unit, node, context, visit, skipFunctions = false) {
  const stack = [node];
  while (stack.length) {
    step(context);
    const current = stack.pop();
    if (!current || skipFunctions && current !== node && FUNCTIONS.has(current.type)) continue;
    if (visit(current) === false) continue;
    for (const key of unit.visitorKeys[current.type] || []) {
      const value = current[key];
      for (const child of Array.isArray(value) ? value : [value]) if (child?.type) stack.push(child);
    }
  }
}
function owner(unit, node) {
  for (let parent = unit.parents.get(node); parent; parent = unit.parents.get(parent)) if (FUNCTIONS.has(parent.type)) return parent;
  return null;
}
function inside(node, ancestor) {
  return Boolean(node && ancestor && node.range[0] >= ancestor.range[0] && node.range[1] <= ancestor.range[1]);
}
function same(unit, left, right) {
  left = unwrap(left); right = unwrap(right);
  if (!left || !right || left.type !== right.type) return false;
  if (left.type === 'Identifier') return Boolean(unit.binding(left) && unit.binding(left) === unit.binding(right));
  if (left.type === 'Literal') return left.value === right.value;
  return left.type === 'MemberExpression' && memberName(left) !== null && memberName(left) === memberName(right) && same(unit, left.object, right.object);
}
function reads(unit, expression, variable, context) {
  let found = false;
  walk(unit, expression, context, (node) => { if (node.type === 'Identifier' && unit.binding(node) === variable) found = true; });
  return found;
}
function fnBinding(unit, fn) {
  if (fn.type === 'FunctionDeclaration') return unit.binding(fn.id);
  const parent = unit.parents.get(fn);
  return parent?.type === 'VariableDeclarator' && parent.id.type === 'Identifier' ? unit.binding(parent.id) : null;
}
function propertyName(property) {
  return !property.computed && (property.key?.name ?? property.key?.value);
}
function propsBinding(unit, fn, name) {
  const pattern = fn.params[0];
  if (pattern?.type !== 'ObjectPattern') return null;
  const field = pattern.properties.find((entry) => entry.type === 'Property' && propertyName(entry) === name);
  let value = field?.value;
  if (value?.type === 'AssignmentPattern') value = value.left;
  return value?.type === 'Identifier' ? unit.binding(value) : null;
}
function fieldName(unit, fn, variable) {
  return fn.params[0]?.type === 'ObjectPattern' ? fn.params[0].properties.find((field) => field.type === 'Property'
    && unit.binding(field.value.type === 'AssignmentPattern' ? field.value.left : field.value) === variable)?.key.name : null;
}
function statement(unit, node) {
  for (let current = node; current; current = unit.parents.get(current)) {
    const parent = unit.parents.get(current);
    if (parent?.type === 'BlockStatement' || parent?.type === 'Program') return current;
  }
  return null;
}
function importEvidence(context, unit, node) {
  const variable = unit.binding(node);
  if (variable?.defs[0]?.type === 'ImportBinding') evidence(context, unit, unit.parents.get(variable.defs[0].node));
}
function relative(context, unit, source) {
  if (typeof source !== 'string' || !source.startsWith('.')) return null;
  const base = path.posix.normalize(path.posix.join(path.posix.dirname(unit.file), source));
  if (!base.startsWith('web/app/src/')) return null;
  const matches = [base, ...['.ts', '.tsx', '.js', '.jsx'].flatMap((extension) => [base + extension, `${base}/index${extension}`])]
    .filter((file) => { try { return fs.statSync(path.join(context.repoRoot, file)).isFile(); } catch { return false; } });
  return matches.length === 1 ? context.read(matches[0]) : null;
}
function resolveFunction(context, unit, expression, seen = new Set(), depth = 0) {
  step(context);
  if (depth > 10) return null;
  expression = unwrap(expression);
  if (!expression) return null;
  if (FUNCTIONS.has(expression.type)) {
    if (expression.generator) return null;
    evidence(context, unit, expression);
    return { unit, fn: expression };
  }
  if (!['Identifier', 'JSXIdentifier'].includes(expression.type)) return null;
  const binding = unit.binding(expression);
  if (!immutable(binding) || seen.has(binding)) return null;
  seen.add(binding);
  const definition = binding.defs[0];
  if (definition.type === 'FunctionName') return resolveFunction(context, unit, definition.node, seen, depth + 1);
  if (definition.type === 'Variable' && definition.node.id.type === 'Identifier') {
    evidence(context, unit, statement(unit, definition.node));
    return resolveFunction(context, unit, definition.node.init, seen, depth + 1);
  }
  if (definition.type !== 'ImportBinding' || definition.node.type !== 'ImportSpecifier') return null;
  const declaration = unit.parents.get(definition.node);
  const target = relative(context, unit, declaration.source.value);
  if (!target) return null;
  const name = definition.node.imported.name ?? definition.node.imported.value;
  const exports = target.ast.body.filter((entry) => entry.type === 'ExportNamedDeclaration' && (
    entry.declaration?.id?.name === name || entry.declaration?.declarations?.some((item) => item.id.name === name)));
  if (exports.length !== 1) return null;
  const declarationNode = exports[0].declaration;
  const id = declarationNode.id ?? declarationNode.declarations.find((entry) => entry.id.name === name).id;
  evidence(context, unit, declaration); evidence(context, target, exports[0]);
  return resolveFunction(context, target, id, seen, depth + 1);
}
function jsxOrigin(unit, name) {
  if (name?.type === 'JSXMemberExpression') {
    const root = jsxOrigin(unit, name.object);
    return root && { ...root, name: `${root.name}.${name.property.name}` };
  }
  const binding = unit.binding(name);
  return binding?.identifiers[0] ? importOrigin(unit, binding.identifiers[0]) : null;
}
const native = (opening) => opening?.name.type === 'JSXIdentifier' && /^[a-z][a-z0-9]*$/.test(opening.name.name);
const nativeControlled = (opening, field) => native(opening) && (field === 'value'
  ? ['input', 'textarea', 'select'].includes(opening.name.name)
  : field === 'checked' && opening.name.name === 'input');
function appDependency(context, name, version) {
  const file = 'web/app/package.json';
  const source = fs.readFileSync(path.join(context.repoRoot, file), 'utf8');
  if (source.length > 65536) return false;
  const data = JSON.parse(source);
  if (data.dependencies?.[name] !== version) return false;
  evidence(context, { file, source }, { range: [0, source.length] });
  return true;
}
// These are the declared provider API contracts, not application component
// eligibility names. Unknown exports, providers and local lookalikes stay opaque.
function provider(context, unit, opening, role, field) {
  const origin = jsxOrigin(unit, opening?.name);
  if (origin?.module !== 'antd' || !appDependency(context, 'antd', '6.6.2')) return false;
  const fields = role === 'edit' ? new Set(['onChange'])
    : role === 'value' ? new Set(['value', 'checked'])
      : role === 'menu' ? new Set(['menu']) : new Set(['onClick', 'onOk', 'onCancel', 'onOpenChange']);
  const exports = role === 'menu' ? ['Dropdown'] : role === 'edit' || role === 'value'
    ? ['Input', 'Input.TextArea', 'Input.Password', 'Input.Search', 'InputNumber', 'Select', 'Switch',
      'Checkbox', 'Checkbox.Group', 'Radio', 'Radio.Group', 'DatePicker', 'DatePicker.RangePicker']
    : ['Button', 'Modal', 'Dropdown'];
  if (!fields.has(field) || !exports.includes(origin.name)) return false;
  if (role === 'value' && (field === 'checked' ? !['Checkbox', 'Radio', 'Switch'].includes(origin.name)
    : ['Checkbox', 'Radio'].includes(origin.name))) return false;
  let root = opening.name;
  while (root.type === 'JSXMemberExpression') root = root.object;
  importEvidence(context, unit, unit.binding(root)?.identifiers[0]);
  evidence(context, unit, opening);
  return true;
}
function attribute(unit, opening, name) {
  const matches = opening.attributes.filter((entry) => entry.type === 'JSXAttribute' && entry.name.name === name);
  return matches.length === 1 ? matches[0] : null;
}
function valueOf(node) {
  return unwrap(node?.value?.type === 'JSXExpressionContainer' ? node.value.expression : node?.value);
}
function types(context, unit, node, seen = new Set()) {
  if (!node || seen.size > 8) return [];
  if (node.type === 'TSTypeAnnotation') return types(context, unit, node.typeAnnotation, seen);
  if (node.type === 'TSIntersectionType') return node.types.flatMap((part) => types(context, unit, part, seen));
  if (node.type === 'TSTypeLiteral' || node.type === 'TSInterfaceBody') return node.members ?? node.body;
  if (node.type !== 'TSTypeReference' || node.typeName.type !== 'Identifier') return [];
  const name = node.typeName.name;
  if (seen.has(name)) return [];
  seen.add(name);
  const declarations = unit.ast.body.flatMap((entry) => entry.type === 'ExportNamedDeclaration' ? [entry.declaration] : [entry])
    .filter((entry) => ['TSTypeAliasDeclaration', 'TSInterfaceDeclaration'].includes(entry?.type) && entry.id.name === name);
  if (declarations.length !== 1) return [];
  evidence(context, unit, declarations[0]);
  return types(context, unit, declarations[0].typeAnnotation ?? declarations[0].body, seen);
}

function stateInfo(unit, call) {
  if (!importedCall(unit, call, 'react', 'useState')) return null;
  const declaration = unit.parents.get(call);
  if (declaration?.type !== 'VariableDeclarator' || declaration.id.type !== 'ArrayPattern'
      || declaration.id.elements.length !== 2 || declaration.id.elements.some((node) => node?.type !== 'Identifier')) return null;
  const value = unit.binding(declaration.id.elements[0]);
  const setter = unit.binding(declaration.id.elements[1]);
  return immutable(value) && immutable(setter) ? { unit, call, declaration, value, setter, fn: owner(unit, call) } : null;
}
function functionReferences(unit, fn) {
  const binding = fnBinding(unit, fn);
  return immutable(binding) ? binding.references.filter((reference) => reference.isRead()) : null;
}
function callbackProp(context, unit, fn, key) {
  step(context);
  const cacheKey = `${unit.file}:${fn.range[0]}:${key}`;
  if (context.callbacks.has(cacheKey)) return context.callbacks.get(cacheKey) !== false;
  context.callbacks.set(cacheKey, null);
  const parameter = propsBinding(unit, fn, key);
  if (parameter) {
    const references = parameter.references.filter((reference) => reference.isRead());
    const valid = references.length > 0 && references.every((reference) => {
      const parent = unit.parents.get(reference.identifier);
      if (parent?.type === 'CallExpression' && parent.callee === reference.identifier) {
        const caller = owner(unit, parent);
        return caller !== fn && eventFunction(context, unit, caller);
      }
      const container = unit.parents.get(reference.identifier);
      const passed = unit.parents.get(container);
      return container?.type === 'JSXExpressionContainer' && passed?.type === 'JSXAttribute' && eventAttribute(context, unit, passed);
    });
    context.callbacks.set(cacheKey, valid);
    return valid;
  }
  // A transparent props forwarder is a closed transport only when that exact
  // event property is neither invoked nor overridden before the rendered host.
  const props = fn.params[0];
  if (props?.type !== 'Identifier') { context.callbacks.set(cacheKey, false); return false; }
  const binding = unit.binding(props);
  let forwarded = false;
  for (const reference of binding.references.filter((entry) => entry.isRead())) {
    const parent = unit.parents.get(reference.identifier);
    if (parent?.type === 'MemberExpression' && memberName(parent) !== key) continue;
    const opening = unit.parents.get(parent);
    if (parent?.type !== 'JSXSpreadAttribute' || opening?.type !== 'JSXOpeningElement'
        || opening.attributes.slice(opening.attributes.indexOf(parent) + 1).some((entry) => entry.type === 'JSXSpreadAttribute' || entry.name.name === key)
        || !eventHost(context, unit, opening, key)) { context.callbacks.set(cacheKey, false); return false; }
    forwarded = true;
  }
  context.callbacks.set(cacheKey, forwarded);
  return forwarded;
}
function eventHost(context, unit, opening, key) {
  if (native(opening)) return ['onClick', 'onChange', 'onInput'].includes(key);
  if (provider(context, unit, opening, 'edit', key) || provider(context, unit, opening, 'event', key)) return true;
  const target = resolveFunction(context, unit, opening.name);
  return Boolean(target && callbackProp(context, target.unit, target.fn, key));
}
function eventAttribute(context, unit, attr) {
  const opening = unit.parents.get(attr);
  if (opening?.type !== 'JSXOpeningElement' || attr.name.type !== 'JSXIdentifier' || !eventHost(context, unit, opening, attr.name.name)) return false;
  evidence(context, unit, attr);
  return true;
}
function registryEvent(context, unit, fn, property) {
  let call, target;
  for (let node = property; node; node = unit.parents.get(node)) {
    if (FUNCTIONS.has(node.type)) {
      const producer = unit.parents.get(node);
      if (producer?.type !== 'CallExpression' || !['map', 'flatMap'].includes(memberName(producer.callee))
          || !arrayOrigin(context, unit, producer.callee.object)) break;
    }
    if (node.type === 'CallExpression' && node.callee.type === 'Identifier') {
      const resolved = resolveFunction(context, unit, node.callee);
      if (resolved) { call = node; target = resolved; break; }
    }
  }
  if (!call) return false;
  if (!target || target.fn.params.length !== 1 || target.fn.params[0].type !== 'Identifier') return false;
  const parameter = target.unit.binding(target.fn.params[0]);
  const refs = target.unit.nodes.filter((node) => importedCall(target.unit, node, 'react', 'useRef')
    && node.arguments[0]?.type === 'Identifier' && target.unit.binding(node.arguments[0]) === parameter);
  if (refs.length !== 1) return false;
  const declaration = target.unit.parents.get(refs[0]);
  if (declaration?.type !== 'VariableDeclarator' || declaration.id.type !== 'Identifier') return false;
  const ref = target.unit.binding(declaration.id);
  const writes = target.unit.nodes.filter((node) => node.type === 'AssignmentExpression' && memberName(node.left) === 'current'
    && target.unit.binding(node.left.object) === ref);
  if (writes.length !== 1 || writes[0].right.type !== 'Identifier' || target.unit.binding(writes[0].right) !== parameter) return false;
  let effect = target.unit.parents.get(owner(target.unit, writes[0]));
  if (!importedCall(target.unit, effect, 'react', 'useLayoutEffect')) return false;
  const key = propertyName(property);
  const invocations = target.unit.nodes.filter((node) => node.type === 'CallExpression' && memberName(node.callee) === key);
  if (!invocations.length || invocations.some((node) => {
    const wrapper = owner(target.unit, node);
    const container = target.unit.parents.get(wrapper);
    return !wrapper || container?.type !== 'Property' || propertyName(container) !== key;
  })) return false;
  evidence(context, target.unit, target.fn);
  evidence(context, unit, call);
  return true;
}
function eventFunction(context, unit, fn) {
  if (!fn) return false;
  const key = `${unit.file}:${fn.range[0]}`;
  if (context.eventFunctions.has(key)) return context.eventFunctions.get(key) === true;
  context.eventFunctions.set(key, false);
  let parent = unit.parents.get(fn);
  if (parent?.type === 'JSXExpressionContainer') {
    const attr = unit.parents.get(parent);
    const valid = attr?.type === 'JSXAttribute' && eventAttribute(context, unit, attr);
    context.eventFunctions.set(key, Boolean(valid));
    return Boolean(valid);
  }
  if (parent?.type === 'Property' && parent.value === fn) {
    for (let current = parent; current && !FUNCTIONS.has(current.type); current = unit.parents.get(current)) {
      const container = unit.parents.get(current);
      if (container?.type === 'JSXExpressionContainer') {
        const attr = unit.parents.get(container);
        const opening = unit.parents.get(attr);
        if (attr?.type === 'JSXAttribute' && propertyName(parent) === 'onClick'
            && provider(context, unit, opening, 'menu', attr.name.name)) {
          evidence(context, unit, attr); context.eventFunctions.set(key, true); return true;
        }
      }
    }
    const valid = registryEvent(context, unit, fn, parent);
    context.eventFunctions.set(key, valid); return valid;
  }
  const references = functionReferences(unit, fn);
  const valid = Boolean(references?.length && references.every((reference) => {
    const use = unit.parents.get(reference.identifier);
    if (use?.type === 'CallExpression' && use.callee === reference.identifier) return eventFunction(context, unit, owner(unit, use));
    const container = unit.parents.get(use);
    return use?.type === 'JSXExpressionContainer' && container?.type === 'JSXAttribute' && eventAttribute(context, unit, container);
  }));
  context.eventFunctions.set(key, valid);
  return valid;
}

function collectCapability(context, unit, binding, visited = new Set()) {
  step(context);
  if (!immutable(binding) || visited.has(binding) || visited.size > 16) return null;
  visited.add(binding);
  const calls = [];
  for (const reference of binding.references.filter((entry) => entry.isRead())) {
    const id = reference.identifier;
    const parent = unit.parents.get(id);
    if (parent?.type === 'CallExpression' && parent.callee === id) { calls.push({ unit, call: parent }); continue; }
    const attr = unit.parents.get(parent);
    if (parent?.type !== 'JSXExpressionContainer' || attr?.type !== 'JSXAttribute') return null;
    const opening = unit.parents.get(attr);
    const receiver = resolveFunction(context, unit, opening.name);
    const callback = receiver && propsBinding(receiver.unit, receiver.fn, attr.name.name);
    if (!callback || !callbackProp(context, receiver.unit, receiver.fn, attr.name.name)) return null;
    for (const field of opening.attributes.filter((entry) => entry.type === 'JSXAttribute')) {
      const value = valueOf(field);
      if (value?.type !== 'Identifier' || !context.priors?.has(unit.binding(value))) continue;
      const alias = propsBinding(receiver.unit, receiver.fn, field.name.name);
      if (alias) context.priors.add(alias);
    }
    const forwarded = collectCapability(context, receiver.unit, callback, new Set(visited));
    if (!forwarded) return null;
    calls.push(...forwarded);
  }
  return calls;
}
function payloadParameter(context, unit, fn, parameter) {
  const index = fn.params.findIndex((node) => node.type === 'Identifier' && unit.binding(node) === parameter);
  const binding = fnBinding(unit, fn);
  if (index < 0 || !binding) return false;
  const calls = collectCapability(context, unit, binding);
  return Boolean(calls?.length && calls.every(({ unit: child, call }) => {
    const value = unwrap(call.arguments[index]);
    return value?.type === 'ObjectExpression' && value.properties.every((property) => property.type === 'Property' && property.kind === 'init')
      && eventFunction(context, child, owner(child, call));
  }));
}
function retainedUpdate(context, unit, expression, info) {
  expression = unwrap(expression);
  let prior = info.value;
  let callback;
  if (expression?.type === 'ArrowFunctionExpression') {
    callback = expression;
    if (callback.params.length !== 1 || callback.params[0].type !== 'Identifier' || callback.body.type === 'BlockStatement') return false;
    prior = unit.binding(callback.params[0]);
    expression = unwrap(callback.body);
  }
  if (expression?.type !== 'ObjectExpression' || expression.properties[0]?.type !== 'SpreadElement') return false;
  const base = unit.binding(unwrap(expression.properties[0].argument));
  if (callback ? base !== prior : !context.priors?.has(base)) return false;
  for (const property of expression.properties.slice(1)) {
    if (property.type === 'Property' && property.kind === 'init') continue;
    if (property.type !== 'SpreadElement' || property.argument.type !== 'Identifier') return false;
    const parameter = unit.binding(property.argument);
    const fn = owner(unit, callback ?? expression);
    if (parameter?.defs[0]?.type !== 'Parameter' || !fn || !payloadParameter(context, unit, fn, parameter)) return false;
  }
  evidence(context, unit, callback ?? expression);
  return expression.properties.length > 1;
}
function controlledEdit(context, info) {
  const unit = info.unit;
  const attributes = unit.nodes.filter((node) => node.type === 'JSXAttribute' && ['value', 'checked'].includes(node.name.name)
    && inside(node, info.fn) && reads(unit, valueOf(node), info.value, context));
  for (const attr of attributes) {
    const opening = unit.parents.get(attr);
    if (nativeControlled(opening, attr.name.name) || provider(context, unit, opening, 'value', attr.name.name)) {
      const change = attribute(unit, opening, 'onChange') ?? attribute(unit, opening, 'onInput');
      if (change && eventAttribute(context, unit, change)) { evidence(context, unit, attr); return true; }
    } else {
      const target = resolveFunction(context, unit, opening.name);
      const field = target && propsBinding(target.unit, target.fn, attr.name.name);
      if (!field) continue;
      const childAttrs = target.unit.nodes.filter((node) => node.type === 'JSXAttribute' && ['value', 'checked'].includes(node.name.name)
        && inside(node, target.fn) && reads(target.unit, valueOf(node), field, context));
      if (childAttrs.some((child) => nativeControlled(target.unit.parents.get(child), child.name.name) || provider(context, target.unit, target.unit.parents.get(child), 'value', child.name.name))) {
        evidence(context, target.unit, target.fn); evidence(context, unit, attr); return true;
      }
    }
  }
  // A private field section may name its value prop freely. Its source must
  // actually bind that field to a rendered controlled input, not merely type it.
  for (const attr of unit.nodes.filter((node) => node.type === 'JSXAttribute' && inside(node, info.fn)
      && reads(unit, valueOf(node), info.value, context))) {
    const opening = unit.parents.get(attr);
    const target = resolveFunction(context, unit, opening.name);
    const field = target && propsBinding(target.unit, target.fn, attr.name.name);
    if (!field) continue;
    if (target.unit.nodes.some((node) => node.type === 'JSXAttribute' && ['value', 'checked'].includes(node.name.name)
        && inside(node, target.fn) && reads(target.unit, valueOf(node), field, context)
        && (nativeControlled(target.unit.parents.get(node), node.name.name) || provider(context, target.unit, target.unit.parents.get(node), 'value', node.name.name)))) {
      evidence(context, target.unit, target.fn); evidence(context, unit, attr); return true;
    }
  }
  return false;
}
function sourceValue(context, unit, value, info, seen = new Set()) {
  value = unwrap(value);
  if (!value) return false;
  if (unit === info.unit && value.type === 'Identifier' && unit.binding(value) === info.value) return true;
  if (value.type === 'ConditionalExpression') return sourceValue(context, unit, value.consequent, info, seen)
    && sourceValue(context, unit, value.alternate, info, seen);
  if (value.type === 'ObjectExpression') return value.properties[0]?.type === 'SpreadElement'
    && sourceValue(context, unit, value.properties[0].argument, info, seen)
    && value.properties.slice(1).every((property) => property.type === 'Property');
  if (value.type !== 'Identifier') return false;
  const binding = unit.binding(value);
  if (!immutable(binding) || seen.has(binding)) return false;
  seen.add(binding);
  const definition = binding.defs[0];
  if (definition.type !== 'Variable' || definition.node.id.type !== 'Identifier') return false;
  evidence(context, unit, statement(unit, definition.node));
  return sourceValue(context, unit, definition.node.init, info, seen);
}
function closeLiteral(node) {
  node = unwrap(node);
  return node?.type === 'Literal' && (node.value === false || node.value === null);
}
function localControls(context, info) {
  return info.unit.nodes.filter((node) => node.type === 'CallExpression' && owner(info.unit, node) === info.fn && node !== info.call)
    .map((node) => stateInfo(info.unit, node)).filter((state) => state && closeLiteral(state.call.arguments[0]));
}
function privateMount(context, info) {
  const unit = info.unit;
  const references = functionReferences(unit, info.fn);
  if (!references?.length || unit.parents.get(info.fn)?.type === 'ExportNamedDeclaration') return null;
  const mounts = [];
  for (const reference of references) {
    const opening = unit.parents.get(reference.identifier);
    if (opening?.type === 'JSXClosingElement') continue;
    if (opening?.type !== 'JSXOpeningElement') return null;
    const element = unit.parents.get(opening);
    const parentOwner = owner(unit, element);
    let guard;
    for (let node = element; node && node !== parentOwner; node = unit.parents.get(node)) {
      if (node.type === 'LogicalExpression' && node.operator === '&&' && inside(element, node.right)) { guard = node; break; }
    }
    if (!guard || !parentOwner || unwrap(guard.left).type !== 'Identifier') return null;
    const sourceName = fieldName(unit, info.fn, unit.binding(info.call.arguments[0]));
    if (!sourceName) return null;
    const explicit = attribute(unit, opening, sourceName);
    const spreads = opening.attributes.filter((entry) => entry.type === 'JSXSpreadAttribute');
    if (!explicit && !spreads.length) return null;
    evidence(context, unit, guard);
    mounts.push({ parentOwner, opening, guard, explicit, spreads });
  }
  return mounts.length ? mounts : null;
}
function propControl(context, unit, fn, call) {
  if (call.callee.type !== 'Identifier' || !closeLiteral(call.arguments[0])) return false;
  const variable = unit.binding(call.callee);
  if (variable?.defs[0]?.type === 'Parameter') fn = variable.defs[0].node;
  const key = fieldName(unit, fn, variable);
  if (!key) return false;
  const fields = types(context, unit, fn.params[0]?.typeAnnotation);
  const callback = fields.find((field) => propertyName(field) === key && field.type === 'TSMethodSignature');
  const argument = callback?.params?.[0];
  const domain = argument?.typeAnnotation?.typeAnnotation;
  if (!domain) return false;
  const signature = unit.source.slice(...domain.range);
  return fields.some((field) => field.type === 'TSPropertySignature' && field.typeAnnotation
    && unit.source.slice(...field.typeAnnotation.typeAnnotation.range) === signature);
}
function closeAction(context, unit, expression, controls, mounts, active = new Set()) {
  expression = unwrap(expression);
  if (!expression) return false;
  if (expression.type === 'CallExpression') {
    if (controls.some((control) => unit === control.unit && unit.binding(expression.callee) === control.setter && closeLiteral(expression.arguments[0]))) return true;
    if (mounts && propControl(context, unit, owner(unit, expression), expression)) return true;
    if (mounts && !expression.arguments.length && expression.callee.type === 'Identifier'
        && unit.binding(expression.callee)?.defs[0]?.type === 'Parameter') {
      return closeAction(context, unit, expression.callee, controls, mounts, active);
    }
  }
  const target = resolveFunction(context, unit, expression.type === 'CallExpression' ? expression.callee : expression);
  if (target && !active.has(target.fn)) {
    active.add(target.fn);
    let closed = false;
    walk(target.unit, target.fn.body, context, (node) => {
      if (node.type === 'CallExpression' && closeAction(context, target.unit, node, controls, mounts, active)) closed = true;
    }, true);
    if (closed) evidence(context, target.unit, target.fn);
    return closed;
  }
  if (expression.type === 'Identifier' && mounts) {
    const binding = unit.binding(expression);
    const definingOwner = binding?.defs[0]?.type === 'Parameter' ? binding.defs[0].node : owner(unit, expression);
    const name = fieldName(unit, definingOwner, binding);
    if (!name) return false;
    return mounts.every(({ opening, parentOwner }) => {
      const supplied = attribute(unit, opening, name);
      if (!supplied) return false;
      const parentControls = unit.nodes.filter((node) => node.type === 'CallExpression' && owner(unit, node) === parentOwner)
        .map((node) => stateInfo(unit, node)).filter(Boolean);
      return closeAction(context, unit, valueOf(supplied), parentControls, null, active);
    });
  }
  return false;
}
function applyCancel(context, info, controls, mounts) {
  const unit = info.unit;
  const commits = unit.nodes.filter((node) => node.type === 'CallExpression' && inside(node, info.fn)
    && node.callee.type === 'Identifier' && unit.binding(node.callee)?.defs[0]?.type === 'Parameter'
    && node.arguments.length === 1 && sourceValue(context, unit, node.arguments[0], info));
  const applies = commits.filter((call) => {
    const awaited = unit.parents.get(call);
    const condition = unit.parents.get(awaited);
    if (awaited?.type !== 'AwaitExpression' || condition?.type !== 'IfStatement' || condition.test !== awaited
        || !eventFunction(context, unit, owner(unit, call))) return false;
    let closed = false;
    walk(unit, condition.consequent, context, (node) => {
      if (node.type === 'CallExpression' && closeAction(context, unit, node, controls, mounts)) closed = true;
    }, true);
    if (closed) evidence(context, unit, condition);
    return closed;
  });
  if (!applies.length) return false;
  const commitBindings = new Set(commits.map((call) => unit.binding(call.callee)));
  const callbacks = unit.nodes.filter((node) => node.type === 'JSXAttribute' && inside(node, info.fn)
    && ['onClick', 'onCancel'].includes(node.name.name) && eventAttribute(context, unit, node));
  return callbacks.some((attr) => {
    const value = valueOf(attr);
    if (value?.type === 'Identifier' && closeAction(context, unit, value, controls, mounts)) { evidence(context, unit, attr); return true; }
    const target = resolveFunction(context, unit, value);
    if (!target) return false;
    let commit = false, closed = false;
    walk(target.unit, target.fn.body, context, (node) => {
      if (node.type !== 'CallExpression') return;
      if (commitBindings.has(target.unit.binding(node.callee))) commit = true;
      if (closeAction(context, target.unit, node, controls, mounts)) closed = true;
    }, true);
    if (!commit && closed) evidence(context, unit, attr);
    return !commit && closed;
  });
}
function seedReset(context, info, call, controls) {
  const unit = info.unit;
  if (call.arguments.length !== 1 || !same(unit, call.arguments[0], info.call.arguments[0])) return false;
  const fn = owner(unit, call);
  if (!fn || fn === info.fn || !eventFunction(context, unit, fn)) return false;
  let awaitBefore = false;
  walk(unit, fn.body, context, (node) => { if (node.type === 'AwaitExpression' && node.range[0] < call.range[0]) awaitBefore = true; }, true);
  if (awaitBefore) return false;
  const copyStatement = statement(unit, call);
  return controls.some((control) => unit.nodes.some((node) => node.type === 'CallExpression' && owner(unit, node) === fn
    && unit.binding(node.callee) === control.setter && !closeLiteral(node.arguments[0])
    && node.range[0] > call.range[0] && unit.parents.get(statement(unit, node)) === unit.parents.get(copyStatement)));
}
function draftProof(context, unit, call, diagnostic) {
  const info = stateInfo(unit, call);
  const seed = unwrap(call.arguments[0]);
  if (!info || seed?.type !== 'Identifier' || !fieldName(unit, info.fn, unit.binding(seed))
      || diagnostic.message !== `Your users see a stale value when prop "${seed.name}" changes because useState copies it once.`) return false;
  const controls = localControls(context, info);
  const mounts = controls.length ? null : privateMount(context, info);
  if (!controls.length && !mounts || !controlledEdit(context, info)) return false;
  context.priors = new Set([info.value]);
  const calls = collectCapability(context, unit, info.setter);
  if (!calls?.length) return false;
  let edits = 0, resets = 0;
  for (const { unit: target, call: write } of calls) {
    if (target === unit && seedReset(context, info, write, controls)) { resets++; evidence(context, unit, write); continue; }
    if (!eventFunction(context, target, owner(target, write)) || write.arguments.length !== 1 || !retainedUpdate(context, target, write.arguments[0], info)) return false;
    edits++;
  }
  if (!edits || controls.length && !resets || !applyCancel(context, info, controls, mounts)) return false;
  evidence(context, unit, statement(unit, info.declaration));
  evidence(context, unit, info.fn);
  return true;
}

function arrayType(context, unit, node) {
  node = node?.type === 'TSTypeAnnotation' ? node.typeAnnotation : node;
  if (node?.type === 'TSTypeOperator' && node.operator === 'readonly') return arrayType(context, unit, node.typeAnnotation);
  return node?.type === 'TSArrayType' || node?.type === 'TSTypeReference' && ['Array', 'ReadonlyArray'].includes(node.typeName.name);
}
function arrayOrigin(context, unit, node, extra = new Set(), seen = new Set()) {
  step(context);
  node = unwrap(node);
  if (!node) return false;
  if (node.type === 'ArrayExpression') return true;
  if (node.type === 'CallExpression' && ['map', 'filter', 'flatMap', 'slice'].includes(memberName(node.callee))) return arrayOrigin(context, unit, node.callee.object, extra, seen);
  if (node.type !== 'Identifier') return false;
  const binding = unit.binding(node);
  if (extra.has(binding)) return true;
  if (!immutable(binding) || seen.has(binding)) return false;
  seen.add(binding);
  const definition = binding.defs[0];
  if (definition.type === 'Variable' && definition.node.id.type === 'Identifier') {
    evidence(context, unit, statement(unit, definition.node));
    return arrayOrigin(context, unit, definition.node.init, extra, seen);
  }
  if (definition.type !== 'Parameter') return false;
  const fn = definition.node;
  const pattern = fn.params?.[0];
  if (pattern?.type !== 'ObjectPattern') return false;
  const name = fieldName(unit, fn, binding);
  const field = types(context, unit, pattern.typeAnnotation).find((entry) => propertyName(entry) === name);
  return Boolean(field && arrayType(context, unit, field.typeAnnotation));
}
function eager(context, unit, fn, root) {
  if (fn === root) return true;
  const call = unit.parents.get(fn);
  return call?.type === 'CallExpression' && ['map', 'filter', 'flatMap', 'forEach'].includes(memberName(call.callee))
    && arrayOrigin(context, unit, call.callee.object) && eager(context, unit, owner(unit, call), root);
}
function returned(unit, node, fn) {
  for (let parent = node; parent && parent !== fn; parent = unit.parents.get(parent)) {
    if (parent.type === 'ReturnStatement' && owner(unit, parent) === fn) return true;
  }
  return fn.body.type !== 'BlockStatement' && inside(node, fn.body);
}
function menuConsumer(context, unit, opening, attr, pathFields) {
  if (provider(context, unit, opening, 'menu', attr.name.name)) return pathFields.length > 0
    && pathFields.includes('label') && pathFields.every((name) => ['label', 'children', 'items'].includes(name));
  const target = resolveFunction(context, unit, opening.name);
  const input = target && propsBinding(target.unit, target.fn, attr.name.name);
  if (!input || pathFields.length !== 1) return false;
  const field = pathFields[0];
  let found = false;
  walk(target.unit, target.fn.body, context, (node) => {
    if (node.type !== 'CallExpression' || memberName(node.callee) !== 'map' || target.unit.binding(node.callee.object) !== input) return;
    const callback = unwrap(node.arguments[0]);
    if (!FUNCTIONS.has(callback?.type) || callback.params[0]?.type !== 'Identifier') return;
    const row = target.unit.binding(callback.params[0]);
    walk(target.unit, callback.body, context, (member) => {
      if (member.type !== 'MemberExpression' || memberName(member) !== field || target.unit.binding(member.object) !== row) return;
      const container = target.unit.parents.get(member);
      if (container?.type === 'JSXExpressionContainer' && target.unit.parents.get(container)?.type !== 'JSXAttribute'
          && returned(target.unit, node, target.fn)) found = true;
    });
  });
  if (found) evidence(context, target.unit, target.fn);
  return found;
}
function visibleFlow(context, unit, start, root) {
  const pending = [{ node: start, fields: [] }];
  const seen = new Set();
  while (pending.length) {
    step(context);
    const { node, fields } = pending.pop();
    if (!node || fields.length > 12) continue;
    const key = `${node.range[0]}:${node.range[1]}:${fields.join('.')}`;
    if (seen.has(key)) continue;
    seen.add(key);
    const parent = unit.parents.get(node);
    if (!parent) continue;
    const next = (target, keys = fields) => pending.push({ node: target, fields: keys });
    if (parent.type === 'Property' && parent.value === node) { next(parent, [...fields, propertyName(parent)]); continue; }
    if (parent.type === 'VariableDeclarator' && parent.init === node && parent.id.type === 'Identifier') {
      const binding = unit.binding(parent.id);
      if (!immutable(binding)) continue;
      evidence(context, unit, statement(unit, parent));
      for (const reference of binding.references.filter((entry) => entry.isRead())) next(reference.identifier);
      continue;
    }
    if (parent.type === 'CallExpression') {
      const method = memberName(parent.callee);
      if (['push', 'unshift'].includes(method) && parent.arguments.includes(node)
          && arrayOrigin(context, unit, parent.callee.object) && eager(context, unit, owner(unit, parent), root)) {
        const binding = unit.binding(parent.callee.object);
        if (!immutable(binding)) continue;
        evidence(context, unit, parent);
        for (const reference of binding.references.filter((entry) => entry.isRead())) {
          const member = unit.parents.get(reference.identifier);
          if (member?.type === 'MemberExpression') continue;
          next(reference.identifier);
        }
      }
      continue;
    }
    if (parent.type === 'ReturnStatement') {
      const fn = owner(unit, parent);
      if (fn === root) { evidence(context, unit, parent); return true; }
      if (eager(context, unit, fn, root)) next(unit.parents.get(fn));
      continue;
    }
    if (FUNCTIONS.has(parent.type)) {
      if (parent.body === node && eager(context, unit, parent, root)) next(unit.parents.get(parent));
      continue;
    }
    if (parent.type === 'JSXExpressionContainer') {
      const attr = unit.parents.get(parent);
      if (attr?.type === 'JSXAttribute') {
        const opening = unit.parents.get(attr);
        if (menuConsumer(context, unit, opening, attr, fields) && returned(unit, opening, root)) {
          evidence(context, unit, attr); return true;
        }
        continue;
      }
    }
    if (['ObjectExpression', 'ArrayExpression', 'SpreadElement', 'Property', 'LogicalExpression', 'ConditionalExpression',
      'JSXExpressionContainer', 'JSXElement', 'JSXFragment', 'TSAsExpression', 'TSSatisfiesExpression'].includes(parent.type)) next(parent);
  }
  return false;
}
function renderedState(context, unit, declaration, diagnostic) {
  const info = declaration.init && stateInfo(unit, declaration.init);
  if (!info || diagnostic.message !== `Each update to "${declaration.id.elements[0].name}" redraws your component for nothing because this useState is set but never shown on screen.`) return false;
  for (const attr of unit.nodes.filter((node) => node.type === 'JSXAttribute' && node.name.name === 'value'
      && inside(node, info.fn) && valueOf(node)?.type === 'Identifier' && unit.binding(valueOf(node)) === info.value)) {
    const opening = unit.parents.get(attr);
    if (!nativeControlled(opening, 'value') && !provider(context, unit, opening, 'value', 'value')) continue;
    const element = unit.parents.get(opening);
    if (!eager(context, unit, owner(unit, element), info.fn) || !visibleFlow(context, unit, element, info.fn)) continue;
    evidence(context, unit, attr); evidence(context, unit, statement(unit, declaration));
    return true;
  }
  return false;
}
function atDiagnostic(unit, diagnostic, type) {
  const { line, column } = diagnostic;
  if (!Number.isInteger(line) || !Number.isInteger(column) || line < 1 || column < 1) return null;
  const lines = unit.source.split('\n');
  if (line > lines.length || column > lines[line - 1].length) return null;
  const offset = lines.slice(0, line - 1).reduce((sum, text) => sum + text.length + 1, 0) + column - 1;
  const found = unit.nodes.filter((node) => node.type === type && node.range[0] === offset);
  return found.length === 1 ? found[0] : null;
}
function createDraftRenderSemanticCorrections(repoRoot) {
  return (diagnostic, toolPackage) => {
    const rule = RULES[diagnostic?.rule];
    if (toolPackage !== TOOL || diagnostic?.plugin !== 'react-doctor' || !rule || typeof diagnostic.filePath !== 'string') return null;
    const file = path.posix.normalize(`web/app/${diagnostic.filePath}`);
    if (!file.startsWith('web/app/src/') || diagnostic.filePath.startsWith('/') || diagnostic.filePath.includes('\\')) return null;
    try {
      const context = { repoRoot, steps: 0, evidence: new Map(), units: new Map(), callbacks: new Map(), eventFunctions: new Map() };
      context.read = (target) => {
        if (!context.units.has(target)) {
          if (context.units.size >= 12) return null;
          context.units.set(target, parseUnit(repoRoot, target));
        }
        return context.units.get(target);
      };
      const unit = context.read(file);
      const draft = diagnostic.rule === 'no-derived-useState';
      const node = unit && atDiagnostic(unit, diagnostic, draft ? 'CallExpression' : 'VariableDeclarator');
      if (!node || !(draft ? draftProof(context, unit, node, diagnostic) : renderedState(context, unit, node, diagnostic))) return null;
      return {
        predicate: draft ? 'editable-session-draft' : 'rendered-collection-state',
        reason: draft
          ? 'The inspected state is an independently edited transaction draft. Its writes, controlled edit, apply/cancel paths and mount or explicit opening reset are source-bound; current props cannot reconstruct open-session edits.'
          : 'This state controls a rendered input reached through eager local collection population and an inspected rendered menu consumer. Replacing it with a ref would remove the render that updates the visible search control.',
        provenance: { toolPackage: TOOL,
          packageIntegrity: 'sha512-o108QscqM/TlC1QmKgDORf/P53PeZ69nZbRsvy53rXkKvWdKZC1Y0u1JRPmThbTh6v+QpMispBwEnFHhlpZQ7w==',
          sourceCommit: COMMIT, ruleSourceSha256: rule.hash,
          references: [`https://github.com/millionco/react-doctor/blob/${COMMIT}/packages/oxlint-plugin-react-doctor/src/plugin/rules/${rule.source}`, ...rule.references] },
        evidence: [...context.evidence].map(([name, record]) => ({ file: name, sha256: createHash('sha256').update(record.source).digest('hex'), spans: [...record.spans.values()] })),
      };
    } catch { return null; }
  };
}

module.exports = { createDraftRenderSemanticCorrections };
