import manifest from './scenario-manifest.json';
import { renderers } from './renderers';
import type {
  StyleBoundaryManifestScene,
  StyleBoundaryRuntimeScene
} from './types';

function getSceneManifest(): StyleBoundaryManifestScene[] {
  return manifest as StyleBoundaryManifestScene[];
}

export function getSceneIdsForFiles(files: string[]): string[] {
  const fileSet = new Set(files);

  return getSceneManifest().flatMap((scene) =>
    scene.impactFiles.some((file) => fileSet.has(file)) ? [scene.id] : []
  );
}

export function getRuntimeScene(sceneId: string): StyleBoundaryRuntimeScene {
  const scene = getSceneManifest().find((entry) => entry.id === sceneId);

  if (!scene || !renderers[scene.id]) {
    throw new Error(`Unknown style boundary scene: ${sceneId}`);
  }

  return {
    ...scene,
    render: renderers[scene.id]
  };
}
