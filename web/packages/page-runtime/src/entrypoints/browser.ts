// Browser execution must not pull the compiler or legacy source evaluator into
// the main thread. Keep type-only compatibility exports erased at runtime.
export {
  NATIVE_REACT_COMPONENT_ARTIFACT_FORMAT,
  NATIVE_REACT_COMPONENT_ARTIFACT_VERSION,
  NATIVE_REACT_COMPILER_ABI,
  NATIVE_REACT_RUNTIME_ABI,
  canonicalizeNativeReactComponentArtifact,
  createNativeReactComponentArtifactIdentity,
  nativeReactComponentArtifactMatchesIdentity,
  sha256Bytes,
  sha256Text,
  type JsonValue,
  type NativeReactComponentArtifactIdentity,
  type NativeReactCompileDiagnostic,
  type NativeReactComponentArtifact,
  type NativeReactComponentCompileResult
} from '../native-react-compiler/artifact-contract';
export * from '../native-react-compiler/artifact-evaluator';
export * from '../native-react-compiler/module-registry/contracts';
export * from '../native-react-compiler/module-registry/loader';
export * from '../native-react-compiler/source-contract';
export * from '../native-block-context/capabilities';
export * from '../native-block-context/effects';
export * from '../native-block-context/external-assets';
export * from '../native-trusted-block-host';
export * from '../native-trusted-block-portal';
export * from '../native-trusted-block/runtime-error';
export type * from '../native-trusted-block/source-evaluator-types';
export type { NativeTrustedBlockPreparePlan } from '../native-trusted-block-manifest';
export type {
  NativeReactCompilerRequest,
  NativeReactCompilerResponse
} from '../native-react-compiler/worker-protocol';
