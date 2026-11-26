const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { createRequire } = require("node:module");

const viteConfigPath = path.resolve(
  __dirname,
  "..",
  "..",
  "..",
  "..",
  "web",
  "app",
  "vite.config.ts",
);
const webSourceRoot = path.resolve(path.dirname(viteConfigPath), "src");

function sourceFiles(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      return entry.name === "_tests" ? [] : sourceFiles(entryPath);
    }
    return entry.isFile() && /\.[cm]?[jt]sx?$/u.test(entry.name)
      ? [entryPath]
      : [];
  });
}

// Pin the pre-React runtime graph, including the narrow API client export.
// Type-only references are erased; the two deferred entry points are not walked.
const webRoot = path.resolve(webSourceRoot, "../..");
const bootstrapFile = "app/src/bootstrap.ts";
const recoveryFile = "app/src/app/bootstrap/dev-module-recovery.ts";
const auth = "app/src/features/auth";
const api = "packages/api-client/src";
const authSubpath = "@1flowbase/api-client/auth";
const preReactImports = new Map([
  [bootstrapFile, [
    "./features/auth/components/hero/document-motion",
    "./features/auth/api/login-entry-discovery",
    "./features/auth/api/auth-session-discovery",
    "./app/bootstrap/dev-module-recovery",
  ]],
  [recoveryFile, []],
  [`${auth}/components/hero/document-motion.ts`, []],
  [`${auth}/api/login-entry-discovery.ts`, ["./session", "./document-start-discovery"]],
  [`${auth}/api/auth-session-discovery.ts`, ["./session", "./document-start-discovery", "./early-console-startup-flight"]],
  [`${auth}/api/document-start-discovery.ts`, ["./session"]],
  [`${auth}/api/early-console-startup-flight.ts`, ["./session"]],
  [`${auth}/api/session.ts`, [authSubpath]],
  [`${api}/auth/index.ts`, [
    "../public/system-metadata", "../errors", "../transport/json",
    "../console/session", "../console/identity/me", "../public-auth",
  ]],
  [`${api}/public/system-metadata.ts`, ["../transport/json"]],
  [`${api}/errors.ts`, []],
  [`${api}/transport/json.ts`, ["../errors"]],
  [`${api}/console/session.ts`, ["../transport/json"]],
  [`${api}/console/identity/me.ts`, ["../../transport/json"]],
  [`${api}/public-auth.ts`, ["./transport/json"]],
]);

const preReactDeferred = new Map([
  [bootstrapFile, ["./app/AnonymousAppRuntime", "./main"]],
  [`${auth}/api/auth-session-discovery.ts`, ["../../../app/bootstrap/AuthenticatedStartup", "../../../app/bootstrap/frontstage-startup-prefetch"]],
]);

function readWebSource(file) {
  return fs.readFileSync(path.resolve(webRoot, file), "utf8");
}

function assertPreReactModuleBoundary(readSource = readWebSource) {
  const webRequire = createRequire(path.join(webRoot, "package.json"));
  const { parseSync } = webRequire("vite");
  const authEntry = JSON.parse(readSource("packages/api-client/package.json"))
    .exports["./auth"].default;
  assert.equal(authEntry, "./src/auth/index.ts");
  // Vite resolves this before consulting package.json exports.
  const aliases = [...readSource("app/vite.config.ts").matchAll(
    /['"]@1flowbase\/api-client\/auth['"]\s*:\s*fileURLToPath\(\s*new URL\(\s*['"]([^'"]+)['"]/gu,
  )].map((match) => path.posix.normalize(path.posix.join("app", match[1])));
  assert.deepEqual(aliases, [path.posix.join("packages/api-client", authEntry)], "Vite auth alias");

  for (const [file, expected] of preReactImports) {
    const parsed = parseSync(file, readSource(file));
    assert.equal(parsed.errors.length, 0, `${file}: syntax error`);
    const eager = [];
    const deferred = [];
    function visit(node) {
      if (
        (node.type === "ImportDeclaration" && node.importKind !== "type") ||
        ((node.type === "ExportNamedDeclaration" ||
          node.type === "ExportAllDeclaration") &&
          node.source &&
          node.exportKind !== "type")
      ) {
        eager.push(node.source.value);
      }
      if (node.type === "TSExternalModuleReference") {
        assert.fail(`${file}: unexpected import-equals dependency`);
      }
      if (node.type === "ImportExpression") {
        const target = node.source;
        assert.ok(
          target?.type === "Literal" && typeof target.value === "string",
          `${file}: nonliteral import()`,
        );
        deferred.push(target.value);
      }
      if (node.type === "CallExpression") {
        assert.ok(
          node.callee.type !== "Identifier" || node.callee.name !== "require",
          `${file}: unexpected require() dependency`,
        );
      }
      for (const value of Object.values(node)) {
        if (Array.isArray(value)) {
          for (const child of value) {
            if (
              child &&
              typeof child === "object" &&
              typeof child.type === "string"
            ) {
              visit(child);
            }
          }
        } else if (
          value &&
          typeof value === "object" &&
          typeof value.type === "string"
        ) {
          visit(value);
        }
      }
    }
    visit(parsed.program);
    assert.deepEqual(eager.sort(), [...expected].sort(), `${file}: eager dependencies`);
    assert.deepEqual(
      [...new Set(deferred)].sort(),
      preReactDeferred.get(file) ?? [],
      `${file}: deferred dependencies`,
    );
    for (const specifier of eager) {
      const target = specifier === authSubpath
        ? path.posix.join("packages/api-client", authEntry)
        : path.posix.join(path.posix.dirname(file), `${specifier}.ts`);
      assert.ok(preReactImports.has(target), `${file}: unverified dependency ${target}`);
    }
  }
}

test("vite config uses the repo default frontend port", () => {
  const viteConfigSource = fs.readFileSync(viteConfigPath, "utf8");

  assert.match(viteConfigSource, /server:\s*\{/u);
  assert.match(viteConfigSource, /host:\s*'0\.0\.0\.0'/u);
  assert.match(viteConfigSource, /VITE_DEV_SERVER_PORT/u);
  assert.match(viteConfigSource, /VITE_DEV_ALLOWED_HOSTS/u);
  assert.match(viteConfigSource, /Number\.parseInt/u);
  assert.match(viteConfigSource, /3100/u);
  assert.match(viteConfigSource, /strictPort:\s*true/u);
});

test("vite config keeps the workspace root while extending fs allow list for shared scripts", () => {
  const viteConfigSource = fs.readFileSync(viteConfigPath, "utf8");

  assert.match(viteConfigSource, /searchForWorkspaceRoot\(process\.cwd\(\)\)/u);
  assert.match(
    viteConfigSource,
    /new URL\('\.\.\/\.\.\/scripts', import\.meta\.url\)/u,
  );
});

test("DV-F04 vite config exposes lifecycle readiness after bounded warmup", () => {
  const viteConfigSource = fs.readFileSync(viteConfigPath, "utf8");
  const runtimeSource = fs.readFileSync(
    path.resolve(path.dirname(viteConfigPath), "vite", "dev-runtime.ts"),
    "utf8",
  );

  assert.match(viteConfigSource, /oneFlowbaseDevRuntimePlugin/u);
  assert.match(viteConfigSource, /warmup:\s*\{/u);
  assert.match(runtimeSource, /\/__1flowbase_dev_ready/u);
  assert.match(runtimeSource, /'Building'/u);
  assert.match(runtimeSource, /'Validating'/u);
  assert.match(runtimeSource, /'Ready'/u);
  assert.match(runtimeSource, /'Degraded'/u);
  assert.match(runtimeSource, /attachPreReadyTrafficGate/u);
  assert.match(runtimeSource, /response\.statusCode = 503/u);
  assert.match(runtimeSource, /fs\.writeFileSync/u);
  assert.match(runtimeSource, /RECOVERY_PROBE_PATH/u);
  assert.match(
    runtimeSource,
    /hmr-probe-\$\{process\.pid\}-\$\{crypto\.randomUUID\(\)\}/u,
  );
  assert.doesNotMatch(
    runtimeSource,
    /allowedHosts|cors|cloudflare|access token/iu,
  );
});

test("DRS-001 critical CommonJS dependencies share the optimized ESM boundary", () => {
  const viteConfigSource = fs.readFileSync(viteConfigPath, "utf8");
  const runtimeSource = fs.readFileSync(
    path.resolve(path.dirname(viteConfigPath), "vite", "dev-runtime.ts"),
    "utf8",
  );

  assert.match(runtimeSource, /'react-is'/u);
  assert.match(runtimeSource, /'is-mobile'/u);
  assert.match(viteConfigSource, /\.\.\.DEV_CRITICAL_INTEROP_SPECIFIERS/u);
  assert.match(
    viteConfigSource,
    /needsInterop:\s*\[\.\.\.DEV_CRITICAL_INTEROP_SPECIFIERS\]/u,
  );
  assert.match(viteConfigSource, /devGenerationCacheDirectory/u);
  assert.match(viteConfigSource, /cacheDir:/u);

  const appRequire = createRequire(
    path.resolve(path.dirname(viteConfigPath), "package.json"),
  );
  const appPackage = appRequire("./package.json");
  for (const dependency of ["is-mobile", "react-is"]) {
    assert.ok(appPackage.dependencies[dependency]);
  }

  const nativeAntSource = fs.readFileSync(
    path.resolve(
      path.dirname(viteConfigPath),
      "build",
      "native-antd-es-modules.ts",
    ),
    "utf8",
  );
  assert.match(viteConfigSource, /nativeAntDesignEsModulesPlugin\(command\)/u);
  assert.match(nativeAntSource, /if \(command === 'serve'\) return undefined/u);
});

test("DRS-002 development runtime publishes a generation and warms the boot boundary", () => {
  const runtimeSource = fs.readFileSync(
    path.resolve(path.dirname(viteConfigPath), "vite", "dev-runtime.ts"),
    "utf8",
  );

  assert.match(runtimeSource, /DEV_GENERATION_META_NAME/u);
  assert.match(runtimeSource, /transformIndexHtml/u);
  assert.match(runtimeSource, /if \(command !== 'serve'\) return \[\]/u);
  assert.match(runtimeSource, /\/src\/bootstrap\.ts/u);
  assert.match(runtimeSource, /generation:/u);
  assert.match(runtimeSource, /verifyCriticalInteropCache/u);
  assert.match(runtimeSource, /waitForCriticalInteropCache/u);
  assert.match(runtimeSource, /pruneDevGenerationCaches/u);
  const cacheSource = fs.readFileSync(
    path.resolve(
      path.dirname(viteConfigPath),
      "vite",
      "dev-generation-cache.ts",
    ),
    "utf8",
  );
  assert.match(cacheSource, /fs\.promises\.rm/u);
});

test("DRS-003 pre-React bootstrap never leaves an empty root after module failure", () => {
  const indexSource = fs.readFileSync(
    path.resolve(path.dirname(viteConfigPath), "index.html"),
    "utf8",
  );
  const bootstrapSource = fs.readFileSync(
    path.resolve(webSourceRoot, "bootstrap.ts"),
    "utf8",
  );
  const appSource = fs.readFileSync(
    path.resolve(webSourceRoot, "app", "App.tsx"),
    "utf8",
  );
  const runtimeBootstrapSource = fs.readFileSync(
    path.resolve(webSourceRoot, "app", "ApplicationRuntimeBootstrap.tsx"),
    "utf8",
  );

  assert.match(indexSource, /data-testid="application-bootstrap-shell"/u);
  assert.match(indexSource, /class="application-bootstrap-shell__spinner"/u);
  assert.match(indexSource, />thinking<\/span>/u);
  assertPreReactModuleBoundary();
  assert.match(bootstrapSource, /if \(recoverDevModuleGraph\(error\)\) return/u);
  assert.match(bootstrapSource, /resetDevModuleRecovery\(\)/u);
  assert.match(bootstrapSource, /root\.replaceChildren\(alert\)/u);
  assert.doesNotMatch(bootstrapSource, /renderBootStage/u);
  assert.match(bootstrapSource, /renderBootFailure/u);
  assert.match(bootstrapSource, /const sessionFlight = startAuthSessionDiscovery\(\)/u);
  assert.match(bootstrapSource, /import\(['"]\.\/main['"]\)\.catch/u);
  assert.match(appSource, /import \{ AnonymousLoadingState \}/u);
  assert.match(appSource, /fallback=\{<AnonymousLoadingState fullscreen \/>\}/u);
  assert.match(runtimeBootstrapSource, /import \{ AnonymousLoadingState \}/u);
  assert.match(
    runtimeBootstrapSource,
    /fallback=\{<AnonymousLoadingState fullscreen \/>\}/u,
  );
  assert.match(appSource, /ApplicationBootBoundary/u);
  assert.doesNotMatch(appSource, /ApplicationBootStage/u);
  assert.doesNotMatch(runtimeBootstrapSource, /ApplicationBootStage/u);
  assert.doesNotMatch(appSource, /Suspense fallback=\{null\}/u);
  assert.doesNotMatch(
    `${indexSource}\n${bootstrapSource}\n${appSource}\n${runtimeBootstrapSource}`,
    /应用正在启动/u,
  );
});

test("DRS-003 rejects new dependencies anywhere in the pre-React graph", () => {
  for (const [file, dependency] of [
    [bootstrapFile, "import React from 'react';"],
    [bootstrapFile, "void import('./unexpected-runtime');"],
    [recoveryFile, "import './heavy-local-module';"],
    [recoveryFile, "export { App } from './App';"],
    [recoveryFile, "void import('./App');"],
    [recoveryFile, "const React = require('react');"],
    [`${auth}/api/document-start-discovery.ts`, "import 'react';"],
    [`${api}/transport/json.ts`, "import 'react-dom';"],
    [`${auth}/api/session.ts`, "import '@1flowbase/api-client';"],
    [`${api}/transport/json.ts`, "export { render } from 'react-dom';"],
  ]) {
    assert.throws(
      () => assertPreReactModuleBoundary((sourceFile) => {
        const source = readWebSource(sourceFile);
        return sourceFile === file ? `${source}\n${dependency}` : source;
      }),
      (error) => error.code === "ERR_ASSERTION" && error.message.startsWith(`${file}:`),
    );
  }
});

test("DRS-003 rejects a Vite auth alias redirected to the API client barrel", () => {
  assert.throws(() => assertPreReactModuleBoundary((file) => {
    const source = readWebSource(file);
    return file === "app/vite.config.ts"
      ? source.replace("../packages/api-client/src/auth/index.ts", "../packages/api-client/src/index.ts")
      : source;
  }), /Vite auth alias/u);
});

test("DV-F07 host UI imports Ant icons through deterministic leaf modules", () => {
  const barrelImports = sourceFiles(webSourceRoot).filter((filePath) =>
    /from\s+['"]@ant-design\/icons['"]/u.test(
      fs.readFileSync(filePath, "utf8"),
    ),
  );

  assert.deepEqual(
    barrelImports.map((filePath) => path.relative(webSourceRoot, filePath)),
    [],
  );
});

test("DV-F08 root router keeps page implementations behind lazy boundaries", () => {
  const routerSource = fs.readFileSync(
    path.resolve(webSourceRoot, "app", "router.tsx"),
    "utf8",
  );
  const appSource = fs.readFileSync(
    path.resolve(webSourceRoot, "app", "App.tsx"),
    "utf8",
  );

  for (const pageName of [
    "HomeRedirect",
    "FrontstageWorkspacePage",
    "MePage",
    "SettingsPage",
  ]) {
    assert.doesNotMatch(
      routerSource,
      new RegExp(`import\\s*\\{\\s*${pageName}\\s*\\}\\s*from`, "u"),
    );
    assert.match(routerSource, new RegExp(`const ${pageName} = lazy`, "u"));
  }
  const { discoverLazyImports } = require('../../vite-lazy-deps-gate/core.js');
  const routerLazyImports = discoverLazyImports({
    repoRoot: path.resolve(__dirname, '../../../..'),
  }).filter((entry) => entry.source === 'web/app/src/app/router.tsx');
  assert.deepEqual(
    routerLazyImports.map((entry) => entry.specifier).sort(),
    [
      '../app-shell/AppShellFrame',
      '../routes/HomeRedirect',
      '../features/frontstage/pages/FrontstageWorkspacePage',
      '../features/me/pages/MePage',
      '../features/settings/pages/SettingsPage',
      '../features/applications/pages/ApplicationDetailPage',
      '../features/auth/pages/AuthenticatedSignInRedirect',
      '../features/auth/pages/mcp-authorization/McpAuthorizePage',
    ].sort(),
  );
  assert.ok(routerLazyImports.every((entry) => entry.resolvedPath));
  assert.doesNotMatch(routerSource, /from ['"][^'"]*pages\/ApplicationDetailPage['"]/u);
  assert.match(routerSource, /const AppShellFrame = lazy/u);
  assert.doesNotMatch(appSource, /features\/workflow\/register/u);
  assert.doesNotMatch(appSource, /from ['"]\.\/router['"]/u);
  assert.doesNotMatch(appSource, /from ['"]\.\/AppProviders['"]/u);
  assert.match(appSource, /const ApplicationRuntimeBootstrap = lazy/u);
  assert.match(appSource, /import\(['"]\.\/ApplicationRuntimeBootstrap['"]\)/u);

  const runtimeBootstrapSource = fs.readFileSync(
    path.resolve(webSourceRoot, "app", "ApplicationRuntimeBootstrap.tsx"),
    "utf8",
  );
  assert.match(
    runtimeBootstrapSource,
    /import\(['"]\.\/AnonymousAppRuntime['"]\)/u,
  );
  assert.match(
    runtimeBootstrapSource,
    /import\(['"]\.\/bootstrap\/AuthenticatedStartup['"]\)/u,
  );
  assert.doesNotMatch(runtimeBootstrapSource, /^import .*\.\/(?:router|AuthenticatedAppRuntime)/mu);
  const preloadSource = readWebSource('app/src/app/bootstrap/authenticated-shell-preload.ts');
  assert.match(preloadSource, /import\(['"]\.\.\/AuthenticatedAppRuntime['"]\)/u);
  assert.doesNotMatch(preloadSource, /from ['"]\.\.\/AuthenticatedAppRuntime['"]/u);

  const publicRouterSource = fs.readFileSync(
    path.resolve(webSourceRoot, "app", "public-router.tsx"),
    "utf8",
  );
  assert.match(publicRouterSource, /features\/auth\/pages\/SignInPage/u);
  assert.doesNotMatch(routerSource, /features\/auth\/pages\/SignInPage/u);
});

test("DV-F08 anonymous bootstrap avoids full API and dynamic auth barrels", () => {
  const htmlSource = fs.readFileSync(
    path.resolve(path.dirname(viteConfigPath), "index.html"),
    "utf8",
  );
  const bootstrapSource = fs.readFileSync(
    path.resolve(webSourceRoot, "bootstrap.ts"),
    "utf8",
  );
  const authApiSource = fs.readFileSync(
    path.resolve(webSourceRoot, "features", "auth", "api", "session.ts"),
    "utf8",
  );
  const authStoreSource = fs.readFileSync(
    path.resolve(webSourceRoot, "state", "auth-store.ts"),
    "utf8",
  );
  const signInSource = fs.readFileSync(
    path.resolve(webSourceRoot, "features", "auth", "pages", "SignInPage.tsx"),
    "utf8",
  );

  assert.match(authApiSource, /@1flowbase\/api-client\/auth/u);
  assert.match(htmlSource, /\/src\/bootstrap\.ts/u);
  assert.match(bootstrapSource, /startAuthSessionDiscovery/u);
  assert.match(bootstrapSource, /import\(['"]\.\/main['"]\)/u);
  assert.match(authStoreSource, /@1flowbase\/api-client\/auth/u);
  assert.doesNotMatch(authApiSource, /from ['"]@1flowbase\/api-client['"]/u);
  assert.match(signInSource, /const CustomAuthRuntime = lazy/u);
  assert.match(signInSource, /import\(['"]\.\.\/components\/public-runtime\/CustomAuthRuntime['"]\)/u);
  assert.doesNotMatch(
    signInSource,
    /import\s*\{\s*PublicAuthBlock\s*\}\s*from/u,
  );

  const registrySource = fs.readFileSync(
    path.resolve(
      webSourceRoot,
      "features",
      "frontstage",
      "lib",
      "native-modules",
      "registry.ts",
    ),
    "utf8",
  );
  assert.doesNotMatch(
    registrySource,
    /import \* as antdModule from ['"]antd['"]/u,
  );
  assert.doesNotMatch(
    registrySource,
    /import \* as uiModule from ['"]@1flowbase\/ui['"]/u,
  );
  assert.match(registrySource, /loadAntDesignRootModule/u);
});

test("DV-F04 vite config exposes lifecycle readiness after bounded warmup", () => {
  const viteConfigSource = fs.readFileSync(viteConfigPath, "utf8");
  const runtimeSource = fs.readFileSync(
    path.resolve(path.dirname(viteConfigPath), "vite", "dev-runtime.ts"),
    "utf8",
  );

  assert.match(viteConfigSource, /oneFlowbaseDevRuntimePlugin/u);
  assert.match(viteConfigSource, /warmup:\s*\{/u);
  assert.match(runtimeSource, /\/__1flowbase_dev_ready/u);
  assert.match(runtimeSource, /'Building'/u);
  assert.match(runtimeSource, /'Validating'/u);
  assert.match(runtimeSource, /'Ready'/u);
  assert.match(runtimeSource, /'Degraded'/u);
  assert.match(runtimeSource, /attachPreReadyTrafficGate/u);
  assert.match(runtimeSource, /response\.statusCode = 503/u);
  assert.match(runtimeSource, /fs\.writeFileSync/u);
  assert.match(runtimeSource, /RECOVERY_PROBE_PATH/u);
  assert.doesNotMatch(
    runtimeSource,
    /allowedHosts|cors|cloudflare|access token/iu,
  );
});
