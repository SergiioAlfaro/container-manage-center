#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
# SOLO PARA DESARROLLO, en una máquina o contenedor de pruebas desechable y como
# root: crea usuarios, grupos, puentes de red, daemons de Docker y servidores
# SSH. No lo ejecutes en tu equipo de trabajo.
#
# Variables: CMC_D26_BIN / CMC_D24_BIN (directorios con dockerd 26 y 24),
# CMC_LAB_DIR (por defecto /tmp/cmc-remote-lab).
#
# Entorno de pruebas de administración remota:
#   A = Docker 29 (socket del sistema)      sshd 127.0.0.1:2201  usuario cmca
#   B = Docker 26 (/run/cmc-b/docker.sock)  sshd 127.0.0.1:2202  usuario cmcb
#   C = Docker 24 (/run/cmc-c/docker.sock)  sshd 127.0.0.1:2203  usuario cmcc
# uso: remote-env.sh up|down|status
set -u
R=${CMC_LAB_DIR:-/tmp/cmc-remote-lab}
start_daemon() { # nombre binarios data bridge subnet
  local n=$1 bin=$2 data=$3 br=$4 net=$5
  ip link show $br >/dev/null 2>&1 || { ip link add $br type bridge; ip addr add $net dev $br; ip link set $br up; }
  mkdir -p /run/cmc-$n
  getent group cmc-$n >/dev/null || groupadd cmc-$n
  (PATH=$bin:$PATH nohup $bin/dockerd --host unix:///run/cmc-$n/docker.sock --group cmc-$n \
     --exec-root /run/cmc-$n/exec --pidfile /run/cmc-$n/docker.pid --data-root $data \
     --bridge $br --iptables=false --ip6tables=false > $R/dockerd-$n.log 2>&1 &)
}
case "${1:-status}" in
up)
  mkdir -p $R
  pgrep -f "/run/cmc-b/docker.sock" >/dev/null || start_daemon b ${CMC_D26_BIN:?define CMC_D26_BIN} $R/data26 br-cmcb 172.31.62.1/24
  pgrep -f "/run/cmc-c/docker.sock" >/dev/null || start_daemon c ${CMC_D24_BIN:?define CMC_D24_BIN} $R/data24 br-cmcc 172.31.63.1/24
  sleep 8
  for u in a b c nope; do
    id cmc$u >/dev/null 2>&1 || useradd -m -s /bin/bash cmc$u
    usermod -p '*' cmc$u
  done
  # cmcb también entra con contraseña (prueba de la autenticación por contraseña).
  echo 'cmcb:Contraseña de prueba 1' | chpasswd
  K=$R/client; mkdir -p $K
  [ -f $K/id_ed25519 ] || ssh-keygen -q -t ed25519 -N '' -f $K/id_ed25519 -C cmc-test
  [ -f $K/id_pass ] || ssh-keygen -q -t ed25519 -N 'frase de prueba' -f $K/id_pass -C cmc-test-pass
  for u in a b c nope; do h=/home/cmc$u; mkdir -p $h/.ssh; cat $K/id_ed25519.pub $K/id_pass.pub > $h/.ssh/authorized_keys; chown -R cmc$u: $h/.ssh; chmod 700 $h/.ssh; chmod 600 $h/.ssh/authorized_keys; done
  usermod -aG docker cmca 2>/dev/null || { groupadd -f docker; usermod -aG docker cmca; }
  usermod -aG cmc-b cmcb; usermod -aG cmc-c cmcc
  mkdir -p /run/sshd
  for i in 1 2 3; do
    u=$(echo a b c | cut -d' ' -f$i); d=$R/sshd$i; mkdir -p $d
    [ -f $d/host_ed25519 ] || ssh-keygen -q -t ed25519 -N '' -f $d/host_ed25519
    sock=""; [ $u = b ] && sock="SetEnv DOCKER_HOST=unix:///run/cmc-b/docker.sock"
    [ $u = c ] && sock="SetEnv DOCKER_HOST=unix:///run/cmc-c/docker.sock"
    # A hace también de servidor intermedio (ProxyJump, 1.1.0): solo hacia B y C.
    fwd="AllowTcpForwarding no"; [ $u = a ] && fwd="AllowTcpForwarding local
PermitOpen 127.0.0.1:2202 127.0.0.1:2203"
    cat > $d/sshd_config <<C
Port 220$i
ListenAddress 127.0.0.1
HostKey $d/host_ed25519
PidFile $d/sshd.pid
AllowUsers cmc$u cmcnope
PubkeyAuthentication yes
PasswordAuthentication yes
KbdInteractiveAuthentication no
UsePAM yes
PermitRootLogin no
X11Forwarding no
$fwd
$sock
Match User cmcnope
  SetEnv PATH=/nonexistent
C
    [ -f $d/sshd.pid ] && kill -0 $(cat $d/sshd.pid) 2>/dev/null || /usr/sbin/sshd -f $d/sshd_config -E $d/sshd.log
  done
  # Huellas de confianza (las de los tres servidores) y descripción del laboratorio.
  : > $K/known_hosts; for i in 1 2 3; do ssh-keyscan -p 220$i -t ed25519 127.0.0.1 2>/dev/null >> $K/known_hosts; done
  cat > $R/lab.json <<J
{"key": "$K/id_ed25519", "passKey": "$K/id_pass", "passphrase": "frase de prueba",
 "knownHosts": "$K/known_hosts",
 "servers": [
  {"name": "A", "user": "cmca", "host": "127.0.0.1", "port": 2201, "socket": "/var/run/docker.sock", "hostKey": "$R/sshd1/host_ed25519.pub"},
  {"name": "B", "user": "cmcb", "host": "127.0.0.1", "port": 2202, "socket": "/run/cmc-b/docker.sock", "hostKey": "$R/sshd2/host_ed25519.pub"},
  {"name": "C", "user": "cmcc", "host": "127.0.0.1", "port": 2203, "socket": "/run/cmc-c/docker.sock", "hostKey": "$R/sshd3/host_ed25519.pub"}
 ],
 "password": {"server": 1, "user": "cmcb", "password": "Contraseña de prueba 1"},
 "noDocker": {"server": 0, "user": "cmcnope"}}
J
  ;;
down)
  for i in 1 2 3; do d=$R/sshd$i; [ -f $d/sshd.pid ] && kill $(cat $d/sshd.pid) 2>/dev/null; done
  pkill -f "/run/cmc-b/docker.sock"; pkill -f "/run/cmc-c/docker.sock"; sleep 4
  ;;
esac
for n in b c; do printf "%s: " $n; DOCKER_HOST=unix:///run/cmc-$n/docker.sock timeout 5 docker version --format '{{.Server.Version}} API {{.Server.APIVersion}}' 2>&1 | tail -1; done
printf "a: "; timeout 5 docker version --format '{{.Server.Version}} API {{.Server.APIVersion}}' 2>&1 | tail -1
for i in 1 2 3; do printf "sshd%s: " $i; (exec 3<>/dev/tcp/127.0.0.1/220$i && head -c 20 <&3) 2>/dev/null | head -1 || echo cerrado; done
