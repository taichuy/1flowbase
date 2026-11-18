const path = require("node:path");
const { createHash } = require("node:crypto");
const { parseUnit, unwrap, immutable } = require("./source-unit.js");

const TOOL = "react-doctor@0.2.16";
const RULE = "design-no-em-dash-in-jsx-text";
const MESSAGE = "Em dash (—) in UI text reads like AI output to your users.";
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
    "1e5908c52d9fc72ed12363e90fc0470b40267b9432009d0b322fb5b175ecda86",
  references: [
    "https://github.com/millionco/react-doctor/blob/2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc/packages/oxlint-plugin-react-doctor/src/plugin/rules/react-ui/no-em-dash-in-jsx-text.ts",
  ],
};

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
  const matches = unit.nodes.filter(
    (node) => node.type === "JSXText" && node.range[0] === offset,
  );
  return matches.length === 1 ? matches[0] : null;
}

function native(element) {
  return (
    element?.type === "JSXElement" &&
    element.openingElement.name.type === "JSXIdentifier" &&
    /^[a-z][a-z0-9]*$/.test(element.openingElement.name.name)
  );
}

function wholeMarker(unit, text) {
  if (text.value !== "—") return null;
  const marker = unit.parents.get(text);
  if (
    !["JSXFragment", "JSXElement"].includes(marker?.type) ||
    marker.children.length !== 1 ||
    marker.children[0] !== text
  )
    return null;
  if (marker.type === "JSXElement") {
    // Keep the admitted host grammar small; opaque components, polymorphic
    // props and children/innerHTML overrides cannot prove literal UI content.
    const opening = marker.openingElement;
    if (
      !native(marker) ||
      opening.name.name !== "span" ||
      opening.attributes.some(
        (attribute) =>
          attribute.type !== "JSXAttribute" ||
          attribute.name.type !== "JSXIdentifier" ||
          ["children", "dangerouslySetInnerHTML"].includes(
            attribute.name.name,
          ) ||
          (attribute.value && attribute.value.type !== "Literal"),
      )
    )
      return null;
  }
  return marker;
}

function guardedValue(unit, test) {
  test = unwrap(test);
  if (test?.type !== "LogicalExpression" || test.operator !== "||") return null;
  const nullCheck = unwrap(test.left),
    emptyCheck = unwrap(test.right);
  if (
    nullCheck?.type !== "BinaryExpression" ||
    nullCheck.operator !== "==" ||
    nullCheck.left.type !== "Identifier" ||
    nullCheck.right.type !== "Literal" ||
    nullCheck.right.value !== null ||
    emptyCheck?.type !== "BinaryExpression" ||
    emptyCheck.operator !== "===" ||
    emptyCheck.left.type !== "Identifier" ||
    emptyCheck.right.type !== "Literal" ||
    emptyCheck.right.value !== ""
  )
    return null;
  const binding = unit.binding(nullCheck.left);
  return immutable(binding) && unit.binding(emptyCheck.left) === binding
    ? binding
    : null;
}

function presentValue(unit, root, binding) {
  if (!native(root)) return null;
  const stack = [root];
  let steps = 0;
  while (stack.length) {
    if (++steps > 20000)
      throw new Error("missing marker proof budget exceeded");
    const node = stack.pop();
    if (!["JSXElement", "JSXFragment"].includes(node.type)) continue;
    for (const child of node.children) {
      if (native(node) && child.type === "JSXExpressionContainer") {
        const value = unwrap(child.expression);
        if (value?.type === "Identifier" && unit.binding(value) === binding)
          return node;
      }
      // Only unconditional JSX children are evidence. Attribute expressions,
      // callbacks and unknown conditional branches do not establish display.
      if (["JSXElement", "JSXFragment"].includes(child.type)) stack.push(child);
    }
  }
  return null;
}

function markerProof(unit, text) {
  const marker = wholeMarker(unit, text);
  if (!marker) return null;
  const returned = unit.parents.get(marker);
  const guard = returned && unit.parents.get(returned);
  const body = guard && unit.parents.get(guard);
  const owner = body && unit.parents.get(body);
  if (
    returned?.type !== "ReturnStatement" ||
    returned.argument !== marker ||
    guard?.type !== "IfStatement" ||
    guard.consequent !== returned ||
    guard.alternate ||
    body?.type !== "BlockStatement" ||
    !FUNCTIONS.has(owner?.type) ||
    owner.body !== body
  )
    return null;
  const binding = guardedValue(unit, guard.test);
  if (!binding) return null;
  const index = body.body.indexOf(guard);
  const present = body.body[index + 1];
  if (
    index < 0 ||
    body.body.length !== index + 2 ||
    present?.type !== "ReturnStatement"
  )
    return null;
  const displayed = presentValue(unit, unwrap(present.argument), binding);
  if (!displayed) return null;
  return {
    predicate: "missing-value-marker",
    reason:
      "This single literal em dash is the complete return for a null, undefined or empty value, while the present branch renders the same immutable value. It marks missing data rather than joining prose; replacing it with sentence punctuation would change the UI value.",
    nodes: [owner, binding.identifiers[0], guard.test, marker, text, displayed],
  };
}

function createMarkerSemanticCorrections(repoRoot) {
  return (diagnostic, toolPackage) => {
    if (
      toolPackage !== TOOL ||
      diagnostic.plugin !== "react-doctor" ||
      diagnostic.rule !== RULE ||
      diagnostic.message !== MESSAGE ||
      typeof diagnostic.filePath !== "string"
    )
      return null;
    const file = path.posix.normalize(`web/app/${diagnostic.filePath}`);
    if (
      !file.startsWith("web/app/src/") ||
      diagnostic.filePath.startsWith("/") ||
      diagnostic.filePath.includes("\\")
    )
      return null;
    try {
      // Re-read eligible sources so a reused inspector cannot retain stale proof.
      const unit = parseUnit(repoRoot, file);
      const text = unit && atDiagnostic(unit, diagnostic);
      const proof = text && markerProof(unit, text);
      if (!proof) return null;
      const spans = new Map();
      for (const node of proof.nodes) {
        if (!node?.range) return null;
        spans.set(node.range.join(":"), {
          start: node.range[0],
          end: node.range[1],
        });
      }
      return {
        predicate: proof.predicate,
        reason: proof.reason,
        provenance: PROVENANCE,
        evidence: [
          {
            file,
            sha256: createHash("sha256").update(unit.source).digest("hex"),
            spans: [...spans.values()],
          },
        ],
      };
    } catch {
      return null;
    }
  };
}

module.exports = { createMarkerSemanticCorrections };
