import { Menu, Tooltip } from 'antd';
import { useEffect } from 'react';

import { useAuthStore } from '../state/auth-store';
import { useFrontstageDesignModeStore } from '../state/frontstage-design-mode-store';
import { i18nText } from '../shared/i18n/text';

const DESIGN_MODE_PERMISSION = 'frontstage.page.design';


function FrontstageDesignModeActionBase() {
  const sessionStatus = useAuthStore((state) => state.sessionStatus);
  const actor = useAuthStore((state) => state.actor);
  const me = useAuthStore((state) => state.me);
  const isDesignMode = useFrontstageDesignModeStore(
    (state) => state.isDesignMode
  );
  const setDesignMode = useFrontstageDesignModeStore(
    (state) => state.setDesignMode
  );
  const toggleDesignMode = useFrontstageDesignModeStore(
    (state) => state.toggleDesignMode
  );

  const canUseDesignMode =
    actor?.effective_display_role === 'root' ||
    Boolean(me?.permissions.includes(DESIGN_MODE_PERMISSION));
  const hasResolvedDesignModePermission = sessionStatus !== 'unknown';

  // Exit design mode only when permission is lost; route changes should not
  // overwrite the browser-persisted preference.
  useEffect(() => {
    if (hasResolvedDesignModePermission && !canUseDesignMode && isDesignMode) {
      setDesignMode(false);
    }
  }, [
    canUseDesignMode,
    hasResolvedDesignModePermission,
    isDesignMode,
    setDesignMode
  ]);

  // Support reading design mode from URL query parameters (for non-SPA transition/initial page load)
  useEffect(() => {
    if (canUseDesignMode) {
      const params = new URLSearchParams(window.location.search);
      if (params.get('design') === 'true') {
        setDesignMode(true);
        // Clean up the URL search params so it doesn't stay there on subsequent actions
        const newUrl = window.location.pathname;
        window.history.replaceState({}, '', newUrl);
      }
    }
  }, [canUseDesignMode, setDesignMode]);

  if (!canUseDesignMode) {
    return null;
  }

  const label = isDesignMode ? i18nText("appShell", "auto.exit_design_mode") : i18nText("appShell", "auto.enter_design_mode");

  const handleClick = () => {
    toggleDesignMode();
  };

  const selectedKeys = isDesignMode ? ['design-mode'] : [];

  return (
    <Tooltip title={label}>
      <Menu
        className="app-shell-design-menu"
        mode="horizontal"
        selectable={false}
        selectedKeys={selectedKeys}
        items={[
          {
            key: 'design-mode',
            className: isDesignMode
              ? 'app-shell-design-mode-button ant-menu-item-selected'
              : 'app-shell-design-mode-button',
            label: (
              <span
                className="app-shell-design-block"
                aria-label={label}
                role="button"
                aria-pressed={isDesignMode}
              >
                UI
              </span>
            )
          }
        ]}
        onClick={handleClick}
        disabledOverflow
      />
    </Tooltip>
  );
}

export function FrontstageDesignModeAction() {
  return <FrontstageDesignModeActionBase />;
}
