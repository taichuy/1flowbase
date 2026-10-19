import { useEffect, useRef } from 'react';
import { inTrajectoryRange, type TimelinePoint, type TrajectoryRange } from '../timeline-model';

export function DenseTrajectoryPoints({
  points,
  range,
  min,
  max
}: {
  points: TimelinePoint[];
  range: TrajectoryRange;
  min: number;
  max: number;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const element = canvas.current;
    if (!element) return;
    const draw = () => {
      const width = element.clientWidth;
      const height = element.clientHeight;
      if (!width || !height) return;
      const ratio = window.devicePixelRatio || 1;
      element.width = Math.round(width * ratio);
      element.height = Math.round(height * ratio);
      const context = element.getContext('2d');
      if (!context) return;
      context.scale(ratio, ratio);
      const style = getComputedStyle(element);
      const colors = Object.fromEntries(
        ['input', 'model', 'tool'].map((lane) => [
          lane,
          style.getPropertyValue('--trajectory-' + lane).trim()
        ])
      );
      const blockWidth = Math.max(
        5,
        width * Math.min(0.02, 0.65 / points.length)
      );
      for (const point of points) {
        context.fillStyle = colors[point.lane] || colors.model;
        context.globalAlpha = inTrajectoryRange(point.value, range) ? 0.9 : 0.2;
        const x = Math.max(
          0,
          Math.min(
            width,
            ((point.value - min) / Math.max(1, max - min)) * width
          )
        );
        const y = point.lane === 'input' ? 4 : point.lane === 'tool' ? 44 : 24;
        context.fillRect(x - 2, y, Math.min(blockWidth, width - x + 2), 10);
      }
    };
    draw();
    const observer = new ResizeObserver(draw);
    observer.observe(element);
    return () => observer.disconnect();
  }, [points, range, min, max]);
  return (
    <canvas
      className="trajectory-timeline__canvas"
      ref={canvas}
      aria-hidden="true"
    />
  );
}
