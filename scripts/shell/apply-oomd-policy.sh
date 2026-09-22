#!/usr/bin/env bash
set -euo pipefail

# Administrator companion to apply-resource-limits.sh; never resizes swap.
user_uid=${1:?Usage: sudo bash apply-oomd-policy.sh USER_UID}
[[ $EUID == 0 && $user_uid =~ ^[0-9]+$ && $user_uid != 0 ]] || exit 2
getent passwd "$user_uid" >/dev/null
mkdir -p /etc/systemd/oomd.conf.d \
  "/etc/systemd/system/user-$user_uid.slice.d" \
  "/etc/systemd/system/user@$user_uid.service.d"
cat >/etc/systemd/oomd.conf.d/60-swap-headroom.conf <<'EOF'
[OOM]
# Emergency fallback, not an admission threshold or a swap capacity limit.
SwapUsedLimit=90%
EOF
for unit in "user-$user_uid.slice" "user@$user_uid.service"; do
  section=Slice
  [[ $unit != *.service ]] || section=Service
  cat >"/etc/systemd/system/$unit.d/90-development-oom-isolation.conf" <<EOF
[$section]
ManagedOOMSwap=auto
ManagedOOMMemoryPressure=auto
EOF
done
systemctl daemon-reload
systemctl restart systemd-oomd
