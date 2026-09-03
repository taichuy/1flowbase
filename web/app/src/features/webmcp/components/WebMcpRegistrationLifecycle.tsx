import { useEffect } from 'react';

import { useAuthStore } from '../../../state/auth-store';
import {
  fetchWebMcpRegistrations,
  invokeWebMcpTool,
  type WebMcpToolRegistration
} from '../api/webmcp';
import { WEBMCP_REGISTRATIONS_CHANGED_EVENT } from '../registration-events';

interface ModelContextTool {
  name: string;
  title: string;
  description: string;
  inputSchema: Record<string, unknown>;
  annotations: {
    readOnlyHint: boolean;
    untrustedContentHint: boolean;
  };
  execute: (
    input: Record<string, unknown>,
    options: { signal: AbortSignal }
  ) => Promise<unknown>;
}

interface WebMcpDocument extends Document {
  modelContext?: {
    registerTool(
      tool: ModelContextTool,
      options: { signal: AbortSignal }
    ): Promise<void>;
  };
}

function browserTool(
  instanceId: string,
  registration: WebMcpToolRegistration,
  csrfToken: string
): ModelContextTool {
  return {
    name: registration.name,
    title: registration.title,
    description: registration.description,
    inputSchema: registration.input_schema,
    annotations: {
      readOnlyHint: registration.annotations.read_only_hint,
      untrustedContentHint: registration.annotations.untrusted_content_hint
    },
    execute: (input, options) =>
      invokeWebMcpTool(
        instanceId,
        registration.operation,
        input,
        csrfToken,
        options.signal
      )
  };
}

export function WebMcpRegistrationLifecycle() {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const workspaceId = useAuthStore(
    (state) => state.actor?.current_workspace_id ?? null
  );

  useEffect(() => {
    const modelContext = (document as WebMcpDocument).modelContext;
    if (!modelContext || !csrfToken || !workspaceId) return;

    let activeRegistrations: AbortController | null = null;
    const refreshRegistrations = () => {
      activeRegistrations?.abort();
      const lifecycle = new AbortController();
      activeRegistrations = lifecycle;
      void fetchWebMcpRegistrations(lifecycle.signal)
        .then((instances) =>
          Promise.all(
            instances.flatMap((instance) =>
              instance.tools.map((registration) =>
                modelContext.registerTool(
                  browserTool(instance.instance_id, registration, csrfToken),
                  { signal: lifecycle.signal }
                )
              )
            )
          )
        )
        .catch((error: unknown) => {
          if (!lifecycle.signal.aborted) {
            console.error('WebMCP tool registration failed', error);
          }
        });
    };

    window.addEventListener(
      WEBMCP_REGISTRATIONS_CHANGED_EVENT,
      refreshRegistrations
    );
    refreshRegistrations();

    return () => {
      window.removeEventListener(
        WEBMCP_REGISTRATIONS_CHANGED_EVENT,
        refreshRegistrations
      );
      activeRegistrations?.abort();
    };
  }, [csrfToken, workspaceId]);

  return null;
}
