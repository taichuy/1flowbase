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
        if not migration.name.endswith('.down.sql'):
            sql += 'begin;\n' + migration.read_text() + '\ncommit;\n'
    sql += 'prepare save_cost(uuid) as ' + (here.parent / 'cost_snapshot.sql').read_text() + ';\n'
    sql += (here / 'cost_snapshot.sql').read_text()
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
