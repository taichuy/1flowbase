const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');

const {
  buildTemporaryFrontendCommand,
  collectRelationshipViolations,
  createScenePage,
  createProbeUrl,
  formatBoundaryFailure,
  formatRelationshipFailure,
  installStyleBoundaryNetworkMocks,
  isStyleBoundaryFrontendReady,
  parseCliArgs,
  resolveStyleBoundaryBaseUrl,
  resolveStyleBoundaryFrontendHost,
  resolveTemporaryFrontendPort,
  resolveSceneIds,
  resolveSceneViewport
} = require('../core.js');

test('parseCliArgs supports component, page, file, and all-pages modes', () => {
  assert.deepEqual(parseCliArgs(['component', 'component.account-popup']), {
    mode: 'component',
    target: 'component.account-popup',
    help: false
  });
  assert.deepEqual(parseCliArgs(['page', 'page.home']), {
    mode: 'page',
    target: 'page.home',
    help: false
  });
  assert.deepEqual(parseCliArgs(['file', 'web/app/src/styles/global.css']), {
    mode: 'file',
    target: 'web/app/src/styles/global.css',
    help: false
  });
  assert.deepEqual(parseCliArgs(['all-pages']), {
    mode: 'all-pages',
    target: null,
    help: false
  });
});

test('resolveSceneIds expands explicit file mappings and errors on missing coverage', () => {
  const manifest = [
    {
      id: 'component.account-popup',
      kind: 'component',
      impactFiles: ['web/app/src/styles/global.css']
    },
    {
      id: 'page.home',
      kind: 'page',
      impactFiles: [
        'web/app/src/styles/global.css',
        'web/app/src/features/home/HomePage.tsx'
      ]
    }
  ];

  assert.deepEqual(
    resolveSceneIds(manifest, {
      mode: 'file',
      target: 'web/app/src/features/home/HomePage.tsx'
    }),
    ['page.home']
  );
  assert.throws(
    () =>
      resolveSceneIds(manifest, {
        mode: 'file',
        target: 'web/app/src/features/unknown/Missing.tsx'
      }),
    /样式扩散失败/u
  );
});

test('createProbeUrl targets the dedicated Vite entry', () => {
  assert.equal(
    createProbeUrl('http://127.0.0.1:3100', 'page.home'),
    'http://127.0.0.1:3100/style-boundary.html?scene=page.home'
  );
});

test('createScenePage applies an explicit scene viewport before navigation', async () => {
  const calls = [];
  const page = {};
  const browser = {
    async newPage(...args) {
      calls.push(args);
      return page;
    }
  };

  assert.equal(
    await createScenePage(browser, {
      id: 'page.settings-i18n.mobile',
      viewport: { width: 390, height: 844 }
    }),
    page
  );
  assert.deepEqual(calls, [[{ viewport: { width: 390, height: 844 } }]]);
});

test('createScenePage preserves the browser default for scenes without viewport', async () => {
  const calls = [];
  const browser = {
    async newPage(...args) {
      calls.push(args);
      return {};
    }
  };

  await createScenePage(browser, { id: 'page.home' });

  assert.deepEqual(calls, [[]]);
  assert.equal(resolveSceneViewport({ id: 'page.home' }), undefined);
});

test('resolveSceneViewport rejects invalid dimensions before creating a page', async () => {
  let pageCreated = false;
  const browser = {
    async newPage() {
      pageCreated = true;
      return {};
    }
  };

  await assert.rejects(
    () =>
      createScenePage(browser, {
        id: 'page.settings-i18n.mobile',
        viewport: { width: 0, height: 844 }
      }),
    /width must be a positive integer/u
  );
  await assert.rejects(
    () =>
      createScenePage(browser, {
        id: 'page.settings-i18n.desktop',
        viewport: { width: 1280, height: 844.5 }
      }),
    /height must be a positive integer/u
  );
  assert.equal(pageCreated, false);
});

test('resolveStyleBoundaryBaseUrl defaults to the user frontend and respects explicit hosts', () => {
  assert.equal(resolveStyleBoundaryBaseUrl({}), 'http://127.0.0.1:3100');
  assert.equal(
    resolveStyleBoundaryBaseUrl({
      STYLE_BOUNDARY_BASE_URL: ' http://127.0.0.1:3199/ '
    }),
    'http://127.0.0.1:3199'
  );
});

test('resolveTemporaryFrontendPort never selects the user frontend port', async () => {
  const probedPorts = [];
  const port = await resolveTemporaryFrontendPort(
    {},
    {
      isPortAvailable: async (candidate) => {
        probedPorts.push(candidate);
        return candidate === 3101;
      }
    }
  );

  assert.equal(port, 3101);
  assert.deepEqual(probedPorts, [3101]);

  await assert.rejects(
    () =>
      resolveTemporaryFrontendPort(
        { STYLE_BOUNDARY_PORT: '3100' },
        { isPortAvailable: async () => true }
      ),
    /3100/u
  );
});

test('buildTemporaryFrontendCommand runs Vite on the isolated style-boundary port', () => {
  const command = buildTemporaryFrontendCommand('/repo', 3101, { PATH: '' });

  assert.equal(command.cwd, path.join('/repo', 'web', 'app'));
  assert.deepEqual(command.args, [
    'exec',
    'vite',
    '--host',
    '127.0.0.1',
    '--port',
    '3101',
    '--strictPort'
  ]);
  assert.equal(command.args.includes('3100'), false);
});

test('resolveStyleBoundaryFrontendHost starts an isolated host when the user frontend is not ready', async () => {
  const calls = [];
  const frontend = {
    baseUrl: 'http://127.0.0.1:3101',
    stop: async () => {}
  };

  const host = await resolveStyleBoundaryFrontendHost(
    {},
    '/repo',
    'page.home',
    {},
    {
      isStyleBoundaryFrontendReady: async (_browser, baseUrl, sceneId) => {
        calls.push(['probe', baseUrl, sceneId]);
        return false;
      },
      resolveTemporaryFrontendPort: async () => {
        calls.push(['resolve-port']);
        return 3101;
      },
      startTemporaryFrontend: (repoRoot, port, options) => {
        calls.push(['start', repoRoot, port, options.env]);
        return frontend;
      },
      waitForTemporaryFrontendReady: async (_browser, startedFrontend, sceneId) => {
        calls.push(['wait', startedFrontend.baseUrl, sceneId]);
      },
      writeStdout: () => {}
    }
  );

  assert.equal(host, frontend);
  assert.deepEqual(calls, [
    ['probe', 'http://127.0.0.1:3100', 'page.home'],
    ['resolve-port'],
    ['start', '/repo', 3101, {}],
    ['wait', 'http://127.0.0.1:3101', 'page.home']
  ]);
});

test('resolveStyleBoundaryFrontendHost does not start a fallback when an explicit host is not ready', async () => {
  let started = false;

  await assert.rejects(
    () =>
      resolveStyleBoundaryFrontendHost(
        {},
        '/repo',
        'page.home',
        { STYLE_BOUNDARY_BASE_URL: 'http://127.0.0.1:3199' },
        {
          isStyleBoundaryFrontendReady: async () => false,
          startTemporaryFrontend: () => {
            started = true;
          },
          writeStdout: () => {}
        }
      ),
    /STYLE_BOUNDARY_BASE_URL/u
  );
  assert.equal(started, false);
});

test('isStyleBoundaryFrontendReady reuses an already running style-boundary host', async () => {
  const calls = [];
  const page = {
    async route(pattern, handler) {
      calls.push({ type: 'route', pattern, handler });
    },
    async goto(url, options) {
      calls.push({ type: 'goto', url, options });
    },
    async waitForFunction(predicate, options) {
      calls.push({
        type: 'waitForFunction',
        predicateSource: predicate.toString(),
        options,
      });
    },
    async close() {
      calls.push({ type: 'close' });
    },
  };

  const ready = await isStyleBoundaryFrontendReady(
    {
      async newPage() {
        calls.push({ type: 'newPage' });
        return page;
      },
    },
    'http://127.0.0.1:3100',
    'page.home'
  );

  assert.equal(ready, true);
  assert.deepEqual(
    calls.map((entry) => entry.type),
    ['newPage', 'route', 'goto', 'waitForFunction', 'close']
  );
  assert.equal(
    calls[2].url,
    'http://127.0.0.1:3100/style-boundary.html?scene=page.home'
  );
});

test('installStyleBoundaryNetworkMocks fulfills user preference writes without a backend', async () => {
  const routes = [];
  const page = {
    async route(pattern, handler) {
      routes.push({ pattern, handler });
    }
  };

  await installStyleBoundaryNetworkMocks(page);

  assert.equal(routes.length, 1);
  assert.equal(routes[0].pattern, '**/api/console/me/meta');

  let fulfilled = null;
  await routes[0].handler({
    request() {
      return {
        postDataJSON() {
          return {
            meta: {
              ui: {
                data_tables: {
                  'applications.logs.runs': {
                    visibleColumnKeys: ['title']
                  }
                }
              }
            }
          };
        }
      };
    },
    async fulfill(payload) {
      fulfilled = payload;
    }
  });

  assert.equal(fulfilled.status, 200);
  assert.equal(fulfilled.contentType, 'application/json');
  assert.deepEqual(JSON.parse(fulfilled.body).data.meta, {
    ui: {
      data_tables: {
        'applications.logs.runs': {
          visibleColumnKeys: ['title']
        }
      }
    }
  });
});

test('formatBoundaryFailure labels style boundary regressions explicitly', () => {
  assert.equal(
    formatBoundaryFailure('page.home', [
      {
        nodeId: 'shell-header',
        property: 'display',
        expected: 'flex',
        actual: 'block',
        matchedRules: [
          {
            sourceUrl: 'http://127.0.0.1:3100/src/styles/global.css',
            selector: '.app-shell-header'
          }
        ]
      }
    ]),
    '样式边界失败：page.home shell-header.display expected=flex actual=block source=http://127.0.0.1:3100/src/styles/global.css::.app-shell-header'
  );
});

test('collectRelationshipViolations detects no_overlap, within_container, min_gap, fully_visible, and fills_container_bottom regressions', () => {
  const assertions = [
    {
      id: 'left-vs-sidebar',
      type: 'no_overlap',
      subjectSelector: '.left',
      referenceSelector: '.sidebar'
    },
    {
      id: 'actions-within-left',
      type: 'within_container',
      subjectSelector: '.actions',
      containerSelector: '.left'
    },
    {
      id: 'left-gap-sidebar',
      type: 'min_gap',
      axis: 'horizontal',
      minGap: 24,
      subjectSelector: '.left',
      referenceSelector: '.sidebar'
    },
    {
      id: 'actions-visible',
      type: 'fully_visible',
      subjectSelector: '.actions'
    },
    {
      id: 'panel-fills-tab-body',
      type: 'fills_container_bottom',
      subjectSelector: '.panel',
      containerSelector: '.tab-body',
      maxBottomGap: 4
    }
  ];
  const measurements = {
    '.left': {
      exists: true,
      rect: { left: 0, top: 0, right: 300, bottom: 200, width: 300, height: 200 }
    },
    '.sidebar': {
      exists: true,
      rect: { left: 280, top: 0, right: 520, bottom: 200, width: 240, height: 200 }
    },
    '.actions': {
      exists: true,
      rect: { left: 260, top: 20, right: 340, bottom: 60, width: 80, height: 40 },
      withinViewport: true,
      visibleSamples: [true, false, true, true, true]
    },
    '.panel': {
      exists: true,
      rect: { left: 0, top: 0, right: 300, bottom: 420, width: 300, height: 420 }
    },
    '.tab-body': {
      exists: true,
      rect: { left: 0, top: 0, right: 300, bottom: 576, width: 300, height: 576 }
    }
  };

  assert.deepEqual(
    collectRelationshipViolations(assertions, measurements).map((violation) => ({
      id: violation.assertionId,
      type: violation.type,
      actual: violation.actual
    })),
    [
      { id: 'left-vs-sidebar', type: 'no_overlap', actual: 'overlap' },
      { id: 'actions-within-left', type: 'within_container', actual: 'outside_container' },
      { id: 'left-gap-sidebar', type: 'min_gap', actual: 'gap_too_small' },
      { id: 'actions-visible', type: 'fully_visible', actual: 'partially_occluded' },
      {
        id: 'panel-fills-tab-body',
        type: 'fills_container_bottom',
        actual: 'bottom_gap_out_of_range'
      }
    ]
  );
});

test('formatRelationshipFailure labels layout relationship regressions explicitly', () => {
  assert.equal(
    formatRelationshipFailure('page.settings', [
      {
        assertionId: 'left-vs-sidebar',
        type: 'no_overlap',
        actual: 'overlap',
        details: 'intersection=20x200',
        subjectSelector: '.left',
        referenceSelector: '.sidebar'
      }
    ]),
    '布局关系失败：page.settings left-vs-sidebar.no_overlap actual=overlap subject=.left reference=.sidebar details=intersection=20x200'
  );
});
