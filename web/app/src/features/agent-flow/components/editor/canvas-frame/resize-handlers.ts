import type {
  Dispatch,
  MouseEvent as ReactMouseEvent,
  MutableRefObject,
  SetStateAction
} from 'react';

import {
  NODE_DETAIL_MIN_CANVAS_WIDTH,
  clampNodeDetailWidth
} from '../../../lib/detail-panel-width';
import {
  CONVERSATION_LOG_MIN_WIDTH,
  DEBUG_CONSOLE_GAP,
  DEBUG_CONSOLE_MIN_WIDTH,
  HISTORY_DOCK_MIN_WIDTH,
  VARIABLE_CACHE_MIN_HEIGHT,
  VARIABLE_CACHE_MIN_SIDEBAR_WIDTH,
  VARIABLES_DOCK_MIN_WIDTH
} from './layout';
import { startCanvasFrameResize } from './resize';

type StopResizeRef = MutableRefObject<(() => void) | null>;

interface CanvasFrameResizeHandlersInput {
  boundedConversationLogWidth: number;
  boundedDebugConsoleWidth: number;
  boundedHistoryDockWidth: number;
  boundedNodeDetailWidth: number;
  boundedVariableCacheHeight: number;
  boundedVariableCacheSidebarWidth: number;
  boundedVariablesDockWidth: number;
  canvasFrameWidth: number;
  conversationVariablesOpen: boolean;
  detailContainerWidth: number;
  environmentVariablesOpen: boolean;
  selectedNodeId: string | null;
  variableCacheMaxHeight: number;
  variableCacheSidebarMaxWidth: number;
  setConversationLogWidth: Dispatch<SetStateAction<number>>;
  setConversationVariablesDockWidth: Dispatch<SetStateAction<number>>;
  setEnvironmentVariablesDockWidth: Dispatch<SetStateAction<number>>;
  setHistoryDockWidth: Dispatch<SetStateAction<number>>;
  setIsResizingConversationLog: Dispatch<SetStateAction<boolean>>;
  setIsResizingDebugConsole: Dispatch<SetStateAction<boolean>>;
  setIsResizingHistoryDock: Dispatch<SetStateAction<boolean>>;
  setIsResizingNodeDetail: Dispatch<SetStateAction<boolean>>;
  setIsResizingVariableCache: Dispatch<SetStateAction<boolean>>;
  setIsResizingVariableCacheSidebar: Dispatch<SetStateAction<boolean>>;
  setIsResizingVariablesDock: Dispatch<SetStateAction<boolean>>;
  setDebugConsoleWidth: Dispatch<SetStateAction<number>>;
  setPanelState: (state: { nodeDetailWidth?: number }) => void;
  setSystemVariablesDockWidth: Dispatch<SetStateAction<number>>;
  setVariableCacheHeight: Dispatch<SetStateAction<number>>;
  setVariableCacheSidebarWidth: Dispatch<SetStateAction<number>>;
  stopConversationLogResizeRef: StopResizeRef;
  stopDebugConsoleResizeRef: StopResizeRef;
  stopHistoryDockResizeRef: StopResizeRef;
  stopNodeDetailResizeRef: StopResizeRef;
  stopVariableCacheResizeRef: StopResizeRef;
  stopVariableCacheSidebarResizeRef: StopResizeRef;
  stopVariablesDockResizeRef: StopResizeRef;
}

export function createCanvasFrameResizeHandlers({
  boundedConversationLogWidth,
  boundedDebugConsoleWidth,
  boundedHistoryDockWidth,
  boundedNodeDetailWidth,
  boundedVariableCacheHeight,
  boundedVariableCacheSidebarWidth,
  boundedVariablesDockWidth,
  canvasFrameWidth,
  conversationVariablesOpen,
  detailContainerWidth,
  environmentVariablesOpen,
  selectedNodeId,
  variableCacheMaxHeight,
  variableCacheSidebarMaxWidth,
  setConversationLogWidth,
  setConversationVariablesDockWidth,
  setEnvironmentVariablesDockWidth,
  setHistoryDockWidth,
  setIsResizingConversationLog,
  setIsResizingDebugConsole,
  setIsResizingHistoryDock,
  setIsResizingNodeDetail,
  setIsResizingVariableCache,
  setIsResizingVariableCacheSidebar,
  setIsResizingVariablesDock,
  setDebugConsoleWidth,
  setPanelState,
  setSystemVariablesDockWidth,
  setVariableCacheHeight,
  setVariableCacheSidebarWidth,
  stopConversationLogResizeRef,
  stopDebugConsoleResizeRef,
  stopHistoryDockResizeRef,
  stopNodeDetailResizeRef,
  stopVariableCacheResizeRef,
  stopVariableCacheSidebarResizeRef,
  stopVariablesDockResizeRef
}: CanvasFrameResizeHandlersInput) {
  function handleNodeDetailResizeStart(event: ReactMouseEvent<HTMLDivElement>) {
    const startX = event.clientX;
    const startWidth = boundedNodeDetailWidth;
    const containerWidth = detailContainerWidth;

    stopNodeDetailResizeRef.current?.();
    const handleMouseMove = (moveEvent: MouseEvent) => {
      const nextWidth = clampNodeDetailWidth(
        startWidth + startX - moveEvent.clientX,
        containerWidth
      );

      setPanelState({ nodeDetailWidth: nextWidth });
    };

    stopNodeDetailResizeRef.current = startCanvasFrameResize(event, {
      cursor: 'col-resize',
      onMove: handleMouseMove,
      onStart: () => setIsResizingNodeDetail(true),
      onStop: () => {
        setIsResizingNodeDetail(false);
        stopNodeDetailResizeRef.current = null;
      }
    });
  }

  function handleDebugConsoleResizeStart(
    event: ReactMouseEvent<HTMLDivElement>
  ) {
    const startX = event.clientX;
    const startWidth = boundedDebugConsoleWidth;
    const containerWidth = canvasFrameWidth;

    stopDebugConsoleResizeRef.current?.();
    const handleMouseMove = (moveEvent: MouseEvent) => {
      const nextWidth = Math.min(
        Math.max(
          startWidth - (moveEvent.clientX - startX),
          DEBUG_CONSOLE_MIN_WIDTH
        ),
        Math.max(
          containerWidth -
            (selectedNodeId ? boundedNodeDetailWidth : 0) -
            NODE_DETAIL_MIN_CANVAS_WIDTH,
          DEBUG_CONSOLE_MIN_WIDTH
        )
      );

      setDebugConsoleWidth(nextWidth);
    };

    stopDebugConsoleResizeRef.current = startCanvasFrameResize(event, {
      cursor: 'col-resize',
      onMove: handleMouseMove,
      onStart: () => setIsResizingDebugConsole(true),
      onStop: () => {
        setIsResizingDebugConsole(false);
        stopDebugConsoleResizeRef.current = null;
      }
    });
  }

  function handleConversationLogResizeStart(
    event: ReactMouseEvent<HTMLDivElement>
  ) {
    const startX = event.clientX;
    const startWidth = boundedConversationLogWidth;
    const containerWidth = canvasFrameWidth;

    stopConversationLogResizeRef.current?.();
    const handleMouseMove = (moveEvent: MouseEvent) => {
      const nextWidth = Math.min(
        Math.max(
          startWidth - (moveEvent.clientX - startX),
          CONVERSATION_LOG_MIN_WIDTH
        ),
        Math.max(
          containerWidth -
            boundedDebugConsoleWidth -
            DEBUG_CONSOLE_GAP -
            (selectedNodeId ? boundedNodeDetailWidth + DEBUG_CONSOLE_GAP : 0) -
            NODE_DETAIL_MIN_CANVAS_WIDTH,
          CONVERSATION_LOG_MIN_WIDTH
        )
      );

      setConversationLogWidth(nextWidth);
    };

    stopConversationLogResizeRef.current = startCanvasFrameResize(event, {
      cursor: 'col-resize',
      onMove: handleMouseMove,
      onStart: () => setIsResizingConversationLog(true),
      onStop: () => {
        setIsResizingConversationLog(false);
        stopConversationLogResizeRef.current = null;
      }
    });
  }

  function handleVariablesDockResizeStart(
    event: ReactMouseEvent<HTMLDivElement>
  ) {
    const startX = event.clientX;
    const startWidth = boundedVariablesDockWidth;
    const containerWidth = canvasFrameWidth;

    stopVariablesDockResizeRef.current?.();
    const handleMouseMove = (moveEvent: MouseEvent) => {
      const nextWidth = Math.min(
        Math.max(
          startWidth - (moveEvent.clientX - startX),
          VARIABLES_DOCK_MIN_WIDTH
        ),
        Math.max(
          containerWidth -
            (selectedNodeId ? boundedNodeDetailWidth : 0) -
            NODE_DETAIL_MIN_CANVAS_WIDTH,
          VARIABLES_DOCK_MIN_WIDTH
        )
      );

      if (conversationVariablesOpen) {
        setConversationVariablesDockWidth(nextWidth);
      } else if (environmentVariablesOpen) {
        setEnvironmentVariablesDockWidth(nextWidth);
      } else {
        setSystemVariablesDockWidth(nextWidth);
      }
    };

    stopVariablesDockResizeRef.current = startCanvasFrameResize(event, {
      cursor: 'col-resize',
      onMove: handleMouseMove,
      onStart: () => setIsResizingVariablesDock(true),
      onStop: () => {
        setIsResizingVariablesDock(false);
        stopVariablesDockResizeRef.current = null;
      }
    });
  }

  function handleHistoryDockResizeStart(
    event: ReactMouseEvent<HTMLDivElement>
  ) {
    const startX = event.clientX;
    const startWidth = boundedHistoryDockWidth;
    const containerWidth = canvasFrameWidth;

    stopHistoryDockResizeRef.current?.();
    const handleMouseMove = (moveEvent: MouseEvent) => {
      const nextWidth = Math.min(
        Math.max(
          startWidth - (moveEvent.clientX - startX),
          HISTORY_DOCK_MIN_WIDTH
        ),
        Math.max(
          containerWidth -
            (selectedNodeId ? boundedNodeDetailWidth : 0) -
            NODE_DETAIL_MIN_CANVAS_WIDTH,
          HISTORY_DOCK_MIN_WIDTH
        )
      );

      setHistoryDockWidth(nextWidth);
    };

    stopHistoryDockResizeRef.current = startCanvasFrameResize(event, {
      cursor: 'col-resize',
      onMove: handleMouseMove,
      onStart: () => setIsResizingHistoryDock(true),
      onStop: () => {
        setIsResizingHistoryDock(false);
        stopHistoryDockResizeRef.current = null;
      }
    });
  }

  function handleVariableCacheResizeStart(
    event: ReactMouseEvent<HTMLDivElement>
  ) {
    const startY = event.clientY;
    const startHeight = boundedVariableCacheHeight;

    stopVariableCacheResizeRef.current?.();
    const handleMouseMove = (moveEvent: MouseEvent) => {
      const nextHeight = Math.min(
        Math.max(
          startHeight + startY - moveEvent.clientY,
          VARIABLE_CACHE_MIN_HEIGHT
        ),
        variableCacheMaxHeight
      );

      setVariableCacheHeight(nextHeight);
    };

    stopVariableCacheResizeRef.current = startCanvasFrameResize(event, {
      cursor: 'row-resize',
      onMove: handleMouseMove,
      onStart: () => setIsResizingVariableCache(true),
      onStop: () => {
        setIsResizingVariableCache(false);
        stopVariableCacheResizeRef.current = null;
      }
    });
  }

  function handleVariableCacheSidebarResizeStart(
    event: ReactMouseEvent<HTMLDivElement>
  ) {
    const startX = event.clientX;
    const startWidth = boundedVariableCacheSidebarWidth;
    const minWidth = VARIABLE_CACHE_MIN_SIDEBAR_WIDTH;
    const maxWidth = variableCacheSidebarMaxWidth;

    stopVariableCacheSidebarResizeRef.current?.();
    const handleMouseMove = (moveEvent: MouseEvent) => {
      const nextWidth = Math.min(
        Math.max(startWidth + moveEvent.clientX - startX, minWidth),
        maxWidth
      );

      setVariableCacheSidebarWidth(nextWidth);
    };

    stopVariableCacheSidebarResizeRef.current = startCanvasFrameResize(event, {
      cursor: 'col-resize',
      onMove: handleMouseMove,
      onStart: () => setIsResizingVariableCacheSidebar(true),
      onStop: () => {
        setIsResizingVariableCacheSidebar(false);
        stopVariableCacheSidebarResizeRef.current = null;
      }
    });
  }

  return {
    handleConversationLogResizeStart,
    handleDebugConsoleResizeStart,
    handleHistoryDockResizeStart,
    handleNodeDetailResizeStart,
    handleVariableCacheResizeStart,
    handleVariableCacheSidebarResizeStart,
    handleVariablesDockResizeStart
  };
}
