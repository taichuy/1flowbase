const fs = require("node:fs");
const path = require("node:path");
const {
  unwrap,
  memberName,
  immutable,
  importedCall,
  finiteValueProof,
} = require("../source-unit.js");
const {
  createNativeReceiverCertificate,
} = require("../native-provenance/receiver.js");
const { MAX_CODE_UNITS } = require("../native-provenance/deferred-loader.js");

const FUNCTIONS = new Set([
  "FunctionDeclaration",
  "FunctionExpression",
  "ArrowFunctionExpression",
]);
const propertyName = (node) =>
  !node.computed && node.key?.type === "Identifier"
    ? node.key.name
    : node.key?.type === "Literal" && typeof node.key.value === "string"
      ? node.key.value
      : null;
const bodyReturn = (fn) =>
  fn.body.type !== "BlockStatement"
    ? unwrap(fn.body)
    : fn.body.body.length === 1 && fn.body.body[0].type === "ReturnStatement"
      ? unwrap(fn.body.body[0].argument)
      : null;

// This adapter proves all effects reached by the diagnostic together. A cache
// effect cannot excuse another state write, an opaque callback, or a store reset.
function completionObservation(unit, effects, root, context) {
  const resolved = (context.completionUnits ??= new Map([[unit.file, unit]]));
  const tick = () => {
    if (++context.steps > 20000)
      throw new Error("completion observation proof budget exceeded");
  };
  const add = (source, node) => {
    tick();
    if (!node?.range) return;
    const spans =
      source === unit
        ? context.spans
        : ((context.resolvedEvidence ??= new Map()).get(source) ?? new Map());
    if (source !== unit) context.resolvedEvidence.set(source, spans);
    spans.set(node.range.join(":"), {
      start: node.range[0],
      end: node.range[1],
    });
  };
  const binding = (source, node) => source.binding(unwrap(node));
  const same = (source, a, b) => {
    a = unwrap(a);
    b = unwrap(b);
    if (!a || !b || a.type !== b.type) return false;
    if (a.type === "Identifier")
      return (
        Boolean(binding(source, a)) && binding(source, a) === binding(source, b)
      );
    return (
      a.type === "MemberExpression" &&
      memberName(a) !== null &&
      memberName(a) === memberName(b) &&
      same(source, a.object, b.object)
    );
  };
  function relative(source, specifier) {
    tick();
    if (typeof specifier !== "string" || !specifier.startsWith("."))
      return null;
    const base = path.posix.normalize(
      path.posix.join(path.posix.dirname(source.file), specifier),
    );
    if (!base.startsWith("web/app/")) return null;
    const candidates = [
      base,
      ...[".ts", ".tsx", ".js", ".jsx"].map((suffix) => base + suffix),
    ].filter((file) => {
      try {
        return fs.statSync(path.join(context.repoRoot, file)).isFile();
      } catch {
        return false;
      }
    });
    if (
      candidates.length !== 1 ||
      (!resolved.has(candidates[0]) && resolved.size >= MAX_CODE_UNITS)
    )
      return null;
    if (!resolved.has(candidates[0]))
      resolved.set(candidates[0], context.read(candidates[0]));
    return resolved.get(candidates[0]);
  }
  function exported(source, name, seen = new Set(), depth = 0) {
    tick();
    const identity = `${source.file}:${name}`;
    if (depth > 12 || seen.has(identity)) return null;
    seen = new Set([...seen, identity]);
    for (const statement of source.ast.body) {
      if (statement.type !== "ExportNamedDeclaration") continue;
      const declaration = statement.declaration;
      if (
        declaration?.type === "FunctionDeclaration" &&
        declaration.id?.name === name
      ) {
        add(source, statement);
        return { unit: source, node: declaration };
      }
      if (declaration?.type === "VariableDeclaration") {
        const entry = declaration.declarations.find(
          (node) => node.id.type === "Identifier" && node.id.name === name,
        );
        if (entry) {
          add(source, statement);
          return { unit: source, node: unwrap(entry.init) };
        }
      }
      const specifier = statement.specifiers?.find(
        (entry) => (entry.exported.name ?? entry.exported.value) === name,
      );
      if (!specifier) continue;
      add(source, statement);
      if (statement.source) {
        const target = relative(source, statement.source.value);
        return (
          target &&
          exported(
            target,
            specifier.local.name ?? specifier.local.value,
            seen,
            depth + 1,
          )
        );
      }
      return identifier(source, specifier.local, seen, depth + 1);
    }
    return null;
  }
  function identifier(source, node, seen = new Set(), depth = 0) {
    tick();
    node = unwrap(node);
    if (depth > 12 || node?.type !== "Identifier") return null;
    const variable = binding(source, node);
    if (!immutable(variable) || seen.has(variable)) return null;
    const definition = variable.defs[0];
    seen = new Set([...seen, variable]);
    if (definition.type === "ImportBinding") {
      const declaration = source.parents.get(definition.node);
      if (
        definition.node.type !== "ImportSpecifier" ||
        declaration?.type !== "ImportDeclaration"
      )
        return null;
      const target = relative(source, declaration.source.value);
      add(source, declaration);
      return (
        target &&
        exported(
          target,
          definition.node.imported.name ?? definition.node.imported.value,
          seen,
          depth + 1,
        )
      );
    }
    if (definition.type === "FunctionName") {
      add(source, definition.node);
      return { unit: source, node: definition.node };
    }
    if (
      definition.type !== "Variable" ||
      definition.node.id.type !== "Identifier"
    )
      return null;
    add(source, definition.node);
    const init = unwrap(definition.node.init);
    return init?.type === "Identifier"
      ? identifier(source, init, seen, depth + 1)
      : { unit: source, node: init };
  }
  function dependency(effect, node) {
    const dependencies = unwrap(effect.arguments[1]);
    return (
      dependencies?.type === "ArrayExpression" &&
      dependencies.elements.every(
        (entry) => entry && entry.type !== "SpreadElement",
      ) &&
      dependencies.elements.some((entry) => same(unit, entry, node))
    );
  }
  function reads(source, node, visit, depth = 0) {
    tick();
    node = unwrap(node);
    if (!node || depth > 12) return false;
    if (node.type === "Literal") return true;
    if (["Identifier", "MemberExpression"].includes(node.type))
      return visit(node);
    if (
      node.type === "UnaryExpression" &&
      ["!", "+", "-"].includes(node.operator)
    )
      return reads(source, node.argument, visit, depth + 1);
    if (["LogicalExpression", "BinaryExpression"].includes(node.type))
      return (
        reads(source, node.left, visit, depth + 1) &&
        reads(source, node.right, visit, depth + 1)
      );
    return false;
  }
  function depends(node, seen = new Set(), depth = 0) {
    tick();
    node = unwrap(node);
    if (!node || depth > 12) return false;
    if (node.type === "Identifier") {
      const variable = binding(unit, node);
      if (variable === root) return true;
      if (
        !immutable(variable) ||
        seen.has(variable) ||
        variable.defs[0].type !== "Variable"
      )
        return false;
      return depends(
        variable.defs[0].node.init,
        new Set([...seen, variable]),
        depth + 1,
      );
    }
    if (FUNCTIONS.has(node.type)) return false;
    return (unit.visitorKeys[node.type] ?? []).some((key) => {
      if (
        node.type === "MemberExpression" &&
        key === "property" &&
        !node.computed
      )
        return false;
      const value = node[key];
      return (Array.isArray(value) ? value : [value]).some(
        (child) => child?.type && depends(child, seen, depth + 1),
      );
    });
  }
  function guardProof(effect, test) {
    if (!reads(unit, test, (node) => dependency(effect, node))) return false;
    const model = finiteValueProof(context);
    const domain = new Set([true, false, null, 0, 1]);
    const stack = [test];
    while (stack.length) {
      const node = stack.pop();
      tick();
      if (node.type === "Literal") domain.add(node.value);
      for (const field of unit.visitorKeys[node.type] ?? []) {
        const value = node[field];
        for (const child of Array.isArray(value) ? value : [value])
          if (child?.type) stack.push(child);
      }
    }
    const key = (node, seen = new Set()) => {
      if (node.type === "Identifier") {
        const variable = binding(unit, node);
        if (!variable || seen.has(variable)) return null;
        const definition = variable.defs[0];
        if (
          immutable(variable) &&
          definition.type === "Variable" &&
          ["Identifier", "MemberExpression"].includes(
            unwrap(definition.node.init)?.type,
          )
        ) {
          return key(
            unwrap(definition.node.init),
            new Set([...seen, variable]),
          );
        }
        return `${variable.identifiers[0]?.range[0]}`;
      }
      if (node.type !== "MemberExpression" || memberName(node) === null)
        return null;
      const receiver = key(unwrap(node.object), seen);
      return receiver === null ? null : `${receiver}.${memberName(node)}`;
    };
    const compile = (epoch) =>
      model.expression(unit, test, (node) => {
        if (!["Identifier", "MemberExpression"].includes(node.type))
          return undefined;
        if (node.type === "Identifier") {
          const variable = binding(unit, node);
          if (!variable) return null;
          const definition = variable.defs[0];
          if (
            immutable(variable) &&
            definition.type === "Variable" &&
            !["CallExpression", "ObjectExpression", "ArrayExpression"].includes(
              unwrap(definition.node.init)?.type,
            )
          )
            return undefined;
        }
        const selector = key(node);
        return selector === null
          ? null
          : model.input(`${selector}:${depends(node) ? epoch : "stable"}`, [
              ...domain,
            ]);
      });
    const a = compile("before");
    const b = compile("after");
    return (
      a &&
      b &&
      model.witness([a, b], ([left, right]) => Boolean(left) !== Boolean(right))
    );
  }
  function client(node, seen = new Set(), depth = 0) {
    tick();
    node = unwrap(node);
    if (depth > 12 || node?.type !== "Identifier") return null;
    const variable = binding(unit, node);
    if (
      !immutable(variable) ||
      seen.has(variable) ||
      variable.defs[0].type !== "Variable"
    )
      return null;
    const declaration = variable.defs[0].node;
    const init = unwrap(declaration.init);
    if (
      importedCall(unit, init, "@tanstack/react-query", "useQueryClient") &&
      !init.arguments.length
    ) {
      add(unit, init);
      add(unit, declaration);
      return variable;
    }
    return client(init, new Set([...seen, variable]), depth + 1);
  }
  function unmodifiedClient(origin) {
    context.nativeReceiverCertificates ??= new Map();
    if (!context.nativeReceiverCertificates.has(unit))
      context.nativeReceiverCertificates.set(
        unit,
        createNativeReceiverCertificate({
          unit,
          kind: "query",
          tick,
          add: (node) => add(unit, node),
          depthLimit: 12,
          repoRoot: context.repoRoot,
          read: context.read,
          resolved,
          addSource: add,
        }),
      );
    return context.nativeReceiverCertificates.get(unit).certify(origin);
  }
  function keyValue(
    source,
    node,
    parameters = new Set(),
    depth = 0,
    seen = new Set(),
  ) {
    tick();
    node = unwrap(node);
    if (!node || depth > 12) return false;
    if (node.type === "Literal") return true;
    if (node.type === "Identifier") {
      const variable = binding(source, node);
      if (parameters.has(variable)) return true;
      if (
        !immutable(variable) ||
        seen.has(variable) ||
        variable.defs[0].type !== "Variable"
      )
        return false;
      return keyValue(
        source,
        variable.defs[0].node.init,
        parameters,
        depth + 1,
        new Set([...seen, variable]),
      );
    }
    if (node.type === "ArrayExpression")
      return node.elements.every(
        (entry) =>
          entry && keyValue(source, entry, parameters, depth + 1, seen),
      );
    return false;
  }
  function cacheKey(effect, node) {
    node = unwrap(node);
    if (node?.type === "ArrayExpression")
      return node.elements.every(
        (entry) =>
          entry && reads(unit, entry, (value) => dependency(effect, value)),
      )
        ? node.elements
        : null;
    if (
      node?.type !== "CallExpression" ||
      unwrap(node.callee)?.type !== "Identifier" ||
      !node.arguments.every(
        (entry) =>
          entry.type !== "SpreadElement" &&
          reads(unit, entry, (value) => dependency(effect, value)),
      )
    )
      return null;
    const helper = identifier(unit, node.callee);
    if (
      !helper ||
      !FUNCTIONS.has(helper.node?.type) ||
      helper.node.async ||
      helper.node.generator ||
      helper.node.params.length !== node.arguments.length ||
      helper.node.params.some((entry) => entry.type !== "Identifier")
    )
      return null;
    const parameters = new Set(
      helper.node.params.map((entry) => binding(helper.unit, entry)),
    );
    const result = bodyReturn(helper.node);
    if (
      result?.type !== "ArrayExpression" ||
      !keyValue(helper.unit, result, parameters)
    )
      return null;
    add(helper.unit, helper.node);
    add(unit, node);
    return node.arguments;
  }
  function conditional(effect) {
    const fn = unwrap(effect.arguments[0]);
    if (
      !FUNCTIONS.has(fn?.type) ||
      fn.async ||
      fn.generator ||
      fn.body.type !== "BlockStatement"
    )
      return null;
    const statements = fn.body.body;
    let condition;
    let statement;
    let invert = false;
    if (
      statements.length === 1 &&
      statements[0].type === "IfStatement" &&
      !statements[0].alternate
    ) {
      condition = statements[0].test;
      const branch = statements[0].consequent;
      statement =
        branch.type === "BlockStatement" && branch.body.length === 1
          ? branch.body[0]
          : branch;
    } else if (
      statements.length === 2 &&
      statements[0].type === "IfStatement" &&
      !statements[0].alternate &&
      statements[0].consequent.type === "ReturnStatement" &&
      !statements[0].consequent.argument
    ) {
      condition = statements[0].test;
      statement = statements[1];
      invert = true;
    } else return null;
    if (
      statement?.type !== "ExpressionStatement" ||
      !guardProof(effect, condition)
    )
      return null;
    let action = unwrap(statement.expression);
    if (action?.type === "UnaryExpression" && action.operator === "void")
      action = unwrap(action.argument);
    if (action?.type !== "CallExpression") return null;
    return {
      effect,
      condition,
      statement,
      action,
      invert,
      first: statements[0],
      sequence: { range: [statements[0].range[0], statements.at(-1).range[1]] },
    };
  }
  function cacheEffect(shape) {
    const { effect, action } = shape;
    if (
      memberName(action.callee) !== "invalidateQueries" ||
      action.arguments.length !== 1
    )
      return null;
    const receiver = unwrap(action.callee).object;
    const origin = client(receiver);
    if (!origin || !dependency(effect, receiver) || !unmodifiedClient(origin))
      return null;
    const options = unwrap(action.arguments[0]);
    if (options?.type !== "ObjectExpression" || options.properties.length !== 1)
      return null;
    const property = options.properties[0];
    if (
      property.type !== "Property" ||
      property.kind !== "init" ||
      property.method ||
      propertyName(property) !== "queryKey"
    )
      return null;
    const values = cacheKey(effect, property.value);
    if (!values) return null;
    add(unit, effect);
    add(unit, shape.first);
    add(unit, shape.sequence);
    add(unit, shape.condition);
    add(unit, shape.statement.expression);
    add(unit, action);
    return { ...shape, values };
  }
  const shapes = effects.map(conditional);
  if (shapes.some((entry) => !entry)) return null;
  if (!shapes.every((shape) => cacheEffect(shape))) return null;
  return {
    predicate: "query-observation-synchronization",
    reason:
      "This effect only synchronizes an external query cache through a proven native query client, a pure observed key, complete dependencies and an effective condition. Other reached effects require their own independent proof.",
  };
}

module.exports = { completionObservation };
