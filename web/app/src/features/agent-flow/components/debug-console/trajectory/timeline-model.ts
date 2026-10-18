export type TrajectoryRange = readonly [number, number] | null;
export type TimelinePoint = {
  id: string;
  value: number;
  lane: string;
  label: string;
};
export function inTrajectoryRange(value: number, range: TrajectoryRange) {
  return !range || (value >= range[0] && value <= range[1]);
}

export function nearestTimelinePoint(points: TimelinePoint[], value: number) {
  if (!points.length) return undefined;
  let low = 0,
    high = points.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if (points[middle].value < value) low = middle + 1;
    else high = middle;
  }
  const before = points[Math.max(0, low - 1)];
  const after = points[Math.min(points.length - 1, low)];
  return Math.abs(before.value - value) <= Math.abs(after.value - value)
    ? before
    : after;
}
