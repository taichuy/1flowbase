const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const { getRepoRoot } = require("../testing/warning-capture.js");

const SOURCE_EXTENSIONS = new Set([".js", ".jsx", ".ts", ".tsx"]);

const ANT_ICON_BARREL_IMPORT =
  /\bimport\s+(?:\{[^;]*\}|\*\s+as\s+\w+)\s+from\s+['"]@ant-design\/icons['"]/gu;

const IMPORT_SPECIFIER =
  /\b(?:import|export)\s+(?:[\s\S]*?\s+from\s+)?['"]([^'"]+)['"]/gu;

const DYNAMIC_IMPORT = /\bimport\s*\(\s*['"]([^'"]+)['"]\s*\)/gu;

function normalizePath(value) {
  return value.replace(/\\/gu, "/");
}

function listSourceFiles(root) {
  const result = [];
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    const absolutePath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      if (!["_tests", "test", "node_modules"].includes(entry.name)) {
        result.push(...listSourceFiles(absolutePath));
      }
    } else if (
      SOURCE_EXTENSIONS.has(path.extname(entry.name)) &&
      !entry.name.endsWith(".d.ts")
    ) {
      result.push(absolutePath);
    }
  }
  return result;
}

function resolveRelativeImport(importer, specifier) {
  if (!specifier.startsWith("./cli")) {
    return null;
  }
  const base = path.resolve(path.dirname(importer), specifier);
  const candidates = path.extname(base)
    ? [base]
    : [...SOURCE_EXTENSIONS].flatMap((extension) => [
        `${base}${extension}`,
        path.join(base, `index${extension}`),
      ]);
  return candidates.find((candidate) => fs.existsSync(candidate)) || null;
}

function collectSpecifiers(source) {
  const values = [];
  for (const pattern of [IMPORT_SPECIFIER, DYNAMIC_IMPORT]) {
    pattern.lastIndex = 0;
    let match = pattern.exec(source);
    while (match) {
      values.push(match[1]);
      match = pattern.exec(source);
    }
  }
  return [...new Set(values)];
}

function buildSourceGraph(sourceRoot) {
  const files = listSourceFiles(sourceRoot);
  const fileSet = new Set(files);
  const graph = new Map(files.map((filePath) => [filePath, []]));
  for (const filePath of files) {
    const source = fs.readFileSync(filePath, "utf8");
    for (const specifier of collectSpecifiers(source)) {
      const resolved = resolveRelativeImport(filePath, specifier);
      if (resolved && fileSet.has(resolved)) {
        graph.get(filePath).push(resolved);
      }
    }
  }
  return graph;
}

function reachableModules(graph, entryFile) {
  const visited = new Set();
  const pending = [entryFile];
  while (pending.length > 0) {
    const current = pending.pop();
    if (!current || visited.has(current) || !graph.has(current)) continue;
    visited.add(current);
    pending.push(...graph.get(current));
  }
  return [...visited].sort();
}

function routeGraphProfiles(graph, repoRoot) {
  const entries = {
    router: "web/app/src/app/router.tsx",
    frontstage: "web/app/src/features/frontstage/pages/FrontStagePage.tsx",
    settings: "web/app/src/features/settings/pages/SettingsPage.tsx",
    workflow: "web/app/src/features/workflow/pages/WorkflowEditorPage.tsx",
  };
  return Object.fromEntries(
    Object.entries(entries).map(([route, relativePath]) => {
      const modules = reachableModules(
        graph,
        path.join(repoRoot, relativePath),
      );
      return [
        route,
        {
          entry: relativePath,
          reachableModuleCount: modules.length,
          reachableModules: modules.map((filePath) =>
            normalizePath(path.relative(repoRoot, filePath)),
          ),
        },
      ];
    }),
  );
}

function highFanoutOwners(graph, repoRoot, limit = 20) {
  return [...graph.entries()]
    .map(([filePath, dependencies]) => ({
      file: normalizePath(path.relative(repoRoot, filePath)),
      directDependencies: dependencies.length,
    }))
    .sort(
      (left, right) =>
        right.directDependencies - left.directDependencies ||
        left.file.localeCompare(right.file),
    )
    .slice(0, limit);
}

function stronglyConnectedComponents(graph) {
  let nextIndex = 0;
  const indices = new Map();
  const lowLinks = new Map();
  const stack = [];
  const onStack = new Set();
  const components = [];

  function visit(node) {
    indices.set(node, nextIndex);
    lowLinks.set(node, nextIndex);
    nextIndex += 1;
    stack.push(node);
    onStack.add(node);
    for (const neighbor of graph.get(node) || []) {
      if (!indices.has(neighbor)) {
        visit(neighbor);
        lowLinks.set(
          node,
          Math.min(lowLinks.get(node), lowLinks.get(neighbor)),
        );
      } else if (onStack.has(neighbor)) {
        lowLinks.set(node, Math.min(lowLinks.get(node), indices.get(neighbor)));
      }
    }
    if (lowLinks.get(node) === indices.get(node)) {
      const component = [];
      let member;
      do {
        member = stack.pop();
        onStack.delete(member);
        component.push(member);
      } while (member !== node);
      components.push(component.sort());
    }
  }

  for (const node of graph.keys()) {
    if (!indices.has(node)) {
      visit(node);
    }
  }
  return components;
}

function buildCondensation(graph, components) {
  const componentByNode = new Map();
  components.forEach((component, index) => {
    component.forEach((node) => componentByNode.set(node, index));
  });
  const edges = components.map(() => new Set());
  for (const [node, neighbors] of graph.entries()) {
    const from = componentByNode.get(node);
    for (const neighbor of neighbors) {
      const to = componentByNode.get(neighbor);
      if (from !== to) {
        edges[from].add(to);
      }
    }
  }
  return { componentByNode, edges };
}

function reachableComponentSets(edges) {
  const cache = new Map();
  function visit(index) {
    if (cache.has(index)) {
      return cache.get(index);
    }
    const reachable = new Set([index]);
    cache.set(index, reachable);
    for (const neighbor of edges[index]) {
      for (const nested of visit(neighbor)) {
        reachable.add(nested);
      }
    }
    return reachable;
  }
  return edges.map((_edge, index) => visit(index));
}

function cacheIdentity({
  files,
  runtime = process.version,
  platform = process.platform,
  arch = process.arch,
  mode = "development",
  env = {},
}) {
  const hash = crypto.createHash("sha256");
  for (const filePath of [...files].sort()) {
    hash.update(normalizePath(filePath));
    hash.update("\0");
    hash.update(
      fs.existsSync(filePath) ? fs.readFileSync(filePath) : "<missing>",
    );
    hash.update("\0");
  }
  hash.update(JSON.stringify({ runtime, platform, arch, mode, env }));
  return hash.digest("hex");
}

function parseOptimizeDepsInclude(source) {
  const marker = /optimizeDeps\s*:\s*\{/u.exec(source);
  if (!marker) return [];
  const openBrace = source.indexOf("{", marker.index);
  let depth = 0;
  let closeBrace = -1;
  for (let index = openBrace; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") depth -= 1;
    if (depth === 0) {
      closeBrace = index;
      break;
    }
  }
  const optimizeDeps =
    closeBrace > openBrace ? source.slice(openBrace + 1, closeBrace) : "";
  const include = /include\s*:\s*\[([\s\S]*?)\]/u.exec(optimizeDeps)?.[1] || "";
  return [...include.matchAll(/['"]([^'"]+)['"]/gu)].map((match) => match[1]);
}

function analyzeStatic({ repoRoot = getRepoRoot() } = {}) {
  const sourceRoot = path.join(repoRoot, "web", "app", "src");
  const graph = buildSourceGraph(sourceRoot);
  const components = stronglyConnectedComponents(graph);
  const condensation = buildCondensation(graph, components);
  const reachable = reachableComponentSets(condensation.edges);
  const findings = [];
  for (const filePath of graph.keys()) {
    const source = fs.readFileSync(filePath, "utf8");
    ANT_ICON_BARREL_IMPORT.lastIndex = 0;
    let match = ANT_ICON_BARREL_IMPORT.exec(source);
    while (match) {
      findings.push({
        code: "high-fanout-icon-barrel",
        file: normalizePath(path.relative(repoRoot, filePath)),
        dependency: "@ant-design/icons",
      });
      match = ANT_ICON_BARREL_IMPORT.exec(source);
    }
  }
  const viteSource = fs.readFileSync(
    path.join(repoRoot, "web", "app", "vite.config.ts"),
    "utf8",
  );
  const optimizedDependencies = new Set(parseOptimizeDepsInclude(viteSource));
  for (const dependency of ["antd", "@ant-design/icons"]) {
    if (!optimizedDependencies.has(dependency)) {
      findings.push({ code: "missing-optimize-dep", dependency });
    }
  }
  return {
    ok: findings.length === 0,
    findings,
    graph: {
      vertices: graph.size,
      edges: [...graph.values()].reduce((sum, edges) => sum + edges.length, 0),
      stronglyConnectedComponents: components.length,
      maxReachableComponents: Math.max(0, ...reachable.map((set) => set.size)),
      routes: routeGraphProfiles(graph, repoRoot),
      highFanoutOwners: highFanoutOwners(graph, repoRoot),
    },
  };
}

module.exports = { buildSourceGraph, stronglyConnectedComponents, buildCondensation, reachableComponentSets, cacheIdentity, parseOptimizeDepsInclude, analyzeStatic };
