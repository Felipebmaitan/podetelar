"""Forced SSH command: accept only a bounded source archive and deploy its SHA."""
import fcntl
import hashlib
import os
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import tempfile

ROOT = Path('/opt/freedomstream')
command = os.environ.get('SSH_ORIGINAL_COMMAND', '')
if command == 'current':
    current = (ROOT / 'current').resolve().name
    if not re.fullmatch(r'[0-9a-f]{40}', current):
        raise SystemExit('Active release has no commit identity')
    print(current)
    raise SystemExit(0)
match = re.fullmatch(r'deploy ([0-9a-f]{40})', command)
if not match:
    raise SystemExit('Only current or deploy COMMIT is supported')
os.umask(0o077)
with (ROOT / 'cd/work/deploy.lock').open('a') as lock:
    # Serialize deployments; never interrupt an activation for a newer commit.
    fcntl.flock(lock, fcntl.LOCK_EX)
    sha = match[1]
    with tempfile.TemporaryFile() as archive:
        size = 0
        digest = hashlib.sha256()
        while chunk := sys.stdin.buffer.read(1024 * 1024):
            size += len(chunk)
            if size > 100 * 1024 * 1024:
                raise SystemExit('Source archive exceeds limit')
            archive.write(chunk)
            digest.update(chunk)
        archive.seek(0)
        release = ROOT / 'releases' / sha
        if release.exists():
            if (release / '.cd-archive.sha256').read_text() != digest.hexdigest():
                raise SystemExit('Immutable release already exists with different source')
        else:
            stage = Path(tempfile.mkdtemp(prefix='source-', dir=ROOT / 'cd/work'))
            with tarfile.open(fileobj=archive, mode='r:gz') as source:
                members = source.getmembers()
                if sum(m.size for m in members) > 256 * 1024 * 1024 or len(members) > 10000:
                    raise SystemExit('Expanded source exceeds limit')
                for member in members:
                    path = Path(member.name)
                    if (path.is_absolute() or '..' in path.parts or not (member.isfile() or member.isdir())
                            or path.parts[0] in ('.git', '.local', 'data', '.env')):
                        raise SystemExit('Unexpected source archive entry')
                source.extractall(stage, members=members, filter='data')
            (stage / '.cd-archive.sha256').write_text(digest.hexdigest())
            stage.rename(release)
        subprocess.run(['bash', str(release / 'deploy/production/cd.sh'), sha], check=True)
