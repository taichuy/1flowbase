import {
  currentDevGeneration,
  recoverDevModuleGraph,
  resetDevModuleRecovery
} from './app/bootstrap/dev-module-recovery';

// A later source edit gets a new recovery attempt, but a reload alone does not.
if (import.meta.hot) {
  import.meta.hot.on('vite:beforeUpdate', resetDevModuleRecovery);
}

function rootElement() {
  const root = document.getElementById('root');
  if (!root) throw new Error('application root is missing');
  return root;
}

function renderBootFailure(error: unknown) {
  console.error('[1flowbase-bootstrap] application module graph failed', error);
  if (recoverDevModuleGraph(error)) return;
  const generation = currentDevGeneration();

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
    resetDevModuleRecovery();
    window.location.reload();
  });
  alert.append(title, detail, retry);
  root.replaceChildren(alert);
}

void import('./features/auth/api/auth-session-discovery')
  .then(({ startAuthSessionDiscovery }) => startAuthSessionDiscovery())
  .catch(() => undefined);
void import('./main').catch(renderBootFailure);
