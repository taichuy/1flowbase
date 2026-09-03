const DEV_GENERATION_META_NAME = '1flowbase-dev-generation';
const DEV_RELOAD_PREFIX = '1flowbase.dev-runtime.reload';

function rootElement() {
  const root = document.getElementById('root');
  if (!root) throw new Error('application root is missing');
  return root;
}

function currentGeneration() {
  return (
    document
      .querySelector(`meta[name="${DEV_GENERATION_META_NAME}"]`)
      ?.getAttribute('content') || 'unknown'
  );
}

function shouldReloadModuleGraph(error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  return /does not provide an export|dynamically imported module|outdated optimize dep|importing a module script failed/iu.test(
    message
  );
}

function renderBootFailure(error: unknown) {
  console.error('[1flowbase-bootstrap] application module graph failed', error);
  const generation = currentGeneration();
  const reloadKey = `${DEV_RELOAD_PREFIX}:${generation}`;
  if (
    import.meta.env.DEV &&
    shouldReloadModuleGraph(error) &&
    sessionStorage.getItem(reloadKey) !== 'attempted'
  ) {
    sessionStorage.setItem(reloadKey, 'attempted');
    window.location.reload();
    return;
  }

  const root = rootElement();
  const alert = document.createElement('div');
  alert.className = 'application-bootstrap-failure';
  alert.setAttribute('role', 'alert');

  const title = document.createElement('strong');
  title.textContent = '开发应用启动失败';
  const detail = document.createElement('span');
  detail.textContent = `模块 generation ${generation.slice(0, 12)} 无法完成加载。`;
  const retry = document.createElement('button');
  retry.type = 'button';
  retry.textContent = '重新加载';
  retry.addEventListener('click', () => {
    sessionStorage.removeItem(reloadKey);
    window.location.reload();
  });
  alert.append(title, detail, retry);
  root.replaceChildren(alert);
}

void import('./features/auth/api/auth-session-discovery')
  .then(({ startAuthSessionDiscovery }) => startAuthSessionDiscovery())
  .catch(() => undefined);
void import('./main').catch(renderBootFailure);
