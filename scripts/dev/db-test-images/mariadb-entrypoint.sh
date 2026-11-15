#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
set -e
mkdir -p /run/mysqld && chown mysql:mysql /run/mysqld /var/lib/mysql
if [ ! -d /var/lib/mysql/mysql ]; then
  mariadb-install-db --user=mysql --datadir=/var/lib/mysql >/dev/null
  mariadbd --user=mysql --skip-networking --socket=/tmp/init.sock &
  pid=$!
  i=0; until mariadb --socket=/tmp/init.sock -e 'select 1' >/dev/null 2>&1; do i=$((i+1)); [ $i -lt 120 ] || exit 1; sleep 0.5; done
  mariadb --socket=/tmp/init.sock -e "ALTER USER 'root'@'localhost' IDENTIFIED BY '${MARIADB_ROOT_PASSWORD}'; CREATE DATABASE IF NOT EXISTS \`${MARIADB_DATABASE:-app}\`;"
  kill "$pid"; wait "$pid" || true
fi
exec mariadbd --user=mysql
