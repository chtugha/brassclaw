# BrassClaw Reborn — Operator Guide

This guide covers deploying, upgrading, and operating BrassClaw Reborn on a
Linux server under systemd. It is written for system administrators setting
up a production or single-host deployment.

---

## Prerequisites (fresh host)

Before running the fresh-install or upgrade sequence, ensure the service user,
directories, and binary are in place.

### On Debian / Ubuntu

```bash
# 1. Create the service user (no home-dir login, no shell):
useradd -r -d /var/lib/brassclaw -s /usr/sbin/nologin brassclaw

# 2. Create the required directories:
install -d -m 0750 -o brassclaw -g brassclaw /var/lib/brassclaw
install -d -m 0750 -o root      -g root      /etc/brassclaw
install -d -m 0755 -o root      -g root      /opt/brassclaw

# 3. Install the binary:
install -m 0755 ./target/release/brassclaw /usr/local/bin/brassclaw
```

### On RHEL / Fedora

Use `/sbin/nologin` instead of `/usr/sbin/nologin` when creating the service user:

```bash
useradd -r -d /var/lib/brassclaw -s /sbin/nologin brassclaw
```

---

## Service Unit Files

Two deployment variants are provided under `deploy/`:

| File | Use case |
|------|----------|
| `deploy/brassclaw.service` | Single-host with embedded Postgres (`local-dev` profile) |
| `deploy/brassclaw-external-pg.service` | Multi-tenant with external Postgres (non-local profiles) |

Copy the appropriate file to `/etc/systemd/system/brassclaw.service` and
complete the `EnvironmentFile=` block before starting the service.

```bash
# Single-host (embedded PG):
cp deploy/brassclaw.service /etc/systemd/system/brassclaw.service

# Multi-tenant (external PG):
cp deploy/brassclaw-external-pg.service /etc/systemd/system/brassclaw.service
```

---

## Environment Variable Model

### Bootstrap tier (inline `Environment=` in the unit file)

These vars are read **before** the database starts. They are safe to set inline
in the unit file because they contain no secret material.

| Variable | Default | Purpose |
|----------|---------|---------|
| `BRASSCLAW_REBORN_HOME` | `~/.brassclaw/reborn` | State root (data dir, embedded PG) |
| `BRASSCLAW_REBORN_PROFILE` | `local-dev` | Boot profile: `local-dev`, `local-dev-yolo`, `production` |
| `BRASSCLAW_REBORN_LOG` | — | Log filter (e.g., `brassclaw=info`) |
| `BRASSCLAW_PG_URL` | — | External Postgres URL; omit to use embedded Postgres |
| `BRASSCLAW_EMBEDDED_PG_PORT` | `5434` | Override embedded Postgres port if 5434 is taken |
| `BRASSCLAW_SECRETS_PASSPHRASE_FILE` | — | Path to master-key file; set **only** when using passphrase-wrapped ceremony |

> **Phase 11 note:** `BRASSCLAW_REBORN_PROFILE` will be renamed to
> `BRASSCLAW_RUNTIME_PROFILE` when Phase 11 ships. Until then use the current
> name. Valid pre-Phase-11 values: `local-dev`, `local-dev-yolo`, `production`.

### Operator-trusted tier (`EnvironmentFile=/etc/brassclaw/secrets.env`)

These vars are read **after** the DB is up. Their *names* are stored in
`brassclaw_config`; their *values* are read from the environment at runtime and
**never persisted** to the DB or any log.

`secrets.env` is read by systemd (running as root) via `EnvironmentFile=`. The
file must be `root:root 0600` — the service process never opens it directly.

Typical contents:

```bash
# BRASSCLAW_SECRETS_PASSPHRASE_FILE: ONLY add this line if you ran
# 'brassclaw secrets rewrap --strategy passphrase-file=...'
# Operators using raw-key-on-disk ceremony must OMIT this line.
# BRASSCLAW_SECRETS_PASSPHRASE_FILE=/var/lib/brassclaw/master.key

BRASSCLAW_REBORN_WEBUI_TOKEN=<your-bearer-token>
BRASSCLAW_REBORN_WEBUI_USER_ID=admin
OPENAI_API_KEY=sk-...
```

> **`master.key` ownership:** If you use the passphrase-wrapped ceremony, the
> `master.key` file is opened by the **service process** (`User=brassclaw`) at
> per-boot unwrap time. It must be `brassclaw:brassclaw 0600` (or
> `root:brassclaw 0640`). A `root:root 0600` file causes `EACCES` and boot
> failure. All setup commands that write `master.key` must run as `sudo -u
> brassclaw` (see sequences below).

---

## Fresh-Install Sequence (no prior BrassClaw state)

All commands that touch the data directory or `master.key` **must run as the
service user** — `initdb` refuses to run as root and `master.key` must be
readable by the `brassclaw` user at per-boot unwrap time.

```bash
# 1. Write initial config as the service user.
#    'config init' starts embedded PG, runs schema migrations (idempotent),
#    writes brassclaw_config rows, and sets boot.initialized = true.
sudo -u brassclaw brassclaw config init --yes \
    --provider openai --model gpt-4o-mini \
    --api-key-env OPENAI_API_KEY \
    --owner admin --tenant default \
    --budget-usd 5.00

# 2. (Optional — passphrase ceremony only) Wrap the master key as the service
#    user so master.key becomes brassclaw:brassclaw 0600.
#    Skip this step to use the raw-key-on-disk ceremony (the default).
#    --tenant must match --tenant passed to config init above (§4.4
#    tenant-resolution rule: explicit flag beats config lookup):
sudo -u brassclaw brassclaw secrets rewrap \
    --tenant default \
    --strategy passphrase-file=/var/lib/brassclaw/master.key

# 3. Populate secrets.env (root:root 0600):
install -m 0600 /dev/null /etc/brassclaw/secrets.env
# Add BRASSCLAW_SECRETS_PASSPHRASE_FILE ONLY if you ran step 2:
# echo "BRASSCLAW_SECRETS_PASSPHRASE_FILE=/var/lib/brassclaw/master.key" >> /etc/brassclaw/secrets.env
echo "BRASSCLAW_REBORN_WEBUI_TOKEN=your-bearer-token"                   >> /etc/brassclaw/secrets.env
echo "BRASSCLAW_REBORN_WEBUI_USER_ID=admin"                             >> /etc/brassclaw/secrets.env
echo "OPENAI_API_KEY=sk-..."                                             >> /etc/brassclaw/secrets.env

# 4. Install and start the service:
cp deploy/brassclaw.service /etc/systemd/system/brassclaw.service
systemctl daemon-reload
systemctl enable --now brassclaw
```

---

## Upgrade Sequence (existing BrassClaw installation)

> **Do NOT run `brassclaw config init`** on an upgrade. The migration
> (`migrate-from-libsql`) migrates `config.toml` and the libSQL DB
> automatically on first serve. Running `config init --yes` would clobber the
> migrated config with defaults.

```bash
# 1. (Optional — passphrase ceremony only) Wrap the master key as the service
#    user. 'rewrap' reads .reborn-local-dev-secrets-master-key (preserving
#    decryptability of migrated persisted secrets), runs schema migrations,
#    writes a brassclaw_secrets_master row, then zeroes and deletes the old
#    key file. Skip this step to use the raw-key-on-disk ceremony (the
#    default); §8.1 step 6 will auto-migrate the raw key file at first serve.
#
#    IMPORTANT: --tenant must match identity.tenant in config.toml.
#    grep tenant $BRASSCLAW_REBORN_HOME/config.toml
sudo -u brassclaw brassclaw secrets rewrap \
    --tenant <identity.tenant from config.toml> \
    --strategy passphrase-file=/var/lib/brassclaw/master.key

# 2. Populate secrets.env (root:root 0600):
install -m 0600 /dev/null /etc/brassclaw/secrets.env
# Add BRASSCLAW_SECRETS_PASSPHRASE_FILE ONLY if you ran step 1:
# echo "BRASSCLAW_SECRETS_PASSPHRASE_FILE=/var/lib/brassclaw/master.key" >> /etc/brassclaw/secrets.env
echo "BRASSCLAW_REBORN_WEBUI_TOKEN=your-bearer-token"                   >> /etc/brassclaw/secrets.env
echo "BRASSCLAW_REBORN_WEBUI_USER_ID=admin"                             >> /etc/brassclaw/secrets.env
echo "OPENAI_API_KEY=sk-..."                                             >> /etc/brassclaw/secrets.env

# 3. Install and start the service — migration runs automatically on first serve:
cp deploy/brassclaw.service /etc/systemd/system/brassclaw.service
systemctl daemon-reload
systemctl enable --now brassclaw
```

---

## master.key Ownership Requirements

The passphrase file (`master.key`) is opened by the **service process** at
per-boot unwrap time:

- The file must be owned by the `brassclaw` user: `brassclaw:brassclaw 0600`
  (or `root:brassclaw 0640`).
- A `root:root 0600` file causes `EACCES` and boot failure.
- All commands that create or write `master.key` must run as
  `sudo -u brassclaw` — see the sequences above.

---

## master.key DR Backup Mandate

> ⚠️ **The `master.key` passphrase file MUST be in your disaster-recovery
> backup set, stored separately from the Postgres data directory.**

Loss of `master.key` without a backup means all `brassclaw_secrets` rows
(OAuth tokens, credential-broker credentials) are **permanently
unrecoverable**. Postgres backups alone are not sufficient — the ciphertext
cannot be decrypted without the key.

Recommended approach: store `master.key` in an offline backup or a secret
manager (e.g., Vault, AWS Secrets Manager) separate from the server.

---

## CLI-Only Users and Maintenance

### Config management

All configuration is DB-backed. Use `brassclaw config` to manage it:

```bash
# Show all config keys
brassclaw config list

# Get a single key
brassclaw config get llm.default.model

# Set a key
brassclaw config set llm.default.model gpt-4o

# Remove a key
brassclaw config unset llm.default.model
```

### Pruning old data

The retention sweep runs automatically in the background during `brassclaw
serve`. For manual pruning outside of serve:

```bash
brassclaw maintenance prune-old-data
```

---

## rewrap vs rotate

These are two distinct operations:

| Operation | What it does | When to use |
|-----------|-------------|-------------|
| `brassclaw secrets rewrap --strategy <...>` | Changes the **wrapping ceremony** — e.g., switches from raw-key-on-disk to passphrase-wrapped. Reads the existing master key and re-writes `brassclaw_secrets_master` with the new wrapping. Does **not** generate a new master key; all existing `brassclaw_secrets` rows remain decryptable. | Upgrading ceremony type; migrating from legacy raw-key file. |
| Key rotation (future) | Generates a **new** master key and re-encrypts all `brassclaw_secrets` rows. | Periodic security rotation; suspected key compromise. |

`rewrap` is the operation documented in the §7.1 and §7.2 sequences. It is
sufficient for upgrade deployments and ceremony changes.

### Passphrase change via rewrap

To change the passphrase on an existing passphrase-wrapped key:

```bash
sudo -u brassclaw brassclaw secrets rewrap \
    --tenant default \
    --strategy passphrase-file=/var/lib/brassclaw/master.key.new \
    --old-passphrase-file=/var/lib/brassclaw/master.key.old
```

If the old passphrase is not available interactively, use
`--old-passphrase-file=<path>`. The env vars
`BRASSCLAW_SECRETS_PASSPHRASE_FILE` and `$CREDENTIALS_DIRECTORY` are the
systemd-injected fallbacks for unattended operation.

---

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|-------------|-----|
| Service exits with `EACCES` | `master.key` is root-owned | Chown to `brassclaw:brassclaw 0600` |
| `boot.initialized` not set after upgrade | `config init` was run on upgrade | Restore from DB backup or run `brassclaw migrate` |
| Embedded PG port in use | Port 5434 occupied | Set `BRASSCLAW_EMBEDDED_PG_PORT=<free-port>` in unit |
| Passphrase mismatch on boot | `rewrap` used wrong `--tenant` | Re-run `rewrap` with the correct `--tenant` value matching `identity.tenant` in `brassclaw_config` |
| PG JIT crash under systemd hardening | `jit=off` missing from `postgresql.conf` | BrassClaw sets `jit=off` in `postgresql.conf` automatically; verify the embedded PG data dir is not stale |
| `brassclaw config get` stops embedded PG | CLI started its own PG and shut it down | CLI uses conditional-shutdown: only shuts down PG if it started it; if a live postmaster is running (from `serve`), it is left running |
