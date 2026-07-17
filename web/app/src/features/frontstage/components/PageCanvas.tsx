import { Alert, Button, Empty, Space, Typography } from 'antd';
import type { CSSProperties, FC } from 'react';
import { useEffect, useMemo, useRef, useState } from 'react';
import {
  Responsive,
  WidthProvider,
  type Layout
} from 'react-grid-layout/legacy';
import 'react-grid-layout/css/styles.css';

import type { FrontstagePageContent } from '../api/page-content';
import { BlockHoverToolbar } from './BlockHoverToolbar';
import { RestrictedBlockRuntimePreview } from './RestrictedBlockRuntimePreview';
import type { FrontstagePageCanvasRuntimeSessionEntry } from '../hooks/use-frontstage-page-canvas-runtime-sessions';
import {
  createFrontstagePageDocument
} from '../lib/page-document';
import {
  createFrontstagePageRenderPlan,
  type FrontstageBlockRenderPlanItem
} from '../lib/page-canvas/render-plan';
import type {
  FrontstagePageCanvasRuntimeSourceState
} from '../lib/page-canvas/runtime-source';
import type { FrontstagePageCanvasRuntimeRunPlanState } from '../lib/page-canvas/runtime-run-plan';
import { i18nText } from '../../../shared/i18n/text';
import { PermissionDeniedState } from '../../../shared/ui/PermissionDeniedState';
import { FRONTSTAGE_DESIGN_BLUE } from '../lib/design-mode-theme';
import {
  createFrontstagePersistedGridLayout,
  createFrontstageResponsiveLayouts,
  FRONTSTAGE_GRID_BREAKPOINTS,
  FRONTSTAGE_GRID_COLUMNS
} from '../lib/responsive-grid-layout';

type DesignBlockActions = {
  onMoveUp: (blockId: string) => void;
  onMoveDown: (blockId: string) => void;
  onConfigure: (blockId: string) => void;
  onEditCode: (blockId: string) => void;
  onDelete: (blockId: string) => void;
};

type PageCanvasProps = {
  content?: FrontstagePageContent;
  isLoading?: boolean;
  hasError?: boolean;
  isPermissionDenied?: boolean;
  selectedBlockId?: string | null;
  onSelectBlock?: (blockId: string | null) => void;
  onRetry?: () => void;
  runtimeSourceState?: FrontstagePageCanvasRuntimeSourceState | null;
  runtimeRunPlanState?: FrontstagePageCanvasRuntimeRunPlanState | null;
  runtimeSessionEntries?:
    | readonly FrontstagePageCanvasRuntimeSessionEntry[]
    | null;
  /** When true, blocks show blue outlines + hover toolbar */
  isDesignMode?: boolean;
  /** Actions triggered from the design mode hover toolbar */
  designActions?: DesignBlockActions;
  /** When true, all hover toolbar buttons are disabled (e.g. during save) */
  toolbarDisabled?: boolean;
  showTitle?: boolean;
  onResponsiveLayoutSave?: (
    layouts: Record<string, Record<string, { x: number; y: number; w: number; h: number }>>
  ) => void;
};

const ResponsiveGridLayout = WidthProvider(Responsive);
function formatPageTitle(content: FrontstagePageContent): string {
  return content.page.title?.trim() || i18nText("frontstage", "auto.unnamed_page");
}

function findRuntimeSessionEntryForSlot({
  item,
  slotIndex,
  runtimeSessionEntries
}: {
  item: { blockId: string; codeRef: string; sourceIndex: number };
  slotIndex: number;
  runtimeSessionEntries?:
    | readonly FrontstagePageCanvasRuntimeSessionEntry[]
    | null;
}): FrontstagePageCanvasRuntimeSessionEntry | null {
  if (!runtimeSessionEntries || runtimeSessionEntries.length === 0) {
    return null;
  }

  return (
    runtimeSessionEntries.find(
      (entry) =>
        entry.slotIndex === slotIndex &&
        entry.blockId === item.blockId &&
        entry.codeRef === item.codeRef
    ) ??
    runtimeSessionEntries.find((entry) => entry.slotIndex === slotIndex) ??
    null
  );
}

// ─── RenderPlanSlot ─────────────────────────────────────────────────

type RenderPlanSlotProps = {
  item: FrontstageBlockRenderPlanItem;
  runtimeSessionEntry?: FrontstagePageCanvasRuntimeSessionEntry | null;
  isSelected: boolean;
  onSelectBlock?: (blockId: string | null) => void;
  isDesignMode?: boolean;
  designActions?: DesignBlockActions;
  toolbarDisabled?: boolean;
  canMoveUp: boolean;
  canMoveDown: boolean;
};

const blockFrameBaseStyle: CSSProperties = {
  width: '100%',
  minWidth: 0,
  borderRadius: 8,
  background: '#fff',
  overflow: 'hidden',
  boxShadow: '0 14px 40px rgba(37, 99, 235, 0.05)'
};

const blockLabelStyle: CSSProperties = {
  position: 'absolute',
  top: 12,
  left: 14,
  zIndex: 2,
  borderRadius: 6,
  background: FRONTSTAGE_DESIGN_BLUE.labelBg,
  color: FRONTSTAGE_DESIGN_BLUE.labelText,
  fontSize: 12,
  lineHeight: '20px',
  padding: '0 8px'
};

function RenderPlanSlot({
  item,
  runtimeSessionEntry,
  isSelected,
  onSelectBlock,
  isDesignMode,
  designActions,
  toolbarDisabled,
  canMoveUp,
  canMoveDown
}: RenderPlanSlotProps) {
  const [isHovered, setIsHovered] = useState(false);

  // Determine border style based on mode
  let borderStyle: CSSProperties;
  if (isDesignMode) {
    if (isSelected) {
      borderStyle = {
        border: `2px solid ${FRONTSTAGE_DESIGN_BLUE.borderSelected}`,
        background: FRONTSTAGE_DESIGN_BLUE.bgSelected
      };
    } else if (isHovered) {
      borderStyle = {
        border: `1px solid ${FRONTSTAGE_DESIGN_BLUE.borderHover}`,
        background: FRONTSTAGE_DESIGN_BLUE.bgHover
      };
    } else {
      borderStyle = {
        border: `1px solid ${FRONTSTAGE_DESIGN_BLUE.borderIdle}`
      };
    }
  } else {
    borderStyle = isSelected
      ? { border: '2px solid #1677ff' }
      : { border: '1px solid transparent' };
  }

  const isToolbarVisible = !!(isDesignMode && (isHovered || isSelected));

  const handleSelect = () => {
    onSelectBlock?.(item.blockId);
  };

  // Render the actual block content
  const renderBlockContent = () => {
    if (runtimeSessionEntry && 'snapshot' in runtimeSessionEntry) {
      return (
        <div
          style={{
            padding: isDesignMode
              ? '48px clamp(16px, 5vw, 72px) 28px'
              : 12
          }}
        >
          <RestrictedBlockRuntimePreview snapshot={runtimeSessionEntry.snapshot} />
        </div>
      );
    }

    if (runtimeSessionEntry?.status === 'factory_failed') {
      return (
        <div style={{ padding: isDesignMode ? '48px 24px 28px' : 12 }}>
          <Alert
            type="error"
            showIcon
            message={i18nText("frontstage", "auto.runtime_preview_unavailable")}
            description={i18nText("frontstage", "auto.restricted_runtime_session_create_failed")}
          />
        </div>
      );
    }

    // Still loading / not ready — show minimal placeholder
    return (
      <div
        style={{
          padding: '24px 12px',
          paddingTop: isDesignMode ? 56 : 24,
          textAlign: 'center',
          color: '#bbb'
        }}
      >
        <Typography.Text type="secondary" style={{ fontSize: 13 }}>
          {runtimeSessionEntry?.status === 'skipped'
            ? i18nText("frontstage", "auto.block_skipped_run")
            : i18nText("frontstage", "auto.block_loading")}
        </Typography.Text>
      </div>
    );
  };

  return (
    <div
      style={{
        ...blockFrameBaseStyle,
        ...borderStyle,
        position: 'relative',
        transition: 'border-color 0.15s, background 0.15s'
      }}
      data-testid={`block-slot-${item.blockId}`}
      aria-label={isDesignMode ? i18nText("frontstage", "auto.block_with_id", { value1: item.blockId }) : undefined}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      onClick={handleSelect}
      role={isDesignMode ? 'button' : undefined}
      tabIndex={isDesignMode ? 0 : -1}
      onKeyDown={(event) => {
        if (isDesignMode && (event.key === 'Enter' || event.key === ' ')) {
          event.preventDefault();
          handleSelect();
        }
      }}
    >
      {isDesignMode ? <span style={blockLabelStyle}>{i18nText("frontstage", "auto.js_block")}</span> : null}
      {renderBlockContent()}

      {isDesignMode && designActions && isToolbarVisible && (
        <BlockHoverToolbar
          blockId={item.blockId}
          onMoveUp={() => designActions.onMoveUp(item.blockId)}
          onMoveDown={() => designActions.onMoveDown(item.blockId)}
          onConfigure={() => designActions.onConfigure(item.blockId)}
          onEditCode={() => designActions.onEditCode(item.blockId)}
          onDelete={() => designActions.onDelete(item.blockId)}
          canMoveUp={canMoveUp}
          canMoveDown={canMoveDown}
          isVisible={isToolbarVisible}
          disabled={toolbarDisabled}
        />
      )}
    </div>
  );
}

// ─── PageCanvas ─────────────────────────────────────────────────────

export const PageCanvas: FC<PageCanvasProps> = ({
  content,
  isLoading,
  hasError,
  isPermissionDenied,
  selectedBlockId = null,
  onSelectBlock,
  onRetry,
  runtimeSessionEntries,
  isDesignMode = false,
  designActions,
  toolbarDisabled = false,
  showTitle = true,
  onResponsiveLayoutSave
}) => {
  const document = useMemo(
    () => (content ? createFrontstagePageDocument(content) : null),
    [content]
  );
  const renderPlan = useMemo(
    () => (document ? createFrontstagePageRenderPlan(document) : null),
    [document]
  );
  const renderItems = useMemo(() => renderPlan?.items ?? [], [renderPlan]);
  const layouts = useMemo(
    () => createFrontstageResponsiveLayouts(renderItems),
    [renderItems]
  );
  const latestLayouts = useRef(layouts);
  useEffect(() => {
    latestLayouts.current = layouts;
  }, [layouts]);

  if (isLoading) {
    return (
      <div style={{ background: '#fafafa', border: '1px solid #f0f0f0', borderRadius: 6, padding: 12 }}>
        <Space direction="vertical" size={4}>
          <Typography.Text strong>{i18nText("frontstage", "auto.page_content_loading")}</Typography.Text>
          <Typography.Text type="secondary">
            {i18nText("frontstage", "auto.reading_page_content_and_blocks")}</Typography.Text>
        </Space>
      </div>
    );
  }

  if (hasError) {
    if (isPermissionDenied) {
      return <PermissionDeniedState />;
    }

    return (
      <Alert
        type="error"
        showIcon
        message={i18nText("frontstage", "auto.page_content_load_failed")}
        description={i18nText("frontstage", "auto.network_retry")}
        action={
          onRetry ? (
            <Button size="small" onClick={onRetry}>
              {i18nText("frontstage", "auto.retry")}</Button>
          ) : null
        }
      />
    );
  }

  if (!content || !document || !renderPlan) {
    return (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={
          <Space direction="vertical" size={2}>
            <Typography.Text>{i18nText("frontstage", "auto.no_page_content_selected")}</Typography.Text>
            <Typography.Text type="secondary">
              {i18nText("frontstage", "auto.page_preview_after_select")}</Typography.Text>
          </Space>
        }
      />
    );
  }

  return (
    <Space direction="vertical" size={12} style={{ width: '100%' }}>
      {showTitle ? (
        <Typography.Title level={4} style={{ margin: 0 }}>
          {formatPageTitle(content)}
        </Typography.Title>
      ) : null}

      {renderPlan.isEmpty ? (
        <div
          style={{
            background: isDesignMode ? FRONTSTAGE_DESIGN_BLUE.bgDashed : '#fafafa',
            border: isDesignMode
              ? `1px dashed ${FRONTSTAGE_DESIGN_BLUE.dashed}`
              : '1px solid #f0f0f0',
            borderRadius: 8,
            padding: 32,
            textAlign: 'center'
          }}
        >
          <Empty
            image={Empty.PRESENTED_IMAGE_SIMPLE}
            description={
              <Typography.Text type="secondary">{i18nText("frontstage", "auto.page_content_empty")}</Typography.Text>
            }
          />
        </div>
      ) : (
        <div data-testid="page-canvas-render-slots">
          <ResponsiveGridLayout
            breakpoints={FRONTSTAGE_GRID_BREAKPOINTS}
            cols={FRONTSTAGE_GRID_COLUMNS}
            layouts={layouts}
            rowHeight={32}
            margin={[16, 16]}
            isDraggable={isDesignMode && !toolbarDisabled}
            isResizable={isDesignMode && !toolbarDisabled}
            draggableCancel="button, input, textarea, select, a"
            onLayoutChange={(_layout: Layout, nextLayouts) => {
              latestLayouts.current = nextLayouts;
            }}
            onDragStop={() => {
              onResponsiveLayoutSave?.(
                createFrontstagePersistedGridLayout(latestLayouts.current)
              );
            }}
            onResizeStop={() => {
              onResponsiveLayoutSave?.(
                createFrontstagePersistedGridLayout(latestLayouts.current)
              );
            }}
          >
            {renderItems.map((item, slotIndex) => (
              <RenderPlanSlot
                key={item.blockId}
                item={item}
                runtimeSessionEntry={findRuntimeSessionEntryForSlot({
                  item,
                  slotIndex,
                  runtimeSessionEntries
                })}
                isSelected={item.blockId === selectedBlockId}
                onSelectBlock={onSelectBlock}
                isDesignMode={isDesignMode}
                designActions={designActions}
                toolbarDisabled={toolbarDisabled}
                canMoveUp={slotIndex > 0}
                canMoveDown={slotIndex < renderItems.length - 1}
              />
            ))}
          </ResponsiveGridLayout>
        </div>
      )}
    </Space>
  );
};
