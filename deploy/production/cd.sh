#!/bin/bash
set -Eeuo pipefail
umask 077
root=/opt/freedomstream
state=$root/shared
commit=${1:?Expected commit}
[[ "$commit" =~ ^[a-f0-9]{40}$ ]] || exit 1
release=$root/releases/$commit
[[ $(readlink -f "$root") == "$root" && $(readlink -f "$state") == "$state" ]] || exit 1
[[ -f "$state/data/freedomstream.db" && ! -L "$state/data/freedomstream.db" ]] || exit 1
old=$(readlink -f "$root/current")
[[ "$old" == "$root/releases/"* ]] || exit 1
compose() { sudo -n docker compose --env-file "$state/.env" --env-file "$state/release.env" -f "$root/current/deploy/production/compose.yaml" "$@"; }
app=freedomstream/app:$commit
build() { sudo -n env DOCKER_BUILDKIT=0 docker build --memory="$1" --cpu-period=100000 --cpu-quota=100000 "${@:2}"; }
build 768m -t "$app" "$release"
sfu_hash=$(cat "$release"/deploy/livekit-usage/{Dockerfile,stats.patch,usage_metrics.go} | sha256sum | cut -c1-40)
sfu=freedomstream/livekit-usage:$sfu_hash
if ! sudo -n docker image inspect "$sfu" >/dev/null 2>&1; then
  build 1536m -f "$release/deploy/livekit-usage/Dockerfile" -t "$sfu" "$release"
fi
backup=$state/backups/cd-$commit-$(date -u +%Y%m%dT%H%M%SZ)-$RANDOM
mkdir -m 700 "$backup"
python3 "$release/deploy/production/cd-state.py" backup "$state/data/freedomstream.db" "$backup/database.db"
cp -p "$state/release.env" "$backup/release.env"
cp -p "$state/livekit.yaml" "$backup/livekit.yaml"
printf '%s\n' "$old" > "$backup/current"
inode=$(stat -c '%d:%i' "$state/data/freedomstream.db")
secrets=$(sha256sum "$state/.env" "$state/secrets.json" "$state/turn-secret")
publisher=$(sha256sum "$state/publisher/publisher.json")
sudo -n docker ps --format '{{.Names}} {{.ID}}' | grep -v '^freedomstream-production-\(app\|livekit\)-1 ' | sort > "$backup/other-containers.before"
atomic_copy() { local temp; temp=$(mktemp "$2.XXXXXX"); cp -p "$1" "$temp"; mv -Tf "$temp" "$2"; }
activate_link() { local temp; temp=$root/current.cd-$RANDOM; ln -s "$1" "$temp"; mv -Tf "$temp" "$root/current"; }
rollback() {
  trap - ERR HUP TERM
  set +e
  failed=0
  atomic_copy "$backup/release.env" "$state/release.env" || failed=1
  atomic_copy "$backup/livekit.yaml" "$state/livekit.yaml" || failed=1
  activate_link "$old" || failed=1
  # Restore code/config only. Never restore a DB over traffic written since backup.
  compose up -d --no-deps --wait --wait-timeout 90 livekit app || failed=1
  curl -fsS --max-time 10 -o /dev/null http://127.0.0.1:18080/api/health || failed=1
  if [[ $failed == 0 ]]; then
    echo 'Deployment failed; previous code/config restored. Live database retained.' >&2
  else
    echo "Deployment and rollback failed; private recovery backup: $backup" >&2
  fi
  exit 1
}
trap rollback ERR HUP TERM
# Preserve existing SFU settings/secrets; add only its private metrics listener.
if ! grep -q '^prometheus_port:' "$state/livekit.yaml"; then
  cp -p "$state/livekit.yaml" "$backup/livekit-next.yaml"
  printf '\nprometheus_port: 6789\n' >> "$backup/livekit-next.yaml"
  atomic_copy "$backup/livekit-next.yaml" "$state/livekit.yaml"
fi
grep -Eq '^prometheus_port: *6789 *$' "$state/livekit.yaml"
printf 'APP_IMAGE=%s\nLIVEKIT_USAGE_IMAGE=%s\n' "$app" "$sfu" > "$backup/release-next.env"
atomic_copy "$backup/release-next.env" "$state/release.env"
activate_link "$release"
compose up -d --no-deps --wait --wait-timeout 90 livekit app
compose exec -T app node --input-type=module - <<'JS'
import { RoomServiceClient } from 'livekit-server-sdk';
const client = new RoomServiceClient(process.env.LIVEKIT_INTERNAL_URL, process.env.LIVEKIT_API_KEY, process.env.LIVEKIT_API_SECRET);
let ready = false;
for (let attempt = 0; attempt < 15; attempt++) {
  try {
    await client.listRooms();
    const response = await fetch('http://livekit:6789/metrics', {signal: AbortSignal.timeout(3000)});
    ready = response.ok && /^freedomstream_usage_enabled 1\s*$/m.test(await response.text());
    if (ready) break;
  } catch { /* Bounded first-start readiness, without logging media authority. */ }
  await new Promise(resolve => setTimeout(resolve, 2000));
}
if (!ready) throw new Error('SFU/usage collector readiness failed');
console.log('SFU API and private traffic collector ready');
JS
curl -fsS --retry 5 --retry-delay 2 --max-time 10 -o /dev/null http://127.0.0.1:18080/api/health
origin=$(sed -n 's/^PUBLIC_URL=//p' "$state/.env")
[[ "$origin" == https://* ]]
curl -fsS --retry 5 --retry-delay 2 --max-time 10 -o /dev/null "$origin/api/health"
[[ "$inode" == "$(stat -c '%d:%i' "$state/data/freedomstream.db")" ]]
[[ "$secrets" == "$(sha256sum "$state/.env" "$state/secrets.json" "$state/turn-secret")" ]]
[[ "$publisher" == "$(sha256sum "$state/publisher/publisher.json")" ]]
python3 "$release/deploy/production/cd-state.py" verify "$backup/database.db" "$state/data/freedomstream.db"
sudo -n docker ps --format '{{.Names}} {{.ID}}' | grep -v '^freedomstream-production-\(app\|livekit\)-1 ' | sort > "$backup/other-containers.after"
cmp "$backup/other-containers.before" "$backup/other-containers.after"
[[ $(sudo -n docker inspect --format '{{.Config.Image}}' freedomstream-production-app-1) == "$app" ]]
printf '%s\n' "$commit" > "$state/deployed-commit"
trap - ERR HUP TERM
echo "Deployed $commit; existing database/traffic, credentials, portable download and unrelated services retained. Backup: $backup"
