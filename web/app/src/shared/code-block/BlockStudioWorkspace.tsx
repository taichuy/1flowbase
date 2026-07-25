import {
  ApiOutlined,
  AppstoreOutlined,
  CodeOutlined,
  DatabaseOutlined,
  PlayCircleOutlined,
  SettingOutlined
} from '@ant-design/icons';
import { Button, Tooltip } from 'antd';
import type { CSSProperties, ReactNode } from 'react';
import { useEffect, useRef, useState } from 'react';

import { i18nText } from '../i18n/text';

export type BlockStudioSection =
  | 'code'
  | 'interfaces'
  | 'variables'
  | 'components'
  | 'configuration'
  | 'run';

const STUDIO_SECTIONS: Array<{
  key: BlockStudioSection;
  label: string;
  icon: ReactNode;
}> = [
  { key: 'code', label: i18nText('frontstage', 'auto.code'), icon: <CodeOutlined /> },
  { key: 'interfaces', label: i18nText('frontstage', 'auto.interfaces'), icon: <ApiOutlined /> },
  { key: 'variables', label: i18nText('frontstage', 'auto.variables'), icon: <DatabaseOutlined /> },
  { key: 'components', label: i18nText('frontstage', 'auto.components'), icon: <AppstoreOutlined /> },
  { key: 'configuration', label: i18nText('frontstage', 'auto.configuration'), icon: <SettingOutlined /> },
  { key: 'run', label: i18nText('frontstage', 'auto.preview'), icon: <PlayCircleOutlined /> }
];

const DEFAULT_RESOURCE_PANEL_WIDTH = 320;
const MIN_RESOURCE_PANEL_WIDTH = 260;
const MIN_EDITOR_PANEL_WIDTH = 320;
const STUDIO_RAIL_WIDTH = 44;
const STUDIO_SPLITTER_WIDTH = 8;

export function BlockStudioWorkspace({
  activeSection,
  editor,
  onSectionChange,
  renderResource,
  windowWidth
}: {
  activeSection: BlockStudioSection;
  editor: ReactNode;
  onSectionChange: (section: BlockStudioSection) => void;
  renderResource: (section: Exclude<BlockStudioSection, 'code'>) => ReactNode;
  windowWidth: number;
}) {
  const [resourcePanelWidth, setResourcePanelWidth] = useState(
    DEFAULT_RESOURCE_PANEL_WIDTH
  );
  const liveResourcePanelWidthRef = useRef(DEFAULT_RESOURCE_PANEL_WIDTH);
  const resourcePanelDragStartRef = useRef<{
    pointerX: number;
    width: number;
  } | null>(null);
  const maxResourcePanelWidth = Math.max(
    MIN_RESOURCE_PANEL_WIDTH,
    windowWidth -
      MIN_EDITOR_PANEL_WIDTH -
      STUDIO_RAIL_WIDTH -
      STUDIO_SPLITTER_WIDTH
  );

  useEffect(() => {
    liveResourcePanelWidthRef.current = resourcePanelWidth;
  }, [resourcePanelWidth]);

  useEffect(() => {
    const handleMouseMove = (event: MouseEvent) => {
      const dragStart = resourcePanelDragStartRef.current;
      if (!dragStart) return;
      setResourcePanelWidth(
        clampResourcePanelWidth(
          dragStart.width + dragStart.pointerX - event.clientX,
          maxResourcePanelWidth
        )
      );
    };
    const handleMouseUp = () => {
      resourcePanelDragStartRef.current = null;
      document.body.classList.remove('frontstage-jsx-studio--resizing-panel');
    };
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.body.classList.remove('frontstage-jsx-studio--resizing-panel');
    };
  }, [maxResourcePanelWidth]);

  useEffect(() => {
    setResourcePanelWidth((current) =>
      clampResourcePanelWidth(current, maxResourcePanelWidth)
    );
  }, [maxResourcePanelWidth]);

  return (
    <div
      className={[
        'frontstage-jsx-studio__workspace',
        activeSection === 'code'
          ? 'frontstage-jsx-studio__workspace--code-only'
          : null
      ]
        .filter(Boolean)
        .join(' ')}
      style={
        { '--resource-panel-width': `${resourcePanelWidth}px` } as CSSProperties
      }
    >
      <nav
        aria-label={i18nText('frontstage', 'auto.jsx_studio_resources')}
        className="frontstage-jsx-studio__rail"
      >
        {STUDIO_SECTIONS.map((section) => (
          <Tooltip key={section.key} title={section.label} placement="left">
            <Button
              aria-label={section.label}
              className={[
                'frontstage-jsx-studio__rail-button',
                activeSection === section.key
                  ? 'frontstage-jsx-studio__rail-button--active'
                  : null
              ]
                .filter(Boolean)
                .join(' ')}
              icon={section.icon}
              type="text"
              onClick={() => onSectionChange(section.key)}
            />
          </Tooltip>
        ))}
      </nav>

      <aside
        className="frontstage-jsx-studio__resource-panel"
        style={{ display: activeSection === 'code' ? 'none' : undefined }}
      >
        {activeSection === 'code' ? null : renderResource(activeSection)}
      </aside>

      {activeSection !== 'code' ? (
        <div
          aria-label={i18nText('frontstage', 'auto.resize_resource_panel')}
          aria-orientation="vertical"
          aria-valuemax={maxResourcePanelWidth}
          aria-valuemin={MIN_RESOURCE_PANEL_WIDTH}
          aria-valuenow={resourcePanelWidth}
          className="frontstage-jsx-studio__panel-resize-handle"
          role="separator"
          tabIndex={0}
          onKeyDown={(event) => {
            if (event.key === 'ArrowLeft') {
              event.preventDefault();
              setResourcePanelWidth((current) =>
                clampResourcePanelWidth(current + 40, maxResourcePanelWidth)
              );
            } else if (event.key === 'ArrowRight') {
              event.preventDefault();
              setResourcePanelWidth((current) =>
                clampResourcePanelWidth(current - 40, maxResourcePanelWidth)
              );
            } else if (event.key === 'Home') {
              event.preventDefault();
              setResourcePanelWidth(MIN_RESOURCE_PANEL_WIDTH);
            } else if (event.key === 'End') {
              event.preventDefault();
              setResourcePanelWidth(maxResourcePanelWidth);
            }
          }}
          onMouseDown={(event) => {
            event.preventDefault();
            resourcePanelDragStartRef.current = {
              pointerX: event.clientX,
              width: liveResourcePanelWidthRef.current
            };
            document.body.classList.add(
              'frontstage-jsx-studio--resizing-panel'
            );
          }}
        />
      ) : null}

      {editor}
    </div>
  );
}

function clampResourcePanelWidth(width: number, maxWidth: number) {
  return Math.min(maxWidth, Math.max(MIN_RESOURCE_PANEL_WIDTH, width));
}
