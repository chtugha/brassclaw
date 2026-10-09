#!/usr/bin/env bash
# Install a verified BrassClaw application/worker pair from a GitHub release.
# Linux amd64 and macOS arm64/amd64. PostgreSQL/pgvector are embedded in the
# application; Monty is a separate companion executable from the same release.
# Usage: bash install.sh [-v version] [--startup-timeout seconds]
# Root on systemd Linux installs a service; other installs are started manually.
set -euo pipefail

GITHUB_REPO="chtugha/brassclaw"
BINARY_NAME="brassclaw-reborn"
LEGACY_BINARY_NAME="brassclaw"
SERVICE_NAME="brassclaw"
SYSTEMD_DIR="/etc/systemd/system"
PINNED_VERSION=""
STARTUP_TIMEOUT=180
INSTALL_MODE="user"
USE_SYSTEMD=false
EXISTING_SERVICE=false
WAS_RUNNING=false
START_ATTEMPTED=false
PAIR_COMMITTING=false
LOCK_DIR=""
STAGE_DIR=""
DOWNLOAD_DIR=""
SERVICE_HOME=""
SERVICE_URL=""
NEW_TOKEN=""
TARGET_NAMES=("$BINARY_NAME" "monty_worker")

log_info()  { printf '[INFO]  %s\n' "$*"; }
log_warn()  { printf '[WARN]  %s\n' "$*" >&2; }
log_error() { printf '[ERROR] %s\n' "$*" >&2; }
log_step()  { printf '[STEP]  %s\n' "$*"; }

usage() {
    printf 'Usage: %s [-v version] [--startup-timeout seconds]\n' "$0"
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -v|--version)
                [[ $# -ge 2 ]] || { usage >&2; return 1; }
                PINNED_VERSION="${2#v}"
                shift 2 ;;
            --startup-timeout)
                [[ $# -ge 2 && "$2" =~ ^[1-9][0-9]{0,4}$ ]] || { usage >&2; return 1; }
                STARTUP_TIMEOUT="$2"
                shift 2 ;;
            -h|--help) usage; exit 0 ;;
            *) usage >&2; return 1 ;;
        esac
    done
    if [[ -n "$PINNED_VERSION" && ! "$PINNED_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+][A-Za-z0-9.-]+)?$ ]]; then
        log_error 'Invalid release version.'
        return 1
    fi
}

require_commands() {
    local name
    for name in curl python3 cat tr sed tail mktemp install cp mv chmod mkdir ln rm rmdir od id uname; do
        command -v "$name" >/dev/null || { log_error "Required command missing: $name"; return 1; }
    done
    python3 -c 'import sys; sys.exit(0 if sys.version_info >= (3, 9) else 1)' || {
        log_error 'Python 3.9+ is required for release metadata and safe uninstallation.'
        return 1
    }
    if ! command -v sha256sum >/dev/null && ! command -v shasum >/dev/null; then
        log_error 'Install sha256sum or shasum; checksum verification is required.'
        return 1
    fi
}

detect_artifact() {
    local platform
    platform="$(uname -s)/$(uname -m)"
    case "$platform" in
        Linux/x86_64) echo 'brassclaw-linux-amd64' ;;
        Darwin/arm64) echo 'brassclaw-macos-arm64' ;;
        Darwin/x86_64) echo 'brassclaw-macos-amd64' ;;
        *)
            log_error "No prebuilt release for $platform. Build and install BOTH binaries:"
            log_info '  cargo build --locked --release -p brassclaw -p brassclaw_monty_host --bins' >&2
            log_info '  sudo install -m 0755 target/release/brassclaw /usr/local/bin/brassclaw-reborn' >&2
            log_info '  sudo install -m 0755 target/release/monty_worker /usr/local/bin/monty_worker' >&2
            log_info 'Start manually as a non-root user; rerunning this installer still requires a supported release.' >&2
            return 1 ;;
    esac
}

select_release() {
    # Resolve a complete release from one catalogue response. Older stable
    # releases may predate the companion worker; never mix release versions.
    python3 -c '
import json, re, sys
asset = sys.argv[1]
required = {asset, asset + ".sha256", asset + "-monty-worker", asset + "-monty-worker.sha256"}
releases = json.load(sys.stdin)
if not isinstance(releases, list):
    raise SystemExit("Invalid GitHub release catalogue.")
for release in sorted(releases, key=lambda r: r.get("published_at") or "", reverse=True):
    tag = release.get("tag_name", "")
    names = {entry.get("name") for entry in release.get("assets", [])}
    if not release.get("draft") and re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+([-+][A-Za-z0-9.-]+)?", tag) and required <= names:
        if release.get("prerelease"):
            print("[WARN]  Newest complete application/worker release is a prerelease: " + tag + ". Use -v to choose a specific release.", file=sys.stderr)
        print(tag[1:])
        break
else:
    raise SystemExit("No complete application/worker release found. Use -v to pin a supported release.")
' "$1"
}

resolve_version() {
    if [[ -n "$PINNED_VERSION" ]]; then
        echo "$PINNED_VERSION"
        return
    fi
    local latest
    latest=$(curl -fsSL --connect-timeout 10 --max-time 60 \
        "https://api.github.com/repos/$GITHUB_REPO/releases?per_page=100" \
        | select_release "$1")
    [[ "$latest" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+][A-Za-z0-9.-]+)?$ ]] || {
        log_error 'Could not determine latest release. Use -v to pin a version.'
        return 1
    }
    echo "$latest"
}

sha256_check() {
    local file="$1" expected_file="$2" expected actual
    # Release checksum assets contain exactly one SHA-256 digest, not filenames.
    expected=$(cat "$expected_file")
    [[ "$expected" =~ ^[[:xdigit:]]{64}$ ]] || {
        log_error 'Malformed SHA-256 checksum.'
        return 1
    }
    if command -v sha256sum >/dev/null; then
        actual=$(sha256sum "$file") || return 1
    elif command -v shasum >/dev/null; then
        actual=$(shasum -a 256 "$file") || return 1
    else
        log_error 'No SHA-256 tool available; refusing installation.'
        return 1
    fi
    actual="${actual%% *}"
    [[ "$(printf '%s' "$actual" | tr 'A-F' 'a-f')" == "$(printf '%s' "$expected" | tr 'A-F' 'a-f')" ]]
}

download_pair() {
    local version="$1" artifact="$2" asset
    local base_url="https://github.com/$GITHUB_REPO/releases/download/v$version"
    DOWNLOAD_DIR=$(mktemp -d "${TMPDIR:-/tmp}/brassclaw-download.XXXXXX")
    for asset in "$artifact" "$artifact-monty-worker"; do
        log_step "Downloading and verifying $asset v$version"
        curl -fsSL --retry 3 --retry-connrefused --connect-timeout 10 --max-time 600 \
            -o "$DOWNLOAD_DIR/$asset" "$base_url/$asset"
        curl -fsSL --retry 3 --connect-timeout 10 --max-time 60 \
            -o "$DOWNLOAD_DIR/$asset.sha256" "$base_url/$asset.sha256"
        sha256_check "$DOWNLOAD_DIR/$asset" "$DOWNLOAD_DIR/$asset.sha256" || {
            log_error "Checksum verification failed for $asset."
            return 1
        }
    done
}

prepare_service() {
    [[ "$USE_SYSTEMD" == true ]] || return 0
    local load_state exec_start
    load_state=$(systemctl show "$SERVICE_NAME" --property=LoadState --value)
    if [[ "$load_state" != not-found ]]; then
        [[ "$load_state" == loaded ]] || { log_error "Existing service is $load_state; repair it first."; return 1; }
        EXISTING_SERVICE=true
        exec_start=$(systemctl show "$SERVICE_NAME" --property=ExecStart --value)
        case "$exec_start" in
            *"path=$INSTALL_DIR/$BINARY_NAME ;"*) ;;
            *"path=$INSTALL_DIR/$LEGACY_BINARY_NAME ;"*) TARGET_NAMES+=("$LEGACY_BINARY_NAME") ;;
            *) log_error 'Existing service uses another executable path. Upgrade that installation explicitly.'; return 1 ;;
        esac
        # Do not rewrite the unit, drop-ins, credentials, User, data root or flags.
        log_info 'Preserving the existing service unit and all operator configuration.'
    fi
}

stage_pair() {
    local artifact="$1" name
    mkdir -p "$INSTALL_DIR"
    LOCK_DIR="$INSTALL_DIR/.brassclaw-install.lock"
    if ! mkdir "$LOCK_DIR" 2>/dev/null; then
        LOCK_DIR=""
        log_error "Another installation holds $INSTALL_DIR/.brassclaw-install.lock. If interrupted, inspect it before removing it."
        return 1
    fi
    printf '%s\n' "$$" > "$LOCK_DIR/owner.pid"
    # Inspect after acquiring the lock: another installer may have created the
    # unit while we downloaded. Never turn that concurrent install into a reset.
    prepare_service
    if [[ ( -e "$INSTALL_DIR/$LEGACY_BINARY_NAME" || -L "$INSTALL_DIR/$LEGACY_BINARY_NAME" ) && " ${TARGET_NAMES[*]} " != *" $LEGACY_BINARY_NAME "* ]]; then
        TARGET_NAMES+=("$LEGACY_BINARY_NAME")
    fi
    STAGE_DIR=$(mktemp -d "$INSTALL_DIR/.brassclaw-install.XXXXXX")
    chmod 700 "$STAGE_DIR"
    mkdir "$STAGE_DIR/previous"
    # Stage on the destination filesystem so replacement uses rename, not copy.
    install -m 0755 "$DOWNLOAD_DIR/$artifact" "$STAGE_DIR/$BINARY_NAME"
    install -m 0755 "$DOWNLOAD_DIR/$artifact-monty-worker" "$STAGE_DIR/monty_worker"
    if [[ " ${TARGET_NAMES[*]} " == *" $LEGACY_BINARY_NAME "* ]]; then
        ln -s "$BINARY_NAME" "$STAGE_DIR/$LEGACY_BINARY_NAME"
    fi
    for name in "${TARGET_NAMES[@]}"; do
        if [[ -e "$INSTALL_DIR/$name" || -L "$INSTALL_DIR/$name" ]]; then
            [[ -f "$INSTALL_DIR/$name" ]] || { log_error "Not a regular executable: $INSTALL_DIR/$name"; return 1; }
            cp -p "$INSTALL_DIR/$name" "$STAGE_DIR/previous/$name"
        fi
    done
}

restore_pair() {
    local name failed=false
    for name in "${TARGET_NAMES[@]}"; do
        if [[ -f "$STAGE_DIR/previous/$name" ]]; then
            # Move the original inode back, with its original permissions.
            mv -f "$STAGE_DIR/previous/$name" "$INSTALL_DIR/$name" || failed=true
        else
            rm -f "$INSTALL_DIR/$name" || failed=true
        fi
    done
    [[ "$failed" == false ]]
}

commit_pair() {
    local name
    # Keep a matching backup pair. Do all fallible backup copies before replacing
    # the live files. Do not retain a stale backup for an absent old companion.
    for name in "${TARGET_NAMES[@]}"; do
        if [[ -f "$STAGE_DIR/previous/$name" ]]; then
            cp -p "$STAGE_DIR/previous/$name" "$STAGE_DIR/$name.bak"
            mv -f "$STAGE_DIR/$name.bak" "$INSTALL_DIR/$name.bak"
        else
            rm -f "$INSTALL_DIR/$name.bak"
        fi
    done
    PAIR_COMMITTING=true
    for name in "${TARGET_NAMES[@]}"; do
        mv -f "$STAGE_DIR/$name" "$INSTALL_DIR/$name"
    done
    PAIR_COMMITTING=false
}

render_systemd_unit() {
    local service_user="$1" reborn_home="$2" home_dir="$3"
    cat <<EOF
[Unit]
Description=BrassClaw Reborn Agent
Documentation=https://github.com/$GITHUB_REPO
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$service_user
WorkingDirectory=$reborn_home
Environment=BRASSCLAW_REBORN_HOME=$reborn_home
Environment=BRASSCLAW_RUNTIME_PROFILE=local_dev
EnvironmentFile=/etc/brassclaw/secrets.env
ExecStart=$INSTALL_DIR/$BINARY_NAME serve --host 127.0.0.1 --port 3000
Restart=on-failure
RestartSec=5
KillSignal=SIGINT
KillMode=mixed
TimeoutStopSec=180
StandardOutput=journal
StandardError=journal
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ReadWritePaths=$home_dir /tmp

[Install]
WantedBy=multi-user.target
EOF
}

create_systemd_service() {
    [[ "$USE_SYSTEMD" == true && "$EXISTING_SERVICE" == false ]] || return 0
    local service_user=brassclaw home_dir=/var/lib/brassclaw group_name
    if ! id "$service_user" >/dev/null 2>&1; then
        useradd --system --home-dir "$home_dir" --no-create-home --shell /usr/sbin/nologin "$service_user"
    fi
    [[ "$(id -u "$service_user")" != 0 ]] || { log_error 'The service account must not be root.'; return 1; }
    group_name=$(id -gn "$service_user")
    SERVICE_HOME="$home_dir/.brassclaw/reborn"
    install -d -m 0700 -o "$service_user" -g "$group_name" "$home_dir" "$home_dir/.brassclaw" "$SERVICE_HOME"
    # Hex avoids short alphanumeric output after filtering random input.
    NEW_TOKEN=$(od -An -N32 -tx1 /dev/urandom | tr -d '[:space:]')
    [[ "$NEW_TOKEN" =~ ^[[:xdigit:]]{64}$ ]] || return 1
    install -d -m 0755 /etc/brassclaw
    # Preserve preexisting operator secrets even when no service is installed.
    if [[ ! -e /etc/brassclaw/secrets.env ]]; then
        (umask 077; printf 'BRASSCLAW_REBORN_WEBUI_TOKEN=%s\nBRASSCLAW_REBORN_WEBUI_USER_ID=brassclaw-admin\n' "$NEW_TOKEN" > /etc/brassclaw/secrets.env)
    else
        NEW_TOKEN=""
        log_info 'Using existing /etc/brassclaw/secrets.env; ensure it contains the WebUI token and identity.'
    fi
    render_systemd_unit "$service_user" "$SERVICE_HOME" "$home_dir" > "$STAGE_DIR/$SERVICE_NAME.service"
    install -m 0644 "$STAGE_DIR/$SERVICE_NAME.service" "$SYSTEMD_DIR/$SERVICE_NAME.service"
    systemctl daemon-reload
    systemctl enable "$SERVICE_NAME"
}

process_env_value() {
    local pid="$1" key="$2" entry
    [[ "$pid" =~ ^[1-9][0-9]*$ && "$key" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]] || return 1
    [[ -r "/proc/$pid/environ" ]] || return 1
    while IFS= read -r -d '' entry; do
        if [[ "${entry%%=*}" == "$key" ]]; then
            printf '%s' "${entry#*=}"
            return 0
        fi
    done < "/proc/$pid/environ"
    return 1
}

loopback_url() {
    local url="$1"
    case "$url" in
        http://0.0.0.0:*) url="http://127.0.0.1:${url##*:}" ;;
        'http://[::]:'*) url="http://[::1]:${url##*:}" ;;
    esac
    # Only the numeric listener address from our own process banner is accepted.
    [[ "$url" =~ ^http://([0-9.]+|\[[0-9a-fA-F:]+\]):[0-9]+$ ]] || return 1
    printf '%s' "$url"
}

wait_for_readiness() {
    local deadline=$((SECONDS + STARTUP_TIMEOUT)) invocation pid logs url token_var token status
    local credential_file="$STAGE_DIR/readiness.header"
    log_step 'Waiting for the application and global Monty to be ready...'
    while (( SECONDS < deadline )); do
        invocation=$(systemctl show "$SERVICE_NAME" --property=InvocationID --value)
        pid=$(systemctl show "$SERVICE_NAME" --property=MainPID --value)
        if [[ -n "$invocation" && "$pid" != 0 ]] && systemctl is-active --quiet "$SERVICE_NAME"; then
            logs=$(journalctl --quiet --no-pager -o cat "_SYSTEMD_INVOCATION_ID=$invocation" 2>/dev/null) || logs=""
            url=$(printf '%s\n' "$logs" | sed -n 's/^[[:space:]]*listen[[:space:]]*: \(http:[^[:space:]]*\).*/\1/p' | tail -n 1)
            token_var=$(printf '%s\n' "$logs" | sed -n 's/.*auth[[:space:]]*:.*token \$\([A-Za-z_][A-Za-z0-9_]*\),.*/\1/p' | tail -n 1)
            if url=$(loopback_url "$url") && token=$(process_env_value "$pid" "$token_var") && [[ -n "$token" && "$token" != *$'\n'* && "$token" != *$'\r'* ]]; then
                # Keep credentials out of curl's argv and logs. Never use a proxy
                # or follow redirects when sending the operator bearer token.
                (umask 077; printf 'Authorization: Bearer %s\n' "$token" > "$credential_file")
                status=$(curl --noproxy '*' --fail --silent --connect-timeout 1 --max-time 2 \
                    --header "@$credential_file" "$url/api/settings/monty-vm/status") || status=""
                rm -f "$credential_file"
                if [[ "$status" =~ \"state\"[[:space:]]*:[[:space:]]*\"running\" ]] \
                    && systemctl is-active --quiet "$SERVICE_NAME" \
                    && [[ "$(systemctl show "$SERVICE_NAME" --property=InvocationID --value)" == "$invocation" ]]; then
                    SERVICE_URL="$url"
                    SERVICE_HOME=$(process_env_value "$pid" BRASSCLAW_REBORN_HOME) || SERVICE_HOME=""
                    log_info 'Application HTTP listener and live Monty status are ready.'
                    return 0
                fi
            fi
        fi
        sleep 1
    done
    log_error "Application readiness timed out after $STARTUP_TIMEOUT seconds. See journalctl -u $SERVICE_NAME -n 100."
    return 1
}

cleanup() {
    local result=$? retain_stage=false
    trap - EXIT
    if [[ "$PAIR_COMMITTING" == true ]]; then
        log_warn 'Executable replacement interrupted; restoring the previous pair.'
        restore_pair || { log_error "Restore failed. Previous files are retained in $STAGE_DIR/previous."; retain_stage=true; }
    fi
    if [[ "$result" != 0 && "$START_ATTEMPTED" == true ]]; then
        # Fence Restart=on-failure while the operator diagnoses a failed boot.
        systemctl stop "$SERVICE_NAME" || log_error 'Could not stop the failed release; inspect the service immediately.'
        log_warn 'New release failed to become ready. It has been stopped; installed executables are retained for diagnosis.'
        if [[ -f "$INSTALL_DIR/$BINARY_NAME.bak" ]]; then
            log_warn "Previous executables are retained in $INSTALL_DIR/*.bak. Check database migration compatibility before restoring them."
        fi
    elif [[ "$result" != 0 && "$WAS_RUNNING" == true && "$retain_stage" == false ]]; then
        systemctl start "$SERVICE_NAME" || log_error 'Could not restart the previous service; inspect its journal.'
    fi
    [[ -z "$DOWNLOAD_DIR" ]] || rm -rf "$DOWNLOAD_DIR" || log_warn "Could not remove $DOWNLOAD_DIR"
    if [[ -n "$STAGE_DIR" && "$retain_stage" == false ]]; then
        rm -rf "$STAGE_DIR" || log_warn "Could not remove $STAGE_DIR"
    fi
    if [[ -n "$LOCK_DIR" ]]; then
        rm -f "$LOCK_DIR/owner.pid"
        rmdir "$LOCK_DIR" || log_warn "Could not remove $LOCK_DIR"
    fi
    exit "$result"
}

print_summary() {
    local version="$1"
    log_info "BrassClaw v$version installed: $INSTALL_DIR/$BINARY_NAME and $INSTALL_DIR/monty_worker"
    if [[ "$USE_SYSTEMD" == true ]]; then
        printf '  Data: %s\n' "${SERVICE_HOME:-see the preserved service configuration}"
        printf '  Local WebUI: %s\n  Service: systemctl status %s\n' "$SERVICE_URL" "$SERVICE_NAME"
        printf '  Logs: journalctl -u %s -f\n' "$SERVICE_NAME"
        if [[ -n "$NEW_TOKEN" ]]; then
            printf '  Save your WebUI token: %s\n  Stored in /etc/brassclaw/secrets.env\n' "$NEW_TOKEN"
        else
            log_info 'Existing operator credentials preserved.'
        fi
    else
        if [[ "$INSTALL_MODE" == user ]]; then
            printf '  Data: %s\n' "${BRASSCLAW_REBORN_HOME:-$HOME/.brassclaw/reborn}"
        else
            printf '  Data: the non-root operator\047s BRASSCLAW_REBORN_HOME (default ~/.brassclaw/reborn)\n'
        fi
        printf '  Start as a non-root user (PostgreSQL refuses root):\n'
        printf '  BRASSCLAW_REBORN_WEBUI_TOKEN=<token> BRASSCLAW_REBORN_WEBUI_USER_ID=me %s serve\n' "$INSTALL_DIR/$BINARY_NAME"
        [[ ":$PATH:" == *":$INSTALL_DIR:"* ]] || printf '  Add %s to your shell PATH.\n' "$INSTALL_DIR"
    fi
}

main() {
    parse_args "$@"
    require_commands
    local artifact version
    artifact=$(detect_artifact)
    version=$(resolve_version "$artifact")
    if [[ $EUID -eq 0 ]]; then
        INSTALL_MODE=system
        INSTALL_DIR=/usr/local/bin
        if [[ "$(uname -s)" == Linux ]] && command -v systemctl >/dev/null; then
            USE_SYSTEMD=true
            command -v journalctl >/dev/null || { log_error 'journalctl is required for readiness checks.'; return 1; }
        fi
    else
        INSTALL_DIR="$HOME/.local/bin"
    fi
    trap cleanup EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    download_pair "$version" "$artifact"
    if [[ "$USE_SYSTEMD" == false ]]; then
        log_info 'For a manual upgrade, stop running BrassClaw instances before replacing executables.'
    fi
    stage_pair "$artifact"
    if [[ "$USE_SYSTEMD" == true ]]; then
        if systemctl is-active --quiet "$SERVICE_NAME"; then WAS_RUNNING=true; fi
        # Stop even an activating/restarting service before the pair changes.
        if [[ "$EXISTING_SERVICE" == true ]]; then systemctl stop "$SERVICE_NAME"; fi
    fi
    commit_pair
    create_systemd_service
    if [[ "$USE_SYSTEMD" == true ]]; then
        START_ATTEMPTED=true
        systemctl start "$SERVICE_NAME"
        wait_for_readiness
    elif [[ "$INSTALL_MODE" == user ]]; then
        mkdir -p "${BRASSCLAW_REBORN_HOME:-$HOME/.brassclaw/reborn}"
    fi
    print_summary "$version"
}

# Sourceable for focused tests without downloading, installing or touching services.
# BASH_SOURCE is unset when the installer is piped into bash.
if [[ "${BASH_SOURCE[0]:-$0}" == "$0" ]]; then
    main "$@"
fi
