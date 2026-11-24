# Run NDM on a server

Run Next Download Manager on a home server or a NAS (Docker, Unraid, any Linux machine) without
a window, and send it downloads from your phone or your browser: the server downloads them into
its own folders, such as a network share.

It is the same NDM as on a desktop: its list, categories, queues, Scheduler, speed limits and
torrents. You control it with the [remote control's page](remote-control.md) (from any phone or
computer on your network), the browser extension pointed at it, or its command line.

> **Feedback:** report server issues at
> [zekeeol/NextDownloadManager Issues](https://github.com/zekeeol/NextDownloadManager/issues).

## With Docker

Build the image yourself from NDM's source:

```sh
git clone https://github.com/zekeeol/NextDownloadManager
cd NextDownloadManager
docker build -t nextdownloadmanager .
```

The first build is long (the release profile enables whole-program optimization). The resulting runtime image is slim (`debian:trixie-slim` with `nextdownloadmanager-backend`, `nextdownloadmanager-cli`, and CA certificates), with no desktop GUI libraries required.

Then run it, with a folder for its settings and one for the downloads. Make both folders first,
as the user NDM will run as: a folder Docker makes for you belongs to `root`, and NDM can't use it.

```sh
mkdir -p /srv/ndm-config /srv/downloads
docker run -d --name ndm --restart unless-stopped \
  -p 8463:8463 \
  -v /srv/ndm-config:/config \
  -v /srv/downloads:/downloads \
  --user "$(id -u):$(id -g)" \
  nextdownloadmanager
```

- `/config` keeps the list, the settings, the certificate and the paired devices.
- `/downloads` is where files land: each category has its folder inside (`Music`, `Video`,
  `Torrents`…). Mount a network share there to download straight onto it.
- `--user`: the user that owns those folders (NDM runs as no particular user).

[docker/compose.yml](../docker/compose.yml) does the same with Docker Compose.

## Pair your phone

`docker logs ndm` shows the address to open and, while no device is paired, a pairing code with a
QR code:

```text
The remote control listens at: https://172.17.0.2:8463/
Its certificate's SHA-256 fingerprint: DE:26:7F:…
No device is paired yet. Open the address (or scan this) and enter the code 1037 1606 …
```

Inside a container, the address NDM finds is the container's own: give it the one you open, the
**server's** address with the port you published, as `-e NDM_REMOTE_URL=https://192.168.1.10:8463/`,
and the log and the QR code show that one. Then pair as
[Control NDM from your phone](remote-control.md#pair-your-phone) says. For another code, at any
time:

```sh
docker exec ndm nextdownloadmanager-cli pair
```

To open the page by the server's name (`https://nas.local:8463/`), give NDM that name:
`-e NDM_REMOTE_HOSTS=nas.local` (several: separated by commas). Other names are refused, so that
no web page can reach NDM through a name of its own.

## On Unraid

[docker/unraid.xml](../docker/unraid.xml) is a template: copy it to
`/boot/config/plugins/dockerMan/templates-user/` on the flash drive, then **Docker → Add
Container** and pick **nextdownloadmanager**. It maps `/config` to `appdata`, `/downloads` to a
share, the port 8463, and runs as `nobody:users` (99:100), as Unraid's shares expect. The image
must be built first (above), on the server or pushed to a registry you use. If the container's
log says a folder belongs to another user, give it to `nobody:users` once:
`chown -R 99:100 /mnt/user/appdata/nextdownloadmanager`.

## Without Docker

On any machine with NDM built or installed:

```sh
nextdownloadmanager-backend --headless --data ~/ndm-server
```

| Flag | Or the variable | What it sets |
|---|---|---|
| `--data <folder>` | `NDM_DATA_DIR` | the list, settings, certificate and paired devices |
| `--downloads <folder>` | `NDM_DOWNLOADS` | where downloads go (each category's folder inside) |
| `--port <number>` | `NDM_REMOTE_PORT` | the remote control's port (8463) |
| `--listen <address>` | `NDM_REMOTE_LISTEN` | where it listens: every local network address unless given |
| `--hosts <names>` | `NDM_REMOTE_HOSTS` | names besides its addresses the page may be opened by |
| | `NDM_REMOTE_URL` | the page's address as you open it, shown with the pairing code |

The remote control is on in this mode (`NDM_REMOTE=off` turns it off). It stops cleanly, saving
its list, when the system stops it (`docker stop`, systemd, Ctrl+C). It works on Windows and
macOS too, the same way.

The command line talks to it as to a desktop NDM, with the same data folder:
`NDM_DATA_DIR=~/ndm-server nextdownloadmanager-cli status` (once `api on`), `pair`, and adding
an address: `nextdownloadmanager /d https://example.com/file.zip`.

## Send downloads from your browser

In the browser extension's menu, under **Send downloads to**:

- Select **Another Next Download Manager (a server)**: type the server's address,
  open it once in that browser to accept its certificate, then enter a pairing code (`pair`
  above).
- The extension remembers paired servers: switch between any paired server and **This computer**
  with a single click without having to re-pair. Switching back leaves the browser paired; click
  **×** next to a server to unpair and forget it on both sides.
- Downloads the extension catches, and **Download with Next Download Manager** on a link,
  then go to the server instead of this computer. The browser's cookies are not sent to the server:
  a download that needs you to be signed in to a site stays with this computer.

## What a server doesn't do

- Video pages (yt-dlp) and image galleries need their helpers, which the image doesn't include.
- Nothing opens a window: a download that would ask a question (a duplicate, files already
  there) takes the choice set in the settings, or isn't added.
- Don't publish the port on the internet; reach your home network through a VPN instead.
