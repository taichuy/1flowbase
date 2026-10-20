import type {
  PublicLoginEntriesResponse,
  RuntimeI18nCatalog
} from '@1flowbase/api-client';
import passwordLocalSource from '../../../../../api/crates/control-plane/src/auth/public_ui_blocks/password_local.tsx?raw';

const runtimeCatalogs: Record<'en_US' | 'zh_Hans', RuntimeI18nCatalog> = {
  en_US: {
    catalog_revision: 1,
    locale: 'en_US',
    digest:
      'sha256:a6259dde9d6976f0b7cb6c04c02029cade9344ed2d44e7d1a613214f8a0cb6f7',
    messages: { Landing: 'Landing', Overview: 'Overview' }
  },
  zh_Hans: {
    catalog_revision: 1,
    locale: 'zh_Hans',
    digest:
      'sha256:cc1b713c00b67e391085a6ab18834fa47c43f6fa431fd297963f5ffb7203b823',
    messages: { Landing: 'Landing', Overview: '概览' }
  }
};

const loginEntries: PublicLoginEntriesResponse = {
  default_login_entry_id: 'auth-password-local',
  login_entries: [
    {
      id: 'auth-password-local',
      entry_action: 'render_ui',
      auth_type: 'password-local',
      is_builtin: true,
      title: 'Password',
      description: 'Local password authentication',
      sort_order: 0,
      public_ui_block: passwordLocalSource,
      public_variables: { self_registration_enabled: false }
    }
  ]
};

export function getStyleBoundaryRuntimeResponse(
  url: URL,
  method: string
): Response | null {
  if (url.origin !== window.location.origin || method.toUpperCase() !== 'GET')
    return null;
  if (url.pathname === '/api/console/i18n/catalog') {
    const locale = url.searchParams.get('locale');
    const catalog =
      locale === 'en_US' || locale === 'zh_Hans'
        ? runtimeCatalogs[locale]
        : undefined;
    if (!catalog) return null;
    // This endpoint returns a raw snapshot, unlike apiFetch's data/meta envelope.
    return new Response(JSON.stringify(catalog), {
      headers: {
        'content-type': 'application/json',
        etag: `"${catalog.digest}"`
      }
    });
  }
  if (url.pathname === '/api/public/auth/login-entries' && !url.search) {
    return new Response(JSON.stringify({ data: loginEntries, meta: null }), {
      headers: { 'content-type': 'application/json' }
    });
  }
  return null;
}
