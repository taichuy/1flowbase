import artifact from 'virtual:1flowbase-default-login-artifact';
import {
  createNativeReactComponentArtifactIdentity,
  nativeReactComponentArtifactMatchesIdentity,
  sha256Text,
  type NativeReactModuleDefinition
} from '@1flowbase/page-runtime/browser';

/** Only memoizes compilation; evaluation, registry loading and auth capabilities
 * still belong to the live block instance. The fallback is the normal compiler. */
export function matchingDefaultLoginArtifact(
  source: string,
  definitions: readonly NativeReactModuleDefinition[]
) {
  if (
    !artifact ||
    !nativeReactComponentArtifactMatchesIdentity(
      artifact,
      createNativeReactComponentArtifactIdentity({
        sourceSha256: sha256Text(source)
      })
    )
  )
    return null;
  for (const { source: moduleSource, bindings } of artifact.program
    .injectedModules) {
    const definition = definitions.find(
      (entry) => entry.module_source === moduleSource
    );
    if (
      !definition ||
      !bindings.every(
        (binding) =>
          binding.kind !== 'namespace' &&
          (definition.exports.includes('*') ||
            definition.exports.includes(
              binding.kind === 'default' ? 'default' : binding.imported
            ))
      )
    )
      return null;
  }
  return artifact;
}
