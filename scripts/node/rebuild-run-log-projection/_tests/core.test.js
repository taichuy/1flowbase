const test = require('node:test');
const assert = require('node:assert/strict');

const {
  buildApplySql,
  buildPsqlInvocation,
  main,
  parseApplyRow,
  parseCliArgs,
  parsePreviewRow,
  resolveDatabaseUrl
} = require('../core.js');

const RUN_ID = '01a0ba4c-f858-7202-8c86-f3f26cb60ac4';

test('parseCliArgs defaults to a read-only preview', () => {
  assert.deepEqual(parseCliArgs([]), {
    after: null,
    batchSize: 200,
    command: 'preview',
    databaseUrl: null,
    help: false,
    maxBatches: 1
  });
});

test('parseCliArgs accepts a bounded apply with a resume cursor', () => {
  const options = parseCliArgs([
    'apply',
    '--batch-size',
    '50',
    '--max-batches',
    '3',
    '--after',
    RUN_ID
  ]);

  assert.equal(options.command, 'apply');
  assert.equal(options.batchSize, 50);
  assert.equal(options.maxBatches, 3);
  assert.equal(options.after, RUN_ID);
});

test('parseCliArgs rejects an unbounded batch size and a non-uuid cursor', () => {
  assert.throws(() => parseCliArgs(['apply', '--batch-size', '999999']), /batch-size/u);
  assert.throws(() => parseCliArgs(['apply', '--after', 'not-a-uuid']), /after/u);
  assert.throws(() => parseCliArgs(['rebuild']), /未知命令/u);
});

test('resolveDatabaseUrl prefers the flag, then the environment, then the api-server env file', () => {
  const repoRoot = '/repo';

  assert.equal(
    resolveDatabaseUrl({
      options: { databaseUrl: 'postgres://flag/db' },
      env: { API_DATABASE_URL: 'postgres://env/db' },
      repoRoot,
      readEnvFileImpl: () => ({})
    }),
    'postgres://flag/db'
  );
  assert.throws(
    () => resolveDatabaseUrl({ options: {}, env: {}, repoRoot, readEnvFileImpl: () => ({}) }),
    /未找到数据库 URL/u
  );
});

test('preview SQL never writes and reports the rebuild scope', () => {
  const { PREVIEW_SQL } = require('../core.js');
  assert.doesNotMatch(PREVIEW_SQL, /delete|update|insert/iu);
  assert.match(PREVIEW_SQL, /application_run_message_projection_watermark/u);
  assert.match(PREVIEW_SQL, /application_run_conversation_message_items/u);
});

test('apply SQL is bounded, locks the run rows and only deletes derived rows', () => {
  const sql = buildApplySql({ after: RUN_ID, batchSize: 25 });

  assert.match(sql, /limit 25/u);
  assert.match(sql, /p\.flow_run_id > '01a0ba4c-f858-7202-8c86-f3f26cb60ac4'::uuid/u);
  assert.match(sql, /for update/u);
  assert.match(sql, /delete from application_run_conversation_message_items/u);
  assert.doesNotMatch(sql, /delete from (flow_runs|runtime_events|node_runs)/u);
  assert.doesNotMatch(sql, /update /iu);
});

test('apply SQL starts without a cursor when none was supplied', () => {
  const sql = buildApplySql({ after: null, batchSize: 10 });
  assert.match(sql, /\(null::uuid is null or p\.flow_run_id > null::uuid\)/u);
});

test('apply SQL rejects an unsafe batch size or cursor instead of interpolating it', () => {
  assert.throws(
    () => buildApplySql({ after: 'x; drop table flow_runs', batchSize: 10 }),
    /无效续跑游标/u
  );
  assert.throws(() => buildApplySql({ after: null, batchSize: '10; drop table' }), /无效批次大小/u);
});

test('preview and apply rows parse into the reported shape', () => {
  assert.deepEqual(parsePreviewRow(`3|41|7|${RUN_ID},other`), {
    staleRuns: 3,
    staleRows: 41,
    unprojectedRuns: 7,
    sampleRuns: [RUN_ID, 'other']
  });
  assert.deepEqual(parseApplyRow(`2|12|${RUN_ID}|2`), {
    purgedRuns: 2,
    purgedRows: 12,
    lastRunId: RUN_ID,
    batchRuns: 2
  });
  assert.deepEqual(parseApplyRow('0|0||0'), {
    purgedRuns: 0,
    purgedRows: 0,
    lastRunId: null,
    batchRuns: 0
  });
});

test('main preview prints the scope and persists nothing', () => {
  const calls = [];
  const output = [];
  const status = main(['preview', '--database-url', 'postgres://user:pw@127.0.0.1:5432/db'], {
    repoRoot: '/repo',
    env: {},
    writeStdout: (text) => output.push(text),
    execFileSyncImpl: (command, args) => {
      calls.push({ command, args });
      return `3|41|7|${RUN_ID},second\n`;
    }
  });

  assert.equal(status, 0);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].command, 'psql');
  assert.match(output.join(''), /stale_runs=3 stale_rows=41 unprojected_runs=7/u);
  assert.match(output.join(''), new RegExp(RUN_ID));
});

test('main apply stops when a batch is empty and reports the resume cursor', () => {
  const sqls = [];
  const output = [];
  const status = main(
    ['apply', '--max-batches', '5', '--batch-size', '2', '--database-url', 'postgres://u:p@h:5432/db'],
    {
      repoRoot: '/repo',
      env: {},
      writeStdout: (text) => output.push(text),
      execFileSyncImpl: (_command, args) => {
        sqls.push(args.at(-1));
        if (sqls.length === 1) {
          return `2|12|${RUN_ID}|2\n`;
        }
        return '0|0||0\n';
      }
    }
  );

  assert.equal(status, 0);
  assert.equal(sqls.length, 2);
  assert.match(sqls[0], /\(null::uuid is null/u);
  assert.match(sqls[1], new RegExp(`> '${RUN_ID}'::uuid`, 'u'));
  assert.match(output.join(''), /purged_runs=2 purged_rows=12/u);
  assert.match(output.join(''), new RegExp(`resume_with=--after ${RUN_ID}`, 'u'));
});

test('psql invocation reads credentials from the URL and keeps the query single-statement safe', () => {
  const invocation = buildPsqlInvocation({
    databaseUrl: 'postgres://postgres:secret@127.0.0.1:35432/1flowbase',
    sql: 'select 1'
  });

  assert.deepEqual(invocation.args.slice(0, 9), [
    '-h',
    '127.0.0.1',
    '-p',
    '35432',
    '-U',
    'postgres',
    '-d',
    '1flowbase',
    '-X'
  ]);
  assert.equal(invocation.env.PGPASSWORD, 'secret');
  assert.match(invocation.args.join(' '), /ON_ERROR_STOP=1/u);
});
