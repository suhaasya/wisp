#!/bin/sh
set -eu

mkdir -p /home/wisp/.ssh
chmod 700 /home/wisp/.ssh
touch /home/wisp/.ssh/authorized_keys
chown -R wisp:wisp /home/wisp/.ssh

if [ -f /keys/id_ed25519.pub ]; then
  cat /keys/id_ed25519.pub >> /home/wisp/.ssh/authorized_keys
fi

exec /usr/sbin/sshd -D -e
