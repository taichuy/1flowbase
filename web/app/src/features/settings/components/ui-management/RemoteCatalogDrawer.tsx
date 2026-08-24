import { useTranslation } from 'react-i18next';

import { ResizableDrawer } from '../../../../shared/ui/resizable-drawer/ResizableDrawer';
import { UiComponentCatalogContent } from './UiComponentCatalogContent';

export function RemoteCatalogDrawer({
  open,
  canManage,
  onClose
}: {
  open: boolean;
  canManage: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation('settingsUiManagement');

  return (
    <ResizableDrawer
      ariaLabel={t('remote_catalog')}
      defaultWidth={840}
      open={open}
      resizeLabel={t('resize_catalog_drawer')}
      title={t('remote_catalog')}
      onClose={onClose}
    >
      {open ? <UiComponentCatalogContent canManage={canManage} /> : null}
    </ResizableDrawer>
  );
}
