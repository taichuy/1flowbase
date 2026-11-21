import type { createDeclarationResources } from './resources';

type DeclarationResources = ReturnType<typeof createDeclarationResources>;
type ClosureGroup = [resourceIds: number[], aliases: string[]];

// Resource IDs are stable for the same inventory regardless of emission order.
// Only identical ordered closures are interned: no package heuristics, dropped
// imports or extra resource fetches are introduced by the wire representation.
export function compactDeclarationIndex(index: DeclarationResources) {
  const hashes = Object.keys(index.resources).sort();
  const resourceIds = new Map(hashes.map((hash, id) => [hash, id]));
  const groups = new Map<string, ClosureGroup>();
  for (const [root, closure] of Object.entries(index.roots)) {
    const ids = closure.map((hash) => {
      const id = resourceIds.get(hash);
      if (id === undefined)
        throw new Error(`Missing declaration resource '${hash}' for '${root}'`);
      return id;
    });
    const key = JSON.stringify(ids);
    const group = groups.get(key);
    if (group) group[1].push(root);
    else groups.set(key, [ids, [root]]);
  }
  return { hashes, groups: [...groups.values()] };
}

// Keep the existing NativeDeclarationIndex v1 runtime contract. A URL expression
// is supplied by Vite so ROLLUP_FILE_URL references retain base-aware semantics.
// Each hash appears once in the index data, not once per root/closure member.
export function serializeDeclarationIndex(
  index: DeclarationResources,
  resourceUrl: (hash: string) => string
) {
  const { hashes, groups } = compactDeclarationIndex(index);
  return `const hashes=${JSON.stringify(hashes)};
const urls=[${hashes.map(resourceUrl).join(',')}];
const groups=${JSON.stringify(groups)};
const roots=Object.fromEntries(groups.flatMap(([ids,aliases])=>{
  const closure=ids.map(id=>hashes[id]);
  return aliases.map(alias=>[alias,closure]);
}));
export default {version:1,roots,resources:Object.fromEntries(hashes.map((hash,id)=>[hash,urls[id]]))};`;
}
