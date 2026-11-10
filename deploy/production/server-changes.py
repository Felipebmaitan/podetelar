"""Compare checked main with the active server release, not only its last commit."""
import subprocess
import sys

PREFIXES = ('apps/server/', 'apps/web/', 'packages/contracts/',
            'deploy/production/', 'deploy/livekit-usage/', 'deploy/relay/')
FILES = {'Dockerfile', '.dockerignore', 'package.json', 'package-lock.json',
         'tsconfig.base.json', 'scripts/prepare-production.mjs',
         'scripts/setup-relay.mjs', '.github/workflows/deploy.yml'}


def relevant(paths):
    return any(path in FILES or path.startswith(PREFIXES) for path in paths)


def changed(base, head):
    if subprocess.run(['git', 'cat-file', '-e', base + '^{commit}'],
                      stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode:
        return True  # Unknown old release: establish a checked deployment baseline.
    paths = subprocess.check_output(
        ['git', 'diff', '--no-renames', '--name-only', '-z', base, head]
    ).decode('utf-8').split('\0')
    return relevant(paths)


if __name__ == '__main__':
    print('true' if changed(*sys.argv[1:]) else 'false')
