#!/usr/bin/env bash
# Construye las imágenes de PRUEBA de bases de datos para las pruebas de copias
# (E4): dcc-test/mariadb:1, dcc-test/postgres:1 y dcc-test/redis:1, a partir
# de Ubuntu 22.04 con sus paquetes (no hace falta Docker Hub si ya tienes la
# imagen base). Solo para el entorno de pruebas.
# Uso: bash scripts/dev/build-db-test-images.sh [imagen-base]
set -euo pipefail
BASE=${1:-ubuntu:22.04}
cd "$(dirname "$0")/db-test-images"
for db in mariadb postgres redis; do
  docker build --network host --build-arg "BASE=$BASE" --label dcc.test.image=true \
    -t "dcc-test/$db:1" -f "$db.Dockerfile" .
done
