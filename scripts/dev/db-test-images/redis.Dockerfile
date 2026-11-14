# Imagen de PRUEBA: Redis de Ubuntu 22.04 con /data como en la imagen oficial.
ARG BASE=ubuntu:22.04
FROM ${BASE}
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends redis-server redis-tools \
 && rm -rf /var/lib/apt/lists/* && mkdir -p /data
VOLUME /data
WORKDIR /data
ENTRYPOINT ["redis-server", "--dir", "/data", "--protected-mode", "no"]
