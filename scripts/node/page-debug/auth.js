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

async function loginAndPersistStorageState({
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

    if (storageStatePath) {
      await requestContext.storageState({ path: storageStatePath });
    }

    return {
      authenticated: true,
      storageStatePath: storageStatePath ?? null,
    };
  } finally {
    await requestContext.dispose();
  }
}

module.exports = {
  loadRootCredentials,
  loginAndPersistStorageState,
};
