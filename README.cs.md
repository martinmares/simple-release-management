# Simple Release Management

Simple Release Management (SRM) je webová aplikace pro řízení release workflow nad tenanty, registry, prostředími a Git repozitáři s Kubernetes manifesty.

Typický release tok:

1. Definovat neměnné **Bundles** s mapováním container images.
2. Spustit **Copy Jobs**, které image zkopírují/otagují do cílových registries.
3. Z úspěšného copy jobu vytvořit auditovatelný **Image Release**.
4. Spustit **Manifest Build**, který přegeneruje deployment manifesty pro zvolené prostředí.
5. Volitelně z aplikace kontrolovat ArgoCD/Kubernetes stav.

## Základní pojmy

- **Tenant**: Logická hranice zákazníka/účtu.
- **Registry**: Zdrojový nebo cílový OCI/Docker registry včetně credentials a per-environment cest.
- **Environment**: Deploy prostředí v rámci tenanta. Obsahuje registry paths, cestu v environment Git repozitáři, cestu v deployment Git repozitáři a volitelný `encjson` key directory.
- **Bundle**: Pojmenovaná sada image mappings. Bundle lze archivovat/obnovit, pokud se už nemá používat pro novou práci.
- **Bundle Version**: Neměnný snapshot image mappings. Starší verze lze archivovat.
- **Copy Job**: Kopíruje images jedné bundle verze do cílového registry. Podporuje normální kopii, selective copy, validate režim, retry, cancel a audit logy.
- **Image Release**: Release manifest vytvořený z nakopírovaných images. Ukládá image references, tagy a digests pro pozdější generování manifestů.
- **Manifest Build**: Regeneruje Kubernetes deployment manifesty z image release pomocí `kube_build_app`, `encjson-rs`, `apply-env-rs` a `kubeconform`.
- **ArgoCD App**: Volitelná integrace pro zobrazení aplikací, sync, cleanup sync/prune workflow a Kubernetes live events.

## Funkce

- Multi-tenant model s tenant-scoped access.
- Konfigurace registries a environments včetně source/target project paths.
- Neměnné bundle verze a Archive/Restore workflow pro bundle.
- Copy jobs přes `skopeo` nebo `oci-patch`.
- `oci-patch` progress integrace pro live průběh kopírování.
- Automatické tagování ve formátu `YYYY.MM.DD.COUNTER`.
- Image release manifesty s digest-aware image references.
- Manifest builds s volitelným režimem image URL:
  - použít URL uložené v release manifestu,
  - přepsat URL podle registry vybraného prostředí.
- Dry-run manifest builds a perzistentní audit logy buildů.
- Kubeconform validace s ignorováním chybějících schémat pro custom/OpenShift resources.
- ArgoCD detail aplikace, sync, cleanup sync s preview a helper akce pro URL.
- Kubernetes instances/namespaces a live events.
- Server-Sent Events pro live job logy.
- Embedded frontend assets pro `cargo install --path=.` deploymenty, s možností `STATIC_DIR` override pro lokální frontend vývoj.
- Volitelná autorizace přes `AUTH_ENABLED` / `AUTH_REQUIRED` a CLI `--disable-auth` pro development/testing.

## Rychlý start

### Požadavky

- Rust toolchain kompatibilní s tímto crate (`edition = "2024"`).
- PostgreSQL.
- Jeden image copy backend:
  - `skopeo`, nebo
  - `oci-patch`.
- Volitelné helper nástroje pro manifest builds:
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

V `.env` je potřeba minimálně nastavit:

```bash
DATABASE_URL=postgresql://release_mgmt:secret@localhost:5433/release_mgmt
ENCRYPTION_SECRET=<silne-nahodne-tajemstvi>
```

Secret lze vygenerovat například takto:

```bash
openssl rand -base64 32
```

### Databáze

Pro lokální vývoj s přiloženým compose souborem:

```bash
docker compose up -d
```

Migrace jsou zabalené v aplikaci a spouští se automaticky při startu.

### Spuštění

```bash
cargo run
```

Vlastní host/port:

```bash
cargo run -- --host 0.0.0.0 --port 8282
```

Development režim bez autorizace:

```bash
cargo run -- --disable-auth
```

CLI help/version:

```bash
cargo run -- --help
cargo run -- --version
```

Aplikace defaultně poslouchá na `http://127.0.0.1:3000`.

## Produkční instalace

Jednoduchý instalační tok:

```bash
cargo install --path=.
```

Frontend assets jsou defaultně embedded přímo v Rust binárce. V produkci nenastavujte `STATIC_DIR`, pokud záměrně nechcete servírovat externí frontend soubory.

Typická user systemd služba používá environment file, například:

```ini
EnvironmentFile=%h/.config/simple-release-management/.env
ExecStart=%h/.cargo/bin/simple-release-management --host 0.0.0.0 --port 8282
```

## Konfigurace

Konfigurace se načítá z environment variables. Bind adresa serveru se nastavuje přes CLI `--host` a `--port`; `--disable-auth` vypne autorizaci pro development/testing.

Kompletní komentovaný příklad je v `.env.example`.

| Proměnná | Popis | Default |
|---|---|---|
| `DATABASE_URL` | PostgreSQL connection string | povinné |
| CLI `--host` | Bind host serveru | `127.0.0.1` |
| CLI `--port` | Port serveru | `3000` |
| `BASE_PATH` | Base path pro reverse proxy deployment | prázdné |
| `STATIC_DIR` | Volitelný override adresáře s frontend assets | embedded assets |
| `AUTH_ENABLED` | Zapnutí autorizační middleware | `true` |
| `AUTH_REQUIRED` | Zpětně kompatibilní autorizační flag | `true` |
| `ENCRYPTION_SECRET` | Secret pro šifrování uložených credentials | povinné |
| `IMAGE_TOOL` | Image backend: `skopeo` nebo `oci-patch` | `skopeo` |
| `IMAGE_TOOL_PATH` | Cesta k vybranému image tool binary | `skopeo` nebo `oci-patch` |
| `SKOPEO_PATH` | Legacy fallback, pokud není nastaveno `IMAGE_TOOL_PATH` | `skopeo` |
| `IMAGE_TOOL_SRC_INSECURE` | Vypnout TLS ověření pro source registry operace | `false` |
| `IMAGE_TOOL_DST_INSECURE` | Vypnout TLS ověření pro target registry operace | `false` |
| `IMAGE_TOOL_EXTRA_INSPECT_ARGS` | Extra shell-style argumenty pro image inspect | prázdné |
| `IMAGE_TOOL_EXTRA_COPY_ARGS` | Extra shell-style argumenty pro image copy | prázdné |
| `KUBE_BUILD_APP_PATH` | Cesta ke `kube_build_app` | `kube_build_app` |
| `APPLY_ENV_PATH` | Cesta k `apply-env-rs` / `apply-env` | `apply-env` |
| `ENCJSON_PATH` | Cesta k moderní `encjson-rs` binárce | `encjson` |
| `ENCJSON_LEGACY_PATH` | Cesta k legacy `encjson` binárce | `encjson` |
| `ENCJSON_KEYDIR` | Volitelný fallback key directory použitý jako `-k`, pokud není key dir nastaven v DB environmentu | nenastaveno |
| `KUBECONFORM_PATH` | Cesta ke `kubeconform` | `kubeconform` |
| `MAX_CONCURRENT_COPY_JOBS` | Limit paralelních image copy operací | `3` |
| `COPY_TIMEOUT_SECONDS` | Timeout jedné image copy operace | `3600` |
| `COPY_MAX_RETRIES` | Počet retry pokusů při copy | `3` |
| `COPY_RETRY_DELAY_SECONDS` | Pauza mezi retry pokusy | `30` |

Poznámky:

- Pro nové deploymenty preferujte `IMAGE_TOOL_*` proměnné.
- `SKOPEO_PATH` zůstává jen jako legacy fallback.
- Migrace starších deploymentů na aktuální image tool konfiguraci je popsána v `docs/ENV_MIGRATION.md`.
- `ENCJSON_KEYDIR` je pouze fallback. Hodnota `environment.encjson_key_dir` z DB má prioritu.

## Image Tool Backends

SRM může používat `skopeo` nebo `oci-patch`.

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

`oci-patch` je vhodnější, pokud potřebujete strukturovaný progress kopírování ve webovém UI.

## Manifest Builds

Manifest builds používají helper nástroje z environment variables a konfiguraci tenant environments.

Typický build flow:

1. Clone environment Git repozitáře.
2. Clone deployment Git repozitáře.
3. Spuštění `kube_build_app`.
4. Načtení inventory/profile dat, pokud jsou dostupná.
5. Decrypt/apply environment souborů přes `encjson-rs` a `apply-env-rs`.
6. Validace manifestů přes `kubeconform`.
7. Commit/tag/push vygenerovaných manifestů, pokud není zapnutý dry-run.

Režim image URL se vybírá ve formuláři buildu:

- **Use release manifest image URLs**: image URL zůstanou přesně tak, jak jsou uložené v image release manifestu.
- **Retarget images to selected environment registry**: digesty zůstanou zachované, ale registry/path se přepíše podle cílového prostředí.

## Archivace Bundle

Bundle jsou historické release definice a běžně by se neměly mazat.

Použijte **Archive**, pokud se bundle už nemá používat pro novou práci. Archivované bundle:

- zůstávají viditelné v historii,
- jsou skryté z defaultního active bundle listu,
- neumožní vytvořit novou verzi,
- neumožní spustit nový copy job,
- lze je později obnovit přes Restore.

Low-level endpoint `DELETE /bundles/{id}` zůstává kvůli kompatibilitě, ale UI používá Archive/Restore.

## Vývoj

Běžné příkazy:

```bash
cargo check
cargo build
cargo run
```

Kontrola syntaxe frontendu:

```bash
node --check src/web/static/js/app.js
node --check src/web/static/js/api.js
```

Příklady logování:

```bash
RUST_LOG=info cargo run
RUST_LOG=simple_release_management=debug,sqlx=warn,axum=info cargo run
```

Ruční spuštění migrací, pokud je potřeba:

```bash
cargo install sqlx-cli --features postgres
sqlx migrate run --database-url postgresql://release_mgmt:secret@localhost:5433/release_mgmt
```

## Licence

AGPLv3. Viz `LICENSE`.
