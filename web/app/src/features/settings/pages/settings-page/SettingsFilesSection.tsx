import { FileManagementPanel } from '../../components/FileManagementPanel';
import { useConsoleOperationAccess, useProjectedConsoleOperationAccess } from '../../../../state/console-operation-access';

export function SettingsFilesSection({
  isRoot
}: {
  isRoot: boolean;
}) {
  const can = useConsoleOperationAccess();
  const canUseHostOperation = useProjectedConsoleOperationAccess();
  const canViewStorages = canUseHostOperation('file_storages.list');
  const canViewTables = can('file_tables.list');
  const canCreateTables = can('file_tables.create');

  return (
    <FileManagementPanel
      isRoot={isRoot}
      canViewStorages={canViewStorages}
      canViewTables={canViewTables}
      canCreateTables={canCreateTables}
    />
  );
}
