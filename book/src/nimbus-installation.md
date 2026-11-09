# Installing Nimbus server

You will need the `nimbus-server` executable, which can be obtained from the following sources.

## Releases

See: [GitHub releases](https://github.com/biblius/nimbus/releases) (tags `nimbus-server-v*`).

Every `nimbus-server` release ships 2 binaries with different database
engines; SQLite and Postgres, the main difference being connection pooling.
SQLite does not allow multiple simultaneous writers, so its max pool size
is set to 1.

Choose the former if you do not plan to have a lot of users and want to
have an out of the box DB that just works. Choose the latter if you
want proper connection pools.

## Building from source

You will need [cargo](https://rust-lang.org/) and [npm](https://nodejs.org/en). You can usually get these from your OS package manager.

1. Build the web client

```bash
npm run build -w @nimbus/web
```

2. Build the server with the desired DB engine

The build target is up to you.

```bash
cargo build -p nimbus-server --release --no-default-features -F sqlite
```

or

```bash
cargo build -p nimbus-server --release --no-default-features -F postgres
```

The binary should be in `./target[/target-arch]/release/nimbus-server`.

## Installation

We will not go through on how to run a Docker container, instead we
will focus on bare-metal Linux because we are ultrachads. If you
want to spin up a Nimbus container then check out the [Dockerfile](https://github.com/biblius/nimbus/blob/master/server/Dockerfile).

The following implies you already have a session on the host you intend
to install this on and have an existing `nimbus-server` binary at
`/opt/nimbus/nimbus-server`. You can use any directory, just make sure
to adjust the values.

There are several ways you can transfer the binary to the host and is beyond
the scope of this guide.

One way you can do it is with `rsync`:

```bash
rsync --mkpath /path/to/nimbus-server user@69.69.69.69:/opt/nimbus
```

### User, group, executable

Create the Nimbus system user and group and make it own the directory.

```bash
sudo groupadd --system nimbus

sudo useradd --system \
  --gid nimbus \
  --home-dir /var/lib/nimbus \
  --shell /usr/sbin/nologin \
  --create-home \
  nimbus

sudo chown nimbus:nimbus -R /opt/nimbus
sudo chmod -R 750 /opt/nimbus
```

### Configure environment

Configure the environment Nimbus will run in. See [example.env](https://github.com/biblius/nimbus/blob/master/example.env)
for the full list of supported variables. For now you will need only:

- `DATA_DIR`: Where Nimbus keeps everything; Files, thumbnails, config, and the SQLite database all reside in this directory. Remember to `chown` it if you are setting it up somewhere else other than `/opt/nimbus`.
- `NIMBUS_ORIGIN`: The origin where Nimbus will ultimately be served from. You can change this later in the app.
- `DATABASE_URL`: Depending on which binary you installed, this is either the SQLite or the Postgres database URL. Must include the user and password.
- `ADDRESS`: the server listen address; Set this to `127.0.0.1:42069` (i.e. localhost). We'll use nginx to forward traffic.

Use a `systemd` unit to run the binary and point it to the `.env` file.
Create and populate `/opt/nimbus/.env` with the at least the above values.

```bash
# Everything ever written by Nimbus goes here
DATA_DIR=/path/to/datadir

# The domain where Nimbus will be hosted
NIMBUS_ORIGIN=https://my.nimbus.org

# The database URL depending on which binary you have.
# Has to be set when using the postgres bin.
# If using SQLite, omitting this will make a DB
# in DATA_DIR.
DATABASE_URL=sqlite:///opt/nimbus/nimbus.db

# Server listen address and port
ADDRESS=127.0.0.1:42069

# The above env variables MUST be set by you. Everything
# below this is optional.

# If you want to use Valkey as the cache. When unset,
# uses an in-memory cache.
VALKEY_URL=redis://127.0.0.1:6379

# Controls the log level
RUST_LOG=info,sqlx=off,h2=off,nimbus_server=debug

# 100 GiB; If unset, defaults to 100GiB. Can be changed in app.
UPLOAD_MAX_BYTES=107374182400
```

If you have postgres set up, create a DB user and password (beyond the scope).
If you are using SQLite as the database, create the file.

```bash
touch /opt/nimbus/nimbus.db
```

### Configure systemd

```systemd
[Unit]
Description=Nimbus file hosting
Documentation=https://github.com/biblius/nimbus
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=nimbus
Group=nimbus
EnvironmentFile=/opt/nimbus/.env
ExecStart=/opt/nimbus/nimbus-server
WorkingDirectory=/opt/nimbus
Restart=on-failure
RestartSec=5
# Capture process output in the journal (`journalctl -u nimbus -f`).
StandardOutput=journal
StandardError=journal
SyslogIdentifier=nimbus

[Install]
WantedBy=multi-user.target
```

Use the above as a template and save it to `/etc/systemd/system/nimbus.service`.

Load and enable the service.

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now nimbus
sudo systemctl status nimbus
```

Check application logs.

```bash
journalctl -u nimbus -f
```

If there are no errors, you have successfully installed Nimbus on bare metal!

Updates to Nimbus' `.env` file will need a restart to take effect.

```bash
sudo systemctl restart nimbus
```

If you have set the listen address to something like `0.0.0.0:42069` then
Nimbus will be available on that machine through the browser in your local
network.

If you want to keep your Nimbus only on your LAN then you are done installing Nimbus.
Head to the [post install checklist](#post-install-checklist).

### Configure nginx

The following implies you have a domain set up that you can point to Nimbus,
have [nginx installed](https://nginx.org/en/linux_packages.html#instructions)
and have [certbot](https://certbot.eff.org/instructions?ws=nginx&os=pip). It also assumes you are running nginx on the same host as Nimbus.

```nginx
# /etc/nginx/sites-enabled/nimbus.vhost.
#
# Replace my.nimbus.org with your domain, i.e. NIMBUS_ORIGIN.
#
# Point server to whatever you set ADDRESS to.

upstream nimbus {
    server 127.0.0.1:42069;
    keepalive 64;
}

server {
    listen 80;
    server_name my.nimbus.org;

    server_tokens off;

    location / {
        # App enforces UPLOAD_MAX_BYTES (default 10GiB)
        client_max_body_size 0;

        # Stream uploads/downloads instead of buffering whole bodies in nginx.
        proxy_request_buffering off;
        proxy_buffering off;
        proxy_http_version 1.1;
        proxy_set_header Connection "";

        proxy_set_header Host              $host;
        proxy_set_header X-Real-IP         $remote_addr;
        proxy_set_header X-Forwarded-For   $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Forwarded-Host  $host;

        # Range / partial content for blob downloads and media seeking.
        proxy_set_header Range            $http_range;
        proxy_set_header If-Range         $http_if_range;

        # Large transfers + SSE (trash empty, root delete, user delete).

        proxy_connect_timeout 60s;
        proxy_send_timeout    3600s;
        proxy_read_timeout    3600s;

        proxy_pass http://nimbus;

        # Browser hardening

        add_header X-Content-Type-Options "nosniff" always;
        add_header X-Frame-Options "DENY" always;
        add_header Referrer-Policy "strict-origin-when-cross-origin" always;
        add_header Permissions-Policy "camera=(), microphone=(), geolocation=()" always;
        add_header Cross-Origin-Opener-Policy "same-origin" always;
        # Set to same-origin if you do not plan on using Nimbus desktop
        add_header Cross-Origin-Resource-Policy "cross-origin" always;
        # SPA + pdf.js worker + blob previews; blob downloads already set their own CSP.
        add_header Content-Security-Policy "default-src 'self'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'; script-src 'self'; worker-src 'self' blob:; style-src 'self' 'unsafe-inline'; img-src 'self' blob: data:; media-src 'self' blob:; font-src 'self'; connect-src 'self'; object-src 'none'" always;
    }

    # Compress text-ish SPA/API responses; skip already-compressed media.
    gzip on;
    gzip_comp_level 5;
    gzip_min_length 256;
    gzip_proxied any;
    gzip_vary on;
    gzip_types
        text/plain
        text/css
        text/javascript
        application/javascript
        application/json
        application/wasm
        image/svg+xml
        application/manifest+json;
}
```

Use the above as a template and put the file in `/etc/nginx/sites-enabled/nimbus.host`.

Configure:

- `server_name` what you set `NIMBUS_ORIGIN` to.
- `server` to what you set `ADDRESS` to.

Run certbot and follow instructions to configure SSL, it should be
pretty straightforward.

Run

```bash
nginx -t
```

to check whether the configuration is correct and reload nginx.

```bash
systemctl reload nginx
```

Your Nimbus should now be reachable via HTTPS on your domain.

## Post-install checklist

- Log in to Nimbus using username:password `admin:admin`
- **CHANGE THE DEFAULT `admin` PASSWORD!!!**
- Configure SMTP in the admin settings page to enable lost password
  emails
- Set up 2FA
- Enable HTTP2 on nginx (not mandatory, but nice to have)

## Scaling Nimbus

If you installed Nimbus on bare metal and only you and
a few other people are using it, you should be fine with SQLite
and the in-memory cache.

If you plan to run a containerized Nimbus and plan to scale it
horizontally, it just won't work with SQLite and in-memory caching.
You need the Postgres binary with Valkey configured so all instances
of `nimbus-server` can talk to the same databases.
