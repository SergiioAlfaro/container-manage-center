## 1. What is Container Manage Center {#about}

Container Manage Center is a desktop application for managing **Docker Engine** on Linux without relying on the terminal. Everything it shows is real data requested from Docker at that moment, and every action is a real operation: nothing is simulated.

- It talks to Docker Engine through its **Unix socket**, using the official API.
- It runs **Docker Compose** with its official program, with separate arguments and no shell.
- Every operation appears in the **activity panel** with its equivalent `docker` command, so you know exactly what was done.

The window has a **sidebar** with the sections (collapse it with Ctrl+B), the **work area**, a **top bar** with the connection indicator and the **Activity** button, and the **activity panel** on the right (Ctrl+Shift+A).

## 2. System requirements and dependencies {#requirements}

- **System**: Debian 12 or later, or Ubuntu 22.04 or later, 64-bit (x86_64).
- **Docker Engine 20.10 or later** (API 1.41+), in normal or rootless mode.
- **Docker Compose v2 or later**, only for the Docker Compose section (as the `docker compose` plugin or the `docker-compose` v2 binary). Compose v1 is not supported.
- **Runtime dependencies**: WebKitGTK 4.1, GTK 3, glibc and related libraries. APT installs them with the package.

The application does not need the Internet. Only the operations Docker Engine performs against remote registries (pulling images, searching Docker Hub, building with base images that are not on the computer) need Docker to have network access.

## 3. Installation and first run {#install}

Install the package with APT, which resolves the dependencies:

```
sudo apt install ./container-manage-center_X.Y.Z_amd64.deb
```

The application appears in the menu as **Container Manage Center**. It runs as your normal user: **do not run it with sudo** (it refuses to start as root).

When it opens, it looks for Docker Engine automatically. If it finds it, the **Dashboard** shows its status; otherwise it shows the diagnosis with the cause and the steps to fix it.

To update, install the new package the same way: your settings and your history are kept.

## 4. Initial setup {#first-setup}

You do not need to configure anything to get started: the Docker connection is automatic. In **Settings** you can choose:

- The **theme** (System, Light or Dark) and the **language**. The primary color is purple.
- Whether the activity panel opens by itself when an operation fails and how many operations it keeps.
- Whether the history of operations and events is saved between sessions.
- A specific Docker socket, if you do not want automatic detection.

Everything is saved immediately in `~/.config/container-manage-center/config.toml`.

## 5. Connecting to Docker Engine {#connection}

The **connection indicator** in the top bar shows the status:

| Appearance | Meaning |
|---|---|
| Green dot and `Docker 29.x` | Connected. Hover over it to see the socket |
| `Connecting to Docker…` | Detecting or connecting |
| Red, `No connection to Docker` | Not connected. Click it to see the diagnosis |

In **automatic** mode, the application tries in this order: the `DOCKER_HOST` variable (`unix://` only), the active Docker context, your rootless socket and the system socket `/var/run/docker.sock`. It checks the connection every 10 seconds and reconnects by itself when Docker becomes available again.

The application never asks for sudo, does not change permissions or groups and does not run the diagnosis instructions for you. Your user needs to be able to open the socket: with rootless Docker or by belonging to the `docker` group (which in practice is equivalent to having administrator privileges).

## 6. Rootless Docker and socket connections {#rootless}

With **rootless Docker** the daemon runs as your user and the socket is at `$XDG_RUNTIME_DIR/docker.sock` (usually `/run/user/1000/docker.sock`). The application detects it by itself, also through the `rootless` context, and marks it with the **rootless** label. It is the recommended option for security.

In **Settings › Docker connection** you can:

- Choose **Custom socket** and type an absolute path.
- **Test connection**: checks the path without saving it.
- **Save and connect**: applies the configuration.
- **Reconnect**: repeats the detection.

`tcp://` connections are not supported. Remote servers are managed over SSH from **Servers** (next section).

## 7. Remote servers over SSH {#remote-servers}

From **Servers** you can manage the Docker of other Linux machines you can reach over **SSH**, alongside the one on this computer. The connection uses the system SSH client (`ssh`) and runs `docker system dial-stdio` on the server: Docker is **never** exposed on the network (no TCP 2375) and no port is opened.

**Server requirements**: a running SSH server, Docker Engine 20.10 or later (API 1.41) and a user who can use Docker. For Compose, `docker compose` or `docker-compose` on the server. The application **does not install Docker, change permissions, touch the SSH configuration or open ports**: if something is missing, it tells you what and how to fix it.

**Add server** (wizard): general details (name, host, user, port 22), authentication, Docker (default, a specific socket or a context) and **Test connection**, which checks seven steps: DNS, SSH connection, server identity, authentication, Docker available, permissions and API compatibility.

- **Authentication**: SSH agent (recommended), private key, key with passphrase or password (not recommended). The private key is **never copied**: only its path is stored.
- **Credentials**: the passphrase or password is asked for when connecting. If you choose **remember**, it is stored in the **system secure store** (GNOME Keyring, KWallet…); without one, it is asked for every time and **never** stored in plain text. You can delete it when editing the server.
- **Server identity**: the first time, the **fingerprint** (`SHA256:…`) is shown so you can check and confirm it. If it changes, the connection stops with a possible **man-in-the-middle attack** warning; it is only replaced after you review it. If the old key is in your `~/.ssh/known_hosts`, the application does not modify that file and gives you the command to do it yourself.

**Several servers**: the **selector** in the top bar shows which server you are working on, with its status, and marks remote ones. When you switch, the whole screen moves to that server. **Overview** shows all of them at once (status, version, containers and resources). Each server is independent: one that is down does not affect the others, and every operation goes to the server where you started it. Confirmations show **server and address**, and the activity panel shows each operation's server and lets you filter by it.

**On a remote server** containers, images, networks, volumes, terminals and Compose work as they do locally. Paths (Compose files, bind mount folders) are **the server's**: you type them and they are checked there; before writing a file on the server the destination is confirmed. In **Terminals** you can also open the **server's SSH shell**, a separate session type.

**Disconnect** closes the SSH session and its terminals; **delete profile** removes only the application's data (profile, credentials and saved identity). Neither touches the server's containers, images, networks, volumes or data.

Being in the server's `docker` group is equivalent to **root** access on it: decide with its administrator; rootless Docker is the safer alternative.

## 8. Dashboard and diagnosis {#dashboard}

When connected, the **Dashboard** shows data obtained at that moment: Docker Engine and API versions, socket and connection origin, containers (running, paused, stopped), images, computer data (system, kernel, CPU, memory), storage and runtime (driver, data directory, cgroups, logs) and the daemon warnings. **Refresh** requests them again.

When not connected, it shows the **diagnosis**: the specific cause, the steps to fix it (with commands to copy into a terminal), the sockets checked with their result and what was found installed. The **Reconnect** and **Configure connection** buttons are in the header.

## 9. Managing containers {#containers}

The **Containers** section lists all containers, like `docker ps -a`: status, name and short ID, Compose project, image, Docker status, published ports and age. You can filter by **All / Running / Stopped** and search by name, image, ID or project. Each row’s **⋯** menu (or a right-click) offers **Edit configuration** and **Create image from this container**.

| Status | Available actions |
|---|---|
| Running | Stop, Restart, Pause, Remove |
| Paused | Resume, Stop, Remove |
| Created or stopped | Start, Remove |

Stop and restart wait 10 seconds for the process to end before forcing it, like Docker. After each action the real status of the container is queried. If Docker rejects an action, its message appears above the table and in the activity panel. Lists update by themselves when something changes in Docker, also from the terminal.

## 10. Creating, duplicating, inspecting and removing containers {#container-lifecycle}

**Create container** opens a wizard with six tabs (only the image is required): General, Environment, Network, Storage, Resources and Advanced.

- **Command** and **Entrypoint** are written as in a terminal and split into arguments, shown below. No shell is run.
- The **Equivalent** panel shows the equivalent `docker run` and the exact body sent to the API.
- **Review** marks errors (they prevent creation) and warnings (privileged ports, sensitive folders mounted writable, privileged mode…).
- **Create** creates the container without starting it; **Create and start** also starts it.

Clicking a container's name opens its **detail**: Summary, Logs, Terminal, Statistics and JSON inspection (the full `docker inspect` response, with a Copy button).

**Duplicate** opens the wizard with the container's configuration and the name `<name>-copy`, and warns about settings that are not carried over.

**Remove** asks for confirmation. A running or paused container is only removed if you tick **Stop it by force and remove it**. Optionally its anonymous volumes are removed; named volumes and bound folders are not touched. It cannot be undone.

## 11. Editing a container's configuration {#container-edit}

**Edit configuration** is in each container's **⋯** menu (also by right-clicking the row) and in the detail header. The configuration is organised in tabs: **General**, **Network**, **Storage**, **Variables**, **Ports**, **Resources** and **Advanced**. Each change shows how it is applied:

| Change | How it is applied |
|---|---|
| Name | Direct (`docker rename`) |
| Restart policy, CPU, memory and memory reservation | Direct (`docker update`), without restarting the container |
| Connecting or disconnecting networks | Direct and immediate (`docker network connect` / `disconnect`) |
| Environment variables, ports, volumes and folders, description | Requires **recreating** the container |
| Removing a CPU or memory limit | Requires recreation (`docker update` cannot remove it) |
| Image, command, user, working directory and the other **Advanced** options | Read-only: Docker does not allow changing them. Use **Duplicate** |

- **Network**: shows the networks, IP, MAC and aliases. Docker does not allow changing the aliases or the IP of an existing connection: disconnect and connect again. If it is the container's only network, you are warned before disconnecting it.
- **Variables**: values that look secret are hidden on screen and never stored in the activity panel.
- **Description**: stored in the container's `cmc.description` label.

**Review changes** compares the current configuration with the new one: what changes (before and after, item by item), what is kept and what requires recreation. Nothing is applied until you confirm.

**Safe recreation.** When a change requires recreating, the wizard:

1. Checks the image, name and port conflicts with other containers, and the reasons that prevent a safe recreation: Compose or Swarm containers, created with `--rm`, paused or restarting. In those cases it does not let you continue.
2. Shows the risks you must accept one by one: files written outside volumes (they are not carried over to the new container), mounts that are no longer mounted, containers that use this container's network…
3. On confirmation: stops the container if it is running, renames it to `<name>-cmc-backup-<date>` (backup), creates the new one with **all** the original configuration plus your changes, connects its networks and, if it was running, starts it and checks it keeps running for a few seconds.
4. If anything fails, it deletes the new container (without touching its volumes), gives the original its name back and starts it again.

**Volumes, folders and images are never deleted.** Anonymous volumes are mounted again with their contents. The backup stays stopped in **General › Backups**, where you can **Restore** it (the current container is set aside, not deleted) or **Delete** it once you no longer need it. All operations are recorded in the activity panel, without sensitive values.

## 12. Creating an image from a container {#container-commit}

**Create image from this container** (list **⋯** menu and detail header) is the equivalent of `docker commit`: it saves the current state of the container's file system as a new image.

- Enter the **repository** and **tag** (empty = `latest`); optionally, **author** and **message**. The full name is shown and validated before creating.
- **It does not include volumes or mounted folders**: only the container's writable layer. The dialog lists the mounts left out.
- The image **does store the container's configuration**, including its environment variables (even those holding passwords or tokens): don't push it to a registry if it contains secrets.
- If the container is running, by default it is **paused** for a few seconds while the image is created, so that it is consistent; you can untick this. It is never stopped.
- If an image with that name already exists, you are warned and must explicitly accept it: the previous one is not deleted, it just becomes untagged.
- The original container is not modified. When finished, **View image** opens its detail in Images.

## 13. Logs, statistics and terminals {#logs-stats-terminal}

In a container's detail:

- **Logs**: live output (stderr in red). You can choose how many previous lines to load, filter, show timestamps, follow the end, copy what is visible and stop following. The last 10,000 lines are kept.
- **Statistics**: CPU, memory, network, disk and processes every second, calculated like `docker stats`.
- **Terminal**: **Open terminal** starts a shell with `docker exec -it` (Automatic uses `bash` if it exists, otherwise `sh`; you can also choose one). **Close session** ends the shell and the processes it left inside the container.

They are real operations that appear in the activity panel while open and are closed when you leave the detail. To work with several terminals at once, use the **Terminals** section.

## 14. Managing images {#images}

The **Images** section lists local images, like `docker image ls`: tags, short ID, size, age and which containers use them. Filters **All / In use / Unused / Untagged** and search by tag or ID.

- **Detail**: Summary (with sensitive variable values hidden), Layers (like `docker image history`), Containers that use it and JSON inspection.
- **Tag** adds a `repository:tag` name.
- **Remove**: if the image has several tags you can remove just one; the whole image requires forcing. Docker does not remove images of running containers.
- **Prune** removes untagged images not used by any container or, with additional confirmation, all unused ones.

## 15. Pulling and building images {#pull-build}

**Pull image** asks for the reference (`nginx:1.27`, `user/app`, `registry.example.com:5000/app:v2`; without a tag `latest` is used). Credentials:

| Option | What it does |
|---|---|
| Docker's | Uses what was saved with `docker login` (or its *credential helper*) |
| None | Anonymous pull |
| Username and password | Only for this pull: they are not saved and do not appear in the activity panel |

The **Downloads** panel shows the overall and per-layer progress; you can switch sections meanwhile and **Cancel**. **Search Docker Hub** searches like `docker search` (Docker Engine does it, and needs Internet access).

**Build image** opens a wizard: context (the folder sent, respecting `.dockerignore`) and Dockerfile, names (`--tag`), arguments (`--build-arg`), metadata (`--label`), target stage, no cache, pull newer base images and builder (**BuildKit** or **classic**). You see the steps with their status and the output like `docker build --progress=plain`; it can be cancelled.

## 16. Docker networks {#networks}

The **Networks** section lists networks, like `docker network ls`, with driver, subnets and containers. The **bridge**, **host** and **none** networks are predefined and cannot be removed.

- **Create network**: name, driver (`bridge`, `macvlan`, `ipvlan`, `overlay` or another), internal network, IPv4 addresses (subnet, gateway and range) and IPv6, driver options and metadata. You see the equivalent command and the exact request, and errors are marked before anything is sent to Docker.
- **Connect** a container (running or stopped), with optional aliases and fixed IP. **Disconnect** asks for confirmation.
- **Remove** and **Prune** warn if stopped containers have the network configured and will not be able to start afterwards.

## 17. Volumes {#volumes}

The **Volumes** section lists volumes, like `docker volume ls`, with their driver and the containers that mount them. Anonymous ones are marked as such. **Calculate sizes** asks Docker for the space of each one (like `docker system df -v`); it is not done automatically because it can take time.

- **Create volume**: name (optional) and type: local, tmpfs, NFS, a folder on the computer or another driver.
- **Detail**: directory, options, metadata, size, containers with their path and mode, and JSON inspection. **Create container with it** mounts it at `/data`.
- **Remove**: Docker does not remove a volume mounted by any container, even a stopped one.
- **Prune**: removes unused anonymous volumes or, with confirmation, all unused volumes.

## 18. Bound directories (Bind Mounts) {#binds}

This section shows the folders and files of your computer mounted inside containers, grouped by path: type, permissions and owner, whether they exist, which containers mount them, where and with what access. Those that need attention are flagged: the **Docker socket** (gives the container control of Docker), the root or system directories and paths that no longer exist.

Docker does not allow adding a folder to an existing container. **Bind a folder…** opens the folder picker and then the container wizard with that folder mounted at `/mnt/<name>`.

## 19. Docker Compose {#compose}

The **Docker Compose** section needs Compose v2 or later; if it is not installed, it explains how to install it. All operations run the official Docker Compose and appear in the activity panel with the exact command.

The list shows the projects with containers in Docker (also those deployed from the terminal) and the ones you add, with their status (running, partial, stopped or not deployed), folder and services.

- **Open project…**: choose its `compose.yaml` (and the additional files in the order they are merged).
- **New project**: choose a folder, write the content (starting from a template) and **Create project**: it is validated with Docker Compose before `compose.yaml` is created.
- **Remove from the list** only removes it from the list; it does not touch Docker or the files.

## 20. Managing Compose projects and profiles {#compose-projects}

In a project's detail:

- **Services**: each service with its image, status, containers, ports and **profiles**. Each row lets you deploy, start, stop, restart or view the logs of that service.
- **Deploy** (`docker compose up --detach`): all services or the chosen ones, **profiles** to enable, build first, image pull policy, recreate containers and remove orphans. The dialog shows the exact command.
- **Start, Stop, Restart**, **Pull images** and **Build**.
- **Remove** (`docker compose down`): the project's containers and networks; optionally its named volumes (their data is lost) and its images. It must be confirmed.
- **Output** of the running operation (it can be **Interrupted**) and live **Logs** of the services.
- **File**: edit the files; **Validate** checks them with Docker Compose without saving, and **Save** validates first and does not overwrite if another program changed them.
- **Configuration**: the final configuration according to Docker Compose. It may contain secrets, so it is hidden until you ask for it and is not saved in the activity panel.

## 21. Simultaneous terminals {#terminals}

The **Terminals** section lets you open and view several real terminals (`docker exec -it`) at the same time in running containers, in the same container or in different ones.

- **New terminal**: choose a running container, the shell (Automatic, bash or sh) and, if you want, a name for the session.
- Each pane shows the **container**, the **session name** and a **connection indicator** (Connecting, Connected, Ended or Error), with buttons to **rename**, **maximize** and **close**. Double-clicking the name also renames it.
- **Layout**: one terminal fills the whole space; two go in two columns; three, one on the left and two on the right; four, in a 2×2 grid; with more than four, in rows of two with vertical scrolling.
- The **splitters** are dragged with the mouse. With the focus on a splitter, the arrow keys move it, Home and End take it to the ends and double-clicking splits the space equally.
- Each session is independent: its own shell, input and output, size and state. What you do in one does not affect the others.
- Sessions **stay open when you switch sections**. **Close all** asks for confirmation. Up to 12 sessions are allowed at once.
- **Closing** a session ends its shell and the processes it left running in the background inside the container. If the container is **stopped** or **removed**, the pane says so; an ended session can be **reopened** if the container still exists.
- **Closing the application** ends all sessions.

The activity panel records the opening, closing and errors of each session, but not what is typed or the terminal output.

## 22. Activity panel and operation log {#activity}

Every operation appears in the **activity panel**: time, **API** badge (request to Docker Engine through the socket), **CLI** badge (process, like `docker compose` or `apt-get`) or **APP** badge (an operation of the application itself, such as a language change, or the summary of a recreation whose steps appear separately), type, resource, duration and status. The main line is the equivalent command, which you can copy. Expanding an entry shows the operation actually run, the result, the error and its HTTP code if it failed, and the output of processes.

The **Events and logs** section shows Docker events live, the saved ones, a query to Docker for a period (`docker events --since … --until …`; Docker only keeps the last 256) and the **operation history** of this and other sessions.

The history saves what was done, when and with what result, but never the request bodies, the command or the outputs, because they may contain passwords. It is in `~/.local/state/container-manage-center/`, readable only by your user.

## 23. Settings, themes and languages {#settings}

- **Appearance**: theme (System, Light, Dark) and language, in this order: **English**, **Español** (default), **Català**, **Galego**, **Euskara**, **Français**, **Italiano** and **Português**. Each language appears under its own name. The language is saved, kept when you reopen the application, and its changes are recorded in the activity panel. The primary color is **purple** (since version 1.0.0 no other can be chosen; if you had teal, it changes to purple without losing the rest of your settings).
- **Activity panel**: open it on errors, operations kept (50–10,000) and **Keep the history between sessions**.
- **Docker connection** and the **Docker Compose** check.
- **Updates**: where versions are published and **Check now**. The application never downloads or installs anything.
- **System**: versions of the application, the package, WebKitGTK and glibc, the system and Docker; **Copy** generates a report without sensitive data.
- **Uninstall and components** and **Help and documentation**.
- **Reset preferences**, after confirming. It does not change the Docker connection.

| Shortcut | Action |
|---|---|
| Ctrl+B | Collapse or expand the sidebar |
| Ctrl+Shift+A | Open or close the activity panel |
| Escape | Close a dialog |

Messages generated by Docker itself (for example, a Docker Engine error) and the output of programs are shown as they arrive, untranslated. The technical summaries that the application engine writes in the activity panel (for example, how a terminal ended) and error messages with specific details are generated in Spanish in every language.

## 24. Uninstall and dependency management {#uninstall}

**Settings › Uninstall and components** shows what the application installed and lets you remove it without using the terminal. **Analyze** queries dpkg and APT without changing anything:

- **Application package** (`container-manage-center`), with its size.
- **Dependencies that can be removed**: only those APT installed automatically **together with the application** (according to its history) and that no other package needs.
- **Kept**, with the reason: shared dependencies (with the packages that use them), explicitly installed, already present before, of uncertain origin (if the APT history does not show how the application was installed) and system components.
- **External tools**: Docker Engine, Docker CLI and Docker Compose. **Always kept.**
- **Data that is not deleted**: your settings, your history and the window data. Nothing in Docker is touched either: images, containers, networks, volumes and Compose projects.
- The full **APT simulation** of the uninstall.

You can choose **Container Manage Center only** or **with the removable dependencies**, or tick packages one by one; the summary shows how much space is freed. **Review and uninstall…** checks the selection again and simulates exactly the command: if APT would remove something you did not choose, you cannot continue. The dialog shows the command that will run:

```
pkexec /usr/bin/apt-get remove -y --no-auto-remove container-manage-center …
```

When you confirm, the system asks for your administrator password (pkexec) **only for that command**; the application still runs without privileges. If you cancel the authentication nothing is removed. `purge` is not used. Everything is recorded in the activity panel. If the application itself is removed, close it when finished.

If `pkexec` is not installed, the application cannot request permissions: the dialog shows the equivalent `sudo` command so you can run it yourself in a terminal.

## 25. Security and credential handling {#security}

- The application **does not run as root**, never asks for sudo to work and does not change socket permissions or groups.
- External commands run **without a shell** and with separate arguments: what you type is never interpreted as commands.
- **It does not store credentials.** Registry credentials are used only for one pull and do not appear in the activity panel. With "Docker's", those from `docker login` are used.
- Values of variables like `PASSWORD`, `TOKEN` or `SECRET` are hidden on screen and in the panel (`********`), although Docker receives them in full.
- The persistent history never saves request bodies, commands or outputs.
- Destructive operations ask for confirmation.
- The window blocks external connections. The application's only own connection is the update check, disabled while you do not set a source. The GitHub button in the Help can only open that address, in your browser.
- Mounting the Docker socket into a container or belonging to the `docker` group gives full control of the computer: the application flags it.

## 26. Troubleshooting common problems {#troubleshooting}

| Cause shown | What to do |
|---|---|
| Docker does not seem to be installed | `sudo apt install docker.io` or Docker's official guide |
| Docker Engine not found / stopped | `sudo systemctl enable --now docker` (rootless: `systemctl --user enable --now docker`) |
| No permission to use the socket | Rootless Docker, or the `docker` group and log in again |
| You already belong to the group, but it does not work | Log out and log back in |
| Unsupported Docker connection | `tcp://` or `ssh://` in the local connection: use a local socket (`docker context use default`); for a remote server, add it in **Servers** |
| Docker does not respond in time | `sudo systemctl restart docker` |
| Docker version too old | Update to Docker 20.10 or later |

- **Docker Compose does not appear**: install it (Debian 13: `sudo apt install docker-compose`; Ubuntu: `sudo apt install docker-compose-v2`; Docker repository: `docker-compose-plugin`) and click **Check** in Settings.
- **A pull or search fails**: Docker Engine needs Internet access; the error message is Docker's.
- **A terminal does not open**: the container must be running and have a shell (`sh` or `bash`).
- **Recreating a container is not allowed**: the reason is shown in the review (Compose container, created with `--rm`, paused, image no longer present…).
- **The uninstall does not offer a dependency**: another program uses it, it was already installed or it cannot be determined safely; the reason appears next to it.
- **To report a problem**: Settings › System › **Copy** generates a report without sensitive data.

## 27. Frequently asked questions {#faq}

**Does the application run `docker` commands?** No: it uses the Docker Engine API. The command you see in the activity panel is the equivalent, so you can reproduce it. Docker Compose is run with its official program.

**Do I need to be an administrator?** No. You only need access to the Docker socket. Only the package uninstall asks for permissions, and only for that command.

**Does it work without the Internet?** Yes. Only Docker operations against remote registries need network access.

**What happens to my containers if I close the application?** They keep running: Docker manages them. Closing ends the open terminals and stops following logs and statistics.

**Can I lose data when editing a container?** No volume, folder or image is deleted, and the original is kept as a backup. Only files written outside volumes stay in the backup, and the review warns you beforehand.

**Do I lose my data when uninstalling?** No. Your settings, your history and everything in Docker are kept.

**Where are the settings and history stored?** In `~/.config/container-manage-center/` and `~/.local/state/container-manage-center/`.

**How do I change the language?** In Settings › Appearance › Language.

## 28. Legal {#legal}

**Name:** Container Manage Center (**CMC** for short).

**Licence:** GNU General Public License, version 3 or later (SPDX identifier `GPL-3.0-or-later`).

**Copyright © 2026 Sergio Alfaro Lorite.**

This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version. It is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.

Full licence text: https://www.gnu.org/licenses/gpl-3.0.html (also in `/usr/share/doc/container-manage-center/LICENSE`).

**Third-party components.** The application includes third-party libraries and fonts that keep their own licences (MIT, Apache-2.0, MPL-2.0, OFL-1.1 and others) and their authors' copyright notices; they do not become GPL nor Sergio Alfaro Lorite's. Their full texts are in `/usr/share/doc/container-manage-center/THIRD_PARTY_NOTICES.md`, and the package copyright notice in `/usr/share/doc/container-manage-center/copyright`.

**Source code.** The corresponding source code is published with every release of the package (`container-manage-center-X.Y.Z-src.tar.gz`).

**Trademarks.** Container Manage Center is an independent project: it is not affiliated with or endorsed by Docker, Inc. “Docker” is a trademark of Docker, Inc. and is used here only to name the technology the application works with (Docker Engine, its API, Docker Compose and the Docker CLI).

The same information is in **Settings › Legal**.
