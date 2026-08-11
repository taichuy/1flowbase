import type {
  FlowAuthoringDocument,
  FlowNodeDocument
} from '@1flowbase/flow-schema';
import { getNodeById, getOutgoingEdges } from '../selectors';
import { isLlmToolSourceHandle } from '../../llm-node-config';
import {
  CANVAS_NODE_BASE_HEIGHT,
  CANVAS_NODE_WIDTH,
  getCanvasNodeMinimumHeightForDocument
} from '../../canvas/node-dimensions';

const ARRANGE_ORIGIN_X = 120;
const ARRANGE_ORIGIN_Y = 160;
const ARRANGE_GAP_X = 280;
const ARRANGE_GAP_Y = 40;
const GRID_SIZE = 20;

function snapToGrid(value: number) {
  return Math.round(value / GRID_SIZE) * GRID_SIZE;
}

function compareNodesByCanvasOrder(
  left: { id: string; position: { x: number; y: number } },
  right: { id: string; position: { x: number; y: number } }
) {
  if (left.position.y !== right.position.y) {
    return left.position.y - right.position.y;
  }

  if (left.position.x !== right.position.x) {
    return left.position.x - right.position.x;
  }

  return left.id.localeCompare(right.id);
}

function compareEdgesByHandleThenId(
  left: FlowAuthoringDocument['graph']['edges'][number],
  right: FlowAuthoringDocument['graph']['edges'][number]
) {
  const leftHandle = left.sourceHandle ?? '';
  const rightHandle = right.sourceHandle ?? '';

  if (leftHandle !== rightHandle) {
    return leftHandle.localeCompare(rightHandle);
  }

  return left.id.localeCompare(right.id);
}

function collectMountLaneIds(
  containerEdges: FlowAuthoringDocument['graph']['edges']
) {
  const incomingEdgesByNodeId = new Map<
    string,
    FlowAuthoringDocument['graph']['edges']
  >();
  const laneByNodeId = new Map<string, string>();

  for (const edge of containerEdges) {
    incomingEdgesByNodeId.set(edge.target, [
      ...(incomingEdgesByNodeId.get(edge.target) ?? []),
      edge
    ]);

    if (isLlmToolSourceHandle(edge.sourceHandle)) {
      laneByNodeId.set(edge.target, edge.id);
    }
  }

  let changed = true;

  while (changed) {
    changed = false;

    for (const edge of containerEdges) {
      if (isLlmToolSourceHandle(edge.sourceHandle)) {
        continue;
      }

      const sourceLaneId = laneByNodeId.get(edge.source);

      if (!sourceLaneId || laneByNodeId.has(edge.target)) {
        continue;
      }

      const incomingEdges = incomingEdgesByNodeId.get(edge.target) ?? [];
      const allIncomingEdgesStayInLane = incomingEdges.every(
        (incomingEdge) => laneByNodeId.get(incomingEdge.source) === sourceLaneId
      );

      if (allIncomingEdgesStayInLane) {
        laneByNodeId.set(edge.target, sourceLaneId);
        changed = true;
      }
    }
  }

  return laneByNodeId;
}

/**
 * 2D AABB 碰撞检测与 Y 轴最小平移向量 (MTV) 垂直消解算法
 */
function resolveAABBCollisions(
  positions: Record<string, { x: number; y: number }>,
  containerNodes: FlowNodeDocument[],
  gapY = 40,
  maxIterations = 8
): Set<string> {
  const shiftedIds = new Set<string>();

  for (let iter = 0; iter < maxIterations; iter++) {
    let anyCollisionResolved = false;

    for (let i = 0; i < containerNodes.length; i++) {
      const nodeA = containerNodes[i];
      const idA = nodeA.id;
      const posA = positions[idA];
      if (!posA) continue;

      for (let j = i + 1; j < containerNodes.length; j++) {
        const nodeB = containerNodes[j];
        const idB = nodeB.id;
        const posB = positions[idB];
        if (!posB) continue;

        const dx =
          CANVAS_NODE_WIDTH + 30 - Math.abs(posB.x - posA.x);

        if (dx <= 0) {
          continue;
        }

        const heightA = getCanvasNodeMinimumHeightForDocument(nodeA);
        const heightB = getCanvasNodeMinimumHeightForDocument(nodeB);
        const shiftId = posB.y < posA.y ? idA : idB;
        const shiftDistance =
          posB.y < posA.y
            ? posB.y + heightB + gapY - posA.y
            : posA.y + heightA + gapY - posB.y;

        if (shiftDistance > 0) {
          positions[shiftId] = {
            x: positions[shiftId].x,
            y: positions[shiftId].y + shiftDistance
          };
          shiftedIds.add(shiftId);
          anyCollisionResolved = true;
        }
      }
    }

    if (!anyCollisionResolved) {
      break;
    }
  }

  return shiftedIds;
}

/**
 * 局部受控力导向物理松弛算法 (Spring-Electrical Model)
 */
function applyLocalPhysicsRelaxation(
  positions: Record<string, { x: number; y: number }>,
  activeIds: Set<string>,
  containerNodes: Array<{ id: string }>,
  edges: FlowAuthoringDocument['graph']['edges'],
  iterations = 25
) {
  if (activeIds.size === 0) return;

  const kr = 30000; // 库仑斥力系数
  const ka = 0.04; // 弹簧引力系数
  const L0 = 280; // 弹簧自然长度
  const damping = 0.7;

  // 仅对发生移动的活跃节点应用受控力学松弛
  const activeNodes = containerNodes.filter((n) => activeIds.has(n.id));

  const velocities: Record<string, { x: number; y: number }> = {};
  for (const node of activeNodes) {
    velocities[node.id] = { x: 0, y: 0 };
  }

  for (let iter = 0; iter < iterations; iter++) {
    const forces: Record<string, { x: number; y: number }> = {};
    for (const node of activeNodes) {
      forces[node.id] = { x: 0, y: 0 };
    }

    // 1. 计算库仑斥力 (来自容器内所有节点)
    for (let i = 0; i < activeNodes.length; i++) {
      const idA = activeNodes[i].id;
      const posA = positions[idA];
      if (!posA) continue;

      for (let j = 0; j < containerNodes.length; j++) {
        const idB = containerNodes[j].id;
        if (idA === idB) continue;

        const posB = positions[idB];
        if (!posB) continue;

        const dx = posA.x - posB.x;
        const dy = posA.y - posB.y;
        const distSq = dx * dx + dy * dy;
        const dist = Math.sqrt(distSq) || 1;

        if (dist < 380) {
          const forceMag = kr / (distSq + 100);
          forces[idA].x += (dx / dist) * forceMag;
          forces[idA].y += (dy / dist) * forceMag;
        }
      }
    }

    // 2. 计算弹簧引力 (沿边双向拉拽)
    for (const edge of edges) {
      const posSource = positions[edge.source];
      const posTarget = positions[edge.target];
      if (!posSource || !posTarget) continue;

      const dx = posTarget.x - posSource.x;
      const dy = posTarget.y - posSource.y;
      const dist = Math.sqrt(dx * dx + dy * dy) || 1;

      const forceMag = ka * (dist - L0);
      const fx = (dx / dist) * forceMag;
      const fy = (dy / dist) * forceMag;

      if (forces[edge.source]) {
        forces[edge.source].x += fx;
        forces[edge.source].y += fy;
      }
      if (forces[edge.target]) {
        forces[edge.target].x -= fx;
        forces[edge.target].y -= fy;
      }
    }

    // 3. 更新速度与位置
    for (const node of activeNodes) {
      const id = node.id;
      const vel = velocities[id];
      const force = forces[id];

      vel.x = (vel.x + force.x) * damping;
      vel.y = (vel.y + force.y) * damping;

      positions[id] = {
        x: positions[id].x + vel.x,
        y: positions[id].y + vel.y
      };
    }
  }
}

/**
 * 有向无环图 (DAG) 拓扑下流 BFS 推进与碰撞消解管道
 */
export function shiftDownstreamNodesBFS(
  document: FlowAuthoringDocument,
  startNodeId: string,
  gapX = 280
): FlowAuthoringDocument {
  const startNode = getNodeById(document, startNodeId);
  if (!startNode) {
    return document;
  }

  // 1. 初始化更新后的坐标映射，预填充所有节点当前坐标
  const positions: Record<string, { x: number; y: number }> = {};
  for (const node of document.graph.nodes) {
    positions[node.id] = { ...node.position };
  }

  // 2. 初始化队列和已访问集合
  const queue: string[] = [startNodeId];
  const visited = new Set<string>();
  const shiftedNodeIds = new Set<string>();

  // 3. 执行阶段一：动态尺寸感知 BFS 拓扑推移
  const netGapX = Math.max(0, gapX - CANVAS_NODE_WIDTH); // 保证 280 默认步长完美兼容

  while (queue.length > 0) {
    const currentNodeId = queue.shift()!;
    visited.add(currentNodeId);

    const currentPos = positions[currentNodeId];
    if (!currentPos) continue;

    // 获取当前节点的所有出边 (下游)
    const outgoingEdges = getOutgoingEdges(document, currentNodeId);

    for (const edge of outgoingEdges) {
      const targetNodeId = edge.target;
      const targetNode = getNodeById(document, targetNodeId);
      if (!targetNode) continue;

      // 仅在同容器（containerId）内进行排布调整
      if (targetNode.containerId !== startNode.containerId) continue;

      const targetCurrentPos = positions[targetNodeId];
      if (!targetCurrentPos) continue;

      // 计算目标节点的最小 X 坐标 (父节点 X 坐标 + 节点宽度 + 净间隙)
      const targetMinX = currentPos.x + CANVAS_NODE_WIDTH + netGapX;

      if (targetCurrentPos.x < targetMinX) {
        positions[targetNodeId] = {
          x: targetMinX,
          y: targetCurrentPos.y
        };
        shiftedNodeIds.add(targetNodeId);

        if (!queue.includes(targetNodeId)) {
          queue.push(targetNodeId);
        }
      }
    }
  }

  // 获取当前容器内所有的节点
  const containerNodes = document.graph.nodes.filter(
    (node) => node.containerId === startNode.containerId
  );

  // 4. 执行阶段二：2D AABB 迭代消解垂直重叠
  let shiftedAABBNodeIds = new Set<string>();
  if (shiftedNodeIds.size > 0) {
    shiftedAABBNodeIds = resolveAABBCollisions(
      positions,
      containerNodes,
      40,
      8
    );
  }

  // 5. 执行阶段三：局部活跃节点力导向物理松弛
  if (shiftedAABBNodeIds.size > 0) {
    const activeRelaxIds = new Set<string>([
      startNodeId,
      ...shiftedNodeIds,
      ...shiftedAABBNodeIds
    ]);
    const activeEdges = document.graph.edges.filter(
      (edge) => edge.containerId === startNode.containerId
    );

    applyLocalPhysicsRelaxation(
      positions,
      activeRelaxIds,
      containerNodes,
      activeEdges,
      25
    );
  }

  // 6. 对受影响节点的坐标进行舍入对齐，确保画布上没有极细微的小数偏移
  const finalPositions: Record<string, { x: number; y: number }> = {};
  for (const key of Object.keys(positions)) {
    finalPositions[key] = {
      x: Math.round(positions[key].x),
      y: Math.round(positions[key].y)
    };
  }

  // 7. 返回包含更新位置的新文档对象
  return {
    ...document,
    graph: {
      ...document.graph,
      nodes: document.graph.nodes.map((node) =>
        finalPositions[node.id]
          ? {
              ...node,
              position: finalPositions[node.id]
            }
          : node
      )
    }
  };
}

export function arrangeCanvasLeftToRight(
  document: FlowAuthoringDocument,
  containerId: string | null
): FlowAuthoringDocument {
  const containerNodes = document.graph.nodes
    .filter((node) => node.containerId === containerId)
    .sort(compareNodesByCanvasOrder);

  if (containerNodes.length === 0) {
    return document;
  }

  const containerNodeIds = new Set(containerNodes.map((node) => node.id));
  const containerEdges = document.graph.edges
    .filter(
      (edge) =>
        edge.containerId === containerId &&
        containerNodeIds.has(edge.source) &&
        containerNodeIds.has(edge.target)
    )
    .sort((left, right) => left.id.localeCompare(right.id));
  const incomingCountByNodeId = new Map<string, number>();
  const outgoingTargetsByNodeId = new Map<string, string[]>();
  const layerByNodeId = new Map<string, number>();

  for (const node of containerNodes) {
    incomingCountByNodeId.set(node.id, 0);
    outgoingTargetsByNodeId.set(node.id, []);
  }

  for (const edge of containerEdges) {
    incomingCountByNodeId.set(
      edge.target,
      (incomingCountByNodeId.get(edge.target) ?? 0) + 1
    );
    outgoingTargetsByNodeId.get(edge.source)?.push(edge.target);
  }

  const queue = containerNodes
    .filter((node) => (incomingCountByNodeId.get(node.id) ?? 0) === 0)
    .map((node) => node.id);
  const remainingIncomingCountByNodeId = new Map(incomingCountByNodeId);
  let visitedCount = 0;

  for (const nodeId of queue) {
    layerByNodeId.set(nodeId, 0);
  }

  while (queue.length > 0) {
    const currentNodeId = queue.shift();

    if (!currentNodeId) {
      continue;
    }

    visitedCount += 1;
    const currentLayer = layerByNodeId.get(currentNodeId) ?? 0;

    for (const targetNodeId of outgoingTargetsByNodeId.get(currentNodeId) ??
      []) {
      layerByNodeId.set(
        targetNodeId,
        Math.max(layerByNodeId.get(targetNodeId) ?? 0, currentLayer + 1)
      );

      const nextIncomingCount =
        (remainingIncomingCountByNodeId.get(targetNodeId) ?? 0) - 1;
      remainingIncomingCountByNodeId.set(targetNodeId, nextIncomingCount);

      if (nextIncomingCount === 0) {
        queue.push(targetNodeId);
      }
    }
  }

  if (visitedCount < containerNodes.length) {
    const fallbackLayer = Math.max(0, ...layerByNodeId.values()) + 1;

    for (const node of containerNodes) {
      if (!layerByNodeId.has(node.id)) {
        layerByNodeId.set(node.id, fallbackLayer);
      }
    }
  }

  const nodesByLayer = new Map<number, typeof containerNodes>();

  for (const node of containerNodes) {
    const layer = layerByNodeId.get(node.id) ?? 0;
    nodesByLayer.set(layer, [...(nodesByLayer.get(layer) ?? []), node]);
  }

  const positions: Record<string, { x: number; y: number }> = {};
  const mountEdgesBySourceId = new Map<
    string,
    FlowAuthoringDocument['graph']['edges']
  >();
  const mountLaneByNodeId = collectMountLaneIds(containerEdges);

  for (const edge of containerEdges) {
    if (!isLlmToolSourceHandle(edge.sourceHandle)) {
      continue;
    }

    mountEdgesBySourceId.set(edge.source, [
      ...(mountEdgesBySourceId.get(edge.source) ?? []),
      edge
    ]);
  }

  for (const [layer, layerNodes] of [...nodesByLayer.entries()].sort(
    ([leftLayer], [rightLayer]) => leftLayer - rightLayer
  )) {
    const sortedLayerNodes = layerNodes
      .filter((node) => !mountLaneByNodeId.has(node.id))
      .sort(compareNodesByCanvasOrder);

    if (sortedLayerNodes.length === 0) {
      continue;
    }

    const layerHeight =
      sortedLayerNodes.reduce(
        (height, node) =>
          height + getCanvasNodeMinimumHeightForDocument(node),
        0
      ) + Math.max(0, sortedLayerNodes.length - 1) * ARRANGE_GAP_Y;
    const layerTop =
      ARRANGE_ORIGIN_Y + CANVAS_NODE_BASE_HEIGHT / 2 - layerHeight / 2;
    let nextNodeTop = layerTop;

    sortedLayerNodes.forEach((node) => {
      const nodeTop = snapToGrid(nextNodeTop);
      positions[node.id] = {
        x: snapToGrid(ARRANGE_ORIGIN_X + layer * ARRANGE_GAP_X),
        y: nodeTop
      };
      nextNodeTop =
        nodeTop +
        getCanvasNodeMinimumHeightForDocument(node) +
        ARRANGE_GAP_Y;
    });
  }

  const nodesByMountLaneId = new Map<string, typeof containerNodes>();

  for (const node of containerNodes) {
    const laneId = mountLaneByNodeId.get(node.id);

    if (!laneId) {
      continue;
    }

    nodesByMountLaneId.set(laneId, [
      ...(nodesByMountLaneId.get(laneId) ?? []),
      node
    ]);
  }

  const sourceIdsWithMounts = [...mountEdgesBySourceId.keys()].sort(
    (leftSourceId, rightSourceId) => {
      const leftLayer = layerByNodeId.get(leftSourceId) ?? 0;
      const rightLayer = layerByNodeId.get(rightSourceId) ?? 0;

      if (leftLayer !== rightLayer) {
        return leftLayer - rightLayer;
      }

      const leftNode = getNodeById(document, leftSourceId);
      const rightNode = getNodeById(document, rightSourceId);

      if (leftNode && rightNode) {
        return compareNodesByCanvasOrder(leftNode, rightNode);
      }

      return leftSourceId.localeCompare(rightSourceId);
    }
  );

  for (const sourceId of sourceIdsWithMounts) {
    const sourceNode = getNodeById(document, sourceId);
    const sourcePosition =
      positions[sourceId] ?? sourceNode?.position;

    if (!sourcePosition || !sourceNode) {
      continue;
    }

    const mountEdges = [...(mountEdgesBySourceId.get(sourceId) ?? [])].sort(
      (leftEdge, rightEdge) => {
        const leftTarget = getNodeById(document, leftEdge.target);
        const rightTarget = getNodeById(document, rightEdge.target);

        if (leftTarget && rightTarget) {
          const canvasOrder = compareNodesByCanvasOrder(
            leftTarget,
            rightTarget
          );

          if (canvasOrder !== 0) {
            return canvasOrder;
          }
        }

        return compareEdgesByHandleThenId(leftEdge, rightEdge);
      }
    );

    let nextLaneTop =
      sourcePosition.y +
      getCanvasNodeMinimumHeightForDocument(sourceNode) +
      ARRANGE_GAP_Y;

    mountEdges.forEach((edge) => {
      const laneNodes = nodesByMountLaneId.get(edge.id);

      if (!laneNodes) {
        return;
      }

      const laneTop = snapToGrid(nextLaneTop);

      for (const node of laneNodes.sort(
        (leftNode, rightNode) =>
          (layerByNodeId.get(leftNode.id) ?? 0) -
            (layerByNodeId.get(rightNode.id) ?? 0) ||
          compareNodesByCanvasOrder(leftNode, rightNode)
      )) {
        positions[node.id] = {
          x: snapToGrid(
            ARRANGE_ORIGIN_X + (layerByNodeId.get(node.id) ?? 0) * ARRANGE_GAP_X
          ),
          y: laneTop
        };
      }

      nextLaneTop =
        laneTop +
        Math.max(
          ...laneNodes.map((node) =>
            getCanvasNodeMinimumHeightForDocument(node)
          )
        ) +
        ARRANGE_GAP_Y;
    });
  }

  resolveAABBCollisions(
    positions,
    containerNodes,
    ARRANGE_GAP_Y,
    8
  );

  const finalPositions: Record<string, { x: number; y: number }> = {};
  for (const nodeId of Object.keys(positions)) {
    finalPositions[nodeId] = {
      x: snapToGrid(positions[nodeId].x),
      y: snapToGrid(positions[nodeId].y)
    };
  }

  return {
    ...document,
    graph: {
      ...document.graph,
      nodes: document.graph.nodes.map((node) =>
        finalPositions[node.id]
          ? {
              ...node,
              position: finalPositions[node.id]
            }
          : node
      )
    }
  };
}
