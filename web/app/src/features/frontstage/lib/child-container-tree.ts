export type ChildContainerPresentation = 'drawer' | 'modal' | 'inline';

export interface ChildContainerNode {
  id: string;
  ownerBlockId: string;
  parentId: string | null;
  rank: string;
  presentation: ChildContainerPresentation;
  title: string;
  blockIds: string[];
}

export interface SerializedChildContainerNode {
  container_id: string;
  owner_block_id: string;
  parent_container_id: string | null;
  rank: string;
  presentation: ChildContainerPresentation;
  title: string;
  block_ids: string[];
}

export interface ChildContainerTreeDiagnostic {
  severity: 'error';
  code: string;
  path: string;
  message: string;
}

export interface ChildContainerNormalization {
  containers: ChildContainerNode[];
  diagnostics: ChildContainerTreeDiagnostic[];
}

export interface ChildContainerDraft {
  ownerBlockId: string;
  presentation: ChildContainerPresentation;
  title: string;
  blockIds: string[];
}

export type ChildContainerIdFactory = () => string;

export class ChildContainerTreeError extends Error {
  constructor(
    public readonly code: string,
    message: string
  ) {
    super(message);
    this.name = 'ChildContainerTreeError';
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function nonEmptyString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0 ? value : null;
}

function diagnostic(
  code: string,
  path: string,
  message: string
): ChildContainerTreeDiagnostic {
  return { severity: 'error', code, path, message };
}

function compareNodes(left: ChildContainerNode, right: ChildContainerNode) {
  return left.rank.localeCompare(right.rank) || left.id.localeCompare(right.id);
}

function indexNodes(containers: ChildContainerNode[]) {
  return new Map(containers.map((container) => [container.id, container]));
}

function findCycle(containers: ChildContainerNode[]): string | null {
  const byId = indexNodes(containers);

  for (const container of containers) {
    const path = new Set<string>();
    let current: ChildContainerNode | undefined = container;
    while (current) {
      if (path.has(current.id)) return current.id;
      path.add(current.id);
      current = current.parentId ? byId.get(current.parentId) : undefined;
    }
  }

  return null;
}

function descendantsOf(
  containers: ChildContainerNode[],
  containerId: string
): Set<string> {
  const descendants = new Set<string>();
  let found = true;
  while (found) {
    found = false;
    for (const container of containers) {
      if (
        container.parentId !== null &&
        (container.parentId === containerId ||
          descendants.has(container.parentId)) &&
        !descendants.has(container.id)
      ) {
        descendants.add(container.id);
        found = true;
      }
    }
  }
  return descendants;
}

function validateNodes(
  containers: ChildContainerNode[]
): ChildContainerTreeDiagnostic[] {
  const diagnostics: ChildContainerTreeDiagnostic[] = [];
  const ids = new Set<string>();
  const byId = indexNodes(containers);
  const blockOwners = new Map<string, string>();

  containers.forEach((container, index) => {
    const path = `child_containers.${index}`;
    if (ids.has(container.id)) {
      diagnostics.push(
        diagnostic(
          'duplicate_container_id',
          `${path}.container_id`,
          `Child container id "${container.id}" is duplicated.`
        )
      );
    }
    ids.add(container.id);

    if (container.parentId !== null && !byId.has(container.parentId)) {
      diagnostics.push(
        diagnostic(
          'missing_parent',
          `${path}.parent_container_id`,
          `Parent child container "${container.parentId}" does not exist.`
        )
      );
    }

    for (const blockId of container.blockIds) {
      const assignedContainerId = blockOwners.get(blockId);
      if (assignedContainerId) {
        diagnostics.push(
          diagnostic(
            'duplicate_block_assignment',
            `${path}.block_ids`,
            `Block "${blockId}" is already assigned to child container "${assignedContainerId}".`
          )
        );
      } else {
        blockOwners.set(blockId, container.id);
      }
    }
  });

  const cycleId = findCycle(containers);
  if (cycleId) {
    diagnostics.push(
      diagnostic(
        'cycle',
        'child_containers',
        `Child container ancestry contains a cycle at "${cycleId}".`
      )
    );
  }

  if (!cycleId) {
    for (const container of containers) {
      const subtreeIds = descendantsOf(containers, container.id);
      subtreeIds.add(container.id);
      const containingNode = containers.find(
        (candidate) =>
          subtreeIds.has(candidate.id) &&
          candidate.blockIds.includes(container.ownerBlockId)
      );
      if (containingNode) {
        diagnostics.push(
          diagnostic(
            'owner_self_containment',
            `child_containers.${containers.indexOf(containingNode)}.block_ids`,
            `Owner block "${container.ownerBlockId}" cannot be assigned to its own child container subtree.`
          )
        );
      }
    }
  }

  return diagnostics;
}

function canonicalOrder(
  containers: ChildContainerNode[]
): ChildContainerNode[] {
  const children = new Map<string | null, ChildContainerNode[]>();
  for (const container of containers) {
    const siblings = children.get(container.parentId) ?? [];
    siblings.push(container);
    children.set(container.parentId, siblings);
  }
  for (const siblings of children.values()) siblings.sort(compareNodes);

  const ordered: ChildContainerNode[] = [];
  const visit = (parentId: string | null) => {
    for (const container of children.get(parentId) ?? []) {
      ordered.push(container);
      visit(container.id);
    }
  };
  visit(null);
  return ordered;
}

function parseSerializedNode(
  value: unknown,
  index: number,
  diagnostics: ChildContainerTreeDiagnostic[]
): ChildContainerNode | null {
  const path = `child_containers.${index}`;
  if (!isRecord(value)) {
    diagnostics.push(
      diagnostic(
        'invalid_container',
        path,
        'Child container must be an object.'
      )
    );
    return null;
  }

  const id = nonEmptyString(value.container_id);
  const ownerBlockId = nonEmptyString(value.owner_block_id);
  const rank = nonEmptyString(value.rank);
  const parentValue = value.parent_container_id;
  const parentId = parentValue === null ? null : nonEmptyString(parentValue);
  const presentation = value.presentation;
  const title = typeof value.title === 'string' ? value.title : null;
  const blockIds = Array.isArray(value.block_ids)
    ? value.block_ids.map(nonEmptyString)
    : null;

  if (!id)
    diagnostics.push(
      diagnostic(
        'invalid_container_id',
        `${path}.container_id`,
        'container_id is required.'
      )
    );
  if (!ownerBlockId)
    diagnostics.push(
      diagnostic(
        'invalid_owner_block_id',
        `${path}.owner_block_id`,
        'owner_block_id is required.'
      )
    );
  if (parentValue !== null && !parentId)
    diagnostics.push(
      diagnostic(
        'invalid_parent_id',
        `${path}.parent_container_id`,
        'parent_container_id must be a string or null.'
      )
    );
  if (!rank)
    diagnostics.push(
      diagnostic('invalid_rank', `${path}.rank`, 'rank is required.')
    );
  if (
    presentation !== 'drawer' &&
    presentation !== 'modal' &&
    presentation !== 'inline'
  ) {
    diagnostics.push(
      diagnostic(
        'invalid_presentation',
        `${path}.presentation`,
        'presentation must be drawer, modal, or inline.'
      )
    );
  }
  if (title === null)
    diagnostics.push(
      diagnostic('invalid_title', `${path}.title`, 'title must be a string.')
    );
  if (!blockIds || blockIds.some((blockId) => !blockId)) {
    diagnostics.push(
      diagnostic(
        'invalid_block_ids',
        `${path}.block_ids`,
        'block_ids must contain non-empty strings.'
      )
    );
  }

  if (
    !id ||
    !ownerBlockId ||
    (parentValue !== null && !parentId) ||
    !rank ||
    (presentation !== 'drawer' &&
      presentation !== 'modal' &&
      presentation !== 'inline') ||
    title === null ||
    !blockIds ||
    blockIds.some((blockId) => !blockId)
  ) {
    return null;
  }

  return {
    id,
    ownerBlockId,
    parentId,
    rank,
    presentation,
    title,
    blockIds: blockIds as string[]
  };
}

export function normalizeChildContainerTree(
  value: unknown
): ChildContainerNormalization {
  if (value === undefined || value === null) {
    return { containers: [], diagnostics: [] };
  }
  if (!Array.isArray(value)) {
    return {
      containers: [],
      diagnostics: [
        diagnostic(
          'invalid_child_containers',
          'child_containers',
          'child_containers must be an array.'
        )
      ]
    };
  }

  const diagnostics: ChildContainerTreeDiagnostic[] = [];
  const containers = value.flatMap((item, index) => {
    const container = parseSerializedNode(item, index, diagnostics);
    return container ? [container] : [];
  });
  diagnostics.push(...validateNodes(containers));

  return diagnostics.length > 0
    ? { containers: [], diagnostics }
    : { containers: canonicalOrder(containers), diagnostics: [] };
}

export function serializeChildContainerTree(
  containers: ChildContainerNode[]
): SerializedChildContainerNode[] {
  assertValidTree(containers);
  return canonicalOrder(containers).map((container) => ({
    container_id: container.id,
    owner_block_id: container.ownerBlockId,
    parent_container_id: container.parentId,
    rank: container.rank,
    presentation: container.presentation,
    title: container.title,
    block_ids: [...container.blockIds]
  }));
}

function assertValidTree(containers: ChildContainerNode[]) {
  const first = validateNodes(containers)[0];
  if (first) throw new ChildContainerTreeError(first.code, first.message);
}

function defaultIdFactory(): string {
  if (
    typeof crypto !== 'undefined' &&
    typeof crypto.randomUUID === 'function'
  ) {
    return crypto.randomUUID();
  }

  return `00000000-0000-4000-8000-${Math.random().toString(16).slice(2, 14).padStart(12, '0')}`;
}

function nextRank(index: number) {
  return String((index + 1) * 1000).padStart(6, '0');
}

function reorderSiblings(
  containers: ChildContainerNode[],
  parentId: string | null,
  orderedSiblingIds: string[]
): ChildContainerNode[] {
  const ranks = new Map(
    orderedSiblingIds.map((id, index) => [id, nextRank(index)])
  );
  return containers.map((container) =>
    container.parentId === parentId
      ? { ...container, rank: ranks.get(container.id) ?? container.rank }
      : container
  );
}

function siblingIds(containers: ChildContainerNode[], parentId: string | null) {
  return containers
    .filter((container) => container.parentId === parentId)
    .sort(compareNodes)
    .map((container) => container.id);
}

function createContainer(
  containers: ChildContainerNode[],
  parentId: string | null,
  draft: ChildContainerDraft,
  idFactory: ChildContainerIdFactory
): ChildContainerNode {
  const id = nonEmptyString(idFactory());
  if (!id) {
    throw new ChildContainerTreeError(
      'invalid_container_id',
      'The child container id factory returned an invalid id.'
    );
  }
  if (containers.some((container) => container.id === id)) {
    throw new ChildContainerTreeError(
      'duplicate_container_id',
      `Child container id "${id}" already exists.`
    );
  }
  return {
    id,
    ownerBlockId: draft.ownerBlockId,
    parentId,
    rank: nextRank(siblingIds(containers, parentId).length),
    presentation: draft.presentation,
    title: draft.title,
    blockIds: [...draft.blockIds]
  };
}

export function addChildContainer(
  containers: ChildContainerNode[],
  parentId: string | null,
  draft: ChildContainerDraft,
  idFactory: ChildContainerIdFactory = defaultIdFactory
): ChildContainerNode[] {
  assertValidTree(containers);
  if (parentId !== null && !containers.some(({ id }) => id === parentId)) {
    throw new ChildContainerTreeError(
      'missing_parent',
      `Parent child container "${parentId}" does not exist.`
    );
  }
  const next = [
    ...containers,
    createContainer(containers, parentId, draft, idFactory)
  ];
  assertValidTree(next);
  return canonicalOrder(next);
}

export function addSiblingChildContainer(
  containers: ChildContainerNode[],
  siblingId: string,
  draft: ChildContainerDraft,
  idFactory: ChildContainerIdFactory = defaultIdFactory
): ChildContainerNode[] {
  assertValidTree(containers);
  const sibling = containers.find(({ id }) => id === siblingId);
  if (!sibling) {
    throw new ChildContainerTreeError(
      'missing_container',
      `Sibling child container "${siblingId}" does not exist.`
    );
  }
  const created = createContainer(
    containers,
    sibling.parentId,
    draft,
    idFactory
  );
  const ids = siblingIds(containers, sibling.parentId);
  ids.splice(ids.indexOf(siblingId) + 1, 0, created.id);
  const next = reorderSiblings([...containers, created], sibling.parentId, ids);
  assertValidTree(next);
  return canonicalOrder(next);
}

export function resolveChildContainerPath(
  containers: ChildContainerNode[],
  containerId: string
): ChildContainerNode[] | null {
  assertValidTree(containers);
  const byId = indexNodes(containers);
  let current = byId.get(containerId);
  if (!current) return null;
  const path: ChildContainerNode[] = [];
  while (current) {
    path.unshift(current);
    current = current.parentId ? byId.get(current.parentId) : undefined;
  }
  return path;
}

export function moveChildContainer(
  containers: ChildContainerNode[],
  containerId: string,
  parentId: string | null,
  index: number
): ChildContainerNode[] {
  assertValidTree(containers);
  const target = containers.find(({ id }) => id === containerId);
  if (!target)
    throw new ChildContainerTreeError(
      'missing_container',
      `Child container "${containerId}" does not exist.`
    );
  if (parentId !== null && !containers.some(({ id }) => id === parentId)) {
    throw new ChildContainerTreeError(
      'missing_parent',
      `Parent child container "${parentId}" does not exist.`
    );
  }
  if (
    parentId === containerId ||
    (parentId !== null && descendantsOf(containers, containerId).has(parentId))
  ) {
    throw new ChildContainerTreeError(
      'cycle',
      'A child container cannot move into its own subtree.'
    );
  }

  let next = containers.map((container) =>
    container.id === containerId ? { ...container, parentId } : container
  );
  const oldSiblingIds = siblingIds(containers, target.parentId).filter(
    (id) => id !== containerId
  );
  next = reorderSiblings(next, target.parentId, oldSiblingIds);
  const newSiblingIds = siblingIds(next, parentId).filter(
    (id) => id !== containerId
  );
  newSiblingIds.splice(
    Math.max(0, Math.min(index, newSiblingIds.length)),
    0,
    containerId
  );
  next = reorderSiblings(next, parentId, newSiblingIds);
  assertValidTree(next);
  return canonicalOrder(next);
}

export function reorderChildContainer(
  containers: ChildContainerNode[],
  containerId: string,
  index: number
): ChildContainerNode[] {
  assertValidTree(containers);
  const target = containers.find(({ id }) => id === containerId);
  if (!target)
    throw new ChildContainerTreeError(
      'missing_container',
      `Child container "${containerId}" does not exist.`
    );
  const ids = siblingIds(containers, target.parentId).filter(
    (id) => id !== containerId
  );
  ids.splice(Math.max(0, Math.min(index, ids.length)), 0, containerId);
  return canonicalOrder(reorderSiblings(containers, target.parentId, ids));
}

export function deleteChildContainer(
  containers: ChildContainerNode[],
  containerId: string,
  references: { targetContainerIds: readonly string[] }
): ChildContainerNode[] {
  assertValidTree(containers);
  const target = containers.find(({ id }) => id === containerId);
  if (!target)
    throw new ChildContainerTreeError(
      'missing_container',
      `Child container "${containerId}" does not exist.`
    );
  if (references.targetContainerIds.includes(containerId)) {
    throw new ChildContainerTreeError(
      'container_referenced',
      `Child container "${containerId}" is still referenced by a block target.`
    );
  }
  if (containers.some(({ parentId }) => parentId === containerId)) {
    throw new ChildContainerTreeError(
      'container_has_children',
      'Only leaf child containers can be deleted.'
    );
  }
  if (target.blockIds.length > 0) {
    throw new ChildContainerTreeError(
      'container_not_empty',
      'Only empty child containers can be deleted.'
    );
  }
  return canonicalOrder(containers.filter(({ id }) => id !== containerId));
}
