# Simple Release Management

Simple Release Management (SRM) is a web application for managing image release workflows across tenants, registries, environments, and Git-based Kubernetes manifest repositories.

It focuses on a practical release flow:

1. Define immutable **Bundles** of container image mappings.
2. Run **Copy Jobs** to copy/tag images into target registries.
3. Create auditable **Image Releases** from successful copy jobs.
4. Run **Manifest Builds** to regenerate deployment manifests for a selected environment.
5. Optionally inspect and operate ArgoCD/Kubernetes state from the same UI.

## Key Concepts

- **Tenant**: Logical customer/account boundary.
- **Registry**: Source or target OCI/Docker registry with credentials and per-environment paths.
- **Environment**: Tenant-specific deployment environment. It carries registry paths, environment Git path, deployment Git path, and optional `encjson` key directory.
- **Bundle**: Named collection of image mappings. Bundles can be archived/restored when they should no longer be used for new work.
- **Bundle Version**: Immutable snapshot of bundle image mappings. Older versions can be archived.
- **Copy Job**: Copies images for one bundle version into a target registry. Supports normal copy, selective copy, validation, retry, cancel, and audit logs.
- **Image Release**: A release manifest created from copied images. It stores image references, tags, and digests for later manifest generation.
- **Manifest Build**: Regenerates Kubernetes deployment manifests from an image release using `kube_build_app`, `encjson-rs`, `apply-env-rs`, and `kubeconform`.
- **ArgoCD App**: Optional integration for viewing apps, syncing, cleanup sync/prune workflows, and Kubernetes live events.

## Features

- Multi-tenant model with tenant-scoped access.
- Registry and environment configuration, including source/target project paths.
- Immutable bundle versions and bundle archive/restore workflow.
- Copy jobs powered by `skopeo` or `oci-patch`.
- `oci-patch` progress integration for live copy progress.
- Auto tag generation in the `YYYY.MM.DD.COUNTER` format.
- Image release manifests with digest-aware image references.
- Manifest builds with selectable image URL mode:
  - use URLs from the release manifest,
  - retarget image URLs to the selected environment registry.
- Dry-run manifest builds and persisted build audit logs.
- Kubeconform validation with ignored missing schemas for custom/OpenShift resources.
- ArgoCD app detail, sync, cleanup sync with preview, and URL helper actions.
- Kubernetes instance/namespace views and live events.
- Server-Sent Events for live job logs.
- Embedded frontend assets for `cargo install --path=.` deployments, with `STATIC_DIR` override for local frontend development.
- Optional authorization middleware with `AUTH_ENABLED` / `AUTH_REQUIRED` and CLI `--disable-auth` for development/testing.

## Quick Start

### Prerequisites

- Rust toolchain compatible with this crate (`edition = "2024"`).
- PostgreSQL.
- One image copy backend:
  - `skopeo`, or
  - `oci-patch`.
- Optional helper tools for manifest builds:
  - `kube_build_app`,
  - `encjson-rs`,
  - `apply-env-rs`,
  - `kubeconform`.

### Setup

```bash
git clone <repo-url>
cd simple-release-management
cp .env.example .env
```

Edit `.env` at minimum:

```bash
DATABASE_URL=postgresql://release_mgmt:secret@localhost:5433/release_mgmt
ENCRYPTION_SECRET=<strong-random-secret>
```

Generate a secret, for example:

```bash
openssl rand -base64 32
```

### Database

For local development with the bundled compose file:

```bash
docker compose up -d
```

Migrations are embedded and run automatically on application startup.

### Run

```bash
cargo run
```

Custom host/port:

```bash
cargo run -- --host 0.0.0.0 --port 8282
```

Development mode without authorization:

```bash
cargo run -- --disable-auth
```

CLI help/version:

```bash
cargo run -- --help
cargo run -- --version
```

The application listens on `http://127.0.0.1:3000` by default.

## Production Install

A simple binary install flow is:

```bash
cargo install --path=.
```

Frontend assets are embedded into the Rust binary by default. Do not set `STATIC_DIR` in production unless you intentionally want to serve external frontend files.

Typical user service configuration uses an environment file such as:

```ini
EnvironmentFile=%h/.config/simple-release-management/.env
ExecStart=%h/.cargo/bin/simple-release-management --host 0.0.0.0 --port 8282
```

## Configuration

Configuration is read from environment variables. Server bind address is controlled by CLI `--host` and `--port`; `--disable-auth` overrides authorization for development/testing.

See `.env.example` for a complete annotated example.

| Variable | Description | Default |
|---|---|---|
| `DATABASE_URL` | PostgreSQL connection string | required |
| CLI `--host` | Server bind host | `127.0.0.1` |
| CLI `--port` | Server port | `3000` |
| `BASE_PATH` | Base path for reverse proxy deployments | empty |
| `STATIC_DIR` | Optional frontend asset directory override | embedded assets |
| `AUTH_ENABLED` | Enable authorization middleware | `true` |
| `AUTH_REQUIRED` | Backward-compatible authorization flag | `true` |
| `ENCRYPTION_SECRET` | Secret used for encrypting stored credentials | required |
| `IMAGE_TOOL` | Image backend: `skopeo` or `oci-patch` | `skopeo` |
| `IMAGE_TOOL_PATH` | Path to selected image tool binary | `skopeo` or `oci-patch` |
| `SKOPEO_PATH` | Legacy fallback when `IMAGE_TOOL_PATH` is unset | `skopeo` |
| `IMAGE_TOOL_SRC_INSECURE` | Skip TLS verification for source registry operations | `false` |
| `IMAGE_TOOL_DST_INSECURE` | Skip TLS verification for target registry operations | `false` |
| `IMAGE_TOOL_EXTRA_INSPECT_ARGS` | Extra shell-style arguments for image inspect | empty |
| `IMAGE_TOOL_EXTRA_COPY_ARGS` | Extra shell-style arguments for image copy | empty |
| `KUBE_BUILD_APP_PATH` | Path to `kube_build_app` | `kube_build_app` |
| `APPLY_ENV_PATH` | Path to `apply-env-rs` / `apply-env` | `apply-env` |
| `ENCJSON_PATH` | Path to modern `encjson-rs` binary | `encjson` |
| `ENCJSON_LEGACY_PATH` | Path to legacy `encjson` binary | `encjson` |
| `ENCJSON_KEYDIR` | Optional fallback key directory passed as `-k` when DB environment key dir is unset | unset |
| `KUBECONFORM_PATH` | Path to `kubeconform` | `kubeconform` |
| `MAX_CONCURRENT_COPY_JOBS` | Parallel image copy limit | `3` |
| `COPY_TIMEOUT_SECONDS` | Timeout for a single image copy operation | `3600` |
| `COPY_MAX_RETRIES` | Copy retry count | `3` |
| `COPY_RETRY_DELAY_SECONDS` | Delay between copy retries | `30` |

Notes:

- Prefer the `IMAGE_TOOL_*` variables for new deployments.
- `SKOPEO_PATH` is retained only as a legacy fallback.
- See `docs/ENV_MIGRATION.md` for migrating older deployments to the current image tool configuration.
- `ENCJSON_KEYDIR` is only a fallback. A configured `environment.encjson_key_dir` from the database has priority.

## Image Tool Backends

SRM can use either `skopeo` or `oci-patch`.

### skopeo

```bash
IMAGE_TOOL=skopeo
IMAGE_TOOL_PATH=/usr/bin/skopeo
```

### oci-patch

```bash
IMAGE_TOOL=oci-patch
IMAGE_TOOL_PATH=/usr/local/bin/oci-patch
IMAGE_TOOL_SRC_INSECURE=false
IMAGE_TOOL_DST_INSECURE=false
```

`oci-patch` is preferred when you need structured copy progress in the web UI.

## Manifest Builds

Manifest builds use helper tools configured by environment variables and tenant environment records.

The build flow generally performs:

1. Clone environment Git repository.
2. Clone deployment Git repository.
3. Run `kube_build_app`.
4. Read inventory/profile data when available.
5. Decrypt/apply environment files using `encjson-rs` and `apply-env-rs`.
6. Validate manifests with `kubeconform`.
7. Commit/tag/push generated manifests unless dry-run is enabled.

Image URL behavior is selectable in the build form:

- **Use release manifest image URLs**: keep image URLs exactly as stored in the image release manifest.
- **Retarget images to selected environment registry**: keep digests but rewrite the registry/path from the target environment.

## Bundle Archiving

Bundles are historical release definitions and should normally not be deleted.

Use **Archive** when a bundle should no longer be used for new work. Archived bundles:

- remain visible in history,
- are hidden from the default active bundle list,
- cannot create new versions,
- cannot start new copy jobs,
- can be restored later.

The low-level `DELETE /bundles/{id}` endpoint still exists for compatibility, but the UI uses Archive/Restore.

## Development

Common commands:

```bash
cargo check
cargo build
cargo run
```

Frontend syntax checks:

```bash
node --check src/web/static/js/app.js
node --check src/web/static/js/api.js
```

Logging examples:

```bash
RUST_LOG=info cargo run
RUST_LOG=simple_release_management=debug,sqlx=warn,axum=info cargo run
```

Manual migration run, if needed:

```bash
cargo install sqlx-cli --features postgres
sqlx migrate run --database-url postgresql://release_mgmt:secret@localhost:5433/release_mgmt
```

## License

AGPLv3. See `LICENSE`.
