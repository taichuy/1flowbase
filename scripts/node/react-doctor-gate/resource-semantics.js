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
function atDiagnostic(unit, diagnostic) {
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
      (node) => FUNCTIONS.has(node.type) && node.range[0] === offset,
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
      diagnostic.rule !== "exhaustive-deps"
    )
      return null;
    const match =
      /^Your cleanup may read the wrong node since the ref `([A-Za-z_$][\w$]*)\.current` can change before it runs\.$/.exec(
        diagnostic.message ?? "",
      );
    if (!match) return null;
    const file = path.posix.normalize(`web/app/${diagnostic.filePath}`);
    if (
      !file.startsWith("web/app/") ||
      diagnostic.filePath.startsWith("/") ||
      diagnostic.filePath.includes("\\")
    )
      return null;
    try {
      const unit = read(file);
      const node = unit && atDiagnostic(unit, diagnostic);
      if (!node) return null;
      const context = {
        repoRoot,
        read,
        steps: 0,
        evidence: new Map(),
        dependencies: new Set([file]),
        activeFunctions: new Set(),
      };
      const proof = inspectCleanup(context, unit, node, match[1]);
      if (!proof) return null;
      return {
        ...proof,
        provenance: PROVENANCE,
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
