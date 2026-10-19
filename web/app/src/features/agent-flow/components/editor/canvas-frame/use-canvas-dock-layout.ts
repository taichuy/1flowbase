import {
  type KeyboardEvent as ReactKeyboardEvent,
  useEffect,
  useRef,
  useState
} from 'react';

import { clampNodeDetailWidth } from '../../../lib/detail-panel-width';

import {
  CONVERSATION_LOG_DEFAULT_WIDTH,
  DEBUG_CONSOLE_DEFAULT_WIDTH,
  ENVIRONMENT_VARIABLES_DOCK_WIDTH,
  HISTORY_DOCK_WIDTH,
  SYSTEM_VARIABLES_DOCK_WIDTH,
  VARIABLE_CACHE_DEFAULT_HEIGHT,
  VARIABLE_CACHE_DEFAULT_SIDEBAR_WIDTH
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
    handleNodeDetailResizeKeyDown
  };
}
