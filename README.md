# Simple Release Management

Simple Release Management is a web app for copying Docker images between registries, tracking immutable bundle versions, and generating deploy-ready releases with auditable logs.

## Key concepts

- **Tenant**: Logical customer/account boundary.
- **Registry**: Source/target registry configuration and credentials.
- **Bundle**: A named collection of image mappings.
- **Bundle version**: Immutable snapshot of mappings.
- **Copy job**: Copies images for a bundle version to a target registry (via `skopeo`).
- **Image release**: Named target tag for a successful copy job.
- **Deploy target**: Build/deploy pipeline config (git repos, env paths, keys).
- **Deploy job**: Regenerates deploy manifests for a release.

## Features

- Multi-tenant model
- Immutable bundle versions
- Copy jobs with live + audit logs
- Auto tag generation: `YYYY.MM.DD.COUNTER`
- Image releases with rename rules + preview
- Deploy targets + deploy jobs (kube_build_app + encjson + apply-env + kubeconform)
- SSE live logs and persisted audit logs

## Quick start

### 1) Prerequisites

- Rust 1.75+
- PostgreSQL 15+ (or Docker Compose)
- `skopeo` installed and in PATH

### 2) Setup

```bash
git clone <repo-url>
cd simple-release-management
cp .env.example .env
```

### 3) Database

```bash
docker-compose up -d
```

### 4) Run

```bash
cargo run

# Custom host/port
cargo run -- --host 0.0.0.0 --port 8080

# CLI help/version
cargo run -- --help
cargo run -- --version
```

App runs at `http://127.0.0.1:3000` by default.

## Configuration

Configuration is read from environment variables (see `.env.example`).

| Variable | Description | Default |
|---|---|---|
| `DATABASE_URL` | PostgreSQL connection string | (required) |
| `BASE_PATH` | Base path for reverse proxy | empty |
| `SKOPEO_PATH` | Path to `skopeo` binary | `skopeo` |
| `KUBE_BUILD_APP_PATH` | Path to `kube_build_app` | `kube_build_app` |
| `APPLY_ENV_PATH` | Path to `apply-env` | `apply-env` |
| `ENCJSON_PATH` | Path to `encjson` | `encjson` |
| `KUBECONFORM_PATH` | Path to `kubeconform` | `kubeconform` |
| `ENCRYPTION_SECRET` | Secret for encrypting credentials | (required) |
| `MAX_CONCURRENT_COPY_JOBS` | Parallel copy limit | `3` |
| `COPY_TIMEOUT_SECONDS` | Copy timeout (seconds) | `3600` |
| `COPY_MAX_RETRIES` | Copy retries | `3` |
| `COPY_RETRY_DELAY_SECONDS` | Retry delay (seconds) | `30` |

Host/port are CLI flags (`--host`, `--port`) and take precedence.

## Migrations

Migrations run automatically on startup. For manual runs:

```bash
cargo install sqlx-cli --features postgres
sqlx migrate run --database-url postgresql://release_mgmt:secret@localhost:5433/release_mgmt
```

## Development

```bash
cargo check
cargo build
cargo run
```

Logging via `RUST_LOG`:

```bash
RUST_LOG=info cargo run
RUST_LOG=simple_release_management=debug,sqlx=warn cargo run
```

## License

AGPLv3. See `LICENSE`.

