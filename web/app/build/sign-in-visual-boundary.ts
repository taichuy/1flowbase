import { signInPreloads, signInPreloadRoots } from './sign-in-resource-hints';

// Check emitted chunks, not source-level lazy() calls. A shared vendor can pull
// a dynamic feature back onto the visual path. File names are content-hashed.
type Chunk = {
  type: 'chunk';
  modules: Record<string, unknown>;
  imports: readonly string[];
};
type Bundle = Record<string, Chunk | { type: 'asset' }>;

const page = '/features/auth/pages/SignInPage.tsx';
const hero = '/features/auth/components/HeroAnimation.tsx';
const deferred = [
  '/features/auth/components/PublicAuthBlock.tsx',
  '/features/auth/pages/mcp-authorization/McpAuthorizePage.tsx'
];

export function assertSignInVisualBoundary(bundle: Bundle): void {
  const owners = new Map<string, string[]>();
  for (const [file, chunk] of Object.entries(bundle)) {
    if (chunk.type === 'chunk') {
      owners.set(
        file,
        Object.keys(chunk.modules).map((id) => id.replaceAll('\\', '/'))
      );
    }
  }
  const rootsFor = (suffixes: readonly string[]) =>
    [...owners]
      .filter(([, ids]) =>
        ids.some((id) => suffixes.some((s) => id.endsWith(s)))
      )
      .map(([file]) => file);
  const pageRoots = rootsFor([page]);
  if (!pageRoots.length) return; // Worker-only outputs do not contain a page.
  const chunks = new Map(
    Object.entries(bundle).flatMap(([name, value]) =>
      value.type === 'chunk' ? [[name, value] as const] : []
    )
  );
  for (const file of signInPreloads(chunks, rootsFor(['/src/bootstrap.ts']))) {
    for (const id of owners.get(file)!) {
      if (/\/node_modules\/(?:react|antd)\//u.test(id))
        throw new Error(`Sign-in discovery waits for ${id} in ${file}`);
    }
  }
  const styleRoots = rootsFor(['/node_modules/antd-style/es/index.js']);
  for (const file of signInPreloads(chunks, styleRoots)) {
    for (const id of owners.get(file)!) {
      if (/\/antd\/es\/(?:table|tour|list)\//u.test(id))
        throw new Error(
          `Sign-in style provider eagerly includes ${id} in ${file}`
        );
    }
  }
  const pageClosure = signInPreloads(chunks, pageRoots);
  if (
    !pageClosure.some((file) =>
      owners.get(file)!.some((id) => id.endsWith(hero))
    )
  )
    throw new Error(
      'Sign-in visual path is missing the original HeroAnimation'
    );
  // Every graph the sign-in HTML speculatively fetches contributes to the cold
  // visit, even if the page component itself doesn't import it.
  const visual = signInPreloads(chunks, [
    ...pageRoots,
    ...rootsFor(signInPreloadRoots)
  ]);
  for (const file of visual) {
    for (const id of owners.get(file)!) {
      if (deferred.some((owner) => id.endsWith(owner)))
        throw new Error(
          `Sign-in visual path eagerly includes ${id} in ${file}`
        );
    }
  }
}
