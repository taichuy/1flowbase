import { FileManagementPanel } from '../../components/FileManagementPanel';
import { useConsoleOperationAccess } from '../../../../state/console-operation-access';

export function SettingsFilesSection({
  isRoot
}: {
  isRoot: boolean;
  permissions: string[];
}) {
  const can = useConsoleOperationAccess();
  const canViewStorages = can('file_storages.list');
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
