const path = require('node:path');

const RTL_MODULE = /^@testing-library\/react(?:\/pure)?$/u;
const TEST_MODULES = new Set(['vitest', '@jest/globals']);
const FUNCTION_TYPES = new Set(['ArrowFunctionExpression', 'FunctionExpression', 'FunctionDeclaration']);

function unwrap(node) {
  while (node && ['ChainExpression', 'TSAsExpression', 'TSNonNullExpression', 'TSTypeAssertion'].includes(node.type)) node = node.expression;
  return node;
}

function variable(identifier, source) {
  if (identifier?.type !== 'Identifier') return null;
  for (let scope = source.getScope(identifier); scope; scope = scope.upper) {
    const found = scope.set.get(identifier.name);
    if (found) return found;
  }
  return null;
}

function binding(identifier, source) {
  const entry = variable(identifier, source);
  if (entry?.defs.length !== 1) return null;
  const definition = entry.defs[0];
  if (definition.type !== 'ImportBinding') return null;
  const node = definition.node;
  const declaration = definition.parent ?? node.parent;
  if (declaration.type !== 'ImportDeclaration' || declaration.importKind === 'type' || node.importKind === 'type') return null;
  return {
    module: declaration.source.value,
    imported: node.type === 'ImportNamespaceSpecifier' ? '*' : node.type === 'ImportSpecifier' ? node.imported.name ?? node.imported.value : 'default'
  };
}

function property(member) {
  if (!member.computed && member.property.type === 'Identifier') return member.property.name;
  return member.computed && member.property.type === 'Literal' ? member.property.value : null;
}

function importedCall(call, source) {
  let callee = unwrap(call?.callee);
  const members = [];
  while (callee?.type === 'MemberExpression') {
    const name = property(callee);
    if (typeof name !== 'string') return null;
    members.unshift(name);
    callee = unwrap(callee.object);
  }
  const imported = binding(callee, source);
  if (!imported) return null;
  return { ...imported, name: imported.imported === '*' ? members.shift() : imported.imported, members };
}

function walk(node, source, visitor, skipFunctions = false) {
  if (!node) return;
  visitor(node);
  for (const key of source.visitorKeys[node.type] ?? []) {
    const children = Array.isArray(node[key]) ? node[key] : [node[key]];
    for (const child of children) {
      if (!child || (skipFunctions && FUNCTION_TYPES.has(child.type))) continue;
      walk(child, source, visitor, skipFunctions);
    }
  }
}

function isAct(call, source) {
  const origin = importedCall(call, source);
  return (origin?.name === 'act' && (RTL_MODULE.test(origin.module) || ['react', 'react-dom/test-utils'].includes(origin.module))) ||
    (origin?.module === 'react' && origin.imported === 'default' && origin.members.join('.') === 'act');
}

function callForReport(node) {
  for (let current = node; current; current = current.parent) {
    if (current.type === 'CallExpression') return current;
    if (current.type === 'ExpressionStatement') return null;
  }
  return null;
}

function isUserEventCall(call, source) {
  if (importedCall(call, source)?.module === '@testing-library/user-event') return true;
  const callee = unwrap(call.callee);
  const object = callee?.type === 'MemberExpression' ? unwrap(callee.object) : null;
  const init = variable(object, source)?.defs?.[0]?.node?.init;
  return init?.type === 'CallExpression' && importedCall(init, source)?.module === '@testing-library/user-event';
}

function isAwaitedAsyncInteraction(report, source) {
  const call = callForReport(report.node);
  const callback = call?.arguments[0];
  if (!isAct(call, source) || call.parent?.type !== 'AwaitExpression' || !FUNCTION_TYPES.has(callback?.type) || !callback.async) return false;
  let interaction = false;
  let alreadyAsync = false;
  walk(callback.body, source, (node) => {
    if (node.type !== 'CallExpression') return;
    const origin = importedCall(node, source);
    if (origin && RTL_MODULE.test(origin.module) && ['render', 'renderHook', 'fireEvent'].includes(origin.name)) interaction = true;
    const callee = unwrap(node.callee);
    const name = callee?.type === 'Identifier' ? callee.name : callee?.type === 'MemberExpression' ? property(callee) : '';
    // Async helpers already own their act boundary. Never waive a mixed scope.
    if (/^find(?:All)?By/u.test(name ?? '') || ['waitFor', 'waitForElementToBeRemoved'].includes(name) ||
      (origin && ['waitFor', 'waitForElementToBeRemoved'].includes(origin.name)) || isUserEventCall(node, source)) alreadyAsync = true;
  }, true);
  return interaction && !alreadyAsync;
}

function isAfterEach(call, source) {
  const callee = unwrap(call?.callee);
  if (callee?.type !== 'Identifier') return false;
  const origin = binding(callee, source);
  if (origin) return origin.imported === 'afterEach' && TEST_MODULES.has(origin.module);
  return callee.name === 'afterEach' && !(variable(callee, source)?.defs.length);
}

function registeredHook(callback, source) {
  if (callback.parent?.type === 'CallExpression' && callback.parent.arguments[0] === callback) return isAfterEach(callback.parent, source);
  const name = callback.type === 'FunctionDeclaration' ? callback.id : callback.parent?.type === 'VariableDeclarator' ? callback.parent.id : null;
  const entry = variable(name, source);
  const references = entry?.references.filter((reference) => reference.identifier !== name) ?? [];
  return references.length > 0 && references.every(({ identifier }) => identifier.parent?.type === 'CallExpression' && identifier.parent.arguments[0] === identifier && isAfterEach(identifier.parent, source));
}

function ownedByAfterEach(node, source) {
  for (let ancestor = node.parent; ancestor; ancestor = ancestor.parent) {
    if (!FUNCTION_TYPES.has(ancestor.type)) continue;
    if (registeredHook(ancestor, source)) return true;
    const call = ancestor.parent;
    if (!isAct(call, source) || call.arguments[0] !== ancestor || (ancestor.async && call.parent?.type !== 'AwaitExpression')) return false;
  }
  return false;
}

function queryClientType(annotation, source) {
  const type = annotation?.type === 'TSTypeAnnotation' ? annotation.typeAnnotation : annotation;
  const origin = type?.type === 'TSTypeReference' ? binding(type.typeName, source) : null;
  return origin?.module === '@tanstack/react-query' && origin.imported === 'QueryClient';
}

function queryClientCollection(identifier, source) {
  const entry = variable(identifier, source);
  if (entry?.defs.length !== 1) return false;
  const init = entry.defs[0].node?.init;
  if (init?.type !== 'NewExpression' || init.callee.type !== 'Identifier' || init.callee.name !== 'Set' || variable(init.callee, source)?.defs.length) return false;
  return queryClientType((init.typeArguments ?? init.typeParameters)?.params?.[0], source);
}

function queryClient(identifier, source, depth = 0) {
  if (depth > 4) return false;
  const entry = variable(identifier, source);
  if (entry?.defs.length !== 1) return false;
  const definition = entry.defs[0];
  if (queryClientType(definition.name?.typeAnnotation, source)) return true;
  const declarator = definition.node;
  if (declarator.type !== 'VariableDeclarator') return false;
  const init = unwrap(declarator.init);
  if (init?.type === 'NewExpression') {
    const origin = binding(init.callee, source);
    return origin?.module === '@tanstack/react-query' && origin.imported === 'QueryClient';
  }
  if (init?.type === 'CallExpression') {
    const origin = importedCall(init, source);
    return origin?.module === '@tanstack/react-query' && origin.name === 'useQueryClient';
  }
  if (init?.type === 'Identifier') return queryClient(init, source, depth + 1);
  const loop = declarator.parent?.parent;
  return loop?.type === 'ForOfStatement' && queryClientCollection(loop.right, source);
}

function externalReset(call, source) {
  const origin = importedCall(call, source);
  if (origin) {
    const module = origin.module.replace(/\.[cm]?[jt]sx?$/u, '');
    if (origin.name === 'resetAuthStore' && module.endsWith('/state/auth-store')) return true;
    if (origin.name === 'resetFrontstageDesignModeStore' && module.endsWith('/state/frontstage-design-mode-store')) return true;
    if (origin.name === 'appI18n' && origin.members.join('.') === 'changeLanguage' && module.endsWith('/shared/i18n/app-i18n')) return true;
  }
  const callee = unwrap(call.callee);
  return callee?.type === 'MemberExpression' && property(callee) === 'clear' && queryClient(unwrap(callee.object), source);
}

function sequence(block, source) {
  const steps = [];
  for (const statement of block.body) {
    if (statement.type === 'BlockStatement') steps.push(...sequence(statement, source));
    else if (statement.type === 'ForOfStatement') {
      const body = statement.body.type === 'BlockStatement' ? sequence(statement.body, source) : expressionSteps(statement.body, source);
      if (body.length && body.every((step) => step.type === 'CallExpression' && externalReset(step, source))) steps.push(...body);
      else steps.push(null);
    } else if (statement.type === 'ExpressionStatement') steps.push(...expressionSteps(statement, source));
    else if (!['VariableDeclaration', 'EmptyStatement'].includes(statement.type)) steps.push(null);
  }
  return steps;
}

function expressionSteps(statement, source) {
  let expression = unwrap(statement.expression);
  if (expression?.type === 'AwaitExpression') expression = unwrap(expression.argument);
  if (expression?.type !== 'CallExpression') return [];
  const callback = expression.arguments[0];
  if (isAct(expression, source) && FUNCTION_TYPES.has(callback?.type) && (!callback.async || expression.parent?.type === 'AwaitExpression')) {
    return callback.body.type === 'BlockStatement' ? sequence(callback.body, source) : expressionSteps({ expression: callback.body }, source);
  }
  return [expression];
}

function orderedCleanup(call, source) {
  if (!ownedByAfterEach(call, source)) return false;
  for (let block = call.parent; block; block = block.parent) {
    if (block.type !== 'BlockStatement') continue;
    const steps = sequence(block, source);
    const index = steps.indexOf(call);
    if (index < 0) continue;
    for (const step of steps.slice(index + 1)) {
      if (!step) break;
      if (externalReset(step, source)) return true;
    }
  }
  return false;
}

function isOrderedCleanupImport(report, source) {
  const node = report.node;
  if (node.type === 'ImportSpecifier') {
    const origin = binding(node.local, source);
    if (!origin || !RTL_MODULE.test(origin.module) || origin.imported !== 'cleanup') return false;
    const references = variable(node.local, source)?.references ?? [];
    return references.length > 0 && references.every(({ identifier }) => {
      const call = identifier.parent;
      return call?.type === 'CallExpression' && call.callee === identifier && orderedCleanup(call, source);
    });
  }
  const call = node.parent?.type === 'MemberExpression' ? node.parent.parent : null;
  const origin = importedCall(call, source);
  return origin?.name === 'cleanup' && RTL_MODULE.test(origin.module) && orderedCleanup(call, source);
}

function isRendererRegistryFactory(report, source, filename) {
  let init = unwrap(report.node.type === 'VariableDeclarator' ? report.node.init : null);
  if (init?.type === 'AwaitExpression') init = unwrap(init.argument);
  const origin = importedCall(init, source);
  if (origin?.name !== 'createRendererRegistry' || !origin.module.startsWith('.')) return false;
  const target = path.resolve(path.dirname(filename), origin.module).replace(/\.[cm]?[jt]sx?$/u, '').split(path.sep).join('/');
  return target.endsWith('/web/app/src/shared/schema-ui/v1/registry/create-renderer-registry');
}

function isUnitHarness(source, filename) {
  const normalized = filename.split(path.sep).join('/');
  return normalized.includes('/_tests/') && !normalized.includes('/browser-fixtures/') && source.ast.body.some((node) =>
    node.type === 'ImportDeclaration' && node.importKind !== 'type' && node.specifiers.some((specifier) => specifier.importKind !== 'type') &&
    (TEST_MODULES.has(node.source.value) || RTL_MODULE.test(node.source.value)));
}

function reactHook(call, name, source) {
  const origin = importedCall(call, source);
  if (origin?.module !== 'react') return false;
  return (origin.name === name && origin.members.length === 0) ||
    (origin.imported === 'default' && origin.members.join('.') === name);
}

function inEffect(identifier, source) {
  for (let ancestor = identifier.parent; ancestor; ancestor = ancestor.parent) {
    if (!FUNCTION_TYPES.has(ancestor.type)) continue;
    const call = ancestor.parent;
    if (call?.type === 'CallExpression' && call.arguments[0] === ancestor &&
      ['useEffect', 'useLayoutEffect', 'useInsertionEffect'].some((name) => reactHook(call, name, source))) return true;
  }
  return false;
}

function effectEventBinding(identifier, source) {
  const entry = variable(identifier, source);
  if (entry?.defs.length !== 1) return false;
  const declaration = entry.defs[0].node;
  if (declaration.type !== 'VariableDeclarator' || declaration.id.type !== 'Identifier' ||
    declaration.parent.kind !== 'const' || !reactHook(unwrap(declaration.init), 'useEffectEvent', source)) return false;
  const references = entry.references.filter(({ identifier: reference }) => reference !== declaration.id);
  return references.length > 0 && references.every(({ identifier: reference }) =>
    reference.parent?.type === 'CallExpression' && reference.parent.callee === reference && inEffect(reference, source));
}

function dependencyName(node) {
  node = unwrap(node);
  if (node?.type === 'Identifier') return node.name;
  if (node?.type !== 'MemberExpression' || node.computed) return null;
  const object = dependencyName(node.object);
  return object ? `${object}.${property(node)}` : null;
}

function isEffectEventDependencyException(report, source) {
  const array = report.node;
  const effect = array?.parent;
  if (array?.type !== 'ArrayExpression' || effect?.type !== 'CallExpression' || effect.arguments[1] !== array ||
    !['useEffect', 'useLayoutEffect', 'useInsertionEffect'].some((name) => reactHook(effect, name, source))) return false;
  const message = report.message?.match(/^React Hook .+ has (?:a )?missing dependenc(?:y|ies): (.+)\. Either include (?:it|them) or remove the dependency array\.$/u);
  if (!message) return false;
  const missing = [...message[1].matchAll(/'([^']+)'/gu)].map((match) => match[1]);
  if (!missing.length || missing.some((name) => !/^[$A-Z_a-z][$\w]*$/u.test(name))) return false;
  const callback = effect.arguments[0];
  if (!FUNCTION_TYPES.has(callback?.type)) return false;
  // Resolve each missing name in the callback's lexical scope, not the array scope.
  const references = [];
  walk(callback.body, source, (node) => {
    if (node.type === 'Identifier' && missing.includes(node.name) && node.parent?.type === 'CallExpression' && node.parent.callee === node) references.push(node);
  });
  if (!missing.every((name) => references.some((node) => node.name === name && effectEventBinding(node, source)))) return false;
  const declared = array.elements.map(dependencyName);
  if (declared.some((name) => !name) || new Set(declared).size !== declared.length) return false;
  const suggestion = report.suggest?.length === 1 ? report.suggest[0] : null;
  if (typeof suggestion?.fix !== 'function') return false;
  let replacement = null;
  try {
    suggestion.fix({ replaceText(node, text) {
      if (node === array) replacement = text;
      return { range: node.range, text };
    } });
  } catch { return false; }
  if (!replacement?.startsWith('[') || !replacement.endsWith(']')) return false;
  const proposed = replacement.slice(1, -1).split(',').map((name) => name.trim().replaceAll('?.', '.'));
  if (proposed.some((name) => !/^[$A-Z_a-z][$\w]*(?:\.[$A-Z_a-z][$\w]*)*$/u.test(name))) return false;
  const expected = new Set([...declared, ...missing]);
  // A hidden unnecessary/duplicate/ordinary dependency must retain the report.
  return proposed.length === expected.size && proposed.every((name) => expected.has(name)) &&
    expected.size === declared.length + missing.length;
}

module.exports = {
  isAwaitedAsyncInteraction, isOrderedCleanupImport, isRendererRegistryFactory,
  isUnitHarness, isEffectEventDependencyException
};
