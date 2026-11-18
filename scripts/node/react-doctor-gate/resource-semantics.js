const fs = require("node:fs");
const path = require("node:path");
const { createHash } = require("node:crypto");
const {
  parseUnit,
  unwrap,
  memberName,
  immutable,
  importOrigin,
  importedCall,
} = require("./source-unit.js");

const TOOL = "react-doctor@0.2.16";
const FUNCTIONS = new Set([
  "FunctionDeclaration",
  "FunctionExpression",
  "ArrowFunctionExpression",
]);
const PROVENANCE = {
  toolPackage: TOOL,
  packageIntegrity:
    "sha512-o108QscqM/TlC1QmKgDORf/P53PeZ69nZbRsvy53rXkKvWdKZC1Y0u1JRPmThbTh6v+QpMispBwEnFHhlpZQ7w==",
  sourceCommit: "2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc",
  ruleSourceSha256:
    "39f93c68922a19ed90687c21bb01ba2cb62daa9a7dab39ccabc8c08ac2ac3f48",
  references: [
    "https://github.com/millionco/react-doctor/blob/2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc/packages/oxlint-plugin-react-doctor/src/plugin/rules/react-builtins/exhaustive-deps.ts",
    "https://github.com/facebook/react/blob/976e87ce22e417ae87395d4d0a5488eba3b07b02/packages/eslint-plugin-react-hooks/src/rules/ExhaustiveDeps.ts",
    "https://react.dev/reference/react/useRef",
  ],
};
const OBSERVATION_MESSAGE =
  "Pushing state up to a parent from a useEffect costs your users an extra render.";
const OBSERVATION_PROVENANCE = {
  toolPackage: TOOL,
  packageIntegrity: PROVENANCE.packageIntegrity,
  sourceCommit: PROVENANCE.sourceCommit,
  ruleSourceSha256:
    "c1ccf61ff705ec90f43e8b6893a0f8e64937af0704683c977157f260222996d8",
  references: [
    "https://github.com/millionco/react-doctor/blob/2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc/packages/oxlint-plugin-react-doctor/src/plugin/rules/state-and-effects/no-pass-live-state-to-parent.ts",
    "https://w3c.github.io/IntersectionObserver/#observe-a-target-element",
    "https://w3c.github.io/IntersectionObserver/#queue-an-intersection-observer-task",
    "https://w3c.github.io/IntersectionObserver/#notify-intersection-observers",
  ],
};

function step(context) {
  if (++context.steps > 20000)
    throw new Error("resource proof budget exceeded");
}
function walk(unit, root, context, visit, skipFunctions = false) {
  const stack = [root];
  while (stack.length) {
    step(context);
    const node = stack.pop();
    if (!node || (skipFunctions && node !== root && FUNCTIONS.has(node.type)))
      continue;
    if (visit(node) === false) continue;
    for (const key of unit.visitorKeys[node.type] || []) {
      const value = node[key];
      for (const child of Array.isArray(value) ? value : [value])
        if (child?.type) stack.push(child);
    }
  }
}
function evidence(context, unit, node) {
  const spans = context.evidence.get(unit) ?? new Map();
  spans.set(node.range.join(":"), { start: node.range[0], end: node.range[1] });
  context.evidence.set(unit, spans);
}
function enclosingFunction(unit, node) {
  for (
    let current = unit.parents.get(node);
    current;
    current = unit.parents.get(current)
  )
    if (FUNCTIONS.has(current.type)) return current;
  return null;
}
function declarationSpan(unit, definition) {
  const parent = unit.parents.get(definition.node);
  return ["VariableDeclaration", "ExportNamedDeclaration"].includes(
    parent?.type,
  )
    ? parent
    : definition.node;
}
function reactCall(unit, call, name, seen = new Set()) {
  if (importedCall(unit, call, "react", name)) return true;
  if (
    call?.type !== "CallExpression" ||
    unwrap(call.callee)?.type !== "Identifier"
  )
    return false;
  const variable = unit.binding(unwrap(call.callee));
  if (
    !immutable(variable) ||
    seen.has(variable) ||
    variable.defs[0].type !== "Variable"
  )
    return false;
  seen.add(variable);
  return reactCall(
    unit,
    { ...call, callee: unwrap(variable.defs[0].node.init) },
    name,
    seen,
  );
}
function refBinding(unit, expression, context, seen = new Set()) {
  expression = unwrap(expression);
  if (expression?.type !== "Identifier") return null;
  const variable = unit.binding(expression);
  if (!immutable(variable) || seen.has(variable)) return null;
  seen.add(variable);
  const definition = variable.defs[0];
  if (
    definition.type !== "Variable" ||
    definition.node.id.type !== "Identifier"
  )
    return null;
  if (reactCall(unit, definition.node.init, "useRef")) {
    if (context) evidence(context, unit, definition.node.init);
    return variable;
  }
  const result = refBinding(unit, definition.node.init, context, seen);
  if (result && context)
    evidence(context, unit, declarationSpan(unit, definition));
  return result;
}
function currentRead(context, unit, expression) {
  expression = unwrap(expression);
  return (
    memberName(expression) === "current" &&
    refBinding(unit, expression.object, null) === context.ref
  );
}
function localFunction(context, unit, expression, seen = new Set()) {
  step(context);
  expression = unwrap(expression);
  if (FUNCTIONS.has(expression?.type)) return { unit, fn: expression };
  if (expression?.type === "MemberExpression") {
    const object = objectValue(context, unit, expression.object, seen);
    const property =
      object && propertyNamed(object.node, memberName(expression));
    return property && FUNCTIONS.has(unwrap(property.value)?.type)
      ? { unit: object.unit, fn: unwrap(property.value) }
      : null;
  }
  if (expression?.type !== "Identifier") return null;
  const variable = unit.binding(expression);
  if (!immutable(variable) || seen.has(variable)) return null;
  seen.add(variable);
  const definition = variable.defs[0];
  if (definition.type === "FunctionName") {
    evidence(context, unit, declarationSpan(unit, definition));
    return { unit, fn: definition.node };
  }
  if (definition.type === "Variable")
    return localFunction(context, unit, definition.node.init, seen);
  const origin = importOrigin(unit, expression);
  if (!origin?.module.startsWith(".") || ["*", "default"].includes(origin.name))
    return null;
  const base = path.posix.normalize(
    path.posix.join(path.posix.dirname(unit.file), origin.module),
  );
  if (!base.startsWith("web/app/") || context.dependencies.size >= 4)
    return null;
  const candidates = [
    base,
    ...[".ts", ".tsx", ".js", ".jsx"].map((extension) => base + extension),
  ].filter((file) => {
    try {
      return fs.statSync(path.join(context.repoRoot, file)).isFile();
    } catch {
      return false;
    }
  });
  if (candidates.length !== 1) return null;
  const helper = context.read(candidates[0]);
  if (!helper) return null;
  const exports = helper.ast.body.filter(
    (statement) =>
      statement.type === "ExportNamedDeclaration" &&
      statement.declaration?.type === "FunctionDeclaration" &&
      statement.declaration.id?.name === origin.name,
  );
  if (exports.length !== 1) return null;
  context.dependencies.add(helper.file);
  evidence(context, helper, exports[0]);
  return { unit: helper, fn: exports[0].declaration };
}
function propertyNamed(object, name) {
  if (
    object?.type !== "ObjectExpression" ||
    object.properties.some(
      (property) =>
        property.type !== "Property" ||
        property.computed ||
        property.kind !== "init",
    )
  )
    return null;
  const properties = object.properties.filter(
    (property) => (property.key.name ?? property.key.value) === name,
  );
  return properties.length === 1 ? properties[0] : null;
}
function objectValue(context, unit, expression, seen = new Set()) {
  step(context);
  expression = unwrap(expression);
  if (expression?.type === "ObjectExpression")
    return { unit, node: expression };
  if (expression?.type === "Identifier") {
    const variable = unit.binding(expression);
    if (!immutable(variable) || seen.has(variable)) return null;
    seen.add(variable);
    const definition = variable.defs[0];
    return definition.type === "Variable" &&
      definition.node.id.type === "Identifier"
      ? objectValue(context, unit, definition.node.init, seen)
      : null;
  }
  if (expression?.type !== "CallExpression" || expression.optional) return null;
  const target = localFunction(context, unit, expression.callee, seen);
  if (
    !target ||
    target.fn.async ||
    target.fn.generator ||
    context.activeFunctions.has(target.fn)
  )
    return null;
  context.activeFunctions.add(target.fn);
  let result = null;
  if (target.fn.body.type !== "BlockStatement")
    result = objectValue(context, target.unit, target.fn.body, seen);
  else {
    const returns = [];
    walk(
      target.unit,
      target.fn.body,
      context,
      (node) => {
        if (node.type === "ReturnStatement") returns.push(node);
      },
      true,
    );
    if (returns.length === 1)
      result = objectValue(context, target.unit, returns[0].argument, seen);
  }
  context.activeFunctions.delete(target.fn);
  if (result) {
    evidence(context, target.unit, target.fn);
    evidence(context, unit, expression);
  }
  return result;
}
function containsType(unit, node, name, context) {
  let found = false;
  walk(unit, node, context, (child) => {
    if (
      child.type === "TSTypeReference" &&
      child.typeName.type === "Identifier" &&
      child.typeName.name === name &&
      !unit.binding(child.typeName)
    )
      found = true;
  });
  return found;
}
function ownsObjectMember(unit, node, object) {
  node = unwrap(node);
  return (
    node?.type === "MemberExpression" &&
    unwrap(node.object)?.type === "Identifier" &&
    unit.binding(unwrap(node.object)) === object
  );
}
function promiseResolver(unit, identifier, context) {
  const variable = unit.binding(identifier);
  if (!variable) return false;
  return variable.references.some((reference) => {
    if (
      !reference.isWrite() ||
      !reference.writeExpr ||
      reference.writeExpr.type !== "Identifier"
    )
      return false;
    const source = unit.binding(reference.writeExpr);
    const definition = source?.defs[0];
    if (definition?.type !== "Parameter") return false;
    const fn = definition.node;
    const call = unit.parents.get(fn);
    return (
      call?.type === "NewExpression" &&
      call.callee.type === "Identifier" &&
      call.callee.name === "Promise" &&
      !unit.binding(call.callee) &&
      unit.binding(fn.params[0]) === unit.binding(reference.writeExpr)
    );
  });
}
function localDisposal(context, object, method) {
  const property = propertyNamed(object.node, method);
  const fn = unwrap(property?.value);
  if (!FUNCTIONS.has(fn?.type) || fn.async || fn.generator || fn.params.length)
    return false;
  const unit = object.unit;
  const declaration = unit.parents.get(object.node);
  const owner =
    declaration?.type === "VariableDeclarator" &&
    declaration.id.type === "Identifier"
      ? unit.binding(declaration.id)
      : null;
  let valid = true;
  walk(unit, fn.body, context, (node) => {
    if (node.type === "AssignmentExpression") {
      if (
        !owner ||
        !ownsObjectMember(unit, node.left, owner) ||
        node.operator !== "=" ||
        unwrap(node.right)?.type !== "Literal"
      )
        valid = false;
    }
    if (node.type !== "CallExpression") return;
    const callee = unwrap(node.callee);
    if (callee?.type === "Identifier") {
      if (
        ["clearTimeout", "clearInterval"].includes(callee.name) &&
        !unit.binding(callee) &&
        node.arguments.length === 1 &&
        owner &&
        ownsObjectMember(unit, node.arguments[0], owner)
      )
        return;
      if (promiseResolver(unit, callee, context)) return;
      valid = false;
      return;
    }
    if (
      memberName(callee) === "abort" &&
      node.arguments.length === 0 &&
      owner &&
      ownsObjectMember(unit, callee.object, owner)
    ) {
      const field = propertyNamed(object.node, memberName(callee.object));
      if (field && containsType(unit, field.value, "AbortController", context))
        return;
    }
    // A promise resolver stored in an owned record remains an owned callback.
    if (
      owner &&
      ownsObjectMember(unit, callee, owner) &&
      node.arguments.length === 0
    ) {
      const key = memberName(callee);
      const assignments = unit.nodes.filter(
        (candidate) =>
          candidate.type === "AssignmentExpression" &&
          ownsObjectMember(unit, candidate.left, owner) &&
          memberName(candidate.left) === key &&
          unwrap(candidate.right)?.type === "Identifier",
      );
      if (
        assignments.some((assignment) => {
          const value = unwrap(assignment.right);
          const definition = unit.binding(value)?.defs[0];
          const executor =
            definition?.type === "Parameter" ? definition.node : null;
          const parent = executor && unit.parents.get(executor);
          return (
            parent?.type === "NewExpression" &&
            parent.callee.type === "Identifier" &&
            parent.callee.name === "Promise" &&
            !unit.binding(parent.callee) &&
            unit.binding(executor.params[0]) === unit.binding(value)
          );
        })
      )
        return;
    }
    valid = false;
  });
  if (valid) evidence(context, unit, property);
  return valid;
}
function resourceValue(context, unit, expression, seen = new Set()) {
  step(context);
  expression = unwrap(expression);
  if (expression?.type === "Identifier") {
    const variable = unit.binding(expression);
    if (
      !immutable(variable) ||
      seen.has(variable) ||
      variable.defs[0].type !== "Variable"
    )
      return null;
    seen.add(variable);
    return resourceValue(context, unit, variable.defs[0].node.init, seen);
  }
  if (
    expression?.type === "NewExpression" &&
    expression.callee.type === "Identifier" &&
    !unit.binding(expression.callee) &&
    expression.arguments.length === 0
  ) {
    if (expression.callee.name === "AbortController") return { kind: "abort" };
    if (expression.callee.name === "Set") return { kind: "registry" };
  }
  const object = objectValue(context, unit, expression);
  return object && localDisposal(context, object, "dispose")
    ? { kind: "dispose", object }
    : null;
}
function singletonTest(context, unit, node) {
  node = unwrap(node);
  if (currentRead(context, unit, node)) return true;
  return (
    node?.type === "LogicalExpression" &&
    node.operator === "||" &&
    (singletonTest(context, unit, node.left) ||
      singletonTest(context, unit, node.right))
  );
}
function onlyReturn(node) {
  return (
    (node?.type === "ReturnStatement" && !node.argument) ||
    (node?.type === "BlockStatement" &&
      node.body.length === 1 &&
      onlyReturn(node.body[0]))
  );
}
function acquisitionGuard(context, unit, assignment) {
  const fn = enclosingFunction(unit, assignment);
  if (
    !fn ||
    fn === context.effect ||
    fn === context.cleanup ||
    fn === context.owner ||
    fn.body.type !== "BlockStatement"
  )
    return null;
  let statement = assignment;
  while (unit.parents.get(statement) !== fn.body) {
    statement = unit.parents.get(statement);
    if (!statement || FUNCTIONS.has(statement.type)) return null;
  }
  for (const candidate of fn.body.body.slice(
    0,
    fn.body.body.indexOf(statement),
  )) {
    if (
      candidate.type === "IfStatement" &&
      !candidate.alternate &&
      onlyReturn(candidate.consequent) &&
      singletonTest(context, unit, candidate.test)
    )
      return candidate;
  }
  return null;
}
function allRefAliases(context, unit) {
  const aliases = new Set();
  for (const variable of unit.bindings.values()) {
    if (
      variable?.identifiers[0] &&
      refBinding(unit, variable.identifiers[0], null) === context.ref
    )
      aliases.add(variable);
  }
  for (const variable of aliases) {
    refBinding(unit, variable.identifiers[0], context);
    for (const reference of variable.references) {
      const token = reference.identifier;
      if (variable.identifiers.includes(token)) continue;
      const parent = unit.parents.get(token);
      if (
        parent?.type === "MemberExpression" &&
        parent.object === token &&
        memberName(parent) === "current"
      )
        continue;
      if (
        parent?.type === "VariableDeclarator" &&
        parent.init === token &&
        parent.id.type === "Identifier" &&
        aliases.has(unit.binding(parent.id))
      )
        continue;
      return null;
    }
  }
  return aliases;
}
function inspectWrites(context, unit) {
  const acquisitions = [];
  for (const node of unit.nodes) {
    if (["AssignmentExpression", "UpdateExpression"].includes(node.type)) {
      let receiver = unwrap(node.left ?? node.argument);
      if (!currentRead(context, unit, receiver)) {
        while (receiver?.type === "MemberExpression") {
          receiver = unwrap(receiver.object);
          if (currentRead(context, unit, receiver)) return null;
        }
      }
    }
    if (
      node.type === "UpdateExpression" &&
      currentRead(context, unit, node.argument)
    )
      return null;
    if (
      node.type === "UnaryExpression" &&
      node.operator === "delete" &&
      currentRead(context, unit, node.argument)
    )
      return null;
    if (
      node.type !== "AssignmentExpression" ||
      !currentRead(context, unit, node.left)
    )
      continue;
    const right = unwrap(node.right);
    if (
      node.operator === "=" &&
      right?.type === "Literal" &&
      right.value === null
    ) {
      evidence(context, unit, node);
      continue;
    }
    if (!["=", "??="].includes(node.operator)) return null;
    const resource = resourceValue(context, unit, node.right);
    if (!resource) return null;
    if (node.operator === "??=" && resource.kind !== "registry") return null;
    const guard =
      node.operator === "??=" ? node : acquisitionGuard(context, unit, node);
    if (
      !guard ||
      enclosingFunction(unit, node) === context.effect ||
      enclosingFunction(unit, node) === context.cleanup
    )
      return null;
    evidence(context, unit, node);
    evidence(context, unit, guard);
    acquisitions.push({ unit, node, resource });
  }
  return acquisitions.length &&
    new Set(acquisitions.map((entry) => entry.resource.kind)).size === 1
    ? acquisitions
    : null;
}
function registryExpression(context, unit, expression, owners) {
  expression = unwrap(expression);
  if (currentRead(context, unit, expression)) return true;
  if (
    expression?.type === "AssignmentExpression" &&
    expression.operator === "??=" &&
    currentRead(context, unit, expression.left)
  )
    return true;
  return (
    expression?.type === "Identifier" &&
    owners.some(
      (owner) =>
        owner.unit === unit && owner.variable === unit.binding(expression),
    )
  );
}
function inspectRegistry(context, unit) {
  const owners = [];
  const visitedCalls = new Set();
  let additions = 0;
  // Local aliases and inspected call parameters form a bounded receiver graph.
  for (let pass = 0; pass < 8; pass++) {
    let changed = false;
    const sources = [unit, ...new Set(owners.map((owner) => owner.unit))];
    for (const source of sources)
      for (const node of source.nodes) {
        step(context);
        if (
          node.type === "VariableDeclarator" &&
          node.id.type === "Identifier" &&
          registryExpression(context, source, node.init, owners)
        ) {
          const variable = source.binding(node.id);
          if (!immutable(variable)) return false;
          if (!owners.some((owner) => owner.variable === variable)) {
            owners.push({ unit: source, variable });
            changed = true;
            evidence(context, source, node);
          }
        }
        if (node.type !== "CallExpression" || visitedCalls.has(node)) continue;
        if (
          memberName(node.callee) === "add" &&
          registryExpression(context, source, node.callee.object, owners)
        ) {
          if (node.arguments.length !== 1) return false;
          const object = objectValue(context, source, node.arguments[0]);
          const method = object && propertyNamed(object.node, "destroy");
          if (!method || !FUNCTIONS.has(unwrap(method.value)?.type))
            return false;
          evidence(context, source, node);
          evidence(context, object.unit, object.node);
          additions++;
          visitedCalls.add(node);
          continue;
        }
        const indices = node.arguments
          .map((argument, index) =>
            registryExpression(context, source, argument, owners) ? index : -1,
          )
          .filter((index) => index >= 0);
        if (!indices.length) continue;
        const target = localFunction(context, source, node.callee);
        if (!target || target.fn.async || indices.length > 1) return false;
        const parameter = target.fn.params[indices[0]];
        if (parameter?.type !== "Identifier") return false;
        const variable = target.unit.binding(parameter);
        if (!immutable(variable)) return false;
        if (!owners.some((owner) => owner.variable === variable)) {
          owners.push({ unit: target.unit, variable });
          changed = true;
        }
        evidence(context, source, node);
        evidence(context, target.unit, target.fn);
        visitedCalls.add(node);
      }
    if (!changed) break;
  }
  if (!additions) return false;
  for (const owner of owners)
    for (const reference of owner.variable.references) {
      const identifier = reference.identifier;
      if (owner.variable.identifiers.includes(identifier)) continue;
      const parent = owner.unit.parents.get(identifier);
      if (
        parent?.type === "MemberExpression" &&
        parent.object === identifier &&
        ["add", "delete", "has", "size"].includes(memberName(parent))
      )
        continue;
      if (
        parent?.type === "VariableDeclarator" &&
        parent.init === identifier &&
        owners.some(
          (candidate) => candidate.variable === owner.unit.binding(parent.id),
        )
      )
        continue;
      const call = parent?.type === "CallExpression" ? parent : null;
      if (call && visitedCalls.has(call)) continue;
      return false;
    }
  return true;
}
function cleanupStatements(context, unit, statement, kind) {
  step(context);
  if (statement.type === "BlockStatement")
    return (
      statement.body.length > 0 &&
      statement.body.every((child) =>
        cleanupStatements(context, unit, child, kind),
      )
    );
  if (statement.type === "ExpressionStatement") {
    const expression = unwrap(statement.expression);
    if (
      expression.type === "AssignmentExpression" &&
      expression.operator === "=" &&
      currentRead(context, unit, expression.left) &&
      unwrap(expression.right)?.type === "Literal" &&
      unwrap(expression.right).value === null
    )
      return true;
    if (expression.type !== "CallExpression" || expression.arguments.length)
      return false;
    const callee = unwrap(expression.callee);
    if (
      memberName(callee) !== kind ||
      !currentRead(context, unit, callee.object)
    )
      return false;
    evidence(context, unit, expression);
    return true;
  }
  if (
    kind !== "registry" ||
    statement.type !== "ForOfStatement" ||
    statement.await
  )
    return false;
  const right = unwrap(statement.right);
  const materialized =
    right?.type === "ArrayExpression" &&
    right.elements.length === 1 &&
    right.elements[0]?.type === "SpreadElement"
      ? unwrap(right.elements[0].argument)
      : null;
  if (
    !materialized ||
    materialized.type !== "LogicalExpression" ||
    materialized.operator !== "??" ||
    !currentRead(context, unit, materialized.left) ||
    materialized.right.type !== "ArrayExpression" ||
    materialized.right.elements.length
  )
    return false;
  const declaration = statement.left;
  const token =
    declaration.type === "VariableDeclaration" &&
    declaration.declarations.length === 1
      ? declaration.declarations[0].id
      : null;
  if (token?.type !== "Identifier") return false;
  const body =
    statement.body.type === "BlockStatement" && statement.body.body.length === 1
      ? statement.body.body[0]
      : statement.body;
  const call =
    body.type === "ExpressionStatement" ? unwrap(body.expression) : null;
  if (
    call?.type !== "CallExpression" ||
    call.arguments.length ||
    memberName(call.callee) !== "destroy" ||
    unwrap(call.callee.object)?.type !== "Identifier" ||
    unit.binding(unwrap(call.callee.object)) !== unit.binding(token)
  )
    return false;
  evidence(context, unit, statement);
  return true;
}
function atDiagnostic(unit, diagnostic, type = null) {
  if (
    !Number.isInteger(diagnostic.line) ||
    !Number.isInteger(diagnostic.column) ||
    diagnostic.line < 1 ||
    diagnostic.column < 1
  )
    return null;
  const lines = unit.source.split("\n");
  if (
    diagnostic.line > lines.length ||
    diagnostic.column > lines[diagnostic.line - 1].length
  )
    return null;
  const offset =
    lines
      .slice(0, diagnostic.line - 1)
      .reduce((sum, line) => sum + line.length + 1, 0) +
    diagnostic.column -
    1;
  return (
    unit.nodes.find(
      (node) =>
        (type ? node.type === type : FUNCTIONS.has(node.type)) &&
        node.range[0] === offset,
    ) ?? null
  );
}
function inspectCleanup(context, unit, effect, refName) {
  const call = unit.parents.get(effect);
  if (
    call?.type !== "CallExpression" ||
    call.arguments[0] !== effect ||
    !reactCall(unit, call, "useEffect") ||
    effect.async ||
    effect.body.type !== "BlockStatement"
  )
    return null;
  const returns = [];
  walk(
    unit,
    effect.body,
    context,
    (node) => {
      if (node.type === "ReturnStatement") returns.push(node);
    },
    true,
  );
  const cleanup = returns.length === 1 ? unwrap(returns[0].argument) : null;
  if (!FUNCTIONS.has(cleanup?.type) || cleanup.async || cleanup.params.length)
    return null;
  const candidates = [];
  walk(unit, cleanup.body, context, (node) => {
    if (
      memberName(node) === "current" &&
      unwrap(node.object)?.type === "Identifier" &&
      unwrap(node.object).name === refName
    )
      candidates.push(node);
  });
  if (!candidates.length) return null;
  const ref = refBinding(unit, candidates[0].object, context);
  if (
    !ref ||
    candidates.some((node) => refBinding(unit, node.object, null) !== ref)
  )
    return null;
  Object.assign(context, {
    ref,
    effect,
    cleanup,
    owner: enclosingFunction(unit, ref.defs[0].node),
  });
  if (
    !context.owner ||
    enclosingFunction(unit, effect) !== context.owner ||
    !allRefAliases(context, unit)
  )
    return null;
  const acquisitions = inspectWrites(context, unit);
  if (!acquisitions) return null;
  const kind = acquisitions[0].resource.kind;
  if (
    !cleanupStatements(context, unit, cleanup.body, kind) ||
    (kind === "registry" && !inspectRegistry(context, unit))
  )
    return null;
  evidence(context, unit, call);
  evidence(context, unit, cleanup);
  return {
    predicate: "owned-current-resource-cleanup",
    reason:
      "This lexical React ref is written by guarded application resource acquisition outside the effect and never passed to React as a DOM ref. Cleanup intentionally reads the live owned resource or registry, including handles created after setup; capturing its initial value would miss those resources.",
  };
}

function observationProperty(expression, depth = 0) {
  expression = unwrap(expression);
  if (depth > 8 || !expression) return null;
  if (expression.type === "Literal" && typeof expression.value === "string")
    return expression.value;
  if (expression.type === "BinaryExpression" && expression.operator === "+") {
    const left = observationProperty(expression.left, depth + 1);
    const right = observationProperty(expression.right, depth + 1);
    return left !== null && right !== null ? left + right : null;
  }
  return null;
}
function observationMember(expression) {
  expression = unwrap(expression);
  return expression?.type === "MemberExpression"
    ? expression.computed
      ? observationProperty(expression.property)
      : expression.property.name
    : null;
}
function observationTimer(unit, expression, seen = new Set(), depth = 0) {
  expression = unwrap(expression);
  if (depth > 8 || !expression) return false;
  if (expression.type === "MemberExpression")
    return ["setTimeout", "setInterval"].includes(
      observationMember(expression),
    );
  if (expression.type !== "Identifier") return false;
  const variable = unit.binding(expression);
  if (!variable?.defs.length)
    return ["setTimeout", "setInterval"].includes(expression.name);
  if (!immutable(variable) || seen.has(variable)) return false;
  seen.add(variable);
  const definition = variable.defs[0];
  return (
    definition.type === "Variable" &&
    observationTimer(unit, definition.node.init, seen, depth + 1)
  );
}
function observationFunction(unit, expression, seen = new Set(), depth = 0) {
  expression = unwrap(expression);
  if (depth > 8 || !expression) return false;
  if (FUNCTIONS.has(expression.type)) return true;
  if (expression.type !== "Identifier") return false;
  const variable = unit.binding(expression);
  if (!immutable(variable) || seen.has(variable)) return false;
  seen.add(variable);
  const definition = variable.defs[0];
  return (
    definition.type === "FunctionName" ||
    (definition.type === "Variable" &&
      observationFunction(unit, definition.node.init, seen, depth + 1))
  );
}
function opaqueObservationCallable(
  context,
  unit,
  expression,
  seen = new Set(),
  depth = 0,
) {
  step(context);
  expression = unwrap(expression);
  if (depth > 8 || !expression) return true;
  if (expression.type === "MemberExpression")
    return (
      (expression.computed && observationMember(expression) === null) ||
      opaqueObservationCallable(
        context,
        unit,
        expression.object,
        seen,
        depth + 1,
      )
    );
  if (expression.type === "Identifier") {
    const variable = unit.binding(expression);
    if (!immutable(variable)) return false;
    if (seen.has(variable)) return true;
    seen.add(variable);
    const definition = variable.defs[0];
    return (
      definition.type === "Variable" &&
      opaqueObservationCallable(
        context,
        unit,
        definition.node.init,
        seen,
        depth + 1,
      )
    );
  }
  if (["CallExpression", "NewExpression"].includes(expression.type))
    return opaqueObservationCallable(
      context,
      unit,
      expression.callee,
      seen,
      depth + 1,
    );
  return false;
}
function nativeObservationCodeClosed(context, unit) {
  const codeCapabilities = new Set([
    "eval",
    "Function",
    "AsyncFunction",
    "GeneratorFunction",
    "AsyncGeneratorFunction",
  ]);
  const reflectiveMembers = new Set([
    ...codeCapabilities,
    "Reflect",
    "constructor",
    "getPrototypeOf",
    "getOwnPropertyDescriptor",
    "getOwnPropertyDescriptors",
  ]);
  // Do not interpret payload strings or assume indirect execution is harmless.
  // A capability reference invalidates this local native-API proof, even in a
  // nested callback. Ordinary strings/comments and non-call data indexing do not.
  for (const node of unit.nodes) {
    step(context);
    if (node.type === "ImportExpression") return false;
    if (
      node.type === "Identifier" &&
      unit.bindings.has(node) &&
      !unit.binding(node)?.defs.length &&
      (codeCapabilities.has(node.name) || node.name === "Reflect")
    )
      return false;
    if (
      (node.type === "MemberExpression" &&
        reflectiveMembers.has(observationMember(node))) ||
      (node.type === "Property" &&
        reflectiveMembers.has(
          node.computed
            ? observationProperty(node.key)
            : (node.key.name ?? node.key.value),
        ))
    )
      return false;
    if (
      ["CallExpression", "NewExpression"].includes(node.type) &&
      opaqueObservationCallable(context, unit, node.callee)
    )
      return false;
    if (
      !["Identifier", "MemberExpression"].includes(node.type) ||
      !observationTimer(unit, node)
    )
      continue;
    const parent = unit.parents.get(node);
    if (parent?.type === "VariableDeclarator" && parent.id === node) continue;
    if (parent?.type === "CallExpression" && parent.callee === node) {
      if (!observationFunction(unit, parent.arguments[0])) return false;
      continue;
    }
    if (
      parent?.type === "VariableDeclarator" &&
      parent.init === node &&
      parent.id.type === "Identifier" &&
      immutable(unit.binding(parent.id)) &&
      unit.parents.get(parent)?.kind === "const"
    )
      continue;
    return false;
  }
  return true;
}

function nativeObservationGlobal(context, unit, constructor) {
  if (
    constructor?.type !== "Identifier" ||
    constructor.name !== "IntersectionObserver" ||
    unit.binding(constructor)?.defs.length ||
    !nativeObservationCodeClosed(context, unit)
  )
    return false;
  // Ambient globals can have a parser binding with no lexical definitions.
  // Reject local native API replacement, prototype access and global-object
  // escapes, including descriptor/assignment helpers, rather than trusting a
  // method named observe on an arbitrary receiver.
  return unit.nodes.every((node) => {
    if (node.type === "Literal" && node.value === "IntersectionObserver")
      return false;
    if (memberName(node) === "IntersectionObserver") return false;
    if (node.type !== "Identifier" || unit.binding(node)?.defs.length)
      return true;
    const parent = unit.parents.get(node);
    if (node.name === "IntersectionObserver")
      return parent?.type === "NewExpression" && parent.callee === node;
    if (!["globalThis", "window", "self"].includes(node.name)) return true;
    return (
      parent?.type === "MemberExpression" &&
      parent.object === node &&
      !parent.computed &&
      memberName(parent) !== "IntersectionObserver"
    );
  });
}

function observationRead(expression) {
  expression = unwrap(expression);
  if (!expression) return false;
  if (["Identifier", "Literal"].includes(expression.type)) return true;
  if (expression.type === "UnaryExpression")
    return (
      ["!", "typeof", "void"].includes(expression.operator) &&
      observationRead(expression.argument)
    );
  return (
    (expression.type === "LogicalExpression" ||
      (expression.type === "BinaryExpression" &&
        ["===", "!=="].includes(expression.operator))) &&
    observationRead(expression.left) &&
    observationRead(expression.right)
  );
}

function observationOptions(context, unit, expression) {
  expression = unwrap(expression);
  if (expression?.type !== "ObjectExpression") return false;
  const names = new Set();
  return expression.properties.every((property) => {
    if (
      property.type !== "Property" ||
      property.computed ||
      property.kind !== "init" ||
      property.method
    )
      return false;
    const name = property.key.name ?? property.key.value;
    if (names.has(name)) return false;
    names.add(name);
    const value = unwrap(property.value);
    if (name === "root") {
      if (value?.type === "Literal" && value.value === null) return true;
      return (
        memberName(value) === "current" &&
        !value.computed &&
        !!refBinding(unit, value.object, context)
      );
    }
    if (name === "rootMargin")
      return value?.type === "Literal" && typeof value.value === "string";
    const threshold = (node) =>
      node?.type === "Literal" && typeof node.value === "number";
    return (
      name === "threshold" &&
      (threshold(value) ||
        (value?.type === "ArrayExpression" && value.elements.every(threshold)))
    );
  });
}

function inspectObservation(context, unit, registration) {
  const callee = unwrap(registration.callee);
  const receiver = unwrap(callee?.object);
  if (
    registration.optional ||
    memberName(callee) !== "observe" ||
    callee.computed ||
    receiver?.type !== "Identifier" ||
    registration.arguments.length !== 1 ||
    unwrap(registration.arguments[0])?.type !== "Identifier"
  )
    return null;
  const variable = unit.binding(receiver);
  const definition = variable?.defs[0];
  const declaration = definition && unit.parents.get(definition.node);
  const acquisition = unwrap(definition?.node.init);
  if (
    !immutable(variable) ||
    definition.type !== "Variable" ||
    definition.node.id.type !== "Identifier" ||
    declaration?.type !== "VariableDeclaration" ||
    declaration.kind !== "const" ||
    declaration.declarations.length !== 1 ||
    acquisition?.type !== "NewExpression" ||
    !nativeObservationGlobal(context, unit, unwrap(acquisition.callee)) ||
    ![1, 2].includes(acquisition.arguments.length)
  )
    return null;
  const callback = unwrap(acquisition.arguments[0]);
  if (
    !FUNCTIONS.has(callback?.type) ||
    callback.async ||
    callback.generator ||
    (acquisition.arguments.length === 2 &&
      !observationOptions(context, unit, acquisition.arguments[1]))
  )
    return null;
  const effect = enclosingFunction(unit, registration);
  const hook = effect && unit.parents.get(effect);
  if (
    !effect ||
    effect.async ||
    effect.generator ||
    effect.params.length ||
    effect.body.type !== "BlockStatement" ||
    hook?.type !== "CallExpression" ||
    hook.arguments[0] !== effect ||
    !reactCall(unit, hook, "useEffect") ||
    unit.parents.get(declaration) !== effect.body
  )
    return null;
  const statements = effect.body.body;
  const returned = statements.at(-1);
  const cleanup =
    returned?.type === "ReturnStatement" ? unwrap(returned.argument) : null;
  if (
    !FUNCTIONS.has(cleanup?.type) ||
    cleanup.async ||
    cleanup.generator ||
    cleanup.params.length
  )
    return null;
  const cleanupBody = cleanup.body;
  const disconnect = unwrap(
    cleanupBody.type === "BlockStatement" &&
      cleanupBody.body.length === 1 &&
      cleanupBody.body[0].type === "ExpressionStatement"
      ? cleanupBody.body[0].expression
      : cleanupBody,
  );
  const disconnectMember = unwrap(disconnect?.callee);
  if (
    disconnect?.type !== "CallExpression" ||
    disconnect.optional ||
    disconnect.arguments.length ||
    memberName(disconnectMember) !== "disconnect" ||
    disconnectMember.computed ||
    unit.binding(unwrap(disconnectMember.object)) !== variable
  )
    return null;
  const registrationStatement = (statement) =>
    statement.type === "ExpressionStatement" &&
    unwrap(statement.expression) === registration;
  let seenDeclaration = false;
  let seenRegistration = false;
  for (const statement of statements.slice(0, -1)) {
    step(context);
    if (statement === declaration && !seenDeclaration && !seenRegistration) {
      seenDeclaration = true;
      continue;
    }
    let candidate = statement;
    if (
      statement.type === "IfStatement" &&
      !statement.alternate &&
      observationRead(statement.test)
    ) {
      candidate = statement.consequent;
      if (
        candidate.type === "ReturnStatement" &&
        !candidate.argument &&
        !seenDeclaration
      )
        continue;
      if (candidate.type === "BlockStatement" && candidate.body.length === 1)
        candidate = candidate.body[0];
    }
    if (
      !seenDeclaration ||
      seenRegistration ||
      !registrationStatement(candidate)
    )
      return null;
    seenRegistration = true;
  }
  if (
    !seenDeclaration ||
    !seenRegistration ||
    variable.references.some((reference) => {
      if (reference.init) return false;
      const parent = unit.parents.get(reference.identifier);
      return !(
        parent?.type === "MemberExpression" &&
        parent.object === reference.identifier &&
        (parent === callee || parent === disconnectMember)
      );
    })
  )
    return null;
  for (const statement of unit.ast.body)
    if (
      statement.type === "ImportDeclaration" &&
      statement.source.value === "react"
    )
      evidence(context, unit, statement);
  for (const node of [
    hook,
    declaration,
    acquisition,
    callback,
    registration,
    cleanup,
    disconnect,
  ])
    evidence(context, unit, node);
  return {
    predicate: "native-intersection-observation-registration",
    reason:
      "The pinned rule mistakes native observe(target) registration for a synchronous parent/state call. This locally owned, unmodified native IntersectionObserver only registers its target here and disconnects during cleanup. Its inline callback runs in a deferred observer task, including initial observation; any callback-driven render belongs to that asynchronous notification. The raw diagnostic is retained.",
  };
}

function createResourceSemanticCorrections(repoRoot) {
  const units = new Map();
  const read = (file) => {
    if (!units.has(file)) units.set(file, parseUnit(repoRoot, file));
    return units.get(file);
  };
  return (diagnostic, toolPackage) => {
    if (
      toolPackage !== TOOL ||
      diagnostic.plugin !== "react-doctor" ||
      !["exhaustive-deps", "no-pass-live-state-to-parent"].includes(
        diagnostic.rule,
      )
    )
      return null;
    const observation = diagnostic.rule === "no-pass-live-state-to-parent";
    const match =
      /^Your cleanup may read the wrong node since the ref `([A-Za-z_$][\w$]*)\.current` can change before it runs\.$/.exec(
        diagnostic.message ?? "",
      );
    if (observation ? diagnostic.message !== OBSERVATION_MESSAGE : !match)
      return null;
    const file = path.posix.normalize(`web/app/${diagnostic.filePath}`);
    if (
      !file.startsWith("web/app/") ||
      diagnostic.filePath.startsWith("/") ||
      diagnostic.filePath.includes("\\")
    )
      return null;
    try {
      const unit = read(file);
      const node =
        unit &&
        atDiagnostic(unit, diagnostic, observation ? "CallExpression" : null);
      if (!node) return null;
      const context = {
        repoRoot,
        read,
        steps: 0,
        evidence: new Map(),
        dependencies: new Set([file]),
        activeFunctions: new Set(),
      };
      const proof = observation
        ? inspectObservation(context, unit, node)
        : inspectCleanup(context, unit, node, match[1]);
      if (!proof) return null;
      return {
        ...proof,
        provenance: observation ? OBSERVATION_PROVENANCE : PROVENANCE,
        evidence: [...context.evidence].map(([source, spans]) => ({
          file: source.file,
          sha256: createHash("sha256").update(source.source).digest("hex"),
          spans: [...spans.values()],
        })),
      };
    } catch {
      return null;
    }
  };
}

module.exports = { createResourceSemanticCorrections };
