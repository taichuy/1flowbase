"""Run the cost SQL regression without linking the full Rust integration host.

Set API_DATABASE_URL (or DATABASE_URL) to a development PostgreSQL database.
The test applies the real forward migrations to an isolated schema and removes
that schema in finally. Output is retained under tmp/test-governance/.
"""
import os
from pathlib import Path
import subprocess
from urllib.parse import unquote, urlparse
from uuid import uuid4


def main():
    here = Path(__file__).resolve().parent
    root = next(p for p in here.parents if (p / 'api/Cargo.toml').exists())
    url = os.environ.get('API_DATABASE_URL') or os.environ['DATABASE_URL']
    connection = urlparse(url)
    env = dict(os.environ, PGHOST=connection.hostname,
               PGPORT=str(connection.port or 5432),
               PGUSER=unquote(connection.username),
               PGPASSWORD=unquote(connection.password or ''),
               PGDATABASE=connection.path.lstrip('/'))
    schema = 'cost_snapshot_test_' + uuid4().hex
    log = root / 'tmp/test-governance/application-log-cost-sql.log'
    log.parent.mkdir(parents=True, exist_ok=True)
    command = ['psql', '-X', '-q', '-v', 'ON_ERROR_STOP=1']
    migrations = root / 'api/crates/storage/durable/postgres/migrations'
    sql = f'create schema {schema}; set search_path to {schema};\n'
    for migration in sorted(migrations.glob('*.sql')):
        if migration.name == '20260921150000_group_application_log_cost_by_currency.sql':
            fixture = (here / 'cost_snapshot.sql').read_text().split('-- A failed provider attempt')[0]
            before, after = (here / 'cost_snapshot_upgrade.sql').read_text().split('-- APPLY MIGRATION')
            sql += fixture + before + migration.read_text() + after + '\nrollback;\n'
        if migration.name == '20260921230000_record_application_log_cost_as_credits.sql':
            fixture = (here / 'cost_snapshot.sql').read_text().split('-- A failed provider attempt')[0]
            before, after = (here / 'cost_credit_upgrade.sql').read_text().split('-- APPLY MIGRATION')
            sql += fixture + before + migration.read_text() + after + '\nrollback;\n'
        if migration.name == '20260922130000_repair_superseded_log_cost_snapshots.sql':
            fixture = (here / 'cost_snapshot.sql').read_text().split('-- A failed provider attempt')[0]
            before, after = (here / 'superseded_cost_upgrade.sql').read_text().split('-- APPLY MIGRATION')
            sql += fixture + before + migration.read_text() + after + '\nrollback;\n'
        if not migration.name.endswith('.down.sql'):
            sql += 'begin;\n' + migration.read_text() + '\ncommit;\n'
    sql += 'prepare save_cost(uuid) as ' + (here.parent / 'cost_snapshot.sql').read_text() + ';\n'
    sql += (here / 'cost_snapshot.sql').read_text()
    sql += (here / 'cost_snapshot.sql').read_text().split('-- A failed provider attempt')[0]
    source = (here.parent / 'run_conversation_message_item_methods.rs').read_text()
    cte = source.split('fn application_run_task_message_items_cte()')[1].split('r#"')[1].split('"#')[0]
    # Exercise the exact production conversation read, not a test-only equivalent.
    sql += 'prepare read_task(uuid,uuid,integer) as ' + cte + ' select * from task_message_items;\n'
    sql += (here / 'projection_settlement.sql').read_text() + '\nrollback;\n'
    with log.open('w') as output:
        try:
            result = subprocess.run(command, input=sql, text=True, env=env,
                                    stdout=output, stderr=output, check=False)
        finally:
            subprocess.run(command, input=f'drop schema if exists {schema} cascade;',
                           text=True, env=env, stdout=output, stderr=output, check=True)
    if result.returncode:
        raise SystemExit(f'Cost snapshot regression failed; see {log}')
    print(f'Cost snapshot regression passed; log: {log}')


if __name__ == '__main__':
    main()
