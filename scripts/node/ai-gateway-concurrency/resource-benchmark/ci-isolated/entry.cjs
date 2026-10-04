'use strict';
const { configuration } = require('../config.cjs');
const config = configuration();
config.sampleMs = 60000;
require('../run.cjs').run(config).then(ok => { if (!ok) process.exitCode = 1; })
  .catch(() => { console.error('isolated fixture failed; inspect sanitized report'); process.exitCode = 1; });
