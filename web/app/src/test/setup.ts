import '@testing-library/jest-dom/vitest';
import { vi } from 'vitest';

window.localStorage.setItem('1flowbase.ui.locale_preference', 'zh_Hans');

const originalConsoleError = console.error.bind(console);
const originalConsoleWarn = console.warn.bind(console);

function isKnownThirdPartyTestWarning(args: unknown[]) {
  const text = args.map((arg) => String(arg)).join(' ');

  if (
    text.includes('Decorators') &&
    text.includes('inside a test was not wrapped in act')
  ) {
    return true;
  }

  return args.some(
    (arg) =>
      typeof arg === 'string' &&
      ((arg.includes('rc-virtual-list') && arg.includes('max limitation')) ||
        arg.includes(
          'An update to Decorators inside a test was not wrapped in act'
        ))
  );
}

console.error = (...args: unknown[]) => {
  if (isKnownThirdPartyTestWarning(args)) {
    return;
  }

  originalConsoleError(...args);
};

console.warn = (...args: unknown[]) => {
  if (isKnownThirdPartyTestWarning(args)) {
    return;
  }

  originalConsoleWarn(...args);
};

Object.defineProperty(window, 'scrollTo', {
  value: vi.fn(),
  writable: true
});

Object.defineProperty(HTMLCanvasElement.prototype, 'getContext', {
  configurable: true,
  value: vi.fn((contextId: string) => {
    if (contextId !== '2d') {
      return null;
    }

    return {
      beginPath: vi.fn(),
      clearRect: vi.fn(),
      ellipse: vi.fn(),
      fill: vi.fn(),
      lineTo: vi.fn(),
      moveTo: vi.fn(),
      quadraticCurveTo: vi.fn(),
      restore: vi.fn(),
      rotate: vi.fn(),
      save: vi.fn(),
      scale: vi.fn(),
      stroke: vi.fn(),
      translate: vi.fn()
    };
  }),
  writable: true
});

Object.defineProperty(window, 'innerWidth', {
  value: 1280,
  writable: true
});

Object.defineProperty(window, 'innerHeight', {
  value: 800,
  writable: true
});

class DOMMatrixReadOnlyMock {
  readonly a = 1;
  readonly b = 0;
  readonly c = 0;
  readonly d = 1;
  readonly e = 0;
  readonly f = 0;
  readonly m11 = 1;
  readonly m12 = 0;
  readonly m13 = 0;
  readonly m14 = 0;
  readonly m21 = 0;
  readonly m22 = 1;
  readonly m23 = 0;
  readonly m24 = 0;
  readonly m31 = 0;
  readonly m32 = 0;
  readonly m33 = 1;
  readonly m34 = 0;
  readonly m41 = 0;
  readonly m42 = 0;
  readonly m43 = 0;
  readonly m44 = 1;
  readonly is2D = true;
  readonly isIdentity = true;

  constructor(_init?: string | number[]) {
    void _init;
  }

  transformPoint(point: DOMPointInit = {}) {
    return {
      x: point.x ?? 0,
      y: point.y ?? 0,
      z: point.z ?? 0,
      w: point.w ?? 1
    };
  }

  toString() {
    return 'matrix(1, 0, 0, 1, 0, 0)';
  }

  toJSON() {
    return { a: this.a, b: this.b, c: this.c, d: this.d, e: this.e, f: this.f };
  }
}

Object.defineProperty(window, 'DOMMatrixReadOnly', {
  writable: true,
  value: DOMMatrixReadOnlyMock
});

Object.defineProperty(HTMLElement.prototype, 'clientWidth', {
  configurable: true,
  get() {
    return 1280;
  }
});

Object.defineProperty(HTMLElement.prototype, 'clientHeight', {
  configurable: true,
  get() {
    return 800;
  }
});

Object.defineProperty(HTMLElement.prototype, 'scrollHeight', {
  configurable: true,
  get() {
    return this.clientHeight;
  }
});

Object.defineProperty(HTMLElement.prototype, 'scrollWidth', {
  configurable: true,
  get() {
    return this.clientWidth;
  }
});

Object.defineProperty(HTMLElement.prototype, 'scrollTo', {
  configurable: true,
  value: vi.fn()
});

Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', {
  configurable: true,
  value: vi.fn()
});

Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn()
  }))
});

class ResizeObserverMock {
  private readonly callback: ResizeObserverCallback;

  constructor(callback: ResizeObserverCallback) {
    this.callback = callback;
  }

  observe(target: Element) {
    const rect = target.getBoundingClientRect();
    const width =
      rect.width ||
      (target instanceof HTMLElement ? target.clientWidth : 1280) ||
      1280;
    const height =
      rect.height ||
      (target instanceof HTMLElement ? target.clientHeight : 800) ||
      800;

    this.callback(
      [
        {
          target,
          contentRect: {
            x: 0,
            y: 0,
            width,
            height,
            top: 0,
            left: 0,
            right: width,
            bottom: height,
            toJSON: () => ({})
          } as DOMRectReadOnly,
          borderBoxSize: [],
          contentBoxSize: [],
          devicePixelContentBoxSize: []
        } as ResizeObserverEntry
      ],
      this
    );
  }

  unobserve() {}
  disconnect() {}
}

Object.defineProperty(globalThis, 'ResizeObserver', {
  writable: true,
  value: ResizeObserverMock
});

const originalGetComputedStyle = window.getComputedStyle.bind(window);

function createCssPixelFallback(
  target: Element,
  propertyName: string,
  value: string
) {
  if (value && value !== 'NaN') {
    return value;
  }

  if (
    propertyName.startsWith('padding-') ||
    propertyName.endsWith('-width') ||
    propertyName === 'width'
  ) {
    return '0px';
  }

  if (propertyName === 'box-sizing') {
    return 'border-box';
  }

  if (propertyName === 'height') {
    return target instanceof HTMLElement &&
      target.classList.contains('ant-tabs-content-holder')
      ? '0px'
      : 'auto';
  }

  return value;
}

Object.defineProperty(window, 'getComputedStyle', {
  writable: true,
  value: vi.fn().mockImplementation((element: Element) => {
    const style = originalGetComputedStyle(element);
    const originalGetPropertyValue = style.getPropertyValue.bind(style);

    return new Proxy(style, {
      get(target, property, receiver) {
        if (property === 'getPropertyValue') {
          return (propertyName: string) =>
            createCssPixelFallback(
              element,
              propertyName,
              originalGetPropertyValue(propertyName)
            );
        }

        const value = Reflect.get(target, property, receiver);
        if (typeof property === 'string') {
          return createCssPixelFallback(element, property, value as string);
        }

        return value;
      }
    });
  })
});
