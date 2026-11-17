const fs = require("node:fs");
const path = require("node:path");
const { createHash } = require("node:crypto");
const { createRequire } = require("node:module");
const {
  parseUnit,
  unwrap,
  memberName,
  immutable,
  importOrigin,
} = require("./source-unit.js");

const TOOL = "react-doctor@0.2.16";
const PROVENANCE = {
  toolPackage: TOOL,
  packageIntegrity:
    "sha512-o108QscqM/TlC1QmKgDORf/P53PeZ69nZbRsvy53rXkKvWdKZC1Y0u1JRPmThbTh6v+QpMispBwEnFHhlpZQ7w==",
  sourceCommit: "2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc",
};
const RULES = {
  "rendering-hydration-mismatch-time": {
    message:
      "This breaks hydration because crypto.randomUUID() reached from JSX gives a different value on the server than in the browser, so move it into useEffect+useState to run only in the browser, or add suppressHydrationWarning to the parent if it's on purpose",
    ruleSourceSha256:
      "26a97648e84587588c40d4bc2b2f547928c708aecd4f806a977b4da3a51a4fc7",
    references: [
      "https://github.com/millionco/react-doctor/pull/1122",
      "https://react.dev/learn/responding-to-events",
      "https://github.com/ant-design/ant-design/blob/6.6.2/components/button/Button.tsx",
    ],
  },
  "click-events-have-key-events": {
    message:
      "Keyboard users can't trigger this click handler because there's no keyboard one, so add `onKeyUp`, `onKeyDown`, or `onKeyPress`.",
    ruleSourceSha256:
      "853dc1db4c6f4ae5af0d9bda8aefd826f7d7841b8678835b4858bd7be8ac86a3",
    references: [
      "https://github.com/millionco/react-doctor/pull/1515",
      "https://react.dev/learn/responding-to-events#stopping-propagation",
    ],
  },
  "no-static-element-interactions": {
    message:
      "Screen reader users can't tell this click handler is interactive because it has no `role`, so add a `role` or use a button or link.",
    ruleSourceSha256:
      "6b3b9ae205ce6a3b85e443d05f2fb9ee6dd4bc897744fc68fce3e99fe8c16831",
    references: [
      "https://github.com/millionco/react-doctor/pull/1515",
      "https://react.dev/learn/responding-to-events#stopping-propagation",
    ],
  },
  "prefer-tag-over-role": {
    message:
      'Screen reader users get more reliable semantics from `<img>` than `role="img"`, so use `<img>` instead.',
    ruleSourceSha256:
      "a424acbda97ead457a61ba3b06ffade0b2af6c755b0ad4d147de08717507d4ca",
    references: [
      "https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Reference/Roles/img_role",
      "https://developer.mozilla.org/en-US/docs/Glossary/Void_element",
      "https://github.com/ant-design/ant-design-icons/blob/6.3.4/packages/icons-react/src/components/AntdIcon.tsx",
    ],
  },
  "js-tosorted-immutable": {
    message:
      "This wastes work because [...array].sort() copies the array just to sort it, so use array.toSorted() to sort without the extra copy (ES2023)",
    ruleSourceSha256:
      "296d54afccde391f23130786c6f601806df3925f1c0983062f6537d017183e48",
    references: [
      "https://github.com/millionco/react-doctor/pull/753",
      "https://www.typescriptlang.org/tsconfig/lib.html",
      "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/toSorted",
    ],
  },
  "js-hoist-intl": {
    message: (name) =>
      `This is slow because new Intl.${name}() rebuilds on every call inside a function, so move it to the top of the file, or wrap it in useMemo`,
    ruleSourceSha256:
      "ad9c0a9d6c992567e6cba6c7a14d852c41c57624d8e743c8206bc7e5b1f4ecf0",
    references: [
      "https://github.com/millionco/react-doctor/blob/2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc/packages/oxlint-plugin-react-doctor/src/plugin/rules/js-performance/js-hoist-intl.ts",
      "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/Nullish_coalescing_assignment",
      "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Intl/NumberFormat",
    ],
  },
};
const INTL_CLASSES = new Set([
  "NumberFormat",
  "DateTimeFormat",
  "Collator",
  "RelativeTimeFormat",
  "ListFormat",
  "PluralRules",
  "Segmenter",
  "DisplayNames",
]);
const FUNCTIONS = new Set([
  "ArrowFunctionExpression",
  "FunctionExpression",
  "FunctionDeclaration",
]);
const GRAPHIC_TAGS = new Set([
  "span",
  "div",
  "svg",
  "g",
  "path",
  "circle",
  "ellipse",
  "rect",
  "line",
  "polyline",
  "polygon",
  "text",
  "tspan",
  "defs",
  "use",
  "title",
  "desc",
  "linearGradient",
  "radialGradient",
  "stop",
  "clipPath",
  "mask",
]);
let typescript;

function step(context) {
  if (++context.steps > 20000)
    throw new Error("render semantic proof budget exceeded");
}
function addProof(context, node) {
  context.spans.set(node.range.join(":"), {
    start: node.range[0],
    end: node.range[1],
  });
}
function nativeTag(opening) {
  return (
    opening?.name.type === "JSXIdentifier" &&
    /^[a-z][a-z0-9]*$/.test(opening.name.name)
  );
}
function attributes(opening) {
  if (opening.attributes.some((attribute) => attribute.type !== "JSXAttribute"))
    return null;
  const result = new Map();
  for (const attribute of opening.attributes) {
    if (
      attribute.name.type !== "JSXIdentifier" ||
      result.has(attribute.name.name)
    )
      return null;
    result.set(attribute.name.name, attribute);
  }
  return result;
}
function attributeExpression(attribute) {
  return attribute?.value?.type === "JSXExpressionContainer"
    ? unwrap(attribute.value.expression)
    : attribute?.value;
}
function jsxOrigin(unit, tag, context) {
  if (tag?.type !== "JSXIdentifier") return null;
  // The scope manager resolves JSX tag reads as well as ordinary identifiers.
  const variable = unit.binding(tag);
  const origin = variable ? importOrigin(unit, variable.identifiers[0]) : null;
  if (origin && context) {
    const seen = new Set();
    for (let owner = variable; owner && !seen.has(owner); ) {
      step(context);
      seen.add(owner);
      const definition = owner.defs[0];
      const declaration = unit.parents.get(definition.node);
      if (definition.type === "ImportBinding") {
        addProof(context, declaration);
        break;
      }
      if (definition.type !== "Variable") break;
      addProof(
        context,
        declaration?.type === "VariableDeclaration"
          ? declaration
          : definition.node,
      );
      owner = unit.binding(unwrap(definition.node.init));
    }
  }
  return origin;
}
function importedButton(unit, opening, context) {
  const origin = jsxOrigin(unit, opening.name, context);
  return origin?.module === "antd" && origin.name === "Button";
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
  const wanted =
    diagnostic.rule === "js-hoist-intl"
      ? "NewExpression"
      : diagnostic.rule === "prefer-tag-over-role"
        ? "JSXAttribute"
        : diagnostic.rule === "click-events-have-key-events" ||
            diagnostic.rule === "no-static-element-interactions"
          ? "JSXIdentifier"
          : "CallExpression";
  return (
    unit.nodes.find(
      (node) => node.range[0] === offset && node.type === wanted,
    ) ?? null
  );
}

function deferredCallback(unit, node, context) {
  if (memberName(node.callee) !== "randomUUID" || node.arguments.length)
    return null;
  const receiver = unwrap(node.callee.object);
  if (
    receiver?.type !== "Identifier" ||
    receiver.name !== "crypto" ||
    unit.binding(receiver)
  )
    return null;
  // Only a function supplied directly to a proven event prop is a boundary.
  // An IIFE, render Hook, iterator or unknown executor is never such a boundary.
  for (
    let current = unit.parents.get(node);
    current;
    current = unit.parents.get(current)
  ) {
    step(context);
    if (!FUNCTIONS.has(current.type)) continue;
    const container = unit.parents.get(current);
    const attribute = unit.parents.get(container);
    const opening = unit.parents.get(attribute);
    if (
      container?.type !== "JSXExpressionContainer" ||
      unwrap(container.expression) !== current ||
      attribute?.type !== "JSXAttribute" ||
      attribute.name.name !== "onClick" ||
      opening?.type !== "JSXOpeningElement"
    )
      continue;
    const attrs = attributes(opening);
    if (
      !attrs ||
      (!nativeTag(opening) && !importedButton(unit, opening, context))
    )
      return null;
    addProof(context, node);
    addProof(context, attribute);
    addProof(context, opening.name);
    return {
      predicate: "deferred-event-callback",
      reason:
        "The nondeterministic call belongs to a callback passed directly to a native click event or a lexically proven AntD Button click contract; rendering does not invoke that callback.",
    };
  }
  return null;
}

function eventRead(unit, expression, event, property) {
  expression = unwrap(expression);
  return (
    memberName(expression) === property &&
    unwrap(expression.object)?.type === "Identifier" &&
    unit.binding(unwrap(expression.object)) === event
  );
}
function targetGuard(unit, expression, event, context, checked = false) {
  step(context);
  expression = unwrap(expression);
  if (
    expression?.type === "BinaryExpression" &&
    expression.operator === "instanceof" &&
    eventRead(unit, expression.left, event, "target") &&
    expression.right.type === "Identifier" &&
    expression.right.name === "Element" &&
    !unit.binding(expression.right)
  )
    return { checked: true };
  if (
    expression?.type === "LogicalExpression" &&
    expression.operator === "&&"
  ) {
    const left = targetGuard(unit, expression.left, event, context, checked);
    return (
      left &&
      targetGuard(
        unit,
        expression.right,
        event,
        context,
        checked || left.checked,
      )
    );
  }
  if (
    checked &&
    expression?.type === "CallExpression" &&
    memberName(expression.callee) === "closest" &&
    eventRead(unit, expression.callee.object, event, "target") &&
    expression.arguments.length === 1 &&
    expression.arguments[0].type === "Literal" &&
    typeof expression.arguments[0].value === "string" &&
    expression.arguments[0].value.trim()
  )
    return { checked: true };
  return null;
}
function propagationBody(unit, node, event, context) {
  step(context);
  node = unwrap(node);
  if (node?.type === "BlockStatement")
    return (
      node.body.length > 0 &&
      node.body.every((statement) =>
        propagationBody(unit, statement, event, context),
      )
    );
  if (node?.type === "ExpressionStatement")
    return propagationBody(unit, node.expression, event, context);
  if (node?.type === "IfStatement")
    return (
      !!targetGuard(unit, node.test, event, context) &&
      propagationBody(unit, node.consequent, event, context) &&
      (!node.alternate || propagationBody(unit, node.alternate, event, context))
    );
  return (
    node?.type === "CallExpression" &&
    node.arguments.length === 0 &&
    memberName(node.callee) === "stopPropagation" &&
    unwrap(node.callee.object)?.type === "Identifier" &&
    unit.binding(unwrap(node.callee.object)) === event
  );
}
function propagationBoundary(unit, node, context) {
  const opening = unit.parents.get(node);
  if (
    opening?.type !== "JSXOpeningElement" ||
    opening.name !== node ||
    !nativeTag(opening) ||
    !["div", "span"].includes(node.name)
  )
    return null;
  const attrs = attributes(opening);
  if (
    !attrs ||
    [...attrs.keys()].some(
      (name) =>
        (/^on[A-Z]/.test(name) && name !== "onClick") ||
        ["role", "tabIndex", "contentEditable", "href"].includes(name),
    )
  )
    return null;
  const handler = attributeExpression(attrs.get("onClick"));
  if (
    !FUNCTIONS.has(handler?.type) ||
    handler.async ||
    handler.generator ||
    handler.params.length !== 1 ||
    handler.params[0].type !== "Identifier"
  )
    return null;
  const event = unit.binding(handler.params[0]);
  if (!immutable(event) || !propagationBody(unit, handler.body, event, context))
    return null;
  addProof(context, opening);
  addProof(context, handler);
  return {
    predicate: "propagation-only-boundary",
    reason:
      "This native wrapper has only an event-bound stopPropagation effect, optionally guarded by native target inspection, and no activation handler; it is not a separate interactive control.",
  };
}

function passiveAttributes(opening) {
  const attrs = attributes(opening);
  if (
    !attrs ||
    [...attrs.keys()].some(
      (name) =>
        /^on[A-Z]/.test(name) ||
        [
          "tabIndex",
          "contentEditable",
          "dangerouslySetInnerHTML",
          "href",
        ].includes(name),
    )
  )
    return null;
  if (
    attrs.has("role") &&
    !["img", "presentation", "none"].includes(
      attributeExpression(attrs.get("role"))?.value,
    )
  )
    return null;
  return attrs;
}
function graphicChild(unit, node, context) {
  step(context);
  if (node.type === "JSXText") return { content: !!node.value.trim() };
  if (node.type === "JSXExpressionContainer")
    return graphicChild(unit, unwrap(node.expression), context);
  if (node.type === "ConditionalExpression") {
    const left = graphicChild(unit, unwrap(node.consequent), context);
    const right = graphicChild(unit, unwrap(node.alternate), context);
    return left && right ? { content: left.content && right.content } : null;
  }
  if (
    node.type === "Literal" &&
    (typeof node.value === "string" || typeof node.value === "number")
  )
    return { content: String(node.value).trim().length > 0 };
  if (node.type === "JSXFragment")
    return graphicChildren(unit, node.children, context);
  if (node.type !== "JSXElement" || !passiveAttributes(node.openingElement))
    return null;
  const tag = node.openingElement.name;
  if (tag.type !== "JSXIdentifier") return null;
  if (GRAPHIC_TAGS.has(tag.name)) {
    const children = graphicChildren(unit, node.children, context);
    return children && { content: tag.name === "svg" || children.content };
  }
  const origin = jsxOrigin(unit, tag, context);
  const icon =
    origin &&
    ((origin.module === "@ant-design/icons" &&
      /^[A-Z]\w*(?:Outlined|Filled|TwoTone)$/.test(origin.name)) ||
      (/^@ant-design\/icons\/es\/icons\/[A-Z]\w*(?:Outlined|Filled|TwoTone)$/.test(
        origin.module,
      ) &&
        origin.name === "default"));
  return icon && node.children.length === 0 ? { content: true } : null;
}
function graphicChildren(unit, children, context) {
  let content = false;
  for (const child of children) {
    const proof = graphicChild(unit, child, context);
    if (!proof) return null;
    content ||= proof.content;
  }
  return { content };
}
function compositeGraphic(unit, node, context) {
  if (node.name.name !== "role" || attributeExpression(node)?.value !== "img")
    return null;
  const opening = unit.parents.get(node);
  const element = unit.parents.get(opening);
  if (
    opening?.type !== "JSXOpeningElement" ||
    element?.type !== "JSXElement" ||
    !nativeTag(opening) ||
    !["span", "div"].includes(opening.name.name)
  )
    return null;
  const attrs = passiveAttributes(opening);
  const name = attributeExpression(attrs?.get("aria-label"));
  if (
    !name ||
    (name.type === "Literal" &&
      (typeof name.value !== "string" || !name.value.trim())) ||
    (name.type === "Identifier" && ["undefined", "null"].includes(name.name))
  )
    return null;
  if (!graphicChildren(unit, element.children, context)?.content) return null;
  addProof(context, node);
  addProof(context, attrs.get("aria-label"));
  for (const child of element.children)
    if (child.type !== "JSXText" || child.value.trim())
      addProof(context, child);
  return {
    predicate: "composite-graphic-container",
    reason:
      "The named ARIA image groups proven passive graphic content; replacing the container with a native img would discard or reject its children because img is a void element.",
  };
}

function nativeCollection(
  unit,
  expression,
  expected,
  context,
  seen = new Set(),
) {
  step(context);
  expression = unwrap(expression);
  if (
    expression?.type === "NewExpression" &&
    expression.callee.type === "Identifier" &&
    expression.callee.name === expected &&
    !unit.binding(expression.callee)
  ) {
    addProof(context, expression);
    return true;
  }
  if (expression?.type !== "Identifier") return false;
  const variable = unit.binding(expression);
  if (!immutable(variable) || seen.has(variable)) return false;
  seen.add(variable);
  const definition = variable.defs[0];
  if (
    definition.type !== "Variable" ||
    definition.node.id.type !== "Identifier" ||
    !nativeCollection(unit, definition.node.init, expected, context, seen)
  )
    return false;
  const declaration = unit.parents.get(definition.node);
  addProof(
    context,
    declaration.type === "VariableDeclaration" ? declaration : definition.node,
  );
  return true;
}
function materializedCollection(unit, expression, context) {
  expression = unwrap(expression);
  if (nativeCollection(unit, expression, "Set", context)) return true;
  return (
    expression?.type === "CallExpression" &&
    expression.arguments.length === 0 &&
    ["values", "keys", "entries"].includes(memberName(expression.callee)) &&
    nativeCollection(unit, expression.callee.object, "Map", context)
  );
}
function oldApiBaseline(repoRoot, file, context) {
  // Find the source's nearest tsconfig; resolve only local, bounded extends.
  const root = fs.realpathSync(repoRoot);
  let directory = path.dirname(path.resolve(repoRoot, file));
  let config;
  while (directory.startsWith(`${root}${path.sep}`)) {
    const candidate = path.join(directory, "tsconfig.json");
    if (fs.existsSync(candidate)) {
      config = candidate;
      break;
    }
    directory = path.dirname(directory);
  }
  if (!config) return false;
  const seen = new Set();
  const load = (absolute, depth = 0) => {
    step(context);
    const real = fs.realpathSync(absolute);
    if (depth > 12 || seen.has(real) || !real.startsWith(`${root}${path.sep}`))
      throw new Error("invalid config ancestry");
    seen.add(real);
    const source = fs.readFileSync(real, "utf8");
    if (!source.length || source.length > 128 * 1024)
      throw new Error("invalid config size");
    typescript ||= createRequire(
      path.resolve(__dirname, "../../../web/package.json"),
    )("typescript");
    const parsed = typescript.parseConfigFileTextToJson(real, source);
    if (
      parsed.error ||
      !parsed.config ||
      typeof parsed.config !== "object" ||
      Array.isArray(parsed.config)
    )
      throw new Error("invalid compiler config");
    const body = parsed.config;
    let options = {};
    if (body.extends !== undefined) {
      if (
        typeof body.extends !== "string" ||
        !body.extends.startsWith(".") ||
        path.isAbsolute(body.extends)
      )
        throw new Error("unsupported config ancestry");
      let parent = path.resolve(path.dirname(real), body.extends);
      if (!path.extname(parent)) parent += ".json";
      options = load(parent, depth + 1);
    }
    if (
      body.compilerOptions !== undefined &&
      (!body.compilerOptions ||
        typeof body.compilerOptions !== "object" ||
        Array.isArray(body.compilerOptions))
    )
      throw new Error("invalid compiler options");
    context.configs.push({
      file: path.relative(root, real).split(path.sep).join("/"),
      source,
    });
    return { ...options, ...body.compilerOptions };
  };
  const options = load(config);
  if (options.lib !== undefined) {
    if (
      !Array.isArray(options.lib) ||
      !options.lib.length ||
      !options.lib.every((lib) => typeof lib === "string")
    )
      return false;
    const libs = options.lib.map((lib) => lib.toLowerCase());
    if (!libs.every((lib) => typescript.libMap.has(lib))) return false;
    if (libs.some((lib) => /^es(?:202[3-9]|20[3-9]\d|next)(?:\.|$)/.test(lib)))
      return false;
    return libs.some((lib) => /^es(?:5|6|201[5-9]|202[0-2])$/.test(lib));
  }
  return (
    typeof options.target === "string" &&
    /^es(?:3|5|6|201[5-9]|202[0-2])$/i.test(options.target)
  );
}
function immutableSort(unit, node, context, repoRoot) {
  if (memberName(node.callee) !== "sort") return null;
  const receiver = unwrap(node.callee.object);
  if (
    receiver?.type !== "ArrayExpression" ||
    receiver.elements.length !== 1 ||
    receiver.elements[0]?.type !== "SpreadElement"
  )
    return null;
  const iterable = receiver.elements[0].argument;
  const collection = materializedCollection(unit, iterable, context);
  if (!collection && !oldApiBaseline(repoRoot, unit.file, context)) return null;
  addProof(context, node);
  return {
    predicate: "immutable-sort-baseline",
    reason: collection
      ? "The spread materializes a lexically proven native Set or Map iterator, which has no Array.toSorted method; copy-sort preserves the collection while producing an array."
      : "The copy-sort preserves its input, and the source-owned compiler configuration declares a standard API baseline before ES2023; the proposed Array.toSorted replacement requires a newer library contract.",
  };
}

function enclosingFunction(unit, node) {
  for (
    let parent = unit.parents.get(node);
    parent;
    parent = unit.parents.get(parent)
  ) {
    if (FUNCTIONS.has(parent.type)) return parent;
  }
  return null;
}
function nativeGlobal(unit, node, name) {
  node = unwrap(node);
  const binding = unit.binding(node);
  return (
    node?.type === "Identifier" &&
    node.name === name &&
    (!binding || binding.defs.length === 0)
  );
}
function nativeUndefined(unit, node) {
  return nativeGlobal(unit, node, "undefined");
}
function privateNullishSlot(unit, binding, context) {
  step(context);
  if (binding?.defs.length !== 1 || binding.defs[0].type !== "Variable")
    return null;
  const declaration = binding.defs[0].node;
  const statement = unit.parents.get(declaration);
  if (
    declaration.id.type !== "Identifier" ||
    statement?.type !== "VariableDeclaration" ||
    !["let", "var"].includes(statement.kind) ||
    unit.parents.get(statement)?.type !== "Program"
  )
    return null;
  const initial = unwrap(declaration.init);
  if (
    initial &&
    !nativeUndefined(unit, initial) &&
    !(initial.type === "Literal" && initial.value === null)
  )
    return null;
  for (const entry of unit.ast.body) {
    step(context);
    if (entry.type === "ExportNamedDeclaration" && !entry.source) {
      for (const specifier of entry.specifiers) {
        step(context);
        if (specifier.local?.name === declaration.id.name) return null;
      }
    }
    if (
      entry.type === "ExportDefaultDeclaration" &&
      unwrap(entry.declaration)?.type === "Identifier" &&
      unit.binding(unwrap(entry.declaration)) === binding
    )
      return null;
  }
  return { binding, declaration, statement };
}
function stableLanguageSignal(
  unit,
  expression,
  owner,
  context,
  seen = new Set(),
) {
  step(context);
  expression = unwrap(expression);
  if (!expression || seen.size > 8) return false;
  if (expression.type === "Literal")
    return (
      expression.regex === undefined &&
      (expression.value === null ||
        ["string", "number", "boolean"].includes(typeof expression.value))
    );
  if (nativeUndefined(unit, expression)) return true;
  if (expression.type === "Identifier") {
    const binding = unit.binding(expression);
    if (
      !immutable(binding) ||
      seen.has(binding) ||
      binding.defs[0].type !== "Variable"
    )
      return false;
    const declaration = binding.defs[0].node;
    if (
      declaration.id.type !== "Identifier" ||
      enclosingFunction(unit, declaration) !== owner
    )
      return false;
    if (
      !stableLanguageSignal(
        unit,
        declaration.init,
        owner,
        context,
        new Set([...seen, binding]),
      )
    )
      return false;
    addProof(context, unit.parents.get(declaration));
    return true;
  }
  const browserLanguage = (node) =>
    node?.type === "MemberExpression" &&
    !node.computed &&
    memberName(node) === "language" &&
    nativeGlobal(unit, node.object, "navigator");
  if (browserLanguage(expression)) return true;
  if (
    expression.type !== "ConditionalExpression" ||
    !nativeUndefined(unit, expression.consequent) ||
    !browserLanguage(unwrap(expression.alternate))
  )
    return false;
  const test = unwrap(expression.test);
  return (
    test?.type === "BinaryExpression" &&
    test.operator === "===" &&
    test.left.type === "UnaryExpression" &&
    test.left.operator === "typeof" &&
    nativeGlobal(unit, test.left.argument, "navigator") &&
    test.right.type === "Literal" &&
    test.right.value === "undefined"
  );
}
function sameSignal(unit, left, right) {
  left = unwrap(left);
  right = unwrap(right);
  if (!left || !right || left.type !== right.type) return false;
  return left.type === "Identifier"
    ? unit.binding(left) === unit.binding(right) && left.name === right.name
    : left.type === "Literal" && left.value === right.value;
}
function languageInvalidation(unit, reset, assignment, owner, context) {
  if (
    reset.operator !== "=" ||
    !nativeUndefined(unit, reset.right) ||
    enclosingFunction(unit, reset) !== owner
  )
    return null;
  const statement = unit.parents.get(reset);
  const block = unit.parents.get(statement);
  const guard = unit.parents.get(block);
  if (
    statement?.type !== "ExpressionStatement" ||
    block?.type !== "BlockStatement" ||
    guard?.type !== "IfStatement" ||
    guard.consequent !== block ||
    guard.alternate ||
    unit.parents.get(guard) !== owner.body ||
    guard.range[1] > assignment.range[0]
  )
    return null;
  const test = unwrap(guard.test);
  if (test?.type !== "BinaryExpression" || test.operator !== "!==") return null;
  for (const [markerExpression, signal] of [
    [test.left, test.right],
    [test.right, test.left],
  ]) {
    if (
      markerExpression.type !== "Identifier" ||
      !stableLanguageSignal(unit, signal, owner, context)
    )
      continue;
    const marker = privateNullishSlot(
      unit,
      unit.binding(markerExpression),
      context,
    );
    if (!marker) continue;
    let update;
    let valid = true;
    for (const child of block.body) {
      step(context);
      const write =
        child.type === "ExpressionStatement" && unwrap(child.expression);
      if (
        write?.type !== "AssignmentExpression" ||
        write.operator !== "=" ||
        write.left.type !== "Identifier"
      ) {
        valid = false;
        break;
      }
      if (unit.binding(write.left) === marker.binding) {
        if (update || !sameSignal(unit, write.right, signal)) {
          valid = false;
          break;
        }
        update = write;
      } else if (
        !nativeUndefined(unit, write.right) ||
        !privateNullishSlot(unit, unit.binding(write.left), context)
      ) {
        valid = false;
        break;
      }
    }
    if (
      !valid ||
      !update ||
      marker.binding.references.some((reference) => {
        step(context);
        return (
          reference.isWrite() &&
          !(reference.init && reference.identifier === marker.declaration.id) &&
          unit.parents.get(reference.identifier) !== update
        );
      })
    )
      continue;
    addProof(context, marker.statement);
    addProof(context, update);
    addProof(context, reset);
    addProof(context, guard);
    return guard;
  }
  return null;
}
function nativeIntlClosed(unit, context) {
  for (const node of unit.nodes) {
    step(context);
    if (
      node.type === "CallExpression" &&
      unwrap(node.callee)?.type === "Identifier" &&
      unwrap(node.callee).name === "eval"
    )
      return false;
    if (
      node.type !== "Identifier" ||
      node.name !== "Intl" ||
      !nativeGlobal(unit, node, "Intl") ||
      !unit.bindings.has(node)
    )
      continue;
    const parent = unit.parents.get(node);
    if (
      (parent?.type === "TSQualifiedName" && parent.left === node) ||
      (parent?.type === "TSTypeReference" && parent.typeName === node)
    )
      continue;
    const construction = unit.parents.get(parent);
    if (
      parent?.type !== "MemberExpression" ||
      parent.object !== node ||
      parent.computed ||
      !INTL_CLASSES.has(memberName(parent)) ||
      construction?.type !== "NewExpression" ||
      construction.callee !== parent
    )
      return false;
  }
  return true;
}
function lazyIntlCache(unit, node, diagnostic, context) {
  const callee = unwrap(node.callee);
  const className = memberName(callee);
  if (
    node.type !== "NewExpression" ||
    callee?.type !== "MemberExpression" ||
    callee.computed ||
    !nativeGlobal(unit, callee.object, "Intl") ||
    !INTL_CLASSES.has(className) ||
    diagnostic.message !== RULES["js-hoist-intl"].message(className)
  )
    return null;
  const assignment = unit.parents.get(node);
  const owner = enclosingFunction(unit, node);
  if (
    !owner ||
    assignment?.type !== "AssignmentExpression" ||
    assignment.operator !== "??=" ||
    assignment.right !== node ||
    assignment.left.type !== "Identifier" ||
    !nativeIntlClosed(unit, context)
  )
    return null;
  const slot = privateNullishSlot(unit, unit.binding(assignment.left), context);
  if (!slot) return null;
  let invalidation;
  for (const reference of slot.binding.references) {
    step(context);
    if (
      !reference.isWrite() ||
      (reference.init && reference.identifier === slot.declaration.id)
    )
      continue;
    const write = unit.parents.get(reference.identifier);
    if (write === assignment) continue;
    if (
      write?.type !== "AssignmentExpression" ||
      write.left !== reference.identifier
    )
      return null;
    const guard = languageInvalidation(unit, write, assignment, owner, context);
    if (!guard || (invalidation && invalidation !== guard)) return null;
    invalidation = guard;
  }
  addProof(context, slot.statement);
  addProof(context, node);
  addProof(context, assignment);
  addProof(context, owner);
  return {
    predicate: "module-owned-lazy-intl-cache",
    reason:
      "This native Intl constructor only initializes a private module-owned nullish cache. All binding writes and any observed-language invalidation are inspected, so unchanged observations reuse the formatter instead of rebuilding it on every call.",
  };
}

function createRenderSemanticCorrections(repoRoot) {
  const units = new Map();
  return (diagnostic, toolPackage) => {
    const rule = RULES[diagnostic.rule];
    if (
      toolPackage !== TOOL ||
      diagnostic.plugin !== "react-doctor" ||
      !rule ||
      (typeof rule.message === "function"
        ? ![...INTL_CLASSES].some(
            (name) => diagnostic.message === rule.message(name),
          )
        : diagnostic.message !== rule.message)
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
      const context = { steps: 0, spans: new Map(), configs: [] };
      const proof =
        diagnostic.rule === "rendering-hydration-mismatch-time"
          ? deferredCallback(unit, node, context)
          : diagnostic.rule === "prefer-tag-over-role"
            ? compositeGraphic(unit, node, context)
            : diagnostic.rule === "js-tosorted-immutable"
              ? immutableSort(unit, node, context, repoRoot)
              : diagnostic.rule === "js-hoist-intl"
                ? lazyIntlCache(unit, node, diagnostic, context)
                : propagationBoundary(unit, node, context);
      if (!proof) return null;
      const { message, ...provenance } = rule;
      return {
        ...proof,
        provenance: { ...PROVENANCE, ...provenance },
        evidence: [
          {
            file,
            sha256: createHash("sha256").update(unit.source).digest("hex"),
            spans: [...context.spans.values()],
          },
          ...context.configs.map(({ file: configFile, source }) => ({
            file: configFile,
            sha256: createHash("sha256").update(source).digest("hex"),
            spans: [{ start: 0, end: source.length }],
          })),
        ],
      };
    } catch {
      return null;
    }
  };
}

module.exports = { createRenderSemanticCorrections };
