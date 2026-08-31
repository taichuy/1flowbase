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
  assert.match(runtimeSource, /'Scanning'/u);
  assert.match(runtimeSource, /'Optimizing'/u);
  assert.match(runtimeSource, /'Warming'/u);
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
  assert.match(runtimeSource, /fs\.promises\.rm/u);
});

test("DRS-003 pre-React bootstrap never leaves an empty root after module failure", () => {
  const bootstrapSource = fs.readFileSync(
    path.resolve(webSourceRoot, "bootstrap.ts"),
    "utf8",
  );
  const appSource = fs.readFileSync(
    path.resolve(webSourceRoot, "app", "App.tsx"),
    "utf8",
  );

  assert.match(bootstrapSource, /renderBootStage/u);
  assert.match(bootstrapSource, /renderBootFailure/u);
  assert.match(bootstrapSource, /import\(['"]\.\/main['"]\)\.catch/u);
  assert.match(appSource, /ApplicationBootBoundary/u);
  assert.doesNotMatch(appSource, /Suspense fallback=\{null\}/u);
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
    "HomePage",
    "FrontstageWorkspacePage",
    "MePage",
    "TemplatesPage",
  ]) {
    assert.doesNotMatch(
      routerSource,
      new RegExp(`import\\s*\\{\\s*${pageName}\\s*\\}\\s*from`, "u"),
    );
    assert.match(routerSource, new RegExp(`const ${pageName} = lazy`, "u"));
  }
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
    /import\(['"]\.\/AuthenticatedAppRuntime['"]\)/u,
  );
  assert.doesNotMatch(runtimeBootstrapSource, /^import .*\.\/router/mu);

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
  assert.match(signInSource, /const PublicAuthBlock = lazy/u);
  assert.match(signInSource, /diagnoseLegacyBlockModuleSource/u);
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
