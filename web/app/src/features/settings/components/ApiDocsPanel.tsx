import { useCallback, useEffect } from 'react';

import { useRouterState } from '@tanstack/react-router';
import { App } from 'antd';

import {
  fetchCurrentSession,
  getScalarApiBaseUrl
} from '../../auth/api/session';
import { ApiDocsExplorer } from '../../../shared/ui/api-docs/ApiDocsExplorer';
import { installScalarClipboardPatch } from '../lib/scalar-clipboard';

import {
  fetchSettingsApiDocsCatalog,
  fetchSettingsApiDocsCategoryOperations,
  fetchSettingsApiDocsOperationSpec,
  settingsApiDocsCatalogQueryKey,
  settingsApiDocsCategoryOperationsQueryKey,
  settingsApiDocsOperationSpecQueryKey
} from '../api/api-docs';
import { SettingsSectionSurface } from './SettingsSectionSurface';
import './api-docs-panel.css';

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function updateDocsQuery(
  {
    categoryId,
    operationId
  }: {
    categoryId: string | null;
    operationId: string | null;
  },
  mode: 'push' | 'replace' = 'push'
) {
  const nextUrl = new URL(window.location.href);

  if (categoryId) {
    nextUrl.searchParams.set('category', categoryId);
  } else {
    nextUrl.searchParams.delete('category');
  }

  if (operationId) {
    nextUrl.searchParams.set('operation', operationId);
  } else {
    nextUrl.searchParams.delete('operation');
  }

  const nextPath = `${nextUrl.pathname}${nextUrl.search}`;

  if (mode === 'replace') {
    window.history.replaceState({}, '', nextPath);
  } else {
    window.history.pushState({}, '', nextPath);
  }

  window.dispatchEvent(new PopStateEvent('popstate'));
}

const docsViewerSessionQueryKey = [
  'settings',
  'docs',
  'viewer-session'
] as const;
const scalarPreferredSecurityScheme = [
  'sessionCookie',
  'csrfHeader',
  'patBearer'
] as const;
type ScalarPreferredSecurityScheme =
  (typeof scalarPreferredSecurityScheme)[number];

function isScalarPreferredSecurityScheme(
  schemeName: string
): schemeName is ScalarPreferredSecurityScheme {
  return scalarPreferredSecurityScheme.some(
    (candidate) => candidate === schemeName
  );
}

function collectPreferredSecuritySchemes(operationSpec: unknown) {
  const securityRequirements =
    isRecord(operationSpec) && Array.isArray(operationSpec.security)
      ? operationSpec.security
      : [];

  for (const requirement of securityRequirements) {
    if (!isRecord(requirement)) {
      continue;
    }

    const requirementSchemes = Object.keys(requirement);

    if (
      requirementSchemes.length > 0 &&
      requirementSchemes.every(isScalarPreferredSecurityScheme)
    ) {
      const requirementSchemeSet = new Set(requirementSchemes);
      return scalarPreferredSecurityScheme.filter((schemeName) =>
        requirementSchemeSet.has(schemeName)
      );
    }
  }

  return [];
}

function buildScalarAuthenticationConfig(
  operationSpec: unknown,
  sessionSnapshot: Awaited<ReturnType<typeof fetchCurrentSession>> | undefined
) {
  const securitySchemes =
    isRecord(operationSpec) &&
    isRecord(operationSpec.components) &&
    isRecord(operationSpec.components.securitySchemes)
      ? operationSpec.components.securitySchemes
      : {};
  const sessionCookieScheme = isRecord(securitySchemes.sessionCookie)
    ? securitySchemes.sessionCookie
    : {};
  const csrfHeaderScheme = isRecord(securitySchemes.csrfHeader)
    ? securitySchemes.csrfHeader
    : {};
  const patBearerScheme = isRecord(securitySchemes.patBearer)
    ? securitySchemes.patBearer
    : {};
  const preferredSecurityScheme =
    collectPreferredSecuritySchemes(operationSpec);

  if (
    Object.keys(sessionCookieScheme).length === 0 &&
    Object.keys(csrfHeaderScheme).length === 0 &&
    Object.keys(patBearerScheme).length === 0
  ) {
    return undefined;
  }

  const authenticationSecuritySchemes: Record<
    string,
    Record<string, unknown>
  > = {};

  if (Object.keys(sessionCookieScheme).length > 0) {
    authenticationSecuritySchemes.sessionCookie = {
      ...sessionCookieScheme
    };
  }

  if (Object.keys(csrfHeaderScheme).length > 0) {
    authenticationSecuritySchemes.csrfHeader = {
      ...csrfHeaderScheme,
      value: sessionSnapshot?.csrf_token ?? ''
    };
  }

  if (Object.keys(patBearerScheme).length > 0) {
    authenticationSecuritySchemes.patBearer = {
      ...patBearerScheme,
      value: ''
    };
  }

  return {
    preferredSecurityScheme,
    securitySchemes: authenticationSecuritySchemes
  };
}

export function ApiDocsPanel() {
  const { message } = App.useApp();
  useEffect(() => installScalarClipboardPatch(message), [message]);
  const locationSearch = useRouterState({
    select: (state) => state.location.search as Record<string, unknown>
  });
  const requestedCategoryId =
    typeof locationSearch.category === 'string'
      ? locationSearch.category
      : null;
  const requestedOperationId =
    typeof locationSearch.operation === 'string'
      ? locationSearch.operation
      : null;
  const handleQueryStateChange = useCallback(
    (
      nextState: { categoryId: string | null; operationId: string | null },
      mode: 'push' | 'replace' = 'push'
    ) => updateDocsQuery(nextState, mode),
    []
  );

  return (
    <SettingsSectionSurface heightMode="fill">
      <ApiDocsExplorer
        queryState={{
          categoryId: requestedCategoryId,
          operationId: requestedOperationId
        }}
        onQueryStateChange={handleQueryStateChange}
        catalogQueryKey={settingsApiDocsCatalogQueryKey}
        fetchCatalog={fetchSettingsApiDocsCatalog}
        categoryOperationsQueryKey={settingsApiDocsCategoryOperationsQueryKey}
        fetchCategoryOperations={fetchSettingsApiDocsCategoryOperations}
        operationSpecQueryKey={settingsApiDocsOperationSpecQueryKey}
        fetchOperationSpec={fetchSettingsApiDocsOperationSpec}
        baseServerUrl={getScalarApiBaseUrl}
        authentication={{
          queryKey: docsViewerSessionQueryKey,
          queryFn: fetchCurrentSession,
          buildConfig: buildScalarAuthenticationConfig
        }}
      />
    </SettingsSectionSurface>
  );
}
