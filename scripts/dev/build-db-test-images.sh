#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
# Construye las imágenes de PRUEBA de bases de datos para las pruebas de copias
# (E4): cmc-test/mariadb:1, cmc-test/postgres:1 y cmc-test/redis:1, a partir
# de Ubuntu 22.04 con sus paquetes (no hace falta Docker Hub si ya tienes la
# imagen base). Solo para el entorno de pruebas.
# Uso: bash scripts/dev/build-db-test-images.sh [imagen-base]
set -euo pipefail
BASE=${1:-ubuntu:22.04}
cd "$(dirname "$0")/db-test-images"
for db in mariadb postgres redis; do
  docker build --network host --build-arg "BASE=$BASE" --label cmc.test.image=true \
    -t "cmc-test/$db:1" -f "$db.Dockerfile" .
done
