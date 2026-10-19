import {
  type KeyboardEvent as ReactKeyboardEvent,
  useEffect,
  useRef,
  useState
} from 'react';

import { clampNodeDetailWidth, NODE_DETAIL_MIN_CANVAS_WIDTH } from '../../../lib/detail-panel-width';

import {
  CONVERSATION_LOG_DEFAULT_WIDTH,
  CONVERSATION_LOG_MIN_WIDTH,
  DEBUG_CONSOLE_DEFAULT_WIDTH,
  DEBUG_CONSOLE_MIN_WIDTH,
  DEBUG_CONSOLE_GAP,
  ENVIRONMENT_VARIABLES_DOCK_WIDTH,
  HISTORY_DOCK_WIDTH,
  HISTORY_DOCK_MIN_WIDTH,
  SYSTEM_VARIABLES_DOCK_WIDTH,
  VARIABLE_CACHE_DEFAULT_HEIGHT,
  VARIABLE_CACHE_MIN_HEIGHT,
  VARIABLE_CACHE_DEFAULT_SIDEBAR_WIDTH,
  VARIABLE_CACHE_MIN_SIDEBAR_WIDTH,
  VARIABLES_DOCK_MIN_WIDTH
} from './layout';
import { deriveCanvasFrameLayout } from './derived-layout';
import { createCanvasFrameResizeHandlers } from './resize-handlers';

import type { AgentFlowDebugMessage } from '../../../api/runtime';

interface CanvasDockLayoutInput {
  selectedNodeId: string | null;
  historyOpen: boolean;
  nodeDetailWidth: number;
  setPanelState: (state: { nodeDetailWidth?: number }) => void;
  conversationLogMessage: AgentFlowDebugMessage | null;
}

function keyboardWidth(
  event: ReactKeyboardEvent<HTMLDivElement>,
  width: number,
  minWidth: number,
  maxWidth: number,
  growKey: 'ArrowLeft' | 'ArrowRight'
): number | null {
  if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key))
    return null;
  event.preventDefault();
  event.stopPropagation();
  if (event.key === 'Home') return minWidth;
  if (event.key === 'End') return maxWidth;
  return Math.min(
    maxWidth,
    Math.max(minWidth, width + (event.key === growKey ? 24 : -24))
  );
}

/** Owns dock geometry, browser observation, and active resize cleanup. */
export function useCanvasDockLayout({
  selectedNodeId,
  historyOpen,
  nodeDetailWidth,
  setPanelState,
  conversationLogMessage
}: CanvasDockLayoutInput) {
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const stopNodeDetailResizeRef = useRef<(() => void) | null>(null);
  const stopDebugConsoleResizeRef = useRef<(() => void) | null>(null);
  const stopConversationLogResizeRef = useRef<(() => void) | null>(null);
  const stopVariablesDockResizeRef = useRef<(() => void) | null>(null);
  const stopHistoryDockResizeRef = useRef<(() => void) | null>(null);
  const stopVariableCacheResizeRef = useRef<(() => void) | null>(null);
  const stopVariableCacheSidebarResizeRef = useRef<(() => void) | null>(null);
  const [bodyWidth, setBodyWidth] = useState(0);
  const [debugConsoleOpen, setDebugConsoleOpen] = useState(false);
  const [debugConsoleWidth, setDebugConsoleWidth] = useState(
    DEBUG_CONSOLE_DEFAULT_WIDTH
  );
  const [bodyHeight, setBodyHeight] = useState(0);
  const [isResizingNodeDetail, setIsResizingNodeDetail] = useState(false);
  const [isResizingDebugConsole, setIsResizingDebugConsole] = useState(false);
  const [conversationLogWidth, setConversationLogWidth] = useState(
    CONVERSATION_LOG_DEFAULT_WIDTH
  );
  const [isResizingConversationLog, setIsResizingConversationLog] =
    useState(false);
  const [isResizingVariablesDock, setIsResizingVariablesDock] = useState(false);
  const [isResizingHistoryDock, setIsResizingHistoryDock] = useState(false);
  const [variableCacheOpen, setVariableCacheOpen] = useState(false);
  const [systemVariablesOpen, setSystemVariablesOpen] = useState(false);
  const [environmentVariablesOpen, setEnvironmentVariablesOpen] =
    useState(false);
  const [conversationVariablesOpen, setConversationVariablesOpen] =
    useState(false);
  const [systemVariablesDockWidth, setSystemVariablesDockWidth] = useState(
    SYSTEM_VARIABLES_DOCK_WIDTH
  );
  const [environmentVariablesDockWidth, setEnvironmentVariablesDockWidth] =
    useState(ENVIRONMENT_VARIABLES_DOCK_WIDTH);
  const [conversationVariablesDockWidth, setConversationVariablesDockWidth] =
    useState(ENVIRONMENT_VARIABLES_DOCK_WIDTH);
  const [historyDockWidth, setHistoryDockWidth] = useState(HISTORY_DOCK_WIDTH);
  const [variableCacheHeight, setVariableCacheHeight] = useState(
    VARIABLE_CACHE_DEFAULT_HEIGHT
  );
  const [isResizingVariableCache, setIsResizingVariableCache] = useState(false);
  const [variableCacheSidebarWidth, setVariableCacheSidebarWidth] = useState(
    VARIABLE_CACHE_DEFAULT_SIDEBAR_WIDTH
  );
  const [isResizingVariableCacheSidebar, setIsResizingVariableCacheSidebar] =
    useState(false);
  useEffect(() => {
    const element = bodyRef.current;

    if (!element) {
      return;
    }

    const resizeObserver = new ResizeObserver((entries) => {
      const entry = entries[0];

      if (!entry) {
        return;
      }

      setBodyWidth(entry.contentRect.width);
      setBodyHeight(entry.contentRect.height);
    });

    resizeObserver.observe(element);
    const bodyRect = element.getBoundingClientRect();
    setBodyWidth(bodyRect.width);
    setBodyHeight(bodyRect.height);

    return () => resizeObserver.disconnect();
  }, []);

  useEffect(() => {
    const stopNodeDetailResize = stopNodeDetailResizeRef;
    const stopDebugConsoleResize = stopDebugConsoleResizeRef;
    const stopConversationLogResize = stopConversationLogResizeRef;
    const stopVariablesDockResize = stopVariablesDockResizeRef;
    const stopHistoryDockResize = stopHistoryDockResizeRef;
    const stopVariableCacheResize = stopVariableCacheResizeRef;
    const stopVariableCacheSidebarResize = stopVariableCacheSidebarResizeRef;

    return () => {
      stopNodeDetailResize.current?.();
      stopDebugConsoleResize.current?.();
      stopConversationLogResize.current?.();
      stopVariablesDockResize.current?.();
      stopHistoryDockResize.current?.();
      stopVariableCacheResize.current?.();
      stopVariableCacheSidebarResize.current?.();
    };
  }, []);

  useEffect(() => {
    if (selectedNodeId) {
      return;
    }

    stopNodeDetailResizeRef.current?.();
  }, [selectedNodeId]);

  useEffect(() => {
    if (conversationLogMessage) {
      return;
    }

    stopConversationLogResizeRef.current?.();
  }, [conversationLogMessage]);

  useEffect(() => {
    if (
      systemVariablesOpen ||
      environmentVariablesOpen ||
      conversationVariablesOpen
    ) {
      return;
    }

    stopVariablesDockResizeRef.current?.();
  }, [
    conversationVariablesOpen,
    environmentVariablesOpen,
    systemVariablesOpen
  ]);

  useEffect(() => {
    if (historyOpen) {
      return;
    }

    stopHistoryDockResizeRef.current?.();
  }, [historyOpen]);

  useEffect(() => {
    if (variableCacheOpen) {
      return;
    }

    stopVariableCacheResizeRef.current?.();
  }, [variableCacheOpen]);

  useEffect(() => {
    if (variableCacheOpen) {
      return;
    }

    stopVariableCacheSidebarResizeRef.current?.();
  }, [variableCacheOpen]);

  const {
    boundedConversationLogWidth,
    boundedDebugConsoleWidth,
    boundedHistoryDockWidth,
    boundedNodeDetailWidth,
    boundedVariableCacheHeight,
    boundedVariableCacheSidebarWidth,
    boundedVariablesDockWidth,
    canvasFrameWidth,
    conversationLogOpen,
    detailContainerWidth,
    nodeDetailLayout,
    sideDockOccupiedWidth,
    variableCacheCenterLeft,
    variableCacheMaxHeight,
    variableCacheRightOffset,
    variableCacheSidebarMaxWidth,
    variablesDockOpen
  } = deriveCanvasFrameLayout({
    bodyHeight,
    bodyWidth,
    conversationLogMessageOpen: conversationLogMessage !== null,
    conversationLogWidth,
    conversationVariablesDockWidth,
    conversationVariablesOpen,
    debugConsoleOpen,
    debugConsoleWidth,
    environmentVariablesDockWidth,
    environmentVariablesOpen,
    historyDockWidth,
    historyOpen,
    nodeDetailWidth,
    selectedNodeId,
    systemVariablesDockWidth,
    systemVariablesOpen,
    variableCacheHeight,
    variableCacheSidebarWidth
  });

  const {
    handleConversationLogResizeStart,
    handleDebugConsoleResizeStart,
    handleHistoryDockResizeStart,
    handleNodeDetailResizeStart,
    handleVariableCacheResizeStart,
    handleVariableCacheSidebarResizeStart,
    handleVariablesDockResizeStart
  } = createCanvasFrameResizeHandlers({
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
    setDebugConsoleWidth,
    setIsResizingConversationLog,
    setIsResizingDebugConsole,
    setIsResizingHistoryDock,
    setIsResizingNodeDetail,
    setIsResizingVariableCache,
    setIsResizingVariableCacheSidebar,
    setIsResizingVariablesDock,
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
  });
  // Match the existing adaptive bounds used by deriveCanvasFrameLayout.
  const maxDebugConsoleWidth = Math.max(
    canvasFrameWidth -
      (selectedNodeId ? nodeDetailWidth : 0) -
      NODE_DETAIL_MIN_CANVAS_WIDTH,
    DEBUG_CONSOLE_MIN_WIDTH
  );
  const maxVariablesDockWidth = Math.max(
    canvasFrameWidth -
      (selectedNodeId ? nodeDetailWidth : 0) -
      NODE_DETAIL_MIN_CANVAS_WIDTH,
    VARIABLES_DOCK_MIN_WIDTH
  );
  const maxHistoryDockWidth = Math.max(
    canvasFrameWidth -
      (selectedNodeId ? nodeDetailWidth : 0) -
      NODE_DETAIL_MIN_CANVAS_WIDTH,
    HISTORY_DOCK_MIN_WIDTH
  );
  const maxConversationLogWidth = Math.max(
    canvasFrameWidth -
      boundedDebugConsoleWidth -
      DEBUG_CONSOLE_GAP -
      (selectedNodeId ? nodeDetailWidth + DEBUG_CONSOLE_GAP : 0) -
      NODE_DETAIL_MIN_CANVAS_WIDTH,
    CONVERSATION_LOG_MIN_WIDTH
  );

  function handleDebugConsoleResizeKeyDown(
    event: ReactKeyboardEvent<HTMLDivElement>
  ) {
    const width = keyboardWidth(
      event,
      boundedDebugConsoleWidth,
      DEBUG_CONSOLE_MIN_WIDTH,
      maxDebugConsoleWidth,
      'ArrowLeft'
    );
    if (width !== null) setDebugConsoleWidth(width);
  }
  function handleConversationLogResizeKeyDown(
    event: ReactKeyboardEvent<HTMLDivElement>
  ) {
    const width = keyboardWidth(
      event,
      boundedConversationLogWidth,
      CONVERSATION_LOG_MIN_WIDTH,
      maxConversationLogWidth,
      'ArrowLeft'
    );
    if (width !== null) setConversationLogWidth(width);
  }
  function handleHistoryDockResizeKeyDown(
    event: ReactKeyboardEvent<HTMLDivElement>
  ) {
    const width = keyboardWidth(
      event,
      boundedHistoryDockWidth,
      HISTORY_DOCK_MIN_WIDTH,
      maxHistoryDockWidth,
      'ArrowLeft'
    );
    if (width !== null) setHistoryDockWidth(width);
  }
  function handleVariablesDockResizeKeyDown(
    event: ReactKeyboardEvent<HTMLDivElement>
  ) {
    const width = keyboardWidth(
      event,
      boundedVariablesDockWidth,
      VARIABLES_DOCK_MIN_WIDTH,
      maxVariablesDockWidth,
      'ArrowLeft'
    );
    if (width === null) return;
    if (conversationVariablesOpen) setConversationVariablesDockWidth(width);
    else if (environmentVariablesOpen) setEnvironmentVariablesDockWidth(width);
    else setSystemVariablesDockWidth(width);
  }
  function handleVariableCacheSidebarResizeKeyDown(
    event: ReactKeyboardEvent<HTMLDivElement>
  ) {
    const width = keyboardWidth(
      event,
      boundedVariableCacheSidebarWidth,
      VARIABLE_CACHE_MIN_SIDEBAR_WIDTH,
      variableCacheSidebarMaxWidth,
      'ArrowRight'
    );
    if (width !== null) setVariableCacheSidebarWidth(width);
  }

  function handleVariableCacheResizeKeyDown(event: ReactKeyboardEvent<HTMLDivElement>) {
    if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return;
    event.preventDefault();
    event.stopPropagation();
    const delta = event.key === 'ArrowUp' ? 16 : -16;
    setVariableCacheHeight(Math.min(variableCacheMaxHeight, Math.max(VARIABLE_CACHE_MIN_HEIGHT, boundedVariableCacheHeight + delta)));
  }

  function handleNodeDetailResizeKeyDown(
    event: ReactKeyboardEvent<HTMLDivElement>
  ) {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') {
      return;
    }

    event.preventDefault();
    const direction = event.key === 'ArrowLeft' ? 1 : -1;
    setPanelState({
      nodeDetailWidth: clampNodeDetailWidth(
        boundedNodeDetailWidth + direction * 24,
        detailContainerWidth
      )
    });
  }

  return {
    stopDebugConsoleResizeRef,
    stopConversationLogResizeRef,
    bodyRef,
    bodyWidth,
    debugConsoleOpen,
    setDebugConsoleOpen,
    isResizingNodeDetail,
    isResizingDebugConsole,
    isResizingConversationLog,
    isResizingVariablesDock,
    isResizingHistoryDock,
    variableCacheOpen,
    setVariableCacheOpen,
    systemVariablesOpen,
    setSystemVariablesOpen,
    environmentVariablesOpen,
    setEnvironmentVariablesOpen,
    conversationVariablesOpen,
    setConversationVariablesOpen,
    isResizingVariableCache,
    isResizingVariableCacheSidebar,
    boundedConversationLogWidth,
    boundedDebugConsoleWidth,
    boundedHistoryDockWidth,
    boundedNodeDetailWidth,
    boundedVariableCacheHeight,
    boundedVariableCacheSidebarWidth,
    boundedVariablesDockWidth,
    canvasFrameWidth,
    conversationLogOpen,
    detailContainerWidth,
    nodeDetailLayout,
    sideDockOccupiedWidth,
    variableCacheCenterLeft,
    variableCacheMaxHeight,
    variableCacheRightOffset,
    variableCacheSidebarMaxWidth,
    variablesDockOpen,
    handleConversationLogResizeStart,
    handleDebugConsoleResizeStart,
    handleHistoryDockResizeStart,
    handleNodeDetailResizeStart,
    handleVariableCacheResizeStart,
    handleVariableCacheSidebarResizeStart,
    handleVariablesDockResizeStart,
    maxDebugConsoleWidth,
    maxConversationLogWidth,
    maxVariablesDockWidth,
    maxHistoryDockWidth,
    handleDebugConsoleResizeKeyDown,
    handleConversationLogResizeKeyDown,
    handleVariablesDockResizeKeyDown,
    handleHistoryDockResizeKeyDown,
    handleVariableCacheSidebarResizeKeyDown,
    handleNodeDetailResizeKeyDown,
    handleVariableCacheResizeKeyDown
  };
}
