import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
import os

spec = importlib.util.spec_from_file_location('changes', Path(__file__).with_name('server-changes.py'))
changes = importlib.util.module_from_spec(spec)
spec.loader.exec_module(changes)


class ServerChanges(unittest.TestCase):
    def test_native_only_and_server_inputs(self):
        self.assertFalse(changes.relevant([
            'apps/publisher/src/main.rs', 'apps/portable/build.rs', 'Cargo.lock',
            'native-e2e/av1.spec.ts', 'deploy/native-decoder.json', 'assets/icon.ico',
            'docs/deployment.md', '.github/workflows/ci.yml',
        ]))
        for path in ('apps/server/src/app.ts', 'apps/web/src/Admin.tsx',
                     'packages/contracts/src/index.ts', 'Dockerfile',
                     'deploy/production/compose.yaml', 'deploy/livekit-usage/stats.patch',
                     'package-lock.json', '.github/workflows/deploy.yml'):
            self.assertTrue(changes.relevant([path]), path)

    def test_pending_server_change_survives_later_native_merge(self):
        with tempfile.TemporaryDirectory() as directory:
            def git(*args):
                return subprocess.check_output(['git', '-C', directory, *args], stderr=subprocess.DEVNULL).decode().strip()
            git('init')
            git('config', 'user.name', 'CD regression')
            git('config', 'user.email', 'test@example.invalid')
            git('config', 'commit.gpgsign', 'false')
            source = Path(directory)
            (source / 'Dockerfile').write_text('server version 1')
            git('add', '.')
            git('commit', '-m', 'deployed baseline')
            base = git('rev-parse', 'HEAD')
            (source / 'Cargo.lock').write_text('native-only change')
            git('add', '.')
            git('commit', '-m', 'native only')
            native = git('rev-parse', 'HEAD')
            # Run the real Git comparison in this isolated fixture repository.
            previous = os.getcwd()
            try:
                os.chdir(directory)
                self.assertFalse(changes.changed(base, native))
                (source / 'Dockerfile').write_text('pending server version 2')
                git('add', '.')
                git('commit', '-m', 'server update')
                (source / 'Cargo.lock').write_text('later native merge')
                git('add', '.')
                git('commit', '-m', 'native after server')
                self.assertTrue(changes.changed(base, git('rev-parse', 'HEAD')))
            finally:
                os.chdir(previous)
