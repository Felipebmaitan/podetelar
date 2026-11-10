import importlib.util
from contextlib import closing, ExitStack
from pathlib import Path
import sqlite3
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('cd_state', Path(__file__).with_name('cd-state.py'))
state = importlib.util.module_from_spec(spec)
spec.loader.exec_module(state)


class HistoryProtection(unittest.TestCase):
    def test_wal_backup_growth_and_destructive_updates(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            live = Path(directory) / 'live.db'
            saved = Path(directory) / 'saved.db'
            db = stack.enter_context(closing(sqlite3.connect(live)))
            db.executescript('''
                PRAGMA journal_mode=WAL;
                CREATE TABLE rooms(id TEXT PRIMARY KEY);
                CREATE TABLE usage_users(id TEXT PRIMARY KEY);
                CREATE TABLE usage_minutes(minute INTEGER,user TEXT,uploaded REAL,downloaded REAL,PRIMARY KEY(minute,user));
                CREATE TABLE usage_coverage(minute INTEGER PRIMARY KEY,milliseconds INTEGER);
                INSERT INTO rooms VALUES ('room');
                INSERT INTO usage_users VALUES ('participant');
                INSERT INTO usage_minutes VALUES (60000,'participant',12345,67890);
                INSERT INTO usage_coverage VALUES (60000,5000);
            ''')
            state.backup(live, saved)
            with self.assertRaises(FileExistsError):
                state.backup(live, saved)
            db.execute('UPDATE usage_minutes SET uploaded=uploaded+100')
            db.commit()
            self.assertTrue(state.verify(saved, live, now_ms=60000))
            for statement in (
                'DELETE FROM usage_minutes',
                'UPDATE usage_minutes SET uploaded=0',
                'DELETE FROM usage_users',
                'DELETE FROM usage_coverage',
                'DELETE FROM rooms',
            ):
                db.execute('SAVEPOINT damage')
                db.execute(statement)
                db.execute('RELEASE damage')
                with self.assertRaises(RuntimeError):
                    state.verify(saved, live, now_ms=60000)
                # Restore the fixture through SQLite, including its WAL.
                with closing(state.connect(saved)) as source:
                    source.backup(db)

    def test_upgrade_from_rooms_without_traffic_tables(self):
        with tempfile.TemporaryDirectory() as directory:
            live = Path(directory) / 'live.db'
            saved = Path(directory) / 'saved.db'
            with closing(sqlite3.connect(live)) as db:
                db.execute('CREATE TABLE rooms(id TEXT PRIMARY KEY)')
                db.execute("INSERT INTO rooms VALUES ('existing-room')")
                db.commit()
            state.backup(live, saved)
            self.assertTrue(state.verify(saved, live))

    def test_normal_expiry_of_unused_users_and_old_buckets(self):
        with tempfile.TemporaryDirectory() as directory:
            live = Path(directory) / 'live.db'
            saved = Path(directory) / 'saved.db'
            with closing(sqlite3.connect(live)) as db:
                db.executescript('''
                    CREATE TABLE rooms(id TEXT PRIMARY KEY);
                    CREATE TABLE usage_users(id TEXT PRIMARY KEY);
                    CREATE TABLE usage_minutes(minute INTEGER,user TEXT,uploaded REAL,downloaded REAL);
                    CREATE TABLE usage_coverage(minute INTEGER,milliseconds INTEGER);
                    INSERT INTO usage_users VALUES ('expired-without-traffic'),('expired-old-traffic');
                    INSERT INTO usage_minutes VALUES (0,'expired-old-traffic',100,200);
                    INSERT INTO usage_coverage VALUES (0,5000);
                ''')
                state.backup(live, saved)
                db.executescript('DELETE FROM usage_minutes; DELETE FROM usage_coverage; DELETE FROM usage_users;')
            self.assertTrue(state.verify(saved, live, now_ms=401 * 86400000))


if __name__ == '__main__':
    unittest.main()
