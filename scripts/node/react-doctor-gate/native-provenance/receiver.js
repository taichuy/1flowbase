const {
  unwrap,
  memberName,
  immutable,
  importOrigin,
  importedCall,
} = require("../source-unit.js");
const { createDeferredLoaderAudit } = require("./deferred-loader.js");

const TRANSPARENT = new Set([
  "ChainExpression",
  "TSAsExpression",
  "TSNonNullExpression",
  "TSTypeAssertion",
  "ParenthesizedExpression",
  "TSSatisfiesExpression",
]);
const DEPENDENCY_HOOKS = [
  "useEffect",
  "useLayoutEffect",
  "useCallback",
  "useMemo",
];
// Public vendor operations, not application method names. The receiver and its
// producer surfaces must independently retain their native imported provenance.
const QUERY_METHODS = new Set([
  "invalidateQueries",
  "cancelQueries",
  "removeQueries",
  "resetQueries",
  "refetchQueries",
  "setQueryData",
  "setQueriesData",
  "getQueryData",
  "getQueriesData",
  "getQueryState",
  "isFetching",
  "isMutating",
  "fetchQuery",
  "prefetchQuery",
  "fetchInfiniteQuery",
  "prefetchInfiniteQuery",
  "ensureQueryData",
  "ensureInfiniteQueryData",
  "getQueryCache",
  "getMutationCache",
  "getDefaultOptions",
  "setDefaultOptions",
  "getQueryDefaults",
  "setQueryDefaults",
  "getMutationDefaults",
  "setMutationDefaults",
  "clear",
  "mount",
  "unmount",
]);
const FORM_METHODS = new Set([
  "setFieldsValue",
  "setFieldValue",
  "resetFields",
  "setFields",
  "getFieldValue",
  "getFieldsValue",
  "getFieldError",
  "getFieldsError",
  "getFieldWarning",
  "getFieldInstance",
  "isFieldsTouched",
  "isFieldTouched",
  "isFieldValidating",
  "isFieldsValidating",
  "validateFields",
  "scrollToField",
  "focusField",
  "submit",
]);
const OBJECT_METHODS = new Set([
  "assign",
  "create",
  "defineProperty",
  "defineProperties",
  "entries",
  "fromEntries",
  "freeze",
  "getOwnPropertyDescriptor",
  "getOwnPropertyDescriptors",
  "getOwnPropertyNames",
  "getOwnPropertySymbols",
  "getPrototypeOf",
  "hasOwn",
  "is",
  "isExtensible",
  "isFrozen",
  "isSealed",
  "keys",
  "preventExtensions",
  "seal",
  "setPrototypeOf",
  "values",
]);

function createNativeReceiverCertificate({
  unit,
  kind,
  tick,
  add,
  depthLimit = 8,
  repoRoot,
  read,
  resolved,
  addSource = (source, node) => {
    if (source === unit) add(node);
    else throw new Error("missing source evidence owner");
  },
  shared,
  moduleDepth = 0,
}) {
  const certificates = new Map(),
    surfaces = new Map();
  let producerSafe, sourceSafe;
  const factoryCalls = [];
  const state = shared ?? {
    repoRoot,
    read,
    units: resolved ?? new Map([[unit.file, unit]]),
    assets: new Map(),
    assetBytes: 0,
    certificates: new Map(),
  };
  const deferred = createDeferredLoaderAudit({
    unit,
    state,
    tick,
    add: addSource,
    depthLimit,
    auditTarget: (target) => {
      if (moduleDepth >= depthLimit) return false;
      const certificate =
        state.certificates.get(target.file) ??
        createNativeReceiverCertificate({
          unit: target,
          kind,
          tick,
          add: (node) => addSource(target, node),
          addSource,
          depthLimit,
          shared: state,
          moduleDepth: moduleDepth + 1,
        });
      return certificate.audit();
    },
  });
  const methods = kind === "form" ? FORM_METHODS : QUERY_METHODS;
  function lift(node) {
    for (
      let parent = unit.parents.get(node);
      parent && TRANSPARENT.has(parent.type) && parent.expression === node;
      parent = unit.parents.get(node)
    )
      node = parent;
    return node;
  }
  function typeOnly(node) {
    for (
      let parent = unit.parents.get(node);
      parent;
      parent = unit.parents.get(parent)
    ) {
      if (parent.type.startsWith("TS") && !TRANSPARENT.has(parent.type))
        return true;
      if (
        parent.type.endsWith("Expression") ||
        parent.type.endsWith("Statement") ||
        [
          "VariableDeclarator",
          "JSXOpeningElement",
          "JSXExpressionContainer",
        ].includes(parent.type)
      )
        return false;
    }
    return false;
  }
  function identifier(variable) {
    return variable?.identifiers[0];
  }
  function surface(variable, seen = new Set(), depth = 0) {
    tick();
    if (!immutable(variable) || depth > depthLimit || seen.has(variable))
      return null;
    if (surfaces.has(variable)) return surfaces.get(variable);
    seen = new Set([...seen, variable]);
    const origin = importOrigin(unit, identifier(variable));
    let result = null;
    if (
      kind === "query" &&
      ["@tanstack/react-query", "@tanstack/query-core"].includes(origin?.module)
    ) {
      if (origin.name === "QueryClient") result = "query-constructor";
      else if (
        origin.module === "@tanstack/react-query" &&
        origin.name === "useQueryClient"
      )
        result = "query-factory";
      else if (["*", "default"].includes(origin.name))
        result = "query-namespace";
    } else if (
      kind === "form" &&
      origin?.module === "antd" &&
      origin.name === "Form"
    )
      result = "form-facade";
    if (origin?.module === "react") {
      if (origin.name === "lazy") result = "lazy-factory";
      else if (["*", "default"].includes(origin.name)) result = "react-facade";
    }
    if (
      !result &&
      variable.defs[0].type === "Variable" &&
      variable.defs[0].node.id.type === "Identifier"
    ) {
      const init = unwrap(variable.defs[0].node.init);
      if (init?.type === "Identifier") {
        const previous = unit.binding(init);
        result = previous?.defs.length
          ? surface(previous, seen, depth + 1)
          : init.name === "Object"
            ? "global-object"
            : init.name === "Promise"
              ? "global-promise"
              : null;
      }
    }
    surfaces.set(variable, result);
    return result;
  }
  function alias(node, accept) {
    const current = lift(node),
      parent = unit.parents.get(current);
    if (
      parent?.type !== "VariableDeclarator" ||
      parent.init !== current ||
      parent.id.type !== "Identifier"
    )
      return false;
    const variable = unit.binding(parent.id);
    if (!immutable(variable) || !accept(variable)) return false;
    add(parent);
    return true;
  }
  function memberChain(node) {
    let current = lift(node),
      parent = unit.parents.get(current);
    const names = [];
    while (
      parent &&
      ["MemberExpression", "JSXMemberExpression"].includes(parent.type) &&
      parent.object === current
    ) {
      const name =
        parent.type === "MemberExpression"
          ? memberName(parent)
          : parent.property.name;
      if (name === null) return null;
      names.push(name);
      current = lift(parent);
      parent = unit.parents.get(current);
    }
    return { current, parent, names };
  }
  function jsxSurface(node, profile) {
    const chain = memberChain(node);
    if (
      chain &&
      profile === "react-facade" &&
      ["JSXOpeningElement", "JSXClosingElement"].includes(chain.parent?.type) &&
      chain.parent.name === chain.current &&
      chain.names.length === 1 &&
      ["Fragment", "Suspense", "StrictMode", "Profiler"].includes(
        chain.names[0],
      )
    )
      return true;
    if (
      !chain ||
      profile !== "form-facade" ||
      !["JSXOpeningElement", "JSXClosingElement"].includes(
        chain.parent?.type,
      ) ||
      chain.parent.name !== chain.current
    )
      return false;
    return (
      !chain.names.length ||
      (chain.names.length === 1 &&
        ["Item", "List", "ErrorList", "Provider"].includes(chain.names[0]))
    );
  }
  function surfaceUse(node, profile) {
    if (typeOnly(node)) return true;
    if (alias(node, (variable) => surface(variable) === profile)) return true;
    if (jsxSurface(node, profile)) return true;
    const chain = memberChain(node);
    if (
      !chain ||
      chain.names.some((name) =>
        ["prototype", "constructor", "__proto__"].includes(name),
      )
    )
      return false;
    const { parent, current, names } = chain;
    if (parent?.type === "NewExpression" && parent.callee === current) {
      return (
        (profile === "query-constructor" && !names.length) ||
        (profile === "query-namespace" &&
          names.length === 1 &&
          names[0] === "QueryClient")
      );
    }
    if (parent?.type !== "CallExpression" || parent.callee !== current)
      return false;
    if (profile === "query-factory") return !names.length;
    if (profile === "lazy-factory")
      return !names.length && deferred.inspectLazy(parent);
    if (profile === "react-facade") {
      if (names.length !== 1) return false;
      if (names[0] === "lazy") return deferred.inspectLazy(parent);
      return [
        "useEffect",
        "useLayoutEffect",
        "useCallback",
        "useMemo",
        "useState",
        "useRef",
        "createContext",
        "useContext",
        "createElement",
        "memo",
        "forwardRef",
        "isValidElement",
        "cloneElement",
      ].includes(names[0]);
    }
    if (profile === "global-promise")
      return (
        names.length === 1 &&
        [
          "all",
          "allSettled",
          "any",
          "race",
          "resolve",
          "reject",
          "withResolvers",
        ].includes(names[0])
      );
    if (profile === "query-namespace")
      return (
        names.length === 1 &&
        [
          "useQueryClient",
          "useMutation",
          "useQuery",
          "useQueries",
          "useInfiniteQuery",
          "useIsFetching",
          "useIsMutating",
        ].includes(names[0])
      );
    if (profile === "form-facade")
      return (
        names.length === 1 &&
        ["useForm", "useFormInstance", "useWatch"].includes(names[0])
      );
    return (
      profile === "global-object" &&
      (!names.length || (names.length === 1 && OBJECT_METHODS.has(names[0])))
    );
  }
  function producersAreClosed() {
    if (producerSafe !== undefined) return producerSafe;
    producerSafe = false;
    const variables = new Set();
    for (const node of unit.nodes) {
      if (
        ![
          "ImportExpression",
          "CallExpression",
          "Identifier",
          "JSXIdentifier",
        ].includes(node.type)
      )
        continue;
      tick();
      if (node.type === "ImportExpression") {
        if (!deferred.inspectImport(node)) return false;
        continue;
      }
      if (node.type === "CallExpression") {
        const callee = unwrap(node.callee);
        if (
          callee?.type === "Identifier" &&
          callee.name === "require" &&
          !unit.binding(callee)?.defs.length
        )
          return false;
        if (
          importedCall(unit, node, "react", "lazy") &&
          !deferred.inspectLazy(node)
        )
          return false;
        if (nativeFactory(node)) factoryCalls.push(node);
        continue;
      }
      if (
        !["Identifier", "JSXIdentifier"].includes(node.type) ||
        !unit.bindings.has(node) ||
        typeOnly(node)
      )
        continue;
      const variable = unit.binding(node);
      if (variable?.defs.length) variables.add(variable);
      else if (["eval", "Function"].includes(node.name)) return false;
      else if (node.name === "Object" && !surfaceUse(node, "global-object"))
        return false;
      else if (node.name === "Promise" && !surfaceUse(node, "global-promise"))
        return false;
      else if (["globalThis", "window", "self"].includes(node.name)) {
        const parent = unit.parents.get(lift(node));
        if (
          parent?.type === "MemberExpression" &&
          (memberName(parent) === null ||
            ["eval", "Function"].includes(memberName(parent)))
        )
          return false;
      }
    }
    for (const variable of variables) {
      const profile = surface(variable);
      if (!profile) continue;
      for (const reference of variable.references) {
        tick();
        // The scope manager can attach an implicit JSX factory reference to
        // the import-local declaration token. That token is not an executable
        // escape; actual calls, writes and argument uses have separate nodes.
        if (
          variable.defs[0].type === "ImportBinding" &&
          reference.identifier === variable.defs[0].node.local
        )
          continue;
        if (
          (reference.init && reference.isWrite()) ||
          typeOnly(reference.identifier)
        )
          continue;
        if (!surfaceUse(reference.identifier, profile)) return false;
      }
      const definition = variable.defs[0];
      add(
        definition.type === "ImportBinding"
          ? unit.parents.get(definition.node)
          : definition.node,
      );
    }
    producerSafe = true;
    return true;
  }
  function nativeFactory(node) {
    if (kind === "query")
      return importedCall(
        unit,
        node,
        "@tanstack/react-query",
        "useQueryClient",
      );
    const callee = unwrap(node.callee);
    const origin =
      callee?.type === "MemberExpression" && memberName(callee) === "useForm"
        ? importOrigin(unit, callee.object)
        : null;
    return origin?.module === "antd" && origin.name === "Form";
  }
  function seed(origin) {
    if (!immutable(origin) || origin.defs[0].type !== "Variable") return false;
    const declaration = origin.defs[0].node,
      init = unwrap(declaration.init);
    if (
      init?.type !== "CallExpression" ||
      init.optional ||
      init.arguments.length
    )
      return false;
    if (kind === "query") {
      if (
        declaration.id.type !== "Identifier" ||
        !importedCall(unit, init, "@tanstack/react-query", "useQueryClient")
      )
        return false;
    } else {
      const callee = unwrap(init.callee);
      if (
        declaration.id.type !== "ArrayPattern" ||
        unit.binding(declaration.id.elements[0]) !== origin ||
        callee?.type !== "MemberExpression" ||
        callee.optional ||
        memberName(callee) !== "useForm"
      )
        return false;
      const imported = importOrigin(unit, callee.object);
      if (imported?.module !== "antd" || imported.name !== "Form") return false;
    }
    add(declaration);
    add(init);
    return true;
  }
  function formConsumer(node) {
    const current = lift(node),
      expression = unit.parents.get(current);
    if (
      kind !== "form" ||
      expression?.type !== "JSXExpressionContainer" ||
      expression.expression !== current
    )
      return false;
    const attribute = unit.parents.get(expression),
      opening = unit.parents.get(attribute);
    if (
      attribute?.type !== "JSXAttribute" ||
      attribute.name.name !== "form" ||
      opening?.type !== "JSXOpeningElement"
    )
      return false;
    if (opening.name.type !== "JSXIdentifier") return false;
    const variable = unit.binding(opening.name);
    if (surface(variable) !== "form-facade") return false;
    add(opening);
    return true;
  }
  function dependency(node) {
    const current = lift(node),
      array = unit.parents.get(current);
    if (array?.type !== "ArrayExpression" || !array.elements.includes(current))
      return false;
    const call = unit.parents.get(lift(array));
    return (
      call?.type === "CallExpression" &&
      call.arguments[1] === lift(array) &&
      DEPENDENCY_HOOKS.some((name) => importedCall(unit, call, "react", name))
    );
  }
  function nativeCall(node) {
    const current = lift(node),
      member = unit.parents.get(current);
    if (
      member?.type !== "MemberExpression" ||
      member.object !== current ||
      !methods.has(memberName(member))
    )
      return false;
    const callee = lift(member),
      call = unit.parents.get(callee);
    return call?.type === "CallExpression" && call.callee === callee;
  }
  function certify(origin) {
    if (certificates.has(origin)) return certificates.get(origin);
    certificates.set(origin, false);
    if (!seed(origin) || !producersAreClosed()) return false;
    const aliases = new Set([origin]),
      pending = [{ variable: origin, depth: 0 }];
    while (pending.length) {
      const { variable, depth } = pending.pop();
      if (depth > depthLimit) return false;
      for (const reference of variable.references) {
        tick();
        const node = reference.identifier;
        if ((reference.init && reference.isWrite()) || typeOnly(node)) continue;
        if (
          alias(node, (target) => {
            if (!aliases.has(target)) {
              aliases.add(target);
              pending.push({ variable: target, depth: depth + 1 });
            }
            return true;
          })
        )
          continue;
        if (nativeCall(node) || dependency(node) || formConsumer(node))
          continue;
        // A carrier, callback/default argument, return, extracted member, JSX or
        // opaque call is not a readonly native consumer. Its shape/name/type is
        // insufficient: retain the finding rather than infer native behavior.
        return false;
      }
    }
    certificates.set(origin, true);
    return true;
  }
  function audit() {
    if (sourceSafe !== undefined) return sourceSafe;
    sourceSafe = false;
    if (!deferred.moduleEffects(unit) || !producersAreClosed()) return false;
    for (const call of factoryCalls) {
      tick();
      const declaration = unit.parents.get(call);
      if (
        declaration?.type !== "VariableDeclarator" ||
        declaration.init !== call
      )
        return false;
      const id =
        kind === "form" && declaration.id.type === "ArrayPattern"
          ? declaration.id.elements[0]
          : declaration.id;
      if (id?.type !== "Identifier" || !certify(unit.binding(id))) return false;
    }
    if (!unit.source.length) return false;
    add(unit.ast);
    sourceSafe = true;
    return true;
  }
  const certificate = { certify, audit };
  state.certificates.set(unit.file, certificate);
  return certificate;
}

module.exports = { createNativeReceiverCertificate };
