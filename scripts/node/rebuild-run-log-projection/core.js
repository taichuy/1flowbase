#!/usr/bin/env node

const path = require('node:path');
const { execFileSync } = require('node:child_process');

const { parseEnvFile } = require('../dev-up/env.js');

const DEFAULT_BATCH_SIZE = 200;
const MAX_BATCH_SIZE = 5000;
const DEFAULT_MAX_BATCHES = 1;

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(`用法：node scripts/node/cli/rebuild-run-log-projection.js [preview|apply] [选项]

派生投影的有界历史重建。它只失效已经过期的派生行，随后由唯一的投影写入者
（运行会话读取路径）按保留的原始事实重新生成；不修改原始事件、请求或回答，
也不重放模型调用。

命令：
  preview   只统计待重建范围，不写入任何数据
  apply     分批失效过期派生行；已失效或已重建的运行自动跳过（幂等）

选项：
  --batch-size <n>     每批处理的运行数，默认 ${DEFAULT_BATCH_SIZE}
  --max-batches <n>    最多执行多少批，默认 ${DEFAULT_MAX_BATCHES}
  --after <uuid>       从该运行之后继续，用于中断后续跑
  --database-url <u>   数据库 URL；默认读取 API_DATABASE_URL / DATABASE_URL / api-server .env
  -h, --help           查看帮助

回滚 / 前滚：本命令只删除派生投影行。回滚即停止执行并保留当前代码的投影写入者
（下次读取按当前代码重新生成）；前滚即再次执行 apply。
`);
}

function parseCliArgs(argv = []) {
  const options = {
    after: null,
    batchSize: DEFAULT_BATCH_SIZE,
    command: 'preview',
    databaseUrl: null,
    help: false,
    maxBatches: DEFAULT_MAX_BATCHES
  };

  const args = [...argv];
  if (args.length > 0 && !args[0].startsWith('-')) {
    options.command = args.shift();
  }

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    const next = () => {
      index += 1;
      const value = args[index];
      if (value === undefined) {
        throw new Error(`选项缺少取值：${arg}`);
      }
      return value;
    };

    switch (arg) {
      case '--after':
        options.after = next();
        break;
      case '--batch-size':
        options.batchSize = Number.parseInt(next(), 10);
        break;
      case '--database-url':
        options.databaseUrl = next();
        break;
      case '--max-batches':
        options.maxBatches = Number.parseInt(next(), 10);
        break;
      case '-h':
      case '--help':
        options.help = true;
        break;
      default:
        throw new Error(`未知选项：${arg}`);
    }
  }

  if (!['preview', 'apply'].includes(options.command)) {
    throw new Error(`未知命令：${options.command}`);
  }
  if (
    !Number.isInteger(options.batchSize) ||
    options.batchSize < 1 ||
    options.batchSize > MAX_BATCH_SIZE
  ) {
    throw new Error(`--batch-size 必须是 1..${MAX_BATCH_SIZE} 的整数`);
  }
  if (!Number.isInteger(options.maxBatches) || options.maxBatches < 1) {
    throw new Error('--max-batches 必须是正整数');
  }
  if (options.after !== null && !isUuid(options.after)) {
    throw new Error(`--after 必须是 uuid：${options.after}`);
  }

  return options;
}

function isUuid(value) {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u.test(
    String(value || '').trim()
  );
}

/// Same precedence as the other maintenance entry: an explicit flag first, then
/// the running api-server environment, then the api-server .env file.
function resolveDatabaseUrl({
  options,
  env = process.env,
  repoRoot,
  readEnvFileImpl = parseEnvFile
} = {}) {
  const fileEnv =
    readEnvFileImpl(
      path.join(repoRoot || getRepoRoot(), 'api', 'apps', 'api-server', '.env')
    ) || {};

  const databaseUrl =
    options?.databaseUrl ||
    env.API_DATABASE_URL ||
    env.DATABASE_URL ||
    fileEnv.API_DATABASE_URL ||
    fileEnv.DATABASE_URL ||
    null;

  if (!databaseUrl) {
    throw new Error(
      '未找到数据库 URL：请传入 --database-url 或设置 API_DATABASE_URL / DATABASE_URL'
    );
  }

  return databaseUrl;
}

/// A run needs a rebuild when it still has derived rows written from facts the
/// retained originals no longer produce. Runs without any derived rows are only
/// reported: the single projection writer creates them on demand.
const PREVIEW_SQL = `with projected as (
    select distinct flow_run_id from application_run_conversation_message_items
), stale as (
    select p.flow_run_id
    from projected p
    where exists (
        select 1
        from application_run_conversation_message_items m
        where m.flow_run_id = p.flow_run_id
          and m.source_revision is distinct from
              application_run_message_projection_watermark(p.flow_run_id)
    )
), unprojected as (
    select s.flow_run_id
    from application_run_log_summaries s
    where not exists (select 1 from projected p where p.flow_run_id = s.flow_run_id)
)
select
    (select count(*) from stale)::text
    || '|' || (select count(*) from application_run_conversation_message_items m
               join stale on stale.flow_run_id = m.flow_run_id)::text
    || '|' || (select count(*) from unprojected)::text
    || '|' || coalesce((select string_agg(flow_run_id::text, ',' order by flow_run_id)
                        from (select flow_run_id from stale order by flow_run_id limit 5) head), '')`;

/// One batch, one statement, one implicit transaction: it either invalidates a
/// whole batch of runs or nothing. The flow_run rows are locked exactly like the
/// read path locks them, so a concurrent read reprojecting the same run and this
/// purge can never interleave into a stale write.
function buildApplySql({ after, batchSize }) {
  const afterLiteral = after === null || after === undefined ? 'null' : `'${after}'`;
  const size =
    typeof batchSize === 'number'
      ? batchSize
      : /^[0-9]+$/u.test(String(batchSize ?? '').trim())
        ? Number.parseInt(String(batchSize).trim(), 10)
        : Number.NaN;
  if (!Number.isInteger(size) || size < 1 || size > MAX_BATCH_SIZE) {
    throw new Error(`无效批次大小：${batchSize}`);
  }
  if (afterLiteral !== 'null' && !isUuid(after)) {
    throw new Error(`无效续跑游标：${after}`);
  }
  const sizeLiteral = String(size);

  return `with projected as (
    select distinct flow_run_id from application_run_conversation_message_items
), batch as (
    select p.flow_run_id
    from projected p
    where (${afterLiteral}::uuid is null or p.flow_run_id > ${afterLiteral}::uuid)
      and exists (
          select 1
          from application_run_conversation_message_items m
          where m.flow_run_id = p.flow_run_id
            and m.source_revision is distinct from
                application_run_message_projection_watermark(p.flow_run_id)
      )
    order by p.flow_run_id
    limit ${sizeLiteral}
), locked as (
    select f.id from flow_runs f join batch on batch.flow_run_id = f.id for update
), removed as (
    delete from application_run_conversation_message_items m
    using locked
    where m.flow_run_id = locked.id
    returning m.flow_run_id
)
select
    (select count(distinct flow_run_id) from removed)::text
    || '|' || (select count(*) from removed)::text
    || '|' || coalesce((select flow_run_id::text from batch order by flow_run_id desc limit 1), '')
    || '|' || (select count(*) from batch)::text`;
}

function buildPsqlInvocation({ databaseUrl, sql }) {
  const url = new URL(databaseUrl);
  const database = decodeURIComponent(url.pathname.replace(/^\//u, ''));
  if (!database) {
    throw new Error('数据库 URL 缺少数据库名');
  }

  return {
    args: [
      '-h',
      url.hostname,
      '-p',
      url.port || '5432',
      '-U',
      decodeURIComponent(url.username),
      '-d',
      database,
      '-X',
      '-A',
      '-t',
      '-v',
      'ON_ERROR_STOP=1',
      '-c',
      sql
    ],
    env: {
      ...process.env,
      PGPASSWORD: decodeURIComponent(url.password)
    }
  };
}

function runPsql({ databaseUrl, sql, execFileSyncImpl = execFileSync }) {
  const invocation = buildPsqlInvocation({ databaseUrl, sql });
  return String(
    execFileSyncImpl('psql', invocation.args, {
      encoding: 'utf8',
      env: invocation.env,
      stdio: ['ignore', 'pipe', 'pipe']
    }) || ''
  ).trim();
}

function parsePreviewRow(row) {
  const [staleRuns, staleRows, unprojectedRuns, sampleRuns] = String(row).split('|');
  return {
    staleRuns: Number.parseInt(staleRuns || '0', 10),
    staleRows: Number.parseInt(staleRows || '0', 10),
    unprojectedRuns: Number.parseInt(unprojectedRuns || '0', 10),
    sampleRuns: (sampleRuns || '').split(',').filter(Boolean)
  };
}

function parseApplyRow(row) {
  const [purgedRuns, purgedRows, lastRunId, batchRuns] = String(row).split('|');
  return {
    purgedRuns: Number.parseInt(purgedRuns || '0', 10),
    purgedRows: Number.parseInt(purgedRows || '0', 10),
    lastRunId: lastRunId || null,
    batchRuns: Number.parseInt(batchRuns || '0', 10)
  };
}

function main(argv = [], deps = {}) {
  const writeStdout = deps.writeStdout || ((text) => process.stdout.write(text));
  const options = parseCliArgs(argv);

  if (options.help) {
    usage(writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const databaseUrl = resolveDatabaseUrl({
    options,
    env: deps.env || process.env,
    repoRoot
  });
  const execFileSyncImpl = deps.execFileSyncImpl || execFileSync;

  if (options.command === 'preview') {
    const preview = parsePreviewRow(
      runPsql({ databaseUrl, sql: PREVIEW_SQL, execFileSyncImpl })
    );
    writeStdout(
      `[rebuild-run-log-projection] preview: stale_runs=${preview.staleRuns} stale_rows=${preview.staleRows} unprojected_runs=${preview.unprojectedRuns}\n`
    );
    if (preview.sampleRuns.length > 0) {
      writeStdout(
        `[rebuild-run-log-projection] sample_runs=${preview.sampleRuns.join(',')}\n`
      );
    }
    return 0;
  }

  let after = options.after;
  let lastRunId = after;
  for (let index = 0; index < options.maxBatches; index += 1) {
    const result = parseApplyRow(
      runPsql({
        databaseUrl,
        sql: buildApplySql({ after, batchSize: options.batchSize }),
        execFileSyncImpl
      })
    );
    writeStdout(
      `[rebuild-run-log-projection] batch=${index + 1} purged_runs=${result.purgedRuns} purged_rows=${result.purgedRows} last_run_id=${result.lastRunId || '-'}\n`
    );
    if (result.batchRuns === 0) {
      break;
    }
    lastRunId = result.lastRunId;
    after = result.lastRunId;
  }

  writeStdout(
    `[rebuild-run-log-projection] resume_with=--after ${lastRunId || '<none>'}\n`
  );
  return 0;
}

module.exports = {
  DEFAULT_BATCH_SIZE,
  MAX_BATCH_SIZE,
  PREVIEW_SQL,
  buildApplySql,
  buildPsqlInvocation,
  isUuid,
  main,
  parseApplyRow,
  parseCliArgs,
  parsePreviewRow,
  resolveDatabaseUrl,
  runPsql,
  usage
};
