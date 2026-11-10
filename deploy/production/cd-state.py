"""WAL-aware deployment backup and monotonic traffic-history verification."""
import json
from contextlib import closing
import os
from pathlib import Path
import sqlite3
import sys
import time


def connect(path):
    return sqlite3.connect(Path(path).resolve().as_uri() + '?mode=ro', uri=True)


def backup(source, destination):
    # Exclusive creation prevents a retry from replacing a previous backup.
    fd = os.open(destination, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    os.close(fd)
    deadline = time.monotonic() + 60

    def progress(*_):
        if time.monotonic() > deadline:
            raise TimeoutError('Database backup deadline exceeded')

    with closing(connect(source)) as live, closing(sqlite3.connect(destination)) as saved:
        live.backup(saved, pages=256, progress=progress)
        if saved.execute('PRAGMA integrity_check').fetchall() != [('ok',)]:
            raise RuntimeError('Database backup integrity failed')


def verify(before, after, now_ms=None):
    # Match the application's existing retention; expiry is not a deploy reset.
    cutoff = (time.time() * 1000 if now_ms is None else now_ms) - 400 * 86400000
    with closing(connect(before)) as old, closing(connect(after)) as live:
        old_tables = {r[0] for r in old.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        # Existing durable room and participant-history identities must survive.
        for table in ('rooms',):
            if table in old_tables:
                for (identity,) in old.execute('SELECT id FROM ' + table):
                    if not live.execute('SELECT 1 FROM ' + table + ' WHERE id=?', (identity,)).fetchone():
                        raise RuntimeError('Deployment removed durable history')
        if 'usage_minutes' in old_tables:
            for (identity,) in old.execute('SELECT DISTINCT user FROM usage_minutes WHERE minute>=?', (cutoff,)):
                if not live.execute('SELECT 1 FROM usage_users WHERE id=?', (identity,)).fetchone():
                    raise RuntimeError('Deployment removed a traffic-history participant')
            for minute, user, uploaded, downloaded in old.execute(
                'SELECT minute,user,uploaded,downloaded FROM usage_minutes WHERE minute>=?', (cutoff,)
            ):
                row = live.execute(
                    'SELECT uploaded,downloaded FROM usage_minutes WHERE minute=? AND user=?',
                    (minute, user),
                ).fetchone()
                if row is None or row[0] < uploaded or row[1] < downloaded:
                    raise RuntimeError('Deployment removed or reduced traffic history')
        if 'usage_coverage' in old_tables:
            for minute, milliseconds in old.execute('SELECT minute,milliseconds FROM usage_coverage WHERE minute>=?', (cutoff,)):
                row = live.execute('SELECT milliseconds FROM usage_coverage WHERE minute=?', (minute,)).fetchone()
                if row is None or row[0] < milliseconds:
                    raise RuntimeError('Deployment removed traffic coverage')
        if live.execute('PRAGMA integrity_check').fetchall() != [('ok',)]:
            raise RuntimeError('Deployed database integrity failed')
    return {'durable_rooms_and_traffic_preserved': True}


if __name__ == '__main__':
    action, source, target = sys.argv[1:]
    if action == 'backup':
        backup(source, target)
        print('WAL-aware database backup verified')
    elif action == 'verify':
        print(json.dumps(verify(source, target)))
    else:
        raise SystemExit('Expected backup or verify')
