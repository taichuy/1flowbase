const path = require("node:path");
const { createHash } = require("node:crypto");
const {
  parseUnit,
  unwrap,
  memberName,
  immutable,
  importedCall,
} = require("./source-unit.js");

const TOOL = "react-doctor@0.2.16";
const EVENT_PROP =
  "Faking an event handler with a prop plus a useEffect costs an extra render & runs late.";
const EVENT_STATE =
  "Faking an event handler with state plus a useEffect costs an extra render & runs late.";
const FUNCTIONS = new Set([
  "FunctionDeclaration",
  "FunctionExpression",
  "ArrowFunctionExpression",
]);
const RULES = {
  "no-derived-state": {
    ruleSourceSha256:
      "db98ba7989c837bb635b3fed080a937ed425ecce6330126dd58b15f66e9eb91f",
    references: [
      "https://react.dev/learn/you-might-not-need-an-effect#adjusting-some-state-when-a-prop-changes",
      "https://react.dev/reference/react/useRef",
    ],
  },
  "no-event-handler": {
    ruleSourceSha256:
      "1f89d763b79829a89a3c00e9d655ff8299e26b98c5f7d46a770f3bb0ccf74062",
    references: [
      "https://react.dev/learn/synchronizing-with-effects",
      "https://tanstack.com/query/latest/docs/framework/react/reference/useInfiniteQuery",
      "https://tanstack.com/query/latest/docs/framework/react/reference/useQuery",
    ],
  },
};
const PROVENANCE = {
  toolPackage: TOOL,
  packageIntegrity:
    "sha512-o108QscqM/TlC1QmKgDORf/P53PeZ69nZbRsvy53rXkKvWdKZC1Y0u1JRPmThbTh6v+QpMispBwEnFHhlpZQ7w==",
  sourceCommit: "2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc",
};

function step(context) {
  if (++context.steps > 20000)
    throw new Error("history observation proof budget exceeded");
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
function evidence(context, node) {
  context.spans.set(node.range.join(":"), {
    start: node.range[0],
    end: node.range[1],
  });
}
function functionOwner(unit, node) {
  for (
    let parent = unit.parents.get(node);
    parent;
    parent = unit.parents.get(parent)
  )
    if (FUNCTIONS.has(parent.type)) return parent;
  return null;
}
function same(unit, left, right) {
  left = unwrap(left);
  right = unwrap(right);
  if (!left || !right || left.type !== right.type) return false;
  if (left.type === "Identifier")
    return unit.binding(left) && unit.binding(left) === unit.binding(right);
  if (left.type === "Literal") return left.value === right.value;
  return (
    left.type === "MemberExpression" &&
    memberName(left) !== null &&
    memberName(left) === memberName(right) &&
    same(unit, left.object, right.object)
  );
}
function within(node, root) {
  return node.range[0] >= root.range[0] && node.range[1] <= root.range[1];
}
function pure(unit, node, context) {
  step(context);
  node = unwrap(node);
  if (!node) return false;
  if (["Identifier", "Literal"].includes(node.type)) return true;
  if (node.type === "MemberExpression")
    return (
      pure(unit, node.object, context) &&
      (!node.computed || pure(unit, node.property, context))
    );
  if (node.type === "TemplateLiteral")
    return node.expressions.every((child) => pure(unit, child, context));
  if (node.type === "UnaryExpression")
    return (
      ["!", "typeof", "void", "+", "-"].includes(node.operator) &&
      pure(unit, node.argument, context)
    );
  if (["LogicalExpression", "BinaryExpression"].includes(node.type))
    return pure(unit, node.left, context) && pure(unit, node.right, context);
  if (node.type === "ConditionalExpression")
    return [node.test, node.consequent, node.alternate].every((child) =>
      pure(unit, child, context),
    );
  return false;
}
function stateInfo(unit, setter) {
  const variable = unit.binding(unwrap(setter));
  if (!immutable(variable) || variable.defs[0].type !== "Variable") return null;
  const declaration = variable.defs[0].node;
  if (
    declaration.id.type !== "ArrayPattern" ||
    declaration.id.elements[0]?.type !== "Identifier" ||
    declaration.id.elements[1]?.type !== "Identifier" ||
    unit.binding(declaration.id.elements[1]) !== variable ||
    !importedCall(unit, declaration.init, "react", "useState")
  )
    return null;
  return {
    variable,
    declaration,
    state: unit.binding(declaration.id.elements[0]),
    name: declaration.id.elements[0].name,
  };
}
function stateCalls(unit, info) {
  return info.variable.references
    .map((reference) => unit.parents.get(reference.identifier))
    .filter(
      (node) =>
        node?.type === "CallExpression" &&
        unwrap(node.callee)?.type === "Identifier" &&
        unit.binding(unwrap(node.callee)) === info.variable,
    );
}
function enclosingEffect(unit, node) {
  for (
    let parent = unit.parents.get(node);
    parent;
    parent = unit.parents.get(parent)
  ) {
    if (!FUNCTIONS.has(parent.type)) continue;
    const call = unit.parents.get(parent);
    return call?.type === "CallExpression" &&
      call.arguments[0] === parent &&
      importedCall(unit, call, "react", "useEffect")
      ? call
      : null;
  }
  return null;
}
function effectCallback(effect) {
  const callback = unwrap(effect.arguments[0]);
  return FUNCTIONS.has(callback?.type) &&
    !callback.async &&
    !callback.generator &&
    callback.body.type === "BlockStatement"
    ? callback
    : null;
}
function declaredDependency(unit, effect, expression) {
  const dependencies = unwrap(effect.arguments[1]);
  expression = unwrap(expression);
  if (expression?.type === "UnaryExpression" && expression.operator === "!")
    expression = unwrap(expression.argument);
  return (
    dependencies?.type === "ArrayExpression" &&
    dependencies.elements.every(
      (element) => element && element.type !== "SpreadElement",
    ) &&
    dependencies.elements.some((element) => same(unit, element, expression))
  );
}
function declarationEvidence(context, unit, declaration) {
  const parent = unit.parents.get(declaration);
  evidence(
    context,
    parent?.type === "VariableDeclaration" ? parent : declaration,
  );
}
function ifOwner(unit, node, effect) {
  for (
    let parent = unit.parents.get(node);
    parent && parent !== effect;
    parent = unit.parents.get(parent)
  ) {
    if (parent.type === "IfStatement" && within(node, parent.consequent))
      return parent;
  }
  return null;
}
function simpleStatement(unit, node, allowed, context) {
  step(context);
  if (node.type === "BlockStatement")
    return (
      node.body.length > 0 &&
      node.body.every((child) => simpleStatement(unit, child, allowed, context))
    );
  if (node.type !== "ExpressionStatement") return false;
  return allowed.includes(unwrap(node.expression));
}
function propertyName(property) {
  return property?.type === "Property" &&
    !property.computed &&
    property.kind === "init"
    ? (property.key.name ?? property.key.value)
    : null;
}
function stateMember(unit, node, state) {
  node = unwrap(node);
  return node?.type === "MemberExpression" &&
    unwrap(node.object)?.type === "Identifier" &&
    unit.binding(unwrap(node.object)) === state
    ? memberName(node)
    : null;
}
function projectedFrom(unit, value, seed) {
  value = unwrap(value);
  if (same(unit, value, seed)) return true;
  return (
    value?.type === "MemberExpression" &&
    projectedFrom(unit, value.object, seed)
  );
}
function snapshotConsumer(
  unit,
  owner,
  info,
  seed,
  properties,
  effect,
  context,
) {
  for (const node of unit.nodes) {
    if (
      node.type !== "LogicalExpression" ||
      node.operator !== "??" ||
      !same(unit, node.left, seed) ||
      functionOwner(unit, node) !== owner
    )
      continue;
    const fallback = unwrap(node.right);
    if (
      fallback?.type !== "ConditionalExpression" ||
      unwrap(fallback.alternate)?.type !== "Literal" ||
      unwrap(fallback.alternate).value !== null
    )
      continue;
    const test = unwrap(fallback.test);
    if (test?.type !== "BinaryExpression" || test.operator !== "===") continue;
    const stamp = stateMember(unit, test.left, info.state)
      ? { key: stateMember(unit, test.left, info.state), value: test.right }
      : stateMember(unit, test.right, info.state)
        ? { key: stateMember(unit, test.right, info.state), value: test.left }
        : null;
    if (!stamp) continue;
    const tag = properties.find(
      (property) => propertyName(property) === stamp.key,
    );
    if (
      !tag ||
      !same(unit, tag.value, stamp.value) ||
      !pure(unit, tag.value, context) ||
      !declaredDependency(unit, effect, tag.value)
    )
      continue;
    const remaining = properties.filter((property) => property !== tag);
    if (
      !remaining.length ||
      !remaining.every((property) => projectedFrom(unit, property.value, seed))
    )
      continue;
    const result = unwrap(fallback.consequent);
    const returnsState =
      result.type === "Identifier" && unit.binding(result) === info.state;
    const selectedKey = stateMember(unit, result, info.state);
    if (
      (returnsState &&
        remaining.every(
          (property) =>
            memberName(unwrap(property.value)) === propertyName(property),
        )) ||
      (selectedKey &&
        remaining.some(
          (property) =>
            propertyName(property) === selectedKey &&
            same(unit, property.value, seed),
        ))
    ) {
      evidence(context, fallback);
      evidence(context, node);
      return true;
    }
  }
  return false;
}
function snapshotProof(unit, call, effect, context) {
  const info = stateInfo(unit, call.callee);
  if (!info || call.arguments.length !== 1) return null;
  const value = unwrap(call.arguments[0]);
  if (
    value?.type !== "ObjectExpression" ||
    value.properties.some((property) => propertyName(property) === null) ||
    new Set(value.properties.map(propertyName)).size !== value.properties.length
  )
    return null;
  const condition = ifOwner(unit, call, effect);
  if (
    !condition ||
    condition.alternate ||
    !pure(unit, condition.test, context) ||
    !["Identifier", "MemberExpression"].includes(
      unwrap(condition.test)?.type,
    ) ||
    !simpleStatement(unit, condition.consequent, [call], context)
  )
    return null;
  const seed = unwrap(condition.test);
  if (!declaredDependency(unit, effect, seed)) return null;
  const callback = effectCallback(effect);
  if (
    !callback ||
    callback.body.body.length !== 1 ||
    callback.body.body[0] !== condition
  )
    return null;
  const owner = functionOwner(unit, effect);
  if (stateCalls(unit, info).some((candidate) => candidate !== call))
    return null;
  if (
    !snapshotConsumer(
      unit,
      owner,
      info,
      seed,
      value.properties,
      effect,
      context,
    )
  )
    return null;
  evidence(context, info.declaration.init);
  evidence(context, call);
  evidence(context, condition);
  evidence(context, effect);
  return {
    predicate: "retained-owner-snapshot",
    reason:
      "This state retains a nonnull completed value while the current value is absent; its consumer explicitly checks the same owner tag before falling back. Current inputs cannot reconstruct that retained completion.",
  };
}
function refInfo(unit, expression) {
  expression = unwrap(expression);
  if (expression?.type !== "Identifier") return null;
  const variable = unit.binding(expression);
  if (!immutable(variable) || variable.defs[0].type !== "Variable") return null;
  const declaration = variable.defs[0].node;
  return declaration.id.type === "Identifier" &&
    importedCall(unit, declaration.init, "react", "useRef")
    ? { variable, declaration }
    : null;
}
function refMember(unit, expression, variable) {
  expression = unwrap(expression);
  return (
    memberName(expression) === "current" &&
    unwrap(expression.object)?.type === "Identifier" &&
    unit.binding(unwrap(expression.object)) === variable
  );
}
function flattenAnd(expression) {
  expression = unwrap(expression);
  return expression?.type === "LogicalExpression" &&
    expression.operator === "&&"
    ? [...flattenAnd(expression.left), ...flattenAnd(expression.right)]
    : [expression];
}
function firstSeedProof(unit, call, effect, context) {
  const info = stateInfo(unit, call.callee);
  if (!info || call.arguments.length !== 1) return null;
  const condition = ifOwner(unit, call, effect);
  if (!condition || condition.alternate) return null;
  const terms = flattenAnd(condition.test);
  if (terms.length !== 2) return null;
  const guarded = terms.find(
    (term) =>
      term?.type === "UnaryExpression" &&
      term.operator === "!" &&
      memberName(unwrap(term.argument)) === "current",
  );
  const latch = guarded && refInfo(unit, unwrap(guarded.argument).object);
  if (
    !latch ||
    unwrap(latch.declaration.init.arguments[0])?.type !== "Literal" ||
    unwrap(latch.declaration.init.arguments[0]).value !== false
  )
    return null;
  const seed = terms.find((term) => term !== guarded);
  if (
    !seed ||
    !["Identifier", "MemberExpression"].includes(seed.type) ||
    !declaredDependency(unit, effect, seed)
  )
    return null;
  const value = unwrap(call.arguments[0]);
  const copied =
    value?.type === "ObjectExpression" &&
    value.properties.length === 1 &&
    value.properties[0].type === "SpreadElement"
      ? value.properties[0].argument
      : value;
  if (!same(unit, copied, seed)) return null;
  const assignments = unit.nodes.filter(
    (node) =>
      node.type === "AssignmentExpression" &&
      refMember(unit, node.left, latch.variable),
  );
  if (
    assignments.length !== 1 ||
    assignments[0].operator !== "=" ||
    unwrap(assignments[0].right)?.type !== "Literal" ||
    unwrap(assignments[0].right).value !== true ||
    !within(assignments[0], condition.consequent)
  )
    return null;
  const assignment = assignments[0];
  if (
    unit.nodes.some(
      (node) =>
        node.type === "UpdateExpression" &&
        refMember(unit, node.argument, latch.variable),
    )
  )
    return null;
  if (!simpleStatement(unit, condition.consequent, [assignment, call], context))
    return null;
  // The seed branch is distinct from user edits; continuous copying in another
  // effect or render would invalidate this first-success ownership proof.
  if (
    stateCalls(unit, info).some(
      (candidate) =>
        candidate !== call &&
        ((enclosingEffect(unit, candidate) &&
          enclosingEffect(unit, candidate) !== effect) ||
          functionOwner(unit, candidate) === functionOwner(unit, effect)),
    )
  )
    return null;
  evidence(context, latch.declaration.init);
  evidence(context, assignment);
  evidence(context, call);
  evidence(context, condition);
  evidence(context, info.declaration.init);
  return {
    predicate: "first-success-seed",
    reason:
      "A lexically proven React ref changes once from false to true around the observed first successful seed. Subsequent source refreshes cannot replace this independently editable state, so it is not a value continuously derivable from current inputs.",
  };
}
function queryFactory(unit, expression, seen = new Set()) {
  expression = unwrap(expression);
  if (
    ["useQuery", "useInfiniteQuery"].some((name) =>
      importedCall(unit, expression, "@tanstack/react-query", name),
    )
  )
    return expression;
  if (expression?.type !== "Identifier") return null;
  const variable = unit.binding(expression);
  if (
    !immutable(variable) ||
    seen.has(variable) ||
    variable.defs[0].type !== "Variable"
  )
    return null;
  seen.add(variable);
  return queryFactory(unit, variable.defs[0].node.init, seen);
}
function queryRefetch(unit, expression, context, seen = new Set()) {
  expression = unwrap(expression);
  if (expression?.type !== "Identifier") return null;
  const variable = unit.binding(expression);
  if (
    !immutable(variable) ||
    seen.has(variable) ||
    variable.defs[0].type !== "Variable"
  )
    return null;
  seen.add(variable);
  const declaration = variable.defs[0].node;
  if (declaration.id.type === "ObjectPattern") {
    const property = declaration.id.properties.find(
      (entry) =>
        entry.type === "Property" &&
        propertyName(entry) === "refetch" &&
        entry.value.type === "Identifier" &&
        unit.binding(entry.value) === variable,
    );
    const factory = property && queryFactory(unit, declaration.init);
    if (factory) {
      declarationEvidence(context, unit, declaration);
      evidence(context, factory);
      return factory;
    }
  }
  const initializer = unwrap(declaration.init);
  if (declaration.id.type !== "Identifier") return null;
  if (memberName(initializer) === "refetch") {
    const factory = queryFactory(unit, initializer.object);
    if (factory) {
      declarationEvidence(context, unit, declaration);
      evidence(context, factory);
      return factory;
    }
  }
  const result = queryRefetch(unit, initializer, context, seen);
  if (result) declarationEvidence(context, unit, declaration);
  return result;
}
function signatureInitializer(unit, expression, context) {
  expression = unwrap(expression);
  if (expression?.type !== "Identifier") return null;
  const variable = unit.binding(expression);
  if (
    !immutable(variable) ||
    variable.defs[0].type !== "Variable" ||
    variable.defs[0].node.id.type !== "Identifier"
  )
    return null;
  const declaration = variable.defs[0].node;
  return pure(unit, declaration.init, context) ? declaration : null;
}
function queryProof(unit, effect, context) {
  const callback = effectCallback(effect);
  if (!callback || callback.body.body.length !== 3) return null;
  const [changedDeclaration, bookkeepingStatement, condition] =
    callback.body.body;
  if (
    changedDeclaration.type !== "VariableDeclaration" ||
    changedDeclaration.kind !== "const" ||
    changedDeclaration.declarations.length !== 1 ||
    changedDeclaration.declarations[0].id.type !== "Identifier" ||
    bookkeepingStatement.type !== "ExpressionStatement" ||
    condition.type !== "IfStatement" ||
    condition.alternate
  )
    return null;
  const changed = changedDeclaration.declarations[0];
  const comparison = unwrap(changed.init);
  if (
    comparison?.type !== "BinaryExpression" ||
    comparison.operator !== "!==" ||
    memberName(comparison.left) !== "current"
  )
    return null;
  const previous = refInfo(unit, unwrap(comparison.left).object);
  const signature = signatureInitializer(unit, comparison.right, context);
  if (
    !previous ||
    !signature ||
    !same(unit, previous.declaration.init.arguments[0], comparison.right) ||
    !declaredDependency(unit, effect, comparison.right)
  )
    return null;
  const bookkeeping = unwrap(bookkeepingStatement.expression);
  if (
    bookkeeping?.type !== "AssignmentExpression" ||
    bookkeeping.operator !== "=" ||
    !refMember(unit, bookkeeping.left, previous.variable) ||
    !same(unit, bookkeeping.right, comparison.right)
  )
    return null;
  const writes = unit.nodes.filter(
    (node) =>
      node.type === "AssignmentExpression" &&
      refMember(unit, node.left, previous.variable),
  );
  if (writes.length !== 1 || writes[0] !== bookkeeping) return null;
  const terms = flattenAnd(condition.test);
  if (
    !terms.some(
      (term) =>
        term?.type === "Identifier" &&
        unit.binding(term) === unit.binding(changed.id),
    ) ||
    !terms.every((term) => pure(unit, term, context)) ||
    !terms.every(
      (term) =>
        (term?.type === "Identifier" &&
          unit.binding(term) === unit.binding(changed.id)) ||
        declaredDependency(unit, effect, term),
    )
  )
    return null;
  const body =
    condition.consequent.type === "BlockStatement" &&
    condition.consequent.body.length === 1
      ? condition.consequent.body[0]
      : condition.consequent;
  if (body.type !== "ExpressionStatement") return null;
  let action = unwrap(body.expression);
  if (action?.type === "UnaryExpression" && action.operator === "void")
    action = unwrap(action.argument);
  if (
    action?.type !== "CallExpression" ||
    action.arguments.length ||
    !queryRefetch(unit, action.callee, context) ||
    !declaredDependency(unit, effect, action.callee)
  )
    return null;
  evidence(context, comparison);
  evidence(context, bookkeeping);
  evidence(context, action);
  evidence(context, body.expression);
  evidence(context, condition);
  evidence(context, previous.declaration.init);
  evidence(context, signature);
  evidence(context, effect);
  return {
    predicate: "query-observation-synchronization",
    reason:
      "This effect only advances a previous-signature ref and invokes a lexically proven TanStack query refetch when the observed signature changes. It synchronizes external cached observations rather than dispatching a user action.",
  };
}
function historyEffects(unit, effect, context) {
  const callback = effectCallback(effect);
  if (!callback) return null;
  const calls = [];
  walk(
    unit,
    callback.body,
    context,
    (node) => {
      if (node.type === "CallExpression" && stateInfo(unit, node.callee))
        calls.push(node);
    },
    true,
  );
  if (calls.length !== 1) return null;
  const proof =
    snapshotProof(unit, calls[0], effect, context) ??
    firstSeedProof(unit, calls[0], effect, context);
  if (!proof) return null;
  // No-event correction requires the entire effect, not one convenient branch.
  if (proof.predicate === "first-success-seed") {
    const condition = ifOwner(unit, calls[0], effect);
    if (callback.body.body.length !== 1 || callback.body.body[0] !== condition)
      return null;
    evidence(context, effect);
  }
  return proof;
}
function depends(unit, expression, root, context, seen = new Set()) {
  step(context);
  expression = unwrap(expression);
  if (!expression) return false;
  if (expression.type === "Identifier") {
    const variable = unit.binding(expression);
    if (variable === root) return true;
    if (
      !immutable(variable) ||
      seen.has(variable) ||
      variable.defs[0].type !== "Variable"
    )
      return false;
    const declaration = variable.defs[0].node;
    if (importedCall(unit, declaration.init, "react", "useReducer"))
      return false;
    return depends(
      unit,
      declaration.init,
      root,
      context,
      new Set([...seen, variable]),
    );
  }
  if (FUNCTIONS.has(expression.type)) return false;
  for (const key of unit.visitorKeys[expression.type] || []) {
    if (
      expression.type === "MemberExpression" &&
      key === "property" &&
      !expression.computed
    )
      continue;
    const value = expression[key];
    for (const child of Array.isArray(value) ? value : [value])
      if (child?.type && depends(unit, child, root, context, seen)) return true;
  }
  return false;
}
function supportedOrigin(unit, token, effects, context) {
  const variable = unit.binding(token);
  if (!variable) return false;
  if (
    variable.identifiers.includes(token) &&
    variable.defs[0]?.type === "Parameter"
  )
    return true;
  for (let current = token; current; current = unit.parents.get(current)) {
    const parent = unit.parents.get(current);
    if (
      parent?.type === "CallExpression" &&
      importedCall(unit, parent, "react", "useReducer")
    )
      return false;
    if (parent?.type === "VariableDeclarator" && within(token, parent.init)) {
      if (importedCall(unit, parent.init, "react", "useReducer")) return false;
      evidence(context, parent.init);
      return true;
    }
    if (
      parent?.type === "IfStatement" &&
      within(token, parent.test) &&
      effects.some((effect) => within(parent, effectCallback(effect).body))
    )
      return true;
    if (parent?.type === "ExpressionStatement" || FUNCTIONS.has(parent?.type))
      return false;
  }
  return false;
}
function observationProof(unit, token, context) {
  const containingEffect = enclosingEffect(unit, token);
  const owner = containingEffect
    ? functionOwner(unit, containingEffect)
    : functionOwner(unit, token);
  const root = unit.binding(token);
  if (!owner || !root) return null;
  const effects = unit.nodes.filter(
    (node) =>
      importedCall(unit, node, "react", "useEffect") &&
      functionOwner(unit, node) === owner &&
      effectCallback(node),
  );
  if (!effects.length || !supportedOrigin(unit, token, effects, context))
    return null;
  const candidates = [];
  for (const effect of effects) {
    let reached = false;
    walk(unit, effectCallback(effect).body, context, (node) => {
      if (
        node.type === "IfStatement" &&
        depends(unit, node.test, root, context)
      )
        reached = true;
    });
    if (reached) candidates.push(effect);
  }
  if (!candidates.length) return null;
  const proofs = candidates.map(
    (effect) =>
      historyEffects(unit, effect, context) ??
      queryProof(unit, effect, context),
  );
  if (
    proofs.some((proof) => !proof) ||
    new Set(proofs.map((proof) => proof.predicate)).size !== 1
  )
    return null;
  evidence(context, token);
  return proofs[0];
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
  const type =
    diagnostic.rule === "no-derived-state" ? "CallExpression" : "Identifier";
  return (
    unit.nodes.find((node) => node.type === type && node.range[0] === offset) ??
    null
  );
}
function createHistoryObservationSemanticCorrections(repoRoot) {
  const units = new Map();
  return (diagnostic, toolPackage) => {
    if (
      toolPackage !== TOOL ||
      diagnostic.plugin !== "react-doctor" ||
      !RULES[diagnostic.rule]
    )
      return null;
    if (
      diagnostic.rule === "no-event-handler" &&
      ![EVENT_PROP, EVENT_STATE].includes(diagnostic.message)
    )
      return null;
    const file = path.posix.normalize(`web/app/${diagnostic.filePath}`);
    if (
      !file.startsWith("web/app/") ||
      diagnostic.filePath.startsWith("/") ||
      diagnostic.filePath.includes("\\")
    )
      return null;
    try {
      if (!units.has(file)) units.set(file, parseUnit(repoRoot, file));
      const unit = units.get(file);
      const node = unit && atDiagnostic(unit, diagnostic);
      if (!node) return null;
      const context = { steps: 0, spans: new Map() };
      let proof;
      if (diagnostic.rule === "no-derived-state") {
        const info = stateInfo(unit, node.callee);
        if (
          !info ||
          diagnostic.message !==
            `Storing "${info.name}" in state when you can derive it from other values costs an extra render.`
        )
          return null;
        const effect = enclosingEffect(unit, node);
        if (!effect) return null;
        proof =
          snapshotProof(unit, node, effect, context) ??
          firstSeedProof(unit, node, effect, context);
      } else proof = observationProof(unit, node, context);
      if (!proof) return null;
      return {
        ...proof,
        provenance: { ...PROVENANCE, ...RULES[diagnostic.rule] },
        evidence: [
          {
            file,
            sha256: createHash("sha256").update(unit.source).digest("hex"),
            spans: [...context.spans.values()],
          },
        ],
      };
    } catch {
      return null;
    }
  };
}

module.exports = { createHistoryObservationSemanticCorrections };
