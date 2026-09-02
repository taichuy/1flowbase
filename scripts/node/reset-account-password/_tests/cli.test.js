const test = require('node:test');
const assert = require('node:assert/strict');

const { main } = require('../cli.js');

test('AC-004 refuses direct account password reset in production', async () => {
  await assert.rejects(
    () => main([], {
      API_ENV: 'production',
      API_DATABASE_URL: 'postgres://postgres:secret@localhost/flowbase',
      BOOTSTRAP_ROOT_ACCOUNT: 'root',
      BOOTSTRAP_ROOT_PASSWORD: 'change-me',
    }),
    /automatic account password reset is disabled in production/u
  );
});
