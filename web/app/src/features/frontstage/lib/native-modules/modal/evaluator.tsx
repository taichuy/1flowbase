import { useState, type ComponentType } from 'react';
import {
  NativeReactModuleRegistryError,
  canonicalizeNativeReactComponentArtifact,
  evaluateNativeReactComponentArtifact,
  evaluateNativeReactComponentArtifactWithRegistry,
  type NativeReactArtifactEvaluationBindings,
  type NativeReactArtifactEvaluationResult,
  type NativeReactModuleRegistry
} from '@1flowbase/page-runtime/browser';
import { createBlockModalRuntime } from './runtime';
import { createBlockNotificationRuntime } from '../notification/runtime';

/** Cache the factory and dependencies; instantiate user closures per React mount. */
export async function evaluateFrontstageReactArtifact(
  value: unknown,
  registry: NativeReactModuleRegistry,
  bindings?: NativeReactArtifactEvaluationBindings
): Promise<NativeReactArtifactEvaluationResult> {
  const artifact = canonicalizeNativeReactComponentArtifact(value);
  if (
    !artifact ||
    !artifact.program.injectedModules.some(
      ({ source }) =>
        source === 'antd' ||
        source.startsWith('antd/es/modal') ||
        source.startsWith('antd/es/notification')
    )
  ) {
    return evaluateNativeReactComponentArtifactWithRegistry(
      value,
      registry,
      bindings
    );
  }
  let modules: Awaited<
    ReturnType<NativeReactModuleRegistry['resolveModuleMap']>
  >;
  try {
    modules = await registry.resolveModuleMap(
      artifact.program.injectedModules.map(({ source }) => source)
    );
  } catch (error) {
    return {
      ok: false,
      diagnostics: [
        {
          phase: 'runtime',
          code: 'runtime_error',
          path:
            error instanceof NativeReactModuleRegistryError
              ? error.path
              : 'moduleRegistry',
          message:
            error instanceof Error
              ? error.message
              : 'Native React module registry failed.'
        }
      ]
    };
  }
  function MountedBlock(props: Record<string, unknown>) {
    const [instance] = useState(() => {
      const runtime = createBlockModalRuntime();
      const notifications = createBlockNotificationRuntime();
      const scoped = { ...modules };
      if (scoped.antd)
        scoped.antd = {
          ...scoped.antd,
          Modal: runtime.Modal,
          notification: notifications.notification
        };
      for (const source of Object.keys(scoped)) {
        if (/^antd\/es\/notification(?:\/index)?(?:\.js)?$/.test(source))
          scoped[source] = {
            ...scoped[source],
            default: notifications.notification
          };
        if (/^antd\/es\/modal(?:\/index)?(?:\.js)?$/.test(source))
          scoped[source] = { ...scoped[source], default: runtime.Modal };
        if (/^antd\/es\/modal\/useModal(?:\/index)?(?:\.js)?$/.test(source))
          scoped[source] = {
            ...scoped[source],
            default: runtime.Modal.useModal
          };
      }
      const evaluated = evaluateNativeReactComponentArtifact(
        artifact,
        scoped,
        bindings
      );
      if (!evaluated.ok) throw new Error(evaluated.diagnostics[0]?.message);
      return {
        ...runtime,
        NotificationProvider: notifications.Provider,
        Component: evaluated.component as ComponentType<Record<string, unknown>>
      };
    });
    return (
      <instance.Provider>
        <instance.NotificationProvider>
          <instance.Component {...props} />
        </instance.NotificationProvider>
      </instance.Provider>
    );
  }
  return {
    ok: true,
    artifact,
    component: MountedBlock as Extract<
      NativeReactArtifactEvaluationResult,
      { ok: true }
    >['component'],
    diagnostics: []
  };
}
