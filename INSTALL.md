# Instalación

Plataformas soportadas: **Debian 12 o superior** y **Ubuntu 22.04 o superior** (x86_64).

## Opción A: paquete .deb

```bash
sudo apt install ./docker-control-center_0.1.0_amd64.deb
```

`apt` instala automáticamente las dependencias de ejecución (`libwebkit2gtk-4.1-0`,
`libgtk-3-0`). La aplicación aparece en el menú como **Docker Control Center**.

Desinstalar:

```bash
sudo apt remove docker-control-center
```

La configuración del usuario (`~/.config/docker-control-center/`) no se borra al
desinstalar.

## Opción B: compilar desde el código fuente

### 1. Paquetes de sistema

```bash
bash scripts/install-build-deps.sh
```

o manualmente:

```bash
sudo apt update
sudo apt install -y build-essential curl wget file pkg-config libssl-dev \
  libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev
```

### 2. Rust (1.80 o superior)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

### 3. Node.js (20 o superior)

Debian 12 y Ubuntu 22.04 incluyen versiones antiguas de Node.js en sus
repositorios. Usa una de estas vías:

- **nvm** (sin sudo):
  ```bash
  curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
  exec "$SHELL"
  nvm install 22
  ```
- **Repositorio de NodeSource**: <https://github.com/nodesource/distributions>

### 4. Compilar y ejecutar

```bash
npm ci
npx tauri build --debug --no-bundle     # compilación rápida para probar
./target/debug/docker-control-center
```

Paquete de distribución:

```bash
npx tauri build --bundles deb
# Resultado: target/release/bundle/deb/docker-control-center_0.1.0_amd64.deb
```

Desarrollo con recarga en caliente:

```bash
npx tauri dev
```

## Acceso a Docker (necesario a partir de la Fase 2)

Docker Control Center **no debe ejecutarse como root** y se niega a hacerlo.
Necesita acceso al socket de Docker con tu usuario. Hay dos opciones:

**Docker en modo rootless (recomendado por seguridad).** El daemon se ejecuta con
tu usuario y el socket está en `$XDG_RUNTIME_DIR/docker.sock`. Guía oficial:
<https://docs.docker.com/engine/security/rootless/>

**Grupo `docker`.**

```bash
sudo usermod -aG docker "$USER"
# Cierra la sesión y vuelve a entrar para que el cambio tenga efecto.
```

> Pertenecer al grupo `docker` equivale en la práctica a tener privilegios de
> root en el equipo: cualquier proceso de tu usuario podrá controlar Docker y,
> a través de él, el sistema. Valóralo antes de hacerlo.

## Variables de entorno

| Variable | Uso |
|---|---|
| `DCC_CONFIG` | Ruta alternativa del archivo de configuración |
| `DCC_LOG` | Nivel de registro del backend (`error`, `warn`, `info`, `debug`, `trace`) |
