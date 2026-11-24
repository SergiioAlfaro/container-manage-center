## 1. What is Container Manage Center {#about}

Container Manage Center (**CMC**) is a desktop application for managing **Docker Engine** without relying on the terminal, on this computer and on remote servers over SSH. Everything it shows is real data requested from Docker at that moment, and every action is a real operation: nothing is simulated.

- It talks to Docker Engine through its **official API**: over the Unix socket on Linux, over Docker Desktop's named pipe on Windows and over SSH with remote servers.
- It runs **Docker Compose** with its official program, with separate arguments and no shell.
- Every operation appears in the **activity panel** with its equivalent `docker` command, so you know exactly what was done.

You do not need to know how to use Docker from the terminal: each section explains what it does before doing it, and operations that delete or change something ask for confirmation. If a term is unfamiliar, see the **Glossary** (section 33).

## 2. System requirements {#requirements}

| Item | Requirement |
|---|---|
| System (Linux) | Debian 12 or later, or Ubuntu 22.04 or later, 64-bit (x86_64) |
| System (Windows, in testing) | 64-bit Windows 10 or 11 with Docker Desktop (section 28) |
| Docker | Docker Engine 20.10 or later (API 1.41+), in normal or rootless mode |
| Docker Compose | v2 or later, only for the Docker Compose section (`docker compose` or `docker-compose` v2). Compose v1 is not supported |
| Libraries (Linux) | WebKitGTK 4.1, GTK 3 and glibc. APT installs them with the package |
| Remote servers | The system SSH client (`ssh`) and, on the server, Docker Engine 20.10 or later |

Docker and Docker Compose are **external requirements**: the application does not install or configure them, but tells you how to do it if they are missing.

The application does not need Internet access. Only the operations Docker Engine performs against remote registries (pulling images, searching Docker Hub, building with base images that are not on the computer) need Docker to have network access.

## 3. Installation and first run {#install}

On Debian and Ubuntu, install the package with APT, which resolves the dependencies:

```
sudo apt install ./container-manage-center_X.Y.Z_amd64.deb
```

For Debian 12 and Ubuntu 22.04, use the compatible package, whose name ends in `-compat-debian12-ubuntu2204.deb`. Before installing you can check the download with the `SHA256SUMS` file published with each version (`sha256sum -c SHA256SUMS`).

The application appears in the menu as **Container Manage Center**. It runs as your normal user: **do not run it with sudo** (it refuses to start as root).

**First run.** When it opens, it looks for Docker Engine automatically. If it finds it, the **Dashboard** shows its status; if not, it shows the diagnosis with the cause and the steps to fix it. Nothing else needs to be configured to get started.

**Updating.** Install the new package the same way: your settings and your history are kept.

Installation on Windows is explained in section 28.

## 4. Main window and navigation {#interface}

The window has four areas:

- **Sidebar** (left): the sections, grouped. Collapse it with **Ctrl+B** or with **Collapse sidebar** at the bottom.
- **Top bar**: the section title, the **server selector** (which Docker you are working on: this computer or a remote server), the **connection indicator** and the **Activity** button.
- **Work area**: the content of the chosen section.
- **Activity panel** (right): the operations that have been run. Open and close it with **Ctrl+Shift+A** or with the **Activity** button.

| Group | Sections |
|---|---|
| (top) | Dashboard, Servers |
| Resources | Containers, Bulk management, Images, Networks, Volumes, Bind mounts |
| Orchestration | Docker Compose |
| Tools | Terminals, Audit, Backups |
| Monitoring | Monitoring, Events and logs |
| (bottom) | Settings |

This manual is in **Settings › Help and documentation**. The index on the left takes you to each section, **Previous** and **Next** go through the manual in order, and the search box shows only the sections that contain the text you type.

| Shortcut | Action |
|---|---|
| Ctrl+B | Collapse or expand the sidebar |
| Ctrl+Shift+A | Open or close the activity panel |
| Escape | Close a dialog or a menu |
| Arrows, Home and End | Move through menus and move the dividers in Terminals |

Status colours are always the same: **green** for what works or is running, **amber** for warnings and **red** for errors.

## 5. Connecting to Docker Engine {#connection}

The **connection indicator** in the top bar shows the status:

| Appearance | Meaning |
|---|---|
| Green dot and `Docker 29.x` | Connected. Hover over it to see the socket |
| `Connecting to Docker…` | Detecting or connecting |
| Red, `No connection to Docker` | Not connected. Click it to see the diagnosis |

In **automatic** mode, the application tries in this order: the `DOCKER_HOST` variable (`unix://` only), the active Docker context, your rootless socket and the system socket `/var/run/docker.sock`. It checks the connection every 10 seconds and reconnects by itself when Docker becomes available again.

The application never asks for sudo, does not change permissions or groups and does not run the diagnosis instructions for you. Your user needs to be able to open the socket: with rootless Docker or by belonging to the `docker` group (which in practice is equivalent to having administrator privileges).

Settings are saved immediately in `~/.config/container-manage-center/config.toml`. If you prefer to specify a particular socket, see the next section.

## 6. Rootless Docker and custom sockets {#rootless}

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

On a remote server, **Audit**, **Backups** and **Monitoring** also work (sections 23 to 25).

## 8. Dashboard and diagnosis {#dashboard}

When connected, the **Dashboard** shows data obtained at that moment: Docker Engine and API versions, socket and connection origin, containers (running, paused, stopped), images, computer data (system, kernel, CPU, memory), storage and runtime (driver, data directory, cgroups, logs) and the daemon warnings. **Refresh** requests them again.

When not connected, it shows the **diagnosis**: the specific cause, the steps to fix it (with commands to copy into a terminal), the sockets checked with their result and what was found installed. The **Reconnect** and **Configure connection** buttons are in the header.

The most common causes of a connection failure and how to fix them are in section 30.

## 9. Managing containers {#containers}

The **Containers** section lists all containers, like `docker ps -a`: status, name and short ID, Compose project, image, Docker status, published ports and age. You can filter by **All / Running / Stopped** and search by name, image, ID or project. Each row’s **⋯** menu (or a right-click) offers **Edit configuration** and **Create image from this container**.

| Status | Available actions |
|---|---|
| Running | Stop, Restart, Pause, Remove |
| Paused | Resume, Stop, Remove |
| Created or stopped | Start, Remove |

Stop and restart wait 10 seconds for the process to end before forcing it, like Docker. After each action the real status of the container is queried. If Docker rejects an action, its message appears above the table and in the activity panel. Lists update by themselves when something changes in Docker, also from the terminal.

To apply the same action to many containers at once, use **Bulk management** (section 13).

## 10. Creating, duplicating, inspecting and removing containers {#container-lifecycle}

**Create container** opens a wizard with six tabs (only the image is required): General, Environment, Network, Storage, Resources and Advanced.

- **Command** and **Entrypoint** are written as in a terminal and split into arguments, shown below. No shell is run.
- The **Equivalent** panel shows the equivalent `docker run` and the exact body sent to the API.
- **Review** marks errors (they prevent creation) and warnings (privileged ports, sensitive folders mounted writable, privileged mode…).
- **Create** creates the container without starting it; **Create and start** also starts it.

Clicking a container's name opens its **detail**: Summary, Logs, Terminal, Statistics and JSON inspection (the full `docker inspect` response, with a Copy button).

**Duplicate** opens the wizard with the container's configuration and the name `<name>-copy`, and warns about settings that are not carried over.

**Remove** asks for confirmation. A running or paused container is only removed if you tick **Stop it by force and remove it**. Optionally its anonymous volumes are removed; named volumes and bound folders are not touched. It cannot be undone.

The logs, statistics and terminal of the detail view are explained in section 21.

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

## 13. Bulk management {#bulk}

**Bulk management** (under “Resources”) applies the same operation or the same command to many containers at once, also across several servers. Containers from this computer and from **connected** remote servers are gathered; the others are listed in a notice. It is divided into tabs: **Summary**, **Containers**, **Operations**, **Bulk configuration**, **Bulk commands** and **History**.

- **Summary**: total, running, stopped, paused and with errors. Clicking a counter filters by that state. **Refresh** and automatic refresh (5 s to 5 min, can be turned off) request the data again.
- **Containers**: search by name or ID; filter by state, image, server and label (`key` or `key=value`); sort by clicking a column title. Select with each row's checkbox, the page's checkbox or **everything filtered**.
- **Operations**: start, stop, restart, pause, unpause and remove. Whatever cannot be applied in its current state is **skipped** and explained.
- **Bulk configuration**: shows the restart policy of the selection and lets you change it on all of them (live, with `docker update`; not on containers created with `--rm`). **Labels** are only shown: Docker does not allow changing them on an existing container (it would have to be recreated). For a single container, use **Duplicate**.
- **History**: previous bulk operations and commands, with what was done on each container.

**Before applying**, a confirmation shows the operation, the servers, the affected and skipped containers, the consequences and how many operations run at once (1 to 16). **Remove** asks you to type the number of containers and **never removes volumes or images**.

**Results**: each container has its own: succeeded (Docker confirmed it), failed (with the error), skipped, cancelled or **unknown** (the connection dropped before the confirmation; refresh to see its state). If one fails, the others carry on; nothing is undone or retried automatically. **Cancel pending** stops sending orders. Each order is recorded in the activity panel with the “Bulk” badge, and **View in activity** shows only that operation.

**Bulk commands** runs the same command inside several running containers with Docker Exec, without going through any shell on this computer:

1. Choose the mode: **Executable with arguments** (one argument per line, sent as is) or **Script with an interpreter** (several lines for `/bin/sh`, `bash`, `python3`…). You can also load an example or a saved frequent command.
2. If needed, set the user, the working directory, the time limit (1 to 3600 s) and how many executions run at once.
3. Check the **Preview**: the equivalent `docker exec`, the exact arguments Docker receives and the risk level (Informative, Modification, Administrative or Potentially destructive). The classification is a guide: it does not replace reviewing the command.
4. Confirm. Commands with administrative or destructive risk ask you to type the number of containers.

Each container reports its status, exit code, duration, and `stdout` and `stderr` separately (copyable and exportable to Markdown or JSON with secrets hidden). **Cancel pending** does not launch the command in the containers that have not started yet; **Interrupt active** also stops waiting for those that are running. Docker does not allow stopping a process once launched: when the time limit is reached or you interrupt, the process may keep running inside the container, and CMC **never** stops or restarts the container to halt it. Frequent commands can be saved, **never with secrets**.

## 14. Images {#images}

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

## 16. Networks {#networks}

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

**The data of a removed volume cannot be recovered.** If you need it, make a backup first (section 24).

## 18. Bind mounts {#binds}

This section shows the folders and files of your computer mounted inside containers, grouped by path: type, permissions and owner, whether they exist, which containers mount them, where and with what access. Those that need attention are flagged: the **Docker socket** (gives the container control of Docker), the root or system directories and paths that no longer exist.

Docker does not allow adding a folder to an existing container. **Bind a folder…** opens the folder picker and then the container wizard with that folder mounted at `/mnt/<name>`.

## 19. Docker Compose: projects {#compose}

The **Docker Compose** section needs Compose v2 or later; if it is not installed, it explains how to install it. All operations run the official Docker Compose and appear in the activity panel with the exact command.

The list shows the projects with containers in Docker (also those deployed from the terminal) and the ones you add, with their status (running, partial, stopped or not deployed), folder and services.

- **Open project…**: choose its `compose.yaml` (and the additional files in the order they are merged).
- **New project**: choose a folder, write the content (starting from a template) and **Create project**: it is validated with Docker Compose before `compose.yaml` is created.
- **Remove from the list** only removes it from the list; it does not touch Docker or the files.

## 20. Docker Compose: services, deployment and profiles {#compose-projects}

In a project's detail:

- **Services**: each service with its image, status, containers, ports and **profiles**. Each row lets you deploy, start, stop, restart or view the logs of that service.
- **Deploy** (`docker compose up --detach`): all services or the chosen ones, **profiles** to enable, build first, image pull policy, recreate containers and remove orphans. The dialog shows the exact command.
- **Start, Stop, Restart**, **Pull images** and **Build**.
- **Remove** (`docker compose down`): the project's containers and networks; optionally its named volumes (their data is lost) and its images. It must be confirmed.
- **Output** of the running operation (it can be **Interrupted**) and live **Logs** of the services.
- **File**: edit the files; **Validate** checks them with Docker Compose without saving, and **Save** validates first and does not overwrite if another program changed them.
- **Configuration**: the final configuration according to Docker Compose. It may contain secrets, so it is hidden until you ask for it and is not saved in the activity panel.

## 21. Logs, statistics and terminal of a container {#logs-stats-terminal}

In a container's detail:

- **Logs**: live output (stderr in red). You can choose how many previous lines to load, filter, show timestamps, follow the end, copy what is visible and stop following. The last 10,000 lines are kept.
- **Statistics**: CPU, memory, network, disk and processes every second, calculated like `docker stats`.
- **Terminal**: **Open terminal** starts a shell with `docker exec -it` (Automatic uses `bash` if it exists, otherwise `sh`; you can also choose one). **Close session** ends the shell and the processes it left inside the container.

They are real operations that appear in the activity panel while open and are closed when you leave the detail. To work with several terminals at once, use the **Terminals** section.

To see usage over time and receive alerts, use **Monitoring** (section 25).

## 22. Simultaneous terminals {#terminals}

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

## 23. Security audit {#audit}

**Tools › Audit** reviews the security of the chosen server (this computer or a remote one) and **changes nothing**: it only reads the configuration.

1. Choose **what to audit**: all containers (running and stopped) or one; also images and Compose projects (their configuration is read with `docker compose config`, without deploying).
2. Click **Audit**. Progress is shown and it can be cancelled.
3. Review the **findings**, by level (critical, high, medium, low) and grouped by container, service or image. Each one explains what is happening, **why it matters** and what is **recommended**. They can be filtered by level, type and text.

It checks the risks to the computer (mounted Docker socket, sensitive folders, capabilities, privileged mode, host network, shared PID and IPC, seccomp, AppArmor or SELinux disabled, devices and ports open on all interfaces) and hardening (process running as root, writable file system, no memory, CPU or process limits, no `no-new-privileges` and images more than a year old). With rootless Docker or `userns-remap`, root inside the container is marked as low.

- **Propose fixes** (containers; the button shows how many): opens **Edit configuration** with the changes already in the draft. Review and apply them like any other edit (section 11); if you discard them, nothing changes.
- **Vulnerabilities**: if you have **Trivy** or **Grype** installed with their database downloaded, tick the box and each image shows how many known vulnerabilities it has, by severity. The application does not install these tools. Without them, or on a remote server, the report says “Not scanned”, never “0”.
- **Export** to JSON, Markdown or HTML: the file is readable only by your user and secrets are hidden.
- **Previous audits**: the last 200 of each server, to open them again.
- **Containers in the other tools**: for each audited container, its worst finding, whether monitoring watches it, its most recent backup and the backup jobs that include it, with shortcuts to **View in Monitoring** and **Prepare a backup**.

## 24. Backups {#backup}

**Tools › Backups** makes backups of the chosen server (this computer or a remote one) and restores them.

1. **Destination folder**: **Choose folder** opens the system dialog. Inside it, `container-manage-center-copias/<server>/<backup>/` is created, readable only by your user. System folders cannot be used; an external disk or an already mounted network folder can.
2. **New backup**: tick what you want to back up.
3. **Back up now** shows progress and can be cancelled. A cancelled or interrupted backup is never considered valid.

| Item | How it is backed up |
|---|---|
| Volumes | Through the Docker API, with a helper container that is never started. If a running container uses it, choose **live** (backup not guaranteed) or **stopping the containers** (they are stopped while copying and started again) |
| Databases | Running MariaDB, MySQL, PostgreSQL, Redis or MongoDB: the dump is made by the engine's own tool inside the container, with the password from its variables. It is consistent even with ongoing writes |
| Container configuration | To recreate them, without variables containing secrets |
| Compose projects | Only on this computer: their files. `.env` files only if you tick “Include .env files” |
| Application settings | Preferences, servers and jobs (passwords are in the keyring and are not copied) |

- **Backups**: those of the server, with their status (Complete, Incomplete), items, size and warnings. **Verify** recalculates the SHA-256 sum of each file and says whether any is altered, missing or extra.
- **Restore**: choose which items and where to, and click **Preview**, which checks integrity and says what will be done with each one. A volume is restored into a **new** or empty one, never over one with data. A container is created new, with another name and not started. Files are written into a new folder.
- **Restoring a database replaces its current data**: you must type “restore” to confirm it.
- **Scheduled jobs**: **Save as job** saves the selection with a name, a frequency (daily, weekly or monthly) and a retention (by default, 7 daily, 4 weekly and 3 monthly). Retention only deletes old backups **of that job**. **Schedule** creates a systemd user timer (on Windows, a Task Scheduler task) that works even when the application is closed; it first shows you what it will create and only creates it if you authorise it. Administrator rights are not needed. Linux user timers only run while you are logged in, unless you enable `loginctl enable-linger` yourself.
- **History**: each backup, manual or scheduled, with its result and the old backups removed by retention.

Backups are **not encrypted**: they contain your data as is. If you store them outside the computer, use an encrypted disk.

## 25. Monitoring and alerts {#monitoring}

**Monitoring › Monitoring** shows container usage on the chosen server over time. It installs nothing on the server: it asks Docker the same as `docker stats`, also over SSH.

- **Enable**: tick “Monitor this server”. It is enabled per server and stays enabled when you reopen the application. The **interval** (2 to 60 s) is shared by all; **Pause** stops all sampling.
- **Summary**: the computer's CPU and memory according to Docker, running, paused and stopped containers, and total usage in charts. The table shows, per container, CPU, memory and its limit, network, disk, processes, uptime and restarts. Click a name to see its charts. CPU is in % of one CPU, like `docker stats`: a container using two whole CPUs shows 200 %.
- **Period**: 5, 15 and 30 minutes come from memory (since you opened the application); 1, 6 and 24 hours from the per-minute summary stored on your computer.
- **Compare**: up to 4 running containers and one metric in the same chart.
- **No connection**: if the server stops responding, the charts show a hatched stretch. No data is invented.

**Alerts**: high CPU and high memory (with a warning and a resolution threshold, so they do not fire at every small rise), the container crashes and Docker restarts it, out of memory (OOM), it exits with an error without anyone stopping it, and the connection to the server is lost. Stopping a container yourself is not an error. After a notification, the same alert is not repeated during the quiet minutes (15 by default). **Desktop notifications** do not leave the computer; **Test notification** sends a test one. All alerts are kept in the **Alert history**.

Monitoring does not record each sample in the activity panel: only when it starts and stops on each server.

## 26. Events, activity and operation history {#activity}

Every operation appears in the **activity panel**: time, **API** badge (request to Docker Engine), **CLI** badge (process, like `docker compose` or `apt-get`) or **APP** badge (an operation of the application itself, such as a language change, or the summary of a recreation whose steps appear separately), type, resource, server, duration and status. The main line is the equivalent command, which you can copy. Expanding an entry shows the operation actually run, the result, the error and its HTTP code if it failed, and the output of processes. Secret values are hidden with `********`.

The **Events and logs** section has four tabs:

- **Live**: Docker events of this session (a container starting, an image pulled…), whether they come from this application or from any other tool. They can be filtered and **Paused**.
- **Saved**: events received while the application was open, also from previous sessions (the 20,000 most recent).
- **Query Docker**: the events of a period (`docker events --since … --until …`). It includes what happened while the application was closed, but Docker only keeps the last 256.
- **Operation history**: what the application has done, also in previous sessions (the 10,000 most recent).

The history stores what was done, when and with what result, but never request bodies, commands or output, because they may contain passwords. It is in `~/.local/state/container-manage-center/`, readable only by your user. You can clear it from each tab or stop saving it in Settings.

## 27. Application settings {#settings}

- **Appearance**: theme (System, Light, Dark) and language, in this order: **English**, **Español** (default), **Català**, **Galego**, **Euskara**, **Français**, **Italiano** and **Português**. Each language appears under its own name. The language is saved and kept when you reopen the application. The primary colour is **purple** and cannot be changed.
- **Activity panel**: open it only when an operation fails, operations kept (50–10,000) and **Keep the history between sessions**.
- **Docker connection** (sections 5 and 6) and the **Docker Compose** check.
- **Updates**: where versions are published and **Check now**. The application never downloads or installs anything.
- **System**: versions of the application, the package, WebKitGTK and glibc, the system and Docker; **Copy** generates a report without sensitive data.
- **Uninstall and components** (section 29), **Help and documentation** (this manual) and **Legal** (section 34).
- **Reset preferences**, after confirming. It does not change the Docker connection.

Everything is saved immediately in `~/.config/container-manage-center/config.toml`.

Messages generated by Docker itself (for example, a Docker Engine error) and program output are shown as they arrive, untranslated. The technical summaries the application engine writes in the activity panel and error messages with specific data are generated in Spanish in all languages.

## 28. Windows {#windows}

The Windows version is **in testing**: the installer is built, but it has not yet been validated on a real Windows system. It is an `.exe` installer for your user, without administrator rights, and it is not signed, so Windows may show a SmartScreen warning when you open it.

On Windows, Container Manage Center works as on Linux, with these differences:

- **Local Docker**: it connects to **Docker Desktop** through its named pipe (`\\.\pipe\docker_engine`). Docker Desktop must be running, and your user must be in the local `docker-users` group (the Docker Desktop installer adds it).
- **Remote servers over SSH**: they use the Windows OpenSSH client (`ssh.exe`, included in Windows 10 and 11). As it does not support multiplexed connections, each operation opens its own connection: the first time the server identity is checked as on Linux, and a saved password or passphrase is used automatically. **Jump hosts** (ProxyJump) are not available yet, and in the **server terminal** resizing the window does not reach the server.
- **Remembered passwords**: in the Windows **Credential Manager** (Windows credentials), named `container-manage-center/…`.
- **Scheduled backups**: with **Task Scheduler**, in the “Container Manage Center” folder, with your user and without administrator privileges. They run while you are signed in; if the computer was off at that time, the backup runs as soon as possible.
- **Notifications**: the system ones (notification centre).
- **Data**: settings in `%APPDATA%\container-manage-center` and history in `%LOCALAPPDATA%\container-manage-center`.
- **Uninstalling**: from Windows Settings › Apps › Installed apps. Your data and Docker Desktop are not touched. The “Uninstall and components” page only exists on Debian and Ubuntu.

## 29. Uninstall and components {#uninstall}

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

## 30. Troubleshooting {#troubleshooting}

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

- **A remote server does not connect**: **Test connection** shows which of the seven steps fails (section 7).
- **A bulk command fails in a container**: the container must be running and have the program or interpreter specified; Docker's error appears in its result.

## 31. Security and good practices {#security}

- The application **does not run as root**, never asks for sudo to work and does not change socket permissions or groups.
- External commands run **without a shell** and with separate arguments: what you type is never interpreted as commands.
- **It does not store credentials.** Registry credentials are used only for one pull and do not appear in the activity panel. With "Docker's", those from `docker login` are used.
- Values of variables like `PASSWORD`, `TOKEN` or `SECRET` are hidden on screen and in the panel (`********`), although Docker receives them in full.
- The persistent history never saves request bodies, commands or outputs.
- Destructive operations ask for confirmation.
- The window blocks external connections. The application's only own connection is the update check, disabled while you do not set a source. The GitHub button in the Help can only open that address, in your browser.
- Mounting the Docker socket into a container or belonging to the `docker` group gives full control of the computer: the application flags it.

**Good practices**: use rootless Docker when you can; check the **Audit** from time to time; make **backups** before removing volumes or upgrading databases; do not publish ports on all interfaces if you only use them on this computer; do not store passwords in images or in frequent commands. In bulk management, removing and commands with administrative or destructive risk ask you to type the number of containers.

## 32. Frequently asked questions {#faq}

**Does the application run `docker` commands?** No: it uses the Docker Engine API. The command you see in the activity panel is the equivalent, so you can reproduce it. Docker Compose is run with its official program.

**Do I need to be an administrator?** No. You only need access to the Docker socket. Only the package uninstall asks for permissions, and only for that command.

**Does it work without the Internet?** Yes. Only Docker operations against remote registries need network access.

**What happens to my containers if I close the application?** They keep running: Docker manages them. Closing ends the open terminals and stops following logs and statistics.

**Can I lose data when editing a container?** No volume, folder or image is deleted, and the original is kept as a backup. Only files written outside volumes stay in the backup, and the review warns you beforehand.

**Do I lose my data when uninstalling?** No. Your settings, your history and everything in Docker are kept.

**Where are the settings and history stored?** In `~/.config/container-manage-center/` and `~/.local/state/container-manage-center/`.

**How do I change the language?** In Settings › Appearance › Language.

**Do scheduled backups keep running with the application closed?** Yes: the system runs them (systemd or the Windows Task Scheduler) while you are logged in.

## 33. Glossary {#glossary}

| Term | Meaning |
|---|---|
| Container | A running program isolated from the rest of the computer, created from an image |
| Image | Read-only template with the file system and configuration for creating containers |
| Image tag | Name of a version of an image, such as `nginx:1.27` |
| Container label | A `key=value` item stored in a container, such as `cmc.description` |
| Registry | Server images are pulled from, such as Docker Hub |
| Volume | Storage managed by Docker that is kept even if the container is removed |
| Bind mount | A folder or file of the computer mounted inside a container |
| Network | Virtual network connecting containers to each other and to the outside |
| Published port | A port of the computer that leads to a port of the container |
| Restart policy | What Docker does when a container exits or the computer starts |
| Docker Engine | The service that runs containers; CMC talks to it |
| Docker API | Interface used to control Docker Engine; the Docker CLI and CMC use it |
| Unix socket | Special file used to talk to Docker on the same computer (`/var/run/docker.sock`) |
| Rootless | Docker run by a normal user, without administrator privileges |
| Docker context | Named configuration of which Docker the CLI connects to (`docker context`) |
| `DOCKER_HOST` | Environment variable telling which Docker to connect to |
| Docker Compose | Tool for defining and running multi-container applications with a YAML file |
| Service and profile (Compose) | A service is each part of a Compose project; a profile groups optional services |
| Docker Exec | Running a program inside a container that is already running |
| `stdout` and `stderr` | A program's normal output and error output |
| Fingerprint | Summary of an SSH server's key (`SHA256:…`) used to check its identity |
| Secure store (keyring) | Place in the system where passwords are stored encrypted (GNOME Keyring, KWallet, Credential Manager) |
| Docker Desktop | Docker application for Windows and macOS |
| Named pipe | Windows mechanism used to connect to Docker Desktop (`\\.\pipe\docker_engine`) |

## 34. Legal information and licence {#legal}

**Name:** Container Manage Center (**CMC** for short).

**Licence:** GNU General Public License, version 3 or later (SPDX identifier `GPL-3.0-or-later`).

**Copyright © 2026 Sergio Alfaro Lorite.**

This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version. It is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.

Full licence text: https://www.gnu.org/licenses/gpl-3.0.html (also in `/usr/share/doc/container-manage-center/LICENSE`).

**Third-party components.** The application includes third-party libraries and fonts that keep their own licences (MIT, Apache-2.0, MPL-2.0, OFL-1.1 and others) and their authors' copyright notices; they do not become GPL nor Sergio Alfaro Lorite's. Their full texts are in `/usr/share/doc/container-manage-center/THIRD_PARTY_NOTICES.md`, and the package copyright notice in `/usr/share/doc/container-manage-center/copyright`.

**Source code.** The corresponding source code is published with every release of the package (`container-manage-center-X.Y.Z-src.tar.gz`).

**Trademarks.** Container Manage Center is an independent project: it is not affiliated with or endorsed by Docker, Inc. “Docker” is a trademark of Docker, Inc. and is used here only to name the technology the application works with (Docker Engine, its API, Docker Compose and the Docker CLI).

**Windows.** The Windows installer includes Microsoft's WebView2 loader, covered by an additional permission under GPL v3 §7.

The same information is in **Settings › Legal**.
