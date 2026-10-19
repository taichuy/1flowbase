import type { SettingsConsolePolicyCatalog } from '../../api/identity-access/permissions';
import type { SettingsRoleConsolePolicy } from '../../api/identity-access/roles';

export type ConsolePolicyCatalogGroup = SettingsConsolePolicyCatalog['groups'][number];
type ConsolePolicyCatalogOperation =
  ConsolePolicyCatalogGroup['operations'][number];
export type ConsolePolicyGroup = SettingsRoleConsolePolicy['groups'][number];
type ConsolePolicyOperation = ConsolePolicyGroup['operations'][number];

export type ConsolePolicyRowOperation = Extract<
  ConsolePolicyOperation,
  { kind: 'row' }
>;

export function disabledConsolePolicyGroup(
  catalogGroup: ConsolePolicyCatalogGroup
): ConsolePolicyGroup {
  return {
    kind: catalogGroup.kind,
    group_id: catalogGroup.group_id,
    enabled: false,
    strategy: 'full',
    operations: []
  };
}

function policyOperationFromFullProfile(
  operation: ConsolePolicyCatalogOperation
): ConsolePolicyOperation {
  if (operation.full_profile.kind === 'simple') {
    return {
      operation_id: operation.operation_id,
      kind: 'simple',
      enabled: operation.full_profile.enabled
    };
  }

  return {
    operation_id: operation.operation_id,
    kind: 'row',
    scope: operation.full_profile.scope
  };
}

function disabledConsolePolicyOperation(
  operation: ConsolePolicyCatalogOperation
): ConsolePolicyOperation {
  if (operation.full_profile.kind === 'simple') {
    return {
      operation_id: operation.operation_id,
      kind: 'simple',
      enabled: false
    };
  }

  return {
    operation_id: operation.operation_id,
    kind: 'row',
    scope: 'disabled'
  };
}

function matchesCatalogOperationKind(
  policyOperation: ConsolePolicyOperation,
  catalogOperation: ConsolePolicyCatalogOperation
) {
  return policyOperation.kind === catalogOperation.full_profile.kind;
}

function detailPolicyOperation(
  policyGroup: ConsolePolicyGroup,
  catalogOperation: ConsolePolicyCatalogOperation
): ConsolePolicyOperation {
  if (policyGroup.strategy === 'full') {
    return policyOperationFromFullProfile(catalogOperation);
  }

  const storedOperation = policyGroup.operations.find(
    (operation) => operation.operation_id === catalogOperation.operation_id
  );
  if (
    storedOperation &&
    matchesCatalogOperationKind(storedOperation, catalogOperation)
  ) {
    return storedOperation;
  }

  return disabledConsolePolicyOperation(catalogOperation);
}

export function materializeDetailPolicyOperations(
  policyGroup: ConsolePolicyGroup,
  catalogGroup: ConsolePolicyCatalogGroup
) {
  return catalogGroup.operations.map((catalogOperation) =>
    detailPolicyOperation(policyGroup, catalogOperation)
  );
}

export function sameConsolePolicyGroup(
  left: Pick<ConsolePolicyGroup, 'kind' | 'group_id'>,
  right: Pick<ConsolePolicyGroup, 'kind' | 'group_id'>
) {
  return left.kind === right.kind && left.group_id === right.group_id;
}

export function replaceConsolePolicyGroup(
  groups: ConsolePolicyGroup[],
  nextGroup: ConsolePolicyGroup
) {
  const existingIndex = groups.findIndex((group) =>
    sameConsolePolicyGroup(group, nextGroup)
  );
  if (existingIndex === -1) {
    return [...groups, nextGroup];
  }

  return groups.map((group, index) =>
    index === existingIndex ? nextGroup : group
  );
}

