export const NODE_DETAIL_DEFAULT_WIDTH = 420;
export const NODE_DETAIL_MIN_WIDTH = 300;
export const NODE_DETAIL_MIN_CANVAS_WIDTH = 300;
export const NODE_DETAIL_COMPACT_THRESHOLD = 360;

export function getNodeDetailLayout(width: number) {
  return width <= NODE_DETAIL_COMPACT_THRESHOLD ? 'compact' : 'regular';
}

export function getMaxNodeDetailWidth(containerWidth: number) {
  return Math.max(
    containerWidth - NODE_DETAIL_MIN_CANVAS_WIDTH,
    NODE_DETAIL_MIN_WIDTH
  );
}

export function clampNodeDetailWidth(width: number, containerWidth: number) {
  return Math.min(
    Math.max(width, NODE_DETAIL_MIN_WIDTH),
    getMaxNodeDetailWidth(containerWidth)
  );
}

export function getNodeDetailWidthFromSplitter(
  sizes: number[],
  containerWidth: number
) {
  const detailWidth = sizes.at(-1) ?? NODE_DETAIL_DEFAULT_WIDTH;

  return clampNodeDetailWidth(detailWidth, containerWidth);
}
