const path = require("node:path");
const { createHash } = require("node:crypto");
const {
  parseUnit,
  unwrap,
  memberName,
  immutable,
} = require("./source-unit.js");
const { createRenderContracts } = require("./render-contracts.js");

const TOOL = "react-doctor@0.2.16";
const TAGS = {
  status: "output",
  table: "table",
  row: "tr",
  columnheader: "th",
  cell: "td",
};
const FUNCTIONS = new Set([
  "FunctionDeclaration",
  "FunctionExpression",
  "ArrowFunctionExpression",
]);
const FLOW_ONLY = new Set([
  "div",
  "p",
  "section",
  "article",
  "header",
  "footer",
  "main",
  "nav",
  "aside",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "ul",
  "ol",
  "dl",
  "pre",
  "blockquote",
  "figure",
  "figcaption",
  "fieldset",
  "table",
  "hr",
]);
const PROVENANCE = {
  toolPackage: TOOL,
  packageIntegrity:
    "sha512-o108QscqM/TlC1QmKgDORf/P53PeZ69nZbRsvy53rXkKvWdKZC1Y0u1JRPmThbTh6v+QpMispBwEnFHhlpZQ7w==",
  sourceCommit: "2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc",
  ruleSourceSha256:
    "a424acbda97ead457a61ba3b06ffade0b2af6c755b0ad4d147de08717507d4ca",
  references: [
    "https://html.spec.whatwg.org/multipage/form-elements.html#the-output-element",
    "https://html.spec.whatwg.org/multipage/tables.html",
    "https://www.w3.org/WAI/ARIA/apg/patterns/table/",
  ],
};
function attributes(opening) {
  if (
    opening.attributes.some(
      (attribute) =>
        attribute.type !== "JSXAttribute" ||
        attribute.name.type !== "JSXIdentifier",
    )
  )
    return null;
  const values = new Map();
  for (const attribute of opening.attributes) {
    if (values.has(attribute.name.name)) return null;
    values.set(attribute.name.name, attribute);
  }
  return values;
}
function attributeExpression(attribute) {
  return attribute?.value?.type === "JSXExpressionContainer"
    ? unwrap(attribute.value.expression)
    : attribute?.value;
}
function roleOf(element) {
  return element?.type === "JSXElement"
    ? attributeExpression(attributes(element.openingElement)?.get("role"))
        ?.value
    : null;
}
function nativeTag(element) {
  return element?.openingElement.name.type === "JSXIdentifier" &&
    /^[a-z]/.test(element.openingElement.name.name)
    ? element.openingElement.name.name
    : null;
}
function walk(unit, root, context, visit, skipFunctions = false) {
  const stack = [root];
  while (stack.length) {
    if (++context.steps > 40000)
      throw new Error("native content proof budget exceeded");
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
function sourceProof(context, unit, node) {
  context.evidence(unit.file, unit.source, node.range);
}
function forbiddenContent(unit, node, context, contracts) {
  if (++context.steps > 40000)
    throw new Error("native content proof budget exceeded");
  node = unwrap(node);
  if (!node) return null;
  if (node.type === "JSXElement") {
    const tag = nativeTag(node);
    if (FLOW_ONLY.has(tag)) return node;
    if (!tag) {
      const contract = contracts.inspected(unit, node);
      return contract && contract.hosts.every((host) => FLOW_ONLY.has(host))
        ? node
        : null;
    }
    if (tag === "svg" || tag === "math") return null;
    for (const child of node.children) {
      const result = forbiddenContent(unit, child, context, contracts);
      if (result) return result;
    }
  }
  if (node.type === "JSXFragment")
    for (const child of node.children) {
      const result = forbiddenContent(unit, child, context, contracts);
      if (result) return result;
    }
  if (node.type === "JSXExpressionContainer")
    return forbiddenContent(unit, node.expression, context, contracts);
  if (node.type === "ConditionalExpression") {
    if (node.test.type === "Literal")
      return forbiddenContent(
        unit,
        node.test.value ? node.consequent : node.alternate,
        context,
        contracts,
      );
    return (
      forbiddenContent(unit, node.consequent, context, contracts) ??
      forbiddenContent(unit, node.alternate, context, contracts)
    );
  }
  if (node.type === "LogicalExpression" && node.operator === "&&") {
    return node.left.type === "Literal" && !node.left.value
      ? null
      : forbiddenContent(unit, node.right, context, contracts);
  }
  return null;
}
function statusProof(unit, element, role, context, contracts) {
  const attrs = attributes(element.openingElement);
  if (!attrs || !["div", "span"].includes(nativeTag(element))) return null;
  for (const child of element.children) {
    const witness = forbiddenContent(unit, child, context, contracts);
    if (!witness) continue;
    sourceProof(context, unit, role);
    sourceProof(context, unit, witness);
    return {
      predicate: "native-status-content-model-conflict",
      reason:
        "The proposed output replacement permits only phrasing content, but the current status contains a source-proven non-phrasing host. Output does have implicit status semantics; the concrete child content prevents this replacement.",
    };
  }
  return null;
}
function nearestTable(unit, element) {
  for (let current = element; current; current = unit.parents.get(current))
    if (roleOf(current) === "table") return current;
  return null;
}
function directReturns(unit, fn, context) {
  if (fn.body.type !== "BlockStatement") return [unwrap(fn.body)];
  const values = [];
  walk(
    unit,
    fn.body,
    context,
    (node) => {
      if (node.type === "ReturnStatement" && node.argument)
        values.push(unwrap(node.argument));
    },
    true,
  );
  return values;
}
function literalArray(unit, expression, seen = new Set()) {
  expression = unwrap(expression);
  if (expression?.type === "ArrayExpression") return true;
  if (expression?.type !== "Identifier") return false;
  const variable = unit.binding(expression);
  if (
    !immutable(variable) ||
    seen.has(variable) ||
    variable.defs[0].type !== "Variable"
  )
    return false;
  seen.add(variable);
  return literalArray(unit, variable.defs[0].node.init, seen);
}
function renderedBranches(unit, expression, context, seen = new Set()) {
  expression = unwrap(expression);
  if (!expression) return [];
  if (expression.type === "JSXElement") return [expression];
  if (expression.type === "JSXFragment")
    return expression.children.flatMap((child) =>
      renderedBranches(unit, child, context, seen),
    );
  if (expression.type === "JSXExpressionContainer")
    return renderedBranches(unit, expression.expression, context, seen);
  if (expression.type === "ConditionalExpression")
    return expression.test.type === "Literal"
      ? renderedBranches(
          unit,
          expression.test.value ? expression.consequent : expression.alternate,
          context,
          seen,
        )
      : [
          ...renderedBranches(unit, expression.consequent, context, seen),
          ...renderedBranches(unit, expression.alternate, context, seen),
        ];
  if (expression.type === "LogicalExpression" && expression.operator === "&&")
    return expression.left.type === "Literal" && !expression.left.value
      ? []
      : renderedBranches(unit, expression.right, context, seen);
  if (
    expression.type === "CallExpression" &&
    memberName(expression.callee) === "map" &&
    expression.arguments.length === 1
  ) {
    const fn = unwrap(expression.arguments[0]);
    return FUNCTIONS.has(fn?.type)
      ? directReturns(unit, fn, context).flatMap((value) =>
          renderedBranches(unit, value, context, seen),
        )
      : [];
  }
  if (expression.type === "Identifier") {
    const variable = unit.binding(expression);
    if (
      !immutable(variable) ||
      seen.has(variable) ||
      variable.defs[0].type !== "Variable"
    )
      return [];
    return renderedBranches(
      unit,
      variable.defs[0].node.init,
      context,
      new Set([...seen, variable]),
    );
  }
  return [];
}
function validRow(unit, element, context) {
  if (
    roleOf(element) !== "row" ||
    !attributes(element.openingElement) ||
    !["div", "span"].includes(nativeTag(element))
  )
    return false;
  const cells = element.children.flatMap((child) =>
    renderedBranches(unit, child, context),
  );
  return (
    cells.length > 0 &&
    cells.every(
      (cell) =>
        ["cell", "columnheader", "rowheader"].includes(roleOf(cell)) &&
        ["div", "span"].includes(nativeTag(cell)) &&
        attributes(cell.openingElement),
    )
  );
}
function slotRows(unit, callback, context) {
  const returns = directReturns(unit, callback, context);
  if (!returns.length) return null;
  const rows = [];
  for (const value of returns) {
    const branches = renderedBranches(unit, value, context);
    if (!branches.length) return null;
    for (const element of branches) {
      if (validRow(unit, element, context)) rows.push(element);
      else if (
        nativeTag(element) === "div" &&
        !roleOf(element) &&
        element.children.every(
          (child) => child.type === "JSXText" && !child.value.trim(),
        )
      )
        continue;
      else return null;
    }
  }
  return rows.length ? rows : null;
}
function tableProof(unit, element, role, context, contracts) {
  const owner = nearestTable(unit, element);
  const attrs = owner && attributes(owner.openingElement);
  const name = attributeExpression(attrs?.get("aria-label"));
  if (
    !owner ||
    !attrs ||
    !["div", "span"].includes(nativeTag(owner)) ||
    !name ||
    (name.type === "Literal" &&
      (typeof name.value !== "string" || !name.value.trim()))
  )
    return null;
  const virtualizers = [];
  walk(unit, owner, context, (node) => {
    if (node !== owner && roleOf(node) === "table") return false;
    if (node.type !== "JSXElement" || nativeTag(node)) return;
    const props = attributes(node.openingElement);
    if (
      !props ||
      [...props.keys()].some((key) => ["as", "component"].includes(key))
    )
      return;
    const virtual = props.get("virtual");
    if (
      !virtual ||
      (virtual.value && attributeExpression(virtual)?.value !== true)
    )
      return;
    const slot = contracts.virtualSlot(unit, node);
    if (slot) virtualizers.push({ element: node, slot });
  });
  if (!virtualizers.length) return null;
  const validSlots = [];
  for (const candidate of virtualizers) {
    const rows = slotRows(unit, candidate.slot.callback, context);
    if (!rows) return null;
    validSlots.push({ ...candidate, rows });
  }
  // A row inside a renderer must actually belong to the inspected slot.
  let callbackOwner = null;
  for (
    let parent = element === owner ? null : unit.parents.get(element);
    parent && parent !== owner;
    parent = unit.parents.get(parent)
  ) {
    if (FUNCTIONS.has(parent.type)) {
      callbackOwner = parent;
      break;
    }
  }
  if (
    callbackOwner &&
    !validSlots.some(
      (candidate) =>
        candidate.slot.callback === callbackOwner ||
        candidate.rows.some(
          (row) =>
            element.range[0] >= row.range[0] &&
            element.range[1] <= row.range[1],
        ),
    )
  ) {
    const iterator = unit.parents.get(callbackOwner);
    if (
      iterator?.type !== "CallExpression" ||
      memberName(iterator.callee) !== "map" ||
      iterator.arguments[0] !== callbackOwner ||
      !literalArray(unit, iterator.callee.object)
    )
      return null;
  }
  const roleName = roleOf(element);
  if (roleName === "row" && !validRow(unit, element, context)) return null;
  if (["cell", "columnheader"].includes(roleName)) {
    let row = unit.parents.get(element);
    while (row && row !== owner && roleOf(row) !== "row")
      row = unit.parents.get(row);
    if (row === owner || !row || !validRow(unit, row, context)) return null;
  }
  sourceProof(context, unit, role);
  sourceProof(context, unit, attrs.get("role"));
  sourceProof(context, unit, attrs.get("aria-label"));
  for (const candidate of validSlots) {
    sourceProof(context, unit, candidate.element);
    sourceProof(context, unit, candidate.slot.attribute);
    for (const row of candidate.rows) {
      sourceProof(context, unit, row);
      walk(unit, row, context, (node) => {
        if (node.type === "JSXAttribute" && node.name.name === "role")
          sourceProof(context, unit, node);
      });
    }
  }
  return {
    predicate: "virtual-table-content-model-conflict",
    reason:
      "This ARIA table has source-proven row/cell descendant ownership through a pinned virtual renderer with fixed neutral div hosts. Even coordinated replacement of its role-bearing nodes leaves div window/item/filler hosts inside native table structure, violating the native table/row/cell content context.",
  };
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
      (node) =>
        node.type === "JSXAttribute" &&
        node.range[0] === offset &&
        node.name.name === "role",
    ) ?? null
  );
}
function createNativeContentSemanticCorrections(repoRoot) {
  const units = new Map();
  return (diagnostic, toolPackage) => {
    if (
      toolPackage !== TOOL ||
      diagnostic.plugin !== "react-doctor" ||
      diagnostic.rule !== "prefer-tag-over-role"
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
      const role = unit && atDiagnostic(unit, diagnostic);
      const roleName = attributeExpression(role)?.value;
      if (
        !role ||
        !TAGS[roleName] ||
        diagnostic.message !==
          `Screen reader users get more reliable semantics from \`<${TAGS[roleName]}>\` than \`role="${roleName}"\`, so use \`<${TAGS[roleName]}>\` instead.`
      )
        return null;
      const opening = unit.parents.get(role),
        element = unit.parents.get(opening);
      if (
        opening?.type !== "JSXOpeningElement" ||
        element?.type !== "JSXElement" ||
        !attributes(opening)
      )
        return null;
      const evidence = new Map();
      const context = {
        steps: 0,
        evidence: (sourceFile, source, range) => {
          const entry = evidence.get(sourceFile) ?? {
            source,
            spans: new Map(),
          };
          entry.spans.set(range.join(":"), { start: range[0], end: range[1] });
          evidence.set(sourceFile, entry);
        },
      };
      const contracts = createRenderContracts(repoRoot, context);
      const proof =
        roleName === "status"
          ? statusProof(unit, element, role, context, contracts)
          : tableProof(unit, element, role, context, contracts);
      if (!proof) return null;
      return {
        ...proof,
        provenance: PROVENANCE,
        evidence: [...evidence].map(([sourceFile, entry]) => ({
          file: sourceFile,
          sha256: createHash("sha256").update(entry.source).digest("hex"),
          spans: [...entry.spans.values()],
        })),
      };
    } catch {
      return null;
    }
  };
}

module.exports = { createNativeContentSemanticCorrections };
