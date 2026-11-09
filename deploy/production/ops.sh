#!/bin/bash
set -euo pipefail
umask 077
root=/opt/freedomstream
state=$root/shared
[[ -d "$state" && "$(readlink -f "$state")" == "$root/shared" ]] || { echo 'Unexpected state directory.' >&2; exit 1; }
compose() { docker compose --env-file "$state/.env" --env-file "$state/release.env" -f "$root/current/deploy/production/compose.yaml" "$@"; }
image=$(sed -n 's/^APP_IMAGE=//p' "$state/release.env")
[[ "$image" =~ ^freedomstream/app:[a-f0-9]{40}$ ]] || { echo 'Use an immutable commit image tag.' >&2; exit 1; }
backup_container() {
  docker run --rm --user 1000:1000 --network none --read-only --cpus=0.25 --memory=192m --pids-limit=32 --cap-drop=ALL \
    -v "$state/data:/data:ro" -v "$state/backups:/backups" "$image" \
    node apps/server/dist/backup.js "$@"
}
backup() {
  name=rooms-$(date -u +%Y%m%dT%H%M%SZ)-$RANDOM.db
  backup_container create /data/freedomstream.db "/backups/$name"
  echo "Created verified private backup: $name"
}
case "${1:-status}" in
  backup) backup ;;
  verify)
    [[ "${2:-}" =~ ^rooms-[a-zA-Z0-9-]+\.db$ ]] || { echo 'Choose a backup filename.' >&2; exit 1; }
    backup_container verify "/backups/$2"
    echo 'Backup integrity and schema verified.'
    ;;
  restore)
    [[ "${2:-}" =~ ^rooms-[a-zA-Z0-9-]+\.db$ ]] || { echo 'Choose a backup filename.' >&2; exit 1; }
    [[ -f "$state/backups/$2" && ! -L "$state/backups/$2" ]] || exit 1
    backup_container verify "/backups/$2"
    backup
    compose stop app
    # The app is stopped. Preserve all old SQLite files together; never mix WALs.
    quarantine=$state/backups/pre-restore-$(date -u +%Y%m%dT%H%M%SZ)-$RANDOM
    install -d -m 700 "$quarantine"
    for suffix in '' -wal -shm; do
      if [[ -e "$state/data/freedomstream.db$suffix" ]]; then mv -- "$state/data/freedomstream.db$suffix" "$quarantine/"; fi
    done
    install -m 600 -o 1000 -g 1000 "$state/backups/$2" "$state/data/freedomstream.db"
    compose up -d --wait --no-deps app
    echo 'Restored room data. Old invitations/sessions in this backup are valid again; rotate invitations/revoke sessions if needed.'
    ;;
  status)
    compose ps
    ids=$(compose ps -q)
    if [[ -n "$ids" ]]; then docker stats --no-stream --format '{{.Name}} CPU={{.CPUPerc}} RAM={{.MemUsage}} net={{.NetIO}}' $ids; fi
    "$root/current/deploy/production/certificates.sh" check
    curl --fail --silent --show-error --max-time 5 http://127.0.0.1:18080/api/health
    echo
    systemctl is-active freedomstream-renew.timer
    ;;
  *) echo 'Usage: ops.sh status|backup|verify BACKUP_NAME|restore BACKUP_NAME' >&2; exit 1 ;;
esac
