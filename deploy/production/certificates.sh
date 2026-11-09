#!/bin/bash
set -euo pipefail
umask 077
root=${FREEDOMSTREAM_ROOT:-/opt/freedomstream}
[[ "$root" == /opt/freedomstream ]] || { echo 'Unexpected installation root.' >&2; exit 1; }
state=$root/shared
action=${1:-check}
host=$(sed -n 's|^PUBLIC_URL=https://\([^:]*\):8443$|\1|p' "$state/.env")
turn_host=$(sed -n 's/^TURN_HOST=//p' "$state/.env")
[[ "$host" =~ ^[a-zA-Z0-9.-]+$ ]] || exit 1
[[ "$turn_host" =~ ^[a-zA-Z0-9.-]+$ ]] || exit 1
certbot_image=certbot/certbot:v5.9.0
certbot() {
  docker run --rm --user 1000:1000 --cpus=0.5 --memory=256m --pids-limit=64 --cap-drop=ALL \
    -v "$state/certbot:/etc/letsencrypt" -v /var/www/freedomstream-acme:/webroot \
    --tmpfs /var/lib/letsencrypt:rw,size=16m,mode=1777 --tmpfs /var/log/letsencrypt:rw,size=16m,mode=1777 \
    "$certbot_image" "$@"
}
case "$action" in
  issue|staging)
    args=(certonly --non-interactive --agree-tos --register-unsafely-without-email --webroot -w /webroot --cert-name freedomstream --preferred-profile shortlived --expand)
    if [[ "$host" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then args+=(--ip-address "$host"); else args+=(-d "$host"); fi
    if [[ "$turn_host" != "$host" ]]; then args+=(-d "$turn_host"); fi
    # Staging uses an independent lineage/config tree and is never installed.
    if [[ "$action" == staging ]]; then args+=(--staging --config-dir /etc/letsencrypt/staging); fi
    certbot "${args[@]}"
    [[ "$action" != staging ]] || exit 0
    ;;
  renew) certbot renew --non-interactive --quiet ;;
  check) ;;
  *) echo 'Usage: certificates.sh issue|staging|renew|check' >&2; exit 1 ;;
esac
source_dir=$state/certbot/live/freedomstream
openssl x509 -in "$source_dir/fullchain.pem" -noout -checkend 86400 >/dev/null
if [[ "$host" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then openssl x509 -in "$source_dir/fullchain.pem" -noout -checkip "$host" >/dev/null; else openssl x509 -in "$source_dir/fullchain.pem" -noout -checkhost "$host" >/dev/null; fi
openssl x509 -in "$source_dir/fullchain.pem" -noout -checkhost "$turn_host" >/dev/null
cert_pub=$(openssl x509 -in "$source_dir/fullchain.pem" -pubkey -noout | openssl pkey -pubin -outform DER | sha256sum | cut -d' ' -f1)
key_pub=$(openssl pkey -in "$source_dir/privkey.pem" -pubout -outform DER | sha256sum | cut -d' ' -f1)
[[ "$cert_pub" == "$key_pub" ]] || { echo 'Certificate/key mismatch.' >&2; exit 1; }
fingerprint=$(sha256sum "$source_dir/fullchain.pem" | cut -d' ' -f1)
applied=$(cat "$state/tls/applied" 2>/dev/null || true)
if [[ "$action" == check ]]; then
  cmp -s "$source_dir/fullchain.pem" "$state/tls/current/fullchain.pem" || { echo 'Renewed certificate is not installed.' >&2; exit 1; }
  [[ "$applied" == "$fingerprint" ]] || { echo 'Certificate service reload is pending; run certificates.sh renew.' >&2; exit 1; }
  exit 0
fi
if [[ -f "$state/tls/current/fullchain.pem" ]] && cmp -s "$source_dir/fullchain.pem" "$state/tls/current/fullchain.pem" && [[ "$applied" == "$fingerprint" ]]; then exit 0; fi
install -d -m 700 -o 1000 -g 1000 "$state/tls/$fingerprint"
install -m 400 -o 1000 -g 1000 "$source_dir/fullchain.pem" "$state/tls/$fingerprint/fullchain.pem"
install -m 400 -o 1000 -g 1000 "$source_dir/privkey.pem" "$state/tls/$fingerprint/privkey.pem"
[[ ! -e "$state/tls/current.new" || -L "$state/tls/current.new" ]] || { echo 'Unexpected certificate staging path.' >&2; exit 1; }
ln -sfn "$fingerprint" "$state/tls/current.new"
mv -Tf "$state/tls/current.new" "$state/tls/current"
# Record application only after BOTH consumers reload successfully. If either
# fails, the next timer retries even though the copied certificate is unchanged.
if [[ -f /etc/nginx/conf.d/freedomstream-https.conf && -f "$root/current/deploy/production/compose.yaml" ]]; then
  nginx -t
  systemctl reload nginx
  docker compose --env-file "$state/.env" --env-file "$state/release.env" -f "$root/current/deploy/production/compose.yaml" restart relay
  printf '%s\n' "$fingerprint" > "$state/tls/applied.new"
  mv -Tf "$state/tls/applied.new" "$state/tls/applied"
fi
echo 'Installed the renewed FreedomStream certificate.'
