#!/usr/bin/env bash
# Start a throw-away OpenSSH server for the core-transport SSH integration
# test (crates/core-transport/tests/ssh_integration.rs) and export the
# variables it reads. Works on a GitHub Actions Ubuntu runner and on any Linux
# box with openssh-server + sudo; nothing it creates outlives the directory it
# prints (plus the `zaptermtest` user, which is harmless on a runner).
#
#   bash scripts/ci/ssh-test-server.sh          # prints `export …` lines
#   eval "$(bash scripts/ci/ssh-test-server.sh)" && cargo test -p core-transport --test ssh_integration
#
# On GitHub Actions the variables are also appended to $GITHUB_ENV.
set -euo pipefail

PORT="${ZAPTERM_SSH_TEST_PORT:-2222}"
USER_NAME="zaptermtest"
PASSWORD="Passw0rd!"
PASSPHRASE="s3cret"
DIR="$(mktemp -d "${TMPDIR:-/tmp}/zapterm-sshd.XXXXXX")"
chmod 700 "$DIR"

SUDO=""
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo"; fi

# Keys: server host key + three user keys (plain ed25519, RSA for the
# rsa-sha2 negotiation path, passphrase-protected ed25519).
ssh-keygen -q -t ed25519 -N "" -f "$DIR/host_ed25519"
ssh-keygen -q -t ed25519 -N "" -f "$DIR/user_ed25519"
ssh-keygen -q -t rsa -b 2048 -N "" -f "$DIR/user_rsa"
ssh-keygen -q -t ed25519 -N "$PASSPHRASE" -f "$DIR/user_enc"

# Test user with a password (for password + keyboard-interactive/PAM) and
# all three public keys authorized.
if ! id "$USER_NAME" >/dev/null 2>&1; then
  $SUDO useradd -m -s /bin/bash "$USER_NAME"
fi
echo "$USER_NAME:$PASSWORD" | $SUDO chpasswd
HOME_DIR="$(getent passwd "$USER_NAME" | cut -d: -f6)"
$SUDO install -d -m 700 -o "$USER_NAME" -g "$USER_NAME" "$HOME_DIR/.ssh"
cat "$DIR/user_ed25519.pub" "$DIR/user_rsa.pub" "$DIR/user_enc.pub" \
  | $SUDO tee "$HOME_DIR/.ssh/authorized_keys" >/dev/null
$SUDO chown "$USER_NAME:$USER_NAME" "$HOME_DIR/.ssh/authorized_keys"
$SUDO chmod 600 "$HOME_DIR/.ssh/authorized_keys"

# The keys must be readable by whoever runs cargo test (root on a container,
# the runner user on GitHub Actions) — they are ours, so open them up.
chmod 600 "$DIR"/user_* 2>/dev/null || true

cat > "$DIR/sshd_config" <<EOF
Port $PORT
ListenAddress 127.0.0.1
HostKey $DIR/host_ed25519
PidFile $DIR/sshd.pid
PasswordAuthentication yes
KbdInteractiveAuthentication yes
PubkeyAuthentication yes
UsePAM yes
AllowTcpForwarding yes
GatewayPorts no
Subsystem sftp internal-sftp
LogLevel VERBOSE
EOF

$SUDO mkdir -p /run/sshd
# sshd insists the host key is owned by the invoking user with tight modes.
$SUDO chown root:root "$DIR/host_ed25519" "$DIR/host_ed25519.pub" 2>/dev/null || true
$SUDO chmod 600 "$DIR/host_ed25519"
$SUDO /usr/sbin/sshd -f "$DIR/sshd_config" -E "$DIR/sshd.log"

# Wait until it answers.
for _ in $(seq 1 50); do
  if (exec 3<>/dev/tcp/127.0.0.1/"$PORT") 2>/dev/null; then break; fi
  sleep 0.1
done

VARS=(
  "ZAPTERM_SSH_TEST_HOST=127.0.0.1"
  "ZAPTERM_SSH_TEST_PORT=$PORT"
  "ZAPTERM_SSH_TEST_USER=$USER_NAME"
  "ZAPTERM_SSH_TEST_PASSWORD=$PASSWORD"
  "ZAPTERM_SSH_TEST_KEY_ED25519=$DIR/user_ed25519"
  "ZAPTERM_SSH_TEST_KEY_RSA=$DIR/user_rsa"
  "ZAPTERM_SSH_TEST_KEY_ENC=$DIR/user_enc"
  "ZAPTERM_SSH_TEST_KEY_ENC_PASSPHRASE=$PASSPHRASE"
  "ZAPTERM_SSH_TEST_SFTP_DIR=/tmp"
)
for v in "${VARS[@]}"; do
  echo "export $v"
  if [ -n "${GITHUB_ENV:-}" ]; then echo "$v" >> "$GITHUB_ENV"; fi
done
echo "# sshd log: $DIR/sshd.log" >&2
