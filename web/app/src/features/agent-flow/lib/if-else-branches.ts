import type {
  FlowBinding,
  FlowConditionExpressionDocument,
  FlowConditionGroupDocument,
  FlowConditionRuleDocument,
  FlowConditionValue,
  FlowNodeDocument,
  IfElseBranchDocument
} from '@1flowbase/flow-schema';

const IF_ELSE_BRANCH_BINDING_KEY = 'branches';
const IF_ELSE_IF_HANDLE_PREFIX = 'else-if-';

const DEFAULT_IF_ELSE_BRANCHES: IfElseBranchDocument[] = [
  {
    id: 'if',
    kind: 'if',
    title: 'If',
    sourceHandle: 'if',
    condition: { operator: 'and', conditions: [] }
  },
  {
    id: 'else',
    kind: 'else',
    title: 'Else',
    sourceHandle: 'else'
  }
];

export function createDefaultIfElseBranches(): IfElseBranchDocument[] {
  return structuredClone(DEFAULT_IF_ELSE_BRANCHES);
}

export function createEmptyConditionGroup(): FlowConditionGroupDocument {
  return { operator: 'and', conditions: [] };
}

export function isConditionGroup(
  expression: FlowConditionExpressionDocument
): expression is FlowConditionGroupDocument {
  return Array.isArray(
    (expression as FlowConditionGroupDocument | null)?.conditions
  );
}

export function isConditionRule(
  expression: FlowConditionExpressionDocument
): expression is FlowConditionRuleDocument {
  return Array.isArray((expression as FlowConditionRuleDocument | null)?.left);
}

export function createElseIfBranch(
  branches: IfElseBranchDocument[]
): IfElseBranchDocument {
  const usedHandles = new Set(branches.map((branch) => branch.sourceHandle));
  let nextIndex = 1;

  while (usedHandles.has(`${IF_ELSE_IF_HANDLE_PREFIX}${nextIndex}`)) {
    nextIndex += 1;
  }

  return {
    id: `${IF_ELSE_IF_HANDLE_PREFIX}${nextIndex}`,
    kind: 'else_if',
    title: `Else If ${nextIndex}`,
    sourceHandle: `${IF_ELSE_IF_HANDLE_PREFIX}${nextIndex}`,
    condition: createEmptyConditionGroup()
  };
}

export function getIfElseBranches(binding?: FlowBinding) {
  if (binding?.kind !== 'if_else_branches') {
    return null;
  }

  return binding.value.branches;
}

export function getIfElseBranchesFromBindings(
  bindings: Record<string, FlowBinding>
) {
  return getIfElseBranches(bindings[IF_ELSE_BRANCH_BINDING_KEY]);
}

export function normalizeIfElseBranches(
  branches: IfElseBranchDocument[] | null | undefined
): IfElseBranchDocument[] {
  if (!branches || branches.length === 0) {
    return createDefaultIfElseBranches();
  }

  const nonElseBranches = branches.filter((branch) => branch.kind !== 'else');
  const elseBranch =
    branches.find((branch) => branch.kind === 'else') ??
    createDefaultIfElseBranches().find((branch) => branch.kind === 'else');

  return [
    ...nonElseBranches.map((branch) =>
      branch.kind === 'else'
        ? branch
        : {
            ...branch,
            condition: branch.condition ?? createEmptyConditionGroup()
          }
    ),
    elseBranch as IfElseBranchDocument
  ];
}

export function getDefaultIfElseSourceHandle(
  node: FlowNodeDocument
): string | null {
  if (node.type !== 'if_else') {
    return null;
  }

  return (
    normalizeIfElseBranches(
      getIfElseBranchesFromBindings(node.bindings)
    )[0]?.sourceHandle ?? null
  );
}

function conditionValueSelectors(value: FlowConditionValue | undefined) {
  return value?.kind === 'selector' ? [value.selector] : [];
}

export function collectConditionSelectors(
  group: FlowConditionGroupDocument
): string[][] {
  return group.conditions.flatMap((condition) => {
    if (isConditionGroup(condition)) {
      return collectConditionSelectors(condition);
    }

    if (!isConditionRule(condition)) {
      return [];
    }

    return [condition.left, ...conditionValueSelectors(condition.right)];
  });
}

export function collectIfElseBranchSelectors(
  branches: IfElseBranchDocument[]
): string[][] {
  return branches.flatMap((branch) =>
    branch.condition ? collectConditionSelectors(branch.condition) : []
  );
}
