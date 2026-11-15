# Imagen de PRUEBA (no para uso real): MariaDB de Ubuntu 22.04 con un arranque
# que imita las variables de la imagen oficial (MARIADB_ROOT_PASSWORD,
# MARIADB_DATABASE). La usan las pruebas de copias de E4 cuando no hay acceso
# a Docker Hub. Sustituible con CMC_TEST_MARIADB_IMAGE=mariadb:11.
ARG BASE=ubuntu:22.04
FROM ${BASE}
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends mariadb-server mariadb-client \
 && rm -rf /var/lib/apt/lists/* /var/lib/mysql/* && mkdir -p /run/mysqld && chown mysql:mysql /run/mysqld /var/lib/mysql
COPY mariadb-entrypoint.sh /usr/local/bin/entrypoint.sh
VOLUME /var/lib/mysql
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
