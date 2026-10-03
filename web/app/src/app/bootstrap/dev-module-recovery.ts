const DEV_GENERATION_META_NAME = '1flowbase-dev-generation';
const DEV_RELOAD_PREFIX = '1flowbase.dev-runtime.reload';

export function currentDevGeneration() {
  return (
    document
      .querySelector(`meta[name="${DEV_GENERATION_META_NAME}"]`)
      ?.getAttribute('content') || 'unknown'
  );
}

function reloadKey() {
  return `${DEV_RELOAD_PREFIX}:${currentDevGeneration()}`;
}

export function resetDevModuleRecovery() {
  try {
    sessionStorage.removeItem(reloadKey());
  } catch {
    // Storage can be disabled; manual reload must still work.
  }
}

export function recoverDevModuleGraph(error: unknown): boolean {
  if (!import.meta.env.DEV) return false;
  const message = error instanceof Error ? error.message : String(error);
  if (
    !/does not provide an export|dynamically imported module|outdated optimize dep|importing a module script failed/iu.test(
      message
    )
  ) {
    return false;
  }

  // Share the budget across bootstrap, React and router boundaries, including
  // across reloads. React.lazy caches rejected imports until the page reloads.
  try {
    const key = reloadKey();
    if (sessionStorage.getItem(key) === 'attempted') return false;
    sessionStorage.setItem(key, 'attempted');
  } catch {
    // Without a persistent budget an automatic reload could loop forever.
    return false;
  }
  window.location.reload();
  return true;
}
