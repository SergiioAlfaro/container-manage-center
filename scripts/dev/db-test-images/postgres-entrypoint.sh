#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
set -e
BIN=$(ls -d /usr/lib/postgresql/*/bin | head -1)
U=${POSTGRES_USER:-postgres}
mkdir -p "$PGDATA" /var/run/postgresql
chown postgres:postgres "$PGDATA" /var/run/postgresql
chmod 700 "$PGDATA"
if [ ! -s "$PGDATA/PG_VERSION" ]; then
  printf '%s\n' "${POSTGRES_PASSWORD}" > /tmp/pw && chown postgres /tmp/pw
  su postgres -s /bin/sh -c "$BIN/initdb -D '$PGDATA' -U '$U' --pwfile=/tmp/pw --auth-local=trust --auth-host=scram-sha-256 >/dev/null"
  rm -f /tmp/pw
  echo "listen_addresses = '*'" >> "$PGDATA/postgresql.conf"
  echo "host all all all scram-sha-256" >> "$PGDATA/pg_hba.conf"
  DB=${POSTGRES_DB:-$U}
  if [ "$DB" != "postgres" ]; then
    su postgres -s /bin/sh -c "$BIN/pg_ctl -D '$PGDATA' -o '-c listen_addresses= -k /tmp' -w start >/dev/null"
    psql -h /tmp -U "$U" -d postgres -v ON_ERROR_STOP=1 -q -c "CREATE DATABASE \"$DB\""
    su postgres -s /bin/sh -c "$BIN/pg_ctl -D '$PGDATA' -m fast -w stop >/dev/null"
  fi
fi
exec su postgres -s /bin/sh -c "exec $BIN/postgres -D '$PGDATA' -k /var/run/postgresql"
