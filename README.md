# Simple Release Management

Aplikace pro správu a kopírování Docker images mezi registry s verzováním a release managementem.

## Funkce

- 🏢 **Multi-tenancy** - Podpora více tenantů
- 📦 **Bundle management** - Správa balíčků Docker images
- 🔄 **Verzování** - Automatické verzování změn
- 🚀 **Release management** - Vytváření production releases
- 🔍 **SHA tracking** - Sledování SHA256 checksumů pro immutability
- 🌐 **Podpora různých registry** - Harbor, Docker Registry v2, Quay, atd.
- 📋 **TOML export/import** - Export/import bundle definic
- 🔐 **Bezpečné credentials** - Integrace s Vault/Secrets

## Technologie

- **Rust** - Backend
- **Axum 0.8** - Web framework
- **PostgreSQL** - Databáze
- **SQLx** - Database driver
- **Skopeo** - Image copy (bez Docker daemon!)
- **Tokio** - Async runtime
- **Tracing** - Logging

## Rychlý start

### 1. Prerekvizity

- Rust 1.75+
- Docker & Docker Compose
- PostgreSQL 15+ (nebo použít Docker Compose)

### 2. Instalace

```bash
# Naklonovat repository
git clone <repo-url>
cd simple-release-management

# Zkopírovat environment config
cp .env.example .env

# Upravit .env podle potřeby
# nano .env
```

### 3. Spuštění databáze

```bash
# Spustit PostgreSQL přes Docker Compose
docker-compose up -d

# Zkontrolovat že běží
docker-compose ps
```

### 4. Spuštění aplikace

```bash
# Build a spuštění (výchozí: 127.0.0.1:3000)
cargo run

# S vlastním portem a hostem
cargo run -- --host 0.0.0.0 --port 8080

# Zobrazit help
cargo run -- --help

# Nebo jen kontrola kompilace
cargo check
```

**CLI parametry:**
- `--host <HOST>` - Server host (výchozí: `127.0.0.1`)
- `--port <PORT>` - Server port (výchozí: `3000`)
- `--help` - Zobrazit nápovědu

**Poznámka:** CLI parametry mají přednost před environment variables.

Aplikace poběží na `http://127.0.0.1:3000` (nebo na adrese kterou specifikuješ)

## Konfigurace

Všechny konfigurační parametry lze nastavit přes environment variables nebo `.env` soubor.

### Základní konfigurace

```bash
# Databáze
DATABASE_URL=postgresql://release_mgmt:secret@localhost:5433/release_mgmt

# Server
HOST=0.0.0.0
PORT=3000
BASE_PATH=

# Logging
RUST_LOG=simple_release_management=info,sqlx=warn
```

### Registry credentials

Credentials pro přístup k registry se ukládají jako JSON soubory vytvořené pomocí `skopeo login`:

```bash
# Přihlásit se k registry
skopeo login registry.datalite.cz

# Credentials se uloží do ~/.docker/config.json
# Nebo můžeš specifikovat vlastní cestu:
skopeo login --authfile /run/secrets/registry-auth.json registry.datalite.cz
```

V produkci mount tento soubor jako secret do podu:

```bash
REGISTRY_CREDENTIALS_PATH=/run/secrets/registry-auth
```

### Copy job konfigurace

```bash
# Maximum současně běžících copy operací
MAX_CONCURRENT_COPY_JOBS=3

# Timeout pro jednu copy operaci (sekundy)
COPY_TIMEOUT_SECONDS=3600

# Počet retry při selhání
COPY_MAX_RETRIES=3

# Delay mezi retry (sekundy)
COPY_RETRY_DELAY_SECONDS=30
```

## Vývoj

### Struktura projektu

```
simple-release-management/
├── .docs/               # Dokumentace
│   ├── AGENTS.md       # Instrukce pro AI agenty
│   ├── IDEA.mh         # Původní nápad a use case
│   └── IMPLEMENTATION.md  # Technická dokumentace
├── migrations/          # SQL migrace
├── src/
│   ├── main.rs         # Entry point
│   ├── config.rs       # Konfigurace
│   ├── db/             # Database modely
│   ├── registry/       # Registry abstraction
│   ├── services/       # Business logika (TODO)
│   ├── api/            # REST API endpoints (TODO)
│   └── web/            # Web UI (TODO)
├── .env                # Environment config (local)
├── .env.example        # Template pro .env
├── Cargo.toml          # Rust dependencies
└── docker-compose.yml  # PostgreSQL pro development
```

### Databázové migrace

Migrace se spouští automaticky při startu aplikace.

Pro manuální spuštění migrací:

```bash
# Nainstalovat sqlx-cli
cargo install sqlx-cli --features postgres

# Spustit migrace
sqlx migrate run --database-url postgresql://release_mgmt:secret@localhost:5433/release_mgmt

# Rollback poslední migrace
sqlx migrate revert --database-url postgresql://release_mgmt:secret@localhost:5433/release_mgmt
```

### Vytvoření nové migrace

```bash
sqlx migrate add create_my_table

# Otevře se nový soubor v migrations/
# Přidej SQL příkazy a commitni
```

### Kontrola a build

```bash
# Jen zkontrolovat kompilaci (rychlé)
cargo check

# Build v debug módu
cargo build

# Build v release módu (optimalizované)
cargo build --release

# Spustit
cargo run

# Spustit s release buildou
cargo run --release
```

### Logy

Nastavení úrovně logování přes `RUST_LOG`:

```bash
# Info pro celou aplikaci
RUST_LOG=info cargo run

# Debug pro specifický modul
RUST_LOG=simple_release_management=debug,sqlx=warn cargo run

# Trace level pro všechno
RUST_LOG=trace cargo run
```

## Deployment

### Docker

```bash
# Build image
docker build -t release-management:latest .

# Run
docker run -d \
  -p 3000:3000 \
  -e DATABASE_URL=postgresql://user:pass@db:5432/release_mgmt \
  -v /path/to/credentials:/run/secrets/registry-auth:ro \
  release-management:latest
```

### Kubernetes

Viz `.docs/IMPLEMENTATION.md` pro Kubernetes deployment manifesty.

## Stav implementace

### ✅ Hotovo

- [x] Databázový schema a migrace
- [x] Základní konfigurace
- [x] Registry abstraction layer (Harbor, Docker Registry v2)
- [x] Datové modely
- [x] Logging a tracing

### 🚧 TODO

- [ ] Skopeo wrapper service
- [ ] REST API endpoints
- [ ] Web UI s Tabler CSS
- [ ] SSE pro real-time progress
- [ ] Bundle CRUD operace
- [ ] Release management
- [ ] TOML export/import
- [ ] CLI interface

Viz `.docs/IMPLEMENTATION.md` sekce TODO pro kompletní seznam plánovaných funkcí.

## Dokumentace

- [IMPLEMENTATION.md](.docs/IMPLEMENTATION.md) - Detailní technická dokumentace
- [IDEA.mh](.docs/IDEA.mh) - Původní use case a požadavky
- [AGENTS.md](.docs/AGENTS.md) - Instrukce pro vývoj

## License

TODO

## Autor

TODO
