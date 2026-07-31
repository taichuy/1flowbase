const { buildServiceEnv, getServiceDefinitions } = require('../dev-up/core.js');

const PASSWORD_LOCAL_AUTHENTICATOR_ID = '00000000-0000-0000-0000-000000000001';

function loadRootCredentials({
  repoRoot,
  accountOverride,
  passwordOverride,
  getServiceDefinitions: getDefinitions = getServiceDefinitions,
  buildServiceEnv: buildEnv = buildServiceEnv,
  sourceEnv = process.env,
}) {
  const apiService = getDefinitions(repoRoot)['api-server'];
  const env = buildEnv(apiService, sourceEnv);
  const account = accountOverride || env.BOOTSTRAP_ROOT_ACCOUNT || 'root';
  const password = passwordOverride || env.BOOTSTRAP_ROOT_PASSWORD;

  if (!password) {
    throw new Error(`缺少 root 密码，请检查 ${apiService.envFile}`);
  }

  return {
    account,
    password,
    envFilePath: apiService.envFile,
  };
}

async function revokeTemporaryConsoleSession(requestContext, csrfToken) {
  const response = await requestContext.delete('/api/console/session', {
    headers: {
      'x-csrf-token': csrfToken,
    },
  });

  if (!response.ok() && response.status() !== 401) {
    const body = typeof response.text === 'function' ? await response.text() : '';
    throw new Error(`临时 console session 回收失败：${response.status()} ${body}`.trim());
  }
}

async function openTemporaryConsoleSession({
  playwright,
  apiBaseUrl,
  account,
  password,
  storageStatePath,
}) {
  const requestContext = await playwright.request.newContext({
    baseURL: apiBaseUrl,
    ignoreHTTPSErrors: true,
  });

  let temporarySession = null;
  try {
    const response = await requestContext.post('/api/public/auth/sign-in', {
      data: {
        authenticator_id: PASSWORD_LOCAL_AUTHENTICATOR_ID,
        identifier: account,
        password,
      },
    });

    if (!response.ok()) {
      const body = typeof response.text === 'function' ? await response.text() : '';
      throw new Error(`root 凭据无效，登录失败：${response.status()} ${body}`.trim());
    }

    const payload = await response.json();
    const csrfToken = payload?.data?.csrf_token;
    if (!csrfToken) {
      throw new Error('登录成功但响应缺少 csrf_token，无法安全回收临时 console session');
    }

    let disposed = false;
    temporarySession = {
      authenticated: true,
      storageStatePath: storageStatePath ?? null,
      async dispose() {
        if (disposed) {
          return;
        }
        disposed = true;
        try {
          await revokeTemporaryConsoleSession(requestContext, csrfToken);
        } finally {
          await requestContext.dispose();
        }
      },
    };

    if (storageStatePath) {
      await requestContext.storageState({ path: storageStatePath });
    }

    return temporarySession;
  } catch (error) {
    if (temporarySession) {
      await temporarySession.dispose();
    } else {
      await requestContext.dispose();
    }
    throw error;
  }
}

module.exports = {
  loadRootCredentials,
  openTemporaryConsoleSession,
};
