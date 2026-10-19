import '@xyflow/react/dist/style.css';

import { act, render, type RenderOptions } from '@testing-library/react';
import type { ReactElement } from 'react';

import { AppProviders } from '../../app/AppProviders';

const DEFAULT_SCENE_WIDTH = 1280;
const DEFAULT_SCENE_HEIGHT = 720;
const REACT_FLOW_TEST_STYLE_ID = 'react-flow-test-scene-style';
let layoutMetricsPatched = false;

interface RenderReactFlowSceneOptions
  extends Omit<RenderOptions, 'wrapper'> {
  width?: number;
  height?: number;
  withProviders?: boolean;
}

function ensureReactFlowTestStyles() {
  if (document.getElementById(REACT_FLOW_TEST_STYLE_ID)) {
    return;
  }

  const style = document.createElement('style');
  style.id = REACT_FLOW_TEST_STYLE_ID;
  style.textContent = `
    .react-flow-test-scene {
      position: relative;
      overflow: hidden;
    }

    .react-flow-test-scene .react-flow__pane {
      z-index: 1;
    }
  `;
  document.head.appendChild(style);
}

function readSceneDimension(
  element: HTMLElement,
  attributeName: 'data-react-flow-test-width' | 'data-react-flow-test-height'
) {
  const scene = element.closest('.react-flow-test-scene');

  if (!scene) {
    return null;
  }

  const rawValue = scene.getAttribute(attributeName);
  const dimension = rawValue ? Number(rawValue) : NaN;

  return Number.isFinite(dimension) ? dimension : null;
}

function patchLayoutMetric(
  propertyName: 'offsetWidth' | 'offsetHeight' | 'clientWidth' | 'clientHeight',
  attributeName: 'data-react-flow-test-width' | 'data-react-flow-test-height'
) {
  const originalDescriptor = Object.getOwnPropertyDescriptor(
    HTMLElement.prototype,
    propertyName
  );

  Object.defineProperty(HTMLElement.prototype, propertyName, {
    configurable: true,
    get() {
      const sceneDimension = readSceneDimension(this, attributeName);

      if (sceneDimension !== null) {
        return sceneDimension;
      }

      return originalDescriptor?.get ? originalDescriptor.get.call(this) : 0;
    }
  });
}

function ensureReactFlowTestLayoutMetrics() {
  if (layoutMetricsPatched) {
    return;
  }

  patchLayoutMetric('offsetWidth', 'data-react-flow-test-width');
  patchLayoutMetric('offsetHeight', 'data-react-flow-test-height');
  patchLayoutMetric('clientWidth', 'data-react-flow-test-width');
  patchLayoutMetric('clientHeight', 'data-react-flow-test-height');
  layoutMetricsPatched = true;
}

function resolveSceneDimension(value: number | undefined, fallback: number) {
  return Number.isFinite(value) && value !== undefined ? value : fallback;
}

function prepareReactFlowScene(
  ui: ReactElement,
  {
    width = DEFAULT_SCENE_WIDTH,
    height = DEFAULT_SCENE_HEIGHT,
    withProviders = true,
    ...renderOptions
  }: RenderReactFlowSceneOptions = {}
) {
  ensureReactFlowTestStyles();
  ensureReactFlowTestLayoutMetrics();

  const sceneWidth = resolveSceneDimension(width, DEFAULT_SCENE_WIDTH);
  const sceneHeight = resolveSceneDimension(height, DEFAULT_SCENE_HEIGHT);

  const frame = (
    <div
      ref={(node) => {
        if (!node) {
          return;
        }

        node.getBoundingClientRect = () =>
          ({
            x: 0,
            y: 0,
            width: sceneWidth,
            height: sceneHeight,
            top: 0,
            right: sceneWidth,
            bottom: sceneHeight,
            left: 0,
            toJSON: () => ({})
          }) as DOMRect;
      }}
      className="react-flow-test-scene"
      data-react-flow-test-width={sceneWidth}
      data-react-flow-test-height={sceneHeight}
      style={{ width: sceneWidth, height: sceneHeight }}
    >
      {ui}
    </div>
  );

  return {
    frame: withProviders ? <AppProviders>{frame}</AppProviders> : frame,
    renderOptions
  };
}

export function renderReactFlowScene(
  ui: ReactElement,
  options: RenderReactFlowSceneOptions = {}
) {
  const { frame, renderOptions } = prepareReactFlowScene(ui, options);
  return render(frame, renderOptions);
}

// Await mount effects at the fixture that owns the actual RTL render call.
export async function renderReactFlowSceneSettled(
  ui: ReactElement,
  options: RenderReactFlowSceneOptions = {}
) {
  const { frame, renderOptions } = prepareReactFlowScene(ui, options);
  let view!: ReturnType<typeof render>;
  await act(async () => {
    view = render(frame, renderOptions);
  });
  return view;
}
