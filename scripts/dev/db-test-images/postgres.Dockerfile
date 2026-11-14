# Imagen de PRUEBA (no para uso real): PostgreSQL de Ubuntu 22.04 con un
# arranque que imita la imagen oficial (POSTGRES_PASSWORD, POSTGRES_USER,
# POSTGRES_DB). Sustituible con DCC_TEST_POSTGRES_IMAGE=postgres:17.
ARG BASE=ubuntu:22.04
FROM ${BASE}
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends postgresql postgresql-client \
 && rm -rf /var/lib/apt/lists/* && rm -rf /var/lib/postgresql/*/main
ENV PGDATA=/var/lib/postgresql/data
COPY postgres-entrypoint.sh /usr/local/bin/entrypoint.sh
VOLUME /var/lib/postgresql/data
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
