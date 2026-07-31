const fs = require('node:fs');
const path = require('node:path');

const { loadRootCredentials } = require('../page-debug/auth.js');

const DEFAULT_API_BASE_URL = 'http://127.0.0.1:7800';
const OUTPUT_ROOT = path.join('tmp', 'test-governance', 'api-debug');
const HTTP_METHODS = new Set(['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']);
const MUTATING_METHODS = new Set(['POST', 'PUT', 'PATCH', 'DELETE']);
const PASSWORD_LOCAL_AUTHENTICATOR_ID = '00000000-0000-0000-0000-000000000001';
const SENSITIVE_HEADER_PATTERN = /(?:authorization|cookie|csrf|token|secret|password|api[-_]?key)/iu;

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function parseCliArgs(argv) {
  const options = {
    help: false,
    method: 'GET',
    target: null,
    apiBaseUrl: DEFAULT_API_BASE_URL,
    account: null,
    password: null,
    body: null,
    bodyFile: null,
    headers: [],
    outDir: null,
    expectStatus: null,
    printBody: false,
  };

  const args = [...argv];
  if (args.length === 0 || args.includes('-h') || args.includes('--help')) {
    return { ...options, help: true };
  }

  if (args[0] && HTTP_METHODS.has(args[0].toUpperCase())) {
    options.method = args.shift().toUpperCase();
  }

  if (args[0] && !args[0].startsWith('--')) {
    options.target = args.shift();
  }

  while (args.length > 0) {
    const arg = args.shift();

    if (arg === '--api-base-url') {
      options.apiBaseUrl = requireValue(args, arg);
    } else if (arg === '--account') {
      options.account = requireValue(args, arg);
    } else if (arg === '--password') {
      options.password = requireValue(args, arg);
    } else if (arg === '--body') {
      options.body = requireValue(args, arg);
    } else if (arg === '--body-file') {
      options.bodyFile = requireValue(args, arg);
    } else if (arg === '--header') {
      options.headers.push(parseHeader(requireValue(args, arg)));
    } else if (arg === '--out-dir') {
      options.outDir = requireValue(args, arg);
    } else if (arg === '--expect-status') {
      options.expectStatus = parseExpectedStatus(requireValue(args, arg));
    } else if (arg === '--print-body') {
      options.printBody = true;
    } else {
      throw new Error(`未知参数：${arg}`);
    }
  }

  if (!options.target) {
    throw new Error('缺少目标 API path 或 URL');
  }

  if (options.body && options.bodyFile) {
    throw new Error('不能同时使用 --body 和 --body-file');
  }

  if ((options.method === 'GET' || options.method === 'HEAD') && (options.body || options.bodyFile)) {
    throw new Error(`${options.method} 请求不能携带 body`);
  }

  return options;
}

function requireValue(args, optionName) {
  const value = args.shift();
  if (!value || value.startsWith('--')) {
    throw new Error(`${optionName} 需要值`);
  }
  return value;
}

function parseHeader(rawHeader) {
  const separatorIndex = rawHeader.indexOf(':');
  if (separatorIndex <= 0) {
    throw new Error(`--header 需要 "name: value" 格式：${rawHeader}`);
  }

  return {
    name: rawHeader.slice(0, separatorIndex).trim(),
    value: rawHeader.slice(separatorIndex + 1).trim(),
  };
}

function parseExpectedStatus(rawStatus) {
  const status = Number.parseInt(rawStatus, 10);
  if (!Number.isInteger(status) || status < 100 || status > 599) {
    throw new Error(`--expect-status 必须是 HTTP status code：${rawStatus}`);
  }
  return status;
}

function resolveTargetUrl(apiBaseUrl, target) {
  return /^https?:\/\//u.test(target) ? target : new URL(target, normalizeBaseUrl(apiBaseUrl)).toString();
}

function normalizeBaseUrl(baseUrl) {
  return baseUrl.endsWith('/') ? baseUrl : `${baseUrl}/`;
}

function buildHeaderObject(headerEntries) {
  const headers = {};
  for (const entry of headerEntries) {
    headers[entry.name] = entry.value;
  }
  return headers;
}

function hasHeader(headers, targetName) {
  const normalizedTarget = targetName.toLowerCase();
  return Object.keys(headers).some((name) => name.toLowerCase() === normalizedTarget);
}

function setHeader(headers, name, value) {
  const existingName = Object.keys(headers).find((candidate) => candidate.toLowerCase() === name.toLowerCase());
  headers[existingName || name] = value;
}

async function runApiDebug(options, deps = {}) {
  const repoRoot = deps.repoRoot || getRepoRoot();
  const fetchImpl = deps.fetchImpl || globalThis.fetch;
  if (typeof fetchImpl !== 'function') {
    throw new Error('当前 Node runtime 不支持 fetch');
  }

  const credentials = (deps.loadRootCredentials || loadRootCredentials)({
    repoRoot,
    accountOverride: options.account,
    passwordOverride: options.password,
  });
  const requestBody = loadRequestBody(options);
  const artifacts = createRunArtifacts({ repoRoot, outDir: options.outDir, now: deps.now });
  fs.mkdirSync(artifacts.runDir, { recursive: true });

  const auth = await loginForApiDebug({
    fetchImpl,
    apiBaseUrl: options.apiBaseUrl,
    account: credentials.account,
    password: credentials.password,
  });
  try {
    const requestHeaders = buildHeaderObject(options.headers);
    if (!hasHeader(requestHeaders, 'cookie')) {
      setHeader(requestHeaders, 'cookie', auth.cookieHeader);
    }
    if (MUTATING_METHODS.has(options.method) && auth.csrfToken && !hasHeader(requestHeaders, 'x-csrf-token')) {
      setHeader(requestHeaders, 'x-csrf-token', auth.csrfToken);
    }
    if (requestBody !== null && !hasHeader(requestHeaders, 'content-type')) {
      setHeader(requestHeaders, 'content-type', 'application/json');
    }

    const targetUrl = resolveTargetUrl(options.apiBaseUrl, options.target);
    const startedAt = new Date().toISOString();
    const response = await fetchImpl(targetUrl, {
      method: options.method,
      headers: requestHeaders,
      body: requestBody === null ? undefined : requestBody,
    });
    const finishedAt = new Date().toISOString();
    const responseText = await response.text();
    const responseJson = parseJsonMaybe(responseText);
    const responseHeaders = headersToObject(response.headers);
    const responseBodyPath = writeResponseBody({
      artifacts,
      responseText,
      responseJson,
    });

    const expectedMismatch = options.expectStatus !== null && response.status !== options.expectStatus;
    const evidencePath = writeEvidence({
      artifacts,
      options,
      credentials,
      auth,
      targetUrl,
      requestHeaders,
      requestBody,
      response,
      responseHeaders,
      responseBodyPath,
      responseJson,
      startedAt,
      finishedAt,
      expectedMismatch,
    });

    return {
      ok: !expectedMismatch,
      httpOk: response.ok,
      status: response.status,
      statusText: response.statusText,
      expectedStatus: options.expectStatus,
      error: expectedMismatch ? `expected status ${options.expectStatus}, got ${response.status}` : null,
      outputDir: artifacts.runDir,
      evidencePath,
      responseBodyPath,
      bodyPreview: responseText.slice(0, 2000),
      printedBody: options.printBody ? responseText : null,
    };
  } finally {
    await revokeApiDebugSession({
      fetchImpl,
      apiBaseUrl: options.apiBaseUrl,
      auth,
    });
  }
}

function loadRequestBody(options) {
  if (options.body !== null) {
    return options.body;
  }
  if (options.bodyFile) {
    return fs.readFileSync(path.resolve(options.bodyFile), 'utf8');
  }
  return null;
}

async function loginForApiDebug({ fetchImpl, apiBaseUrl, account, password }) {
  const loginUrl = resolveTargetUrl(apiBaseUrl, '/api/public/auth/sign-in');
  const response = await fetchImpl(loginUrl, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
    },
    body: JSON.stringify({
      authenticator_id: PASSWORD_LOCAL_AUTHENTICATOR_ID,
      identifier: account,
      password,
    }),
  });
  const responseText = await response.text();
  const responseJson = parseJsonMaybe(responseText);

  if (!response.ok) {
    throw new Error(`root 凭据无效，登录失败：${response.status} ${responseText.slice(0, 500)}`.trim());
  }

  const setCookieHeaders = extractSetCookieHeaders(response.headers);
  const cookieHeader = setCookieHeaders
    .map((value) => value.split(';')[0].trim())
    .filter(Boolean)
    .join('; ');

  if (!cookieHeader) {
    throw new Error('登录成功但响应缺少 set-cookie，无法构造认证请求');
  }

  return {
    cookieHeader,
    csrfToken: responseJson.json?.data?.csrf_token ?? null,
    currentWorkspaceId: responseJson.json?.data?.current_workspace_id ?? null,
  };
}

async function revokeApiDebugSession({ fetchImpl, apiBaseUrl, auth }) {
  if (!auth.csrfToken) {
    throw new Error('登录成功但响应缺少 csrf_token，无法安全回收临时 console session');
  }

  const response = await fetchImpl(resolveTargetUrl(apiBaseUrl, '/api/console/session'), {
    method: 'DELETE',
    headers: {
      cookie: auth.cookieHeader,
      'x-csrf-token': auth.csrfToken,
    },
  });

  if (!response.ok && response.status !== 401) {
    const body = await response.text();
    throw new Error(`临时 console session 回收失败：${response.status} ${body.slice(0, 500)}`.trim());
  }
}

function extractSetCookieHeaders(headers) {
  if (typeof headers.getSetCookie === 'function') {
    return headers.getSetCookie();
  }
  if (typeof headers.raw === 'function') {
    return headers.raw()['set-cookie'] || [];
  }
  const setCookie = typeof headers.get === 'function' ? headers.get('set-cookie') : null;
  return setCookie ? [setCookie] : [];
}

function parseJsonMaybe(text) {
  if (!text) {
    return { isJson: false, json: null };
  }

  try {
    return { isJson: true, json: JSON.parse(text) };
  } catch (_error) {
    return { isJson: false, json: null };
  }
}

function headersToObject(headers) {
  const collected = {};
  if (headers && typeof headers.forEach === 'function') {
    headers.forEach((value, name) => {
      collected[name] = value;
    });
    return collected;
  }

  return collected;
}

function createRunArtifacts({ repoRoot, outDir, now = () => new Date() }) {
  const timestamp = now().toISOString().replaceAll(':', '-').replaceAll('.', '-');
  const runDir = outDir
    ? path.resolve(repoRoot, outDir)
    : path.join(repoRoot, OUTPUT_ROOT, timestamp);

  return {
    runDir,
    evidencePath: path.join(runDir, 'evidence.json'),
    responseJsonPath: path.join(runDir, 'response.json'),
    responseTextPath: path.join(runDir, 'response.txt'),
  };
}

function writeResponseBody({ artifacts, responseText, responseJson }) {
  if (responseJson.isJson) {
    fs.writeFileSync(artifacts.responseJsonPath, JSON.stringify(responseJson.json, null, 2) + '\n', 'utf8');
    return artifacts.responseJsonPath;
  }

  fs.writeFileSync(artifacts.responseTextPath, responseText, 'utf8');
  return artifacts.responseTextPath;
}

function writeEvidence({
  artifacts,
  options,
  credentials,
  auth,
  targetUrl,
  requestHeaders,
  requestBody,
  response,
  responseHeaders,
  responseBodyPath,
  responseJson,
  startedAt,
  finishedAt,
  expectedMismatch,
}) {
  const evidence = {
    tool: 'api-debug',
    startedAt,
    finishedAt,
    auth: {
      account: credentials.account,
      envFilePath: credentials.envFilePath,
      currentWorkspaceId: auth.currentWorkspaceId,
    },
    request: {
      method: options.method,
      target: options.target,
      url: targetUrl,
      headers: redactSensitiveHeaders(requestHeaders),
      bodyBytes: requestBody ? Buffer.byteLength(requestBody, 'utf8') : 0,
    },
    response: {
      status: response.status,
      statusText: response.statusText,
      ok: response.ok,
      expectedStatus: options.expectStatus,
      expectedMismatch,
      headers: redactSensitiveHeaders(responseHeaders),
      bodyKind: responseJson.isJson ? 'json' : 'text',
      bodyPath: responseBodyPath,
    },
  };

  fs.writeFileSync(artifacts.evidencePath, JSON.stringify(evidence, null, 2) + '\n', 'utf8');
  return artifacts.evidencePath;
}

function redactSensitiveHeaders(headers) {
  const redacted = {};

  for (const [name, value] of Object.entries(headers)) {
    redacted[name] = SENSITIVE_HEADER_PATTERN.test(name) ? '[redacted]' : value;
  }

  return redacted;
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(`用法：node scripts/node/api-debug/cli.js [METHOD] <api-path-or-url> [options]

示例：
  node scripts/node/api-debug/cli.js /api/console/me
  node scripts/node/api-debug/cli.js POST /api/console/widgets --body '{"name":"demo"}' --expect-status 201
  node scripts/node/tooling.js api-debug GET /api/console/me --print-body

认证：
  默认从 api-server .env 读取 BOOTSTRAP_ROOT_ACCOUNT / BOOTSTRAP_ROOT_PASSWORD。
  可用 --account / --password 覆盖。

选项：
  --api-base-url <url>     默认 ${DEFAULT_API_BASE_URL}
  --account <account>      覆盖登录账号
  --password <password>    覆盖登录密码
  --body <json-or-text>    请求 body，默认 content-type: application/json
  --body-file <path>       从文件读取请求 body
  --header "name: value"   追加请求头，可重复
  --expect-status <code>   期望 HTTP status，不匹配时退出码为 1
  --out-dir <path>         evidence 输出目录，默认 ${OUTPUT_ROOT}/<timestamp>
  --print-body             stdout 额外输出 response body
`);
}

async function main(argv = process.argv.slice(2), deps = {}) {
  const options = parseCliArgs(argv);
  const writeStdout = deps.writeStdout || ((text) => process.stdout.write(text));

  if (options.help) {
    usage(writeStdout);
    return 0;
  }

  const result = await runApiDebug(options, deps);
  writeStdout(`${JSON.stringify({
    ok: result.ok,
    httpOk: result.httpOk,
    status: result.status,
    statusText: result.statusText,
    expectedStatus: result.expectedStatus,
    error: result.error,
    outputDir: result.outputDir,
    evidencePath: result.evidencePath,
    responseBodyPath: result.responseBodyPath,
    bodyPreview: result.bodyPreview,
  }, null, 2)}\n`);

  if (options.printBody && result.printedBody !== null) {
    writeStdout(`${result.printedBody}\n`);
  }

  return result.ok ? 0 : 1;
}

module.exports = {
  DEFAULT_API_BASE_URL,
  createRunArtifacts,
  loginForApiDebug,
  main,
  parseCliArgs,
  redactSensitiveHeaders,
  resolveTargetUrl,
  runApiDebug,
  usage,
};
