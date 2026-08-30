type IconWindow = {
  startRow: number;
  endRow: number;
  startIndex: number;
  endIndex: number;
};

function iconWindow({
  itemCount,
  scrollTop,
  columnCount,
  cellSize,
  viewportHeight,
  overscanRows
}: {
  itemCount: number;
  scrollTop: number;
  columnCount: number;
  cellSize: number;
  viewportHeight: number;
  overscanRows: number;
}): IconWindow {
  const rowCount = Math.ceil(itemCount / columnCount);
  const visibleRows = Math.ceil(viewportHeight / cellSize);
  const startRow = Math.max(
    0,
    Math.floor(scrollTop / cellSize) - overscanRows
  );
  const endRow = Math.min(
    rowCount,
    startRow + visibleRows + overscanRows * 2
  );
  return {
    startRow,
    endRow,
    startIndex: startRow * columnCount,
    endIndex: endRow * columnCount
  };
}

export { iconWindow };
export type { IconWindow };
