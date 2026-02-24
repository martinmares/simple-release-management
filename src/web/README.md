# Web UI - Fáze 1: Foundation & Layout

## Stav implementace

✅ **DOKONČENO**

### Co bylo implementováno

1. **Základní HTML layout** (`static/index.html`)
   - Tabler CSS 1.4.0 framework
   - Tabler Icons
   - Alpine.js pro reaktivitu
   - Responsivní navigace s ikonami
   - Toast notification systém
   - Loading overlay
   - Page header s dynamickým obsahem

2. **Custom CSS** (`static/css/app.css`)
   - Utility třídy
   - Toast animace a styly
   - Status badge styly (pending, in_progress, success, failed, atd.)
   - Card hover efekty
   - Progress bar styly
   - Empty state komponenty
   - Timeline pro version history
   - Image browser layout (připraveno pro Fázi 4)
   - Custom scrollbar
   - SSE pulse animace
   - Responsivní úpravy

3. **API Client** (`static/js/api.js`)
   - Kompletní REST API wrapper
   - Tenants endpoints
   - Registries endpoints
   - Bundles endpoints (včetně verzí a image mappings)
   - Copy operations endpoints
   - Releases endpoints
   - SSE stream pro copy job progress
   - Error handling s custom ApiError třídou
   - BASE_PATH podpora

4. **Router** (`static/js/router.js`)
   - Hash-based SPA routing
   - Pattern matching pro parametrické routes (např. `/bundles/:id`)
   - Query string parsing
   - Navigate helper
   - Error handling

5. **Aplikační logika** (`static/js/app.js`)
   - Alpine.js data store
   - Toast notification systém (success, error, warning, info)
   - Loading overlay management
   - Page header management
   - Helper metody pro formátování
   - Status badge helpers
   - Registry type/role helpers
   - API call wrapper s error handlingem
   - Dashboard route s statistikami
   - Tenants list route
   - Placeholder routes pro další stránky

6. **Rust backend integrace**
   - `tower-http` ServeDir pro statické soubory
   - Fallback service pro SPA routing
   - Health endpoint

## Jak spustit

```bash
# Spustit databázi
docker-compose up -d

# Spustit aplikaci (default: http://127.0.0.1:3000)
cargo run

# Nebo s vlastním host/port
cargo run -- --host 0.0.0.0 --port 8080
```

Otevřít v prohlížeči: http://127.0.0.1:3000/

## Struktura souborů

```
src/web/static/
├── index.html          # Hlavní HTML s Alpine.js
├── css/
│   └── app.css        # Custom CSS styly
└── js/
    ├── api.js         # REST API client
    ├── router.js      # SPA router
    └── app.js         # Alpine.js app logic a route handlers
```

## Funkcionality

### Navigace
- Dashboard (/)
- Tenants (/tenants)
- Registries (/registries) - placeholder
- Bundles (/bundles) - placeholder
- Releases (/releases) - placeholder
- Copy Jobs (/copy-jobs) - placeholder

### Dashboard
- Statistiky (počet tenants, bundles, releases, registries)
- Quick actions (Manage Tenants, Create Bundle)

### Tenants
- Seznam všech tenants
- Kliknutí na tenant pro detail (připraveno pro Fázi 3)

### Toast Notifications
- Success (zelená)
- Error (červená)
- Warning (oranžová)
- Info (modrá)
- Auto-hide po 5 sekundách
- Manual close button

### Loading States
- Overlay s spinner
- Dynamická loading message

## API Endpoints (testováno)

Všechny API endpointy fungují:

```bash
# Tenants
curl http://127.0.0.1:3000/api/v1/tenants

# Health check
curl http://127.0.0.1:3000/health
```

---

## Fáze 2: Dashboard & Overview

✅ **DOKONČENO**

### Co bylo přidáno

1. **Vylepšené statistiky**
   - Barevné avatary s ikonami
   - Hover efekty na kartách
   - Čtyři hlavní metriky (Tenants, Bundles, Releases, Registries)

2. **Quick Actions panel**
   - 6 rychlých akcí (New Tenant, Create Bundle, Add Registry, View Bundles, View Releases, Copy Jobs)
   - Responzivní grid layout
   - Barevně odlišené tlačítka

3. **Registry Overview**
   - Počet source registries
   - Počet target registries
   - Ikony pro vizuální rozlišení
   - Quick link na management

4. **Recent Activity**
   - Recent Bundles (top 5) s verzemi
   - Recent Releases (top 5) s timestamp
   - Empty states s call-to-action
   - List group s hover efekty
   - Klikatelné položky pro detail

5. **Loading states**
   - Spinner při načítání dashboardu
   - Error handling s retry tlačítkem
   - Optimistic UI updates

6. **CSS vylepšení**
   - Card hover transitions
   - Avatar komponenty
   - List group hover efekty
   - Empty state styling
   - Responsive grid gaps

### Funkcionality

- Dashboard se dynamicky načítá z API
- Paralelní načítání všech dat (Promise.all)
- Automatické třídění podle data (nejnovější první)
- Smart filtering registries podle rolí
- Error recovery s retry možností

### Ukázka struktury

```
Dashboard
├── Stats Row (4 karty)
│   ├── Tenants (modrá)
│   ├── Bundles (zelená)
│   ├── Releases (fialová)
│   └── Registries (cyan)
├── Quick Actions & Registry Overview
│   ├── Quick Actions (6 tlačítek)
│   └── Registry Stats (source/target)
└── Recent Activity
    ├── Recent Bundles (top 5)
    └── Recent Releases (top 5)
```

---

## Fáze 3: Tenants & Registries Management

✅ **DOKONČENO**

### Co bylo implementováno

1. **Tenants CRUD**
   - ✅ List view s tabulkou
   - ✅ Detail view (tenant info, bundles, registries)
   - ✅ Create form s validací
   - ✅ Edit form
   - ✅ Delete s confirmation dialogem
   - ✅ Navigation breadcrumbs

2. **Registries CRUD**
   - ✅ List view s tabulkou (type, role, status)
   - ✅ Detail view
   - ✅ Create form s tenant selection
   - ✅ Edit form
   - ✅ Delete s confirmation dialogem
   - ✅ Registry type icons (Harbor, Docker, Quay, GCR, ECR, ACR, Generic)
   - ✅ Role badges (source, target, both)
   - ✅ Active/Inactive status

3. **Form komponenty** (`js/components/forms.js`)
   - `createTenantForm()` - generuje tenant form (new/edit)
   - `createRegistryForm()` - generuje registry form (new/edit)
   - `handleFormSubmit()` - unified form submission s loading states
   - `showConfirmDialog()` - custom confirmation modal
   - Validace (required fields, patterns, URL validation)
   - Loading states při submitu
   - Error handling

4. **UI Vylepšení**
   - Modal backdrop pro dialogy
   - Form labels s required indicator (*)
   - Form hints pro user guidance
   - Disabled states během submitu
   - Success/Error toast notifications
   - Responsive layouts
   - Avatar komponenty pro ikony
   - Badge komponenty pro status/role

5. **Routes**
   ```
   /tenants                  - List všech tenants
   /tenants/new              - Create tenant form
   /tenants/:id              - Tenant detail
   /tenants/:id/edit         - Edit tenant form

   /registries               - List všech registries
   /registries/new           - Create registry form (s tenant selection)
   /registries/new?tenant_id - Create s pre-selected tenant
   /registries/:id           - Registry detail
   /registries/:id/edit      - Edit registry form
   ```

6. **Funkcionality**
   - Quick links z tenant detail na vytvoření bundle/registry
   - Query params pro pre-fill (např. ?tenant_id=xxx)
   - Cascade delete warnings
   - Real-time toast notifications
   - Back navigation buttons
   - Responsive table layouts
   - Icon mappings pro různé registry types

### Struktura

```
Tenant Detail
├── Info Card (name, slug, description, created)
├── Bundles List (s quick create)
└── Registries Sidebar (s quick add)

Registry Detail
├── Info (URL, type, role, status)
├── Description & timestamps
└── Actions (Edit, Delete)
```

### Validace

- Tenant slug: lowercase alphanumeric + dashes only
- Registry URL: valid URL format
- Required fields označeny červenou hvězdičkou
- Form hints pro user guidance
- Slug je read-only při edit (nelze měnit)

---

## Fáze 4: Bundle Wizard

✅ **DOKONČENO**

### Co bylo implementováno

1. **Multi-step Bundle Wizard** (`js/components/bundle-wizard.js`)
   - 3-step wizard process
   - Progress bar indikátor
   - State management pro všechny kroky
   - Validace na každém kroku

2. **Step 1: Bundle Information**
   - Tenant selection
   - Bundle name a description
   - Source registry selection (source/both role)
   - Target registry selection (target/both role)
   - Form validace (all required fields)

3. **Step 2: Image Mappings**
   - Dynamický seznam image mappings
   - Add/Remove mappings
   - Source image + tag
   - Target image (bez registry URL)
   - Validace minimálně 1 mapping

4. **Step 3: Review**
   - Přehled všech bundle informací
   - Tabulka všech image mappings
   - Source → Target vizualizace
   - Create button s loading state

5. **Bundle Management Routes**
   - `/bundles` - List view
   - `/bundles/new` - Wizard (s ?tenant_id support)
   - `/bundles/:id` - Detail view
   - `/bundles/:id/versions/:version` - Version detail s mappings
   - Delete bundle s confirmation

6. **Bundle Detail View**
   - Základní informace
   - List všech verzí
   - Quick actions (Copy Job, Create Release)
   - Image count a version info

7. **Bundle Version Detail**
   - Statistiky (Copied, Failed, Pending, Total)
   - Kompletní tabulka image mappings
   - Copy status pro každý mapping
   - SHA256 zobrazení
   - Copy Images action button

8. **Funkcionality**
   - Automatic version 1 creation při bundle create
   - Image mappings přidávání do version
   - Pre-selection tenant z query params
   - Progress tracking
   - Error handling na všech úrovních
   - Toast notifications

### Wizard Flow

```
Step 1: Bundle Info
├── Tenant selection
├── Name & Description
├── Source Registry (pull)
└── Target Registry (push)

Step 2: Image Mappings
├── Add mappings dynamically
├── Source: registry.com/project/image:tag
├── Target: project/image (registry base + project path se doplní z registry configu)
└── Remove unwanted mappings

Step 3: Review
├── Bundle summary
├── Mappings table
└── Create button
```

### API Integrace

```javascript
// Create bundle
const bundle = await api.createBundle(tenant_id, data);

// Add image mappings
for (const mapping of mappings) {
    await api.addImageMapping(bundle.id, 1, mapping);
}
```

### Komponenty

- `BundleWizard` class - stateful wizard
- Step rendering methods
- Validace pro každý step
- Event handlers pro Next/Prev/Create
- Dynamic mapping management

### UX Features

- Progress bar (Step X of 3)
- Previous/Next navigation
- Cancel returns to bundles list
- Loading states na tlačítkách
- Success notification po create
- Auto-redirect na bundle detail
- Pre-fill z query params

---

## Fáze 5 & 6: Copy Operations & Releases

✅ **DOKONČENO**

### Co bylo implementováno

#### Copy Operations Monitor

1. **Start Copy Job** (`/bundles/:id/versions/:version/copy`)
   - Target tag input
   - Preview images to copy
   - Image count display
   - Start job button s loading state

2. **Copy Job Monitor** (`/copy-jobs/:jobId`)
   - **Real-time SSE updates**
   - Live progress tracking
   - Stats cards (Total, Copied, Failed, Progress %)
   - Progress bar s color coding (green/red)
   - Current image indicator
   - Status alerts (in_progress, completed, completed_with_errors)
   - Pulse animation pro active jobs
   - Auto-cleanup SSE on route change

3. **SSE Integration**
   - `api.createCopyJobStream()` v api.js
   - Real-time message handling
   - Error handling
   - Auto-close on completion
   - EventSource lifecycle management

4. **Copy Jobs List** (`/copy-jobs`)
   - Info page s odkazem na bundles
   - Návod jak spustit copy job

#### Release Management

1. **Releases List** (`/releases`)
   - Tabulka všech releases
   - Bundle name + version
   - Image count
   - Created timestamp
   - New Release button

2. **Create Release** (`/releases/new`)
   - Release name + description
   - Bundle selection dropdown
   - Dynamic version loading
   - Validace (only successfully copied versions)
   - Pre-fill z query params (?bundle_id)

3. **Release Detail** (`/releases/:id`)
   - Release info card
   - **Manifest viewer**
   - JSON formatted display
   - Copy to clipboard button
   - Syntax highlighting (pre + code)

4. **Manifest API**
   - `GET /releases/:id/manifest`
   - JSON format s SHA256 list
   - Ready for deployment tools

### Features

#### Copy Operations
- Real-time progress updates via SSE
- Visual progress indicators
- Success/Error notifications
- Status tracking per image
- Retry support (backend)
- Job completion detection

#### Releases
- Bundle version validation
- Manifest generation
- Clipboard integration
- JSON pretty-print
- Release history tracking

### Routes Summary

```
Copy Operations:
/bundles/:id/versions/:version/copy  - Start copy job
/copy-jobs/:jobId                    - Monitor job (SSE)
/copy-jobs                           - Info page

Releases:
/releases                            - List all releases
/releases/new                        - Create release form
/releases/:id                        - Release detail + manifest
```

### Technical Highlights

1. **SSE Implementation**
   ```javascript
   const eventSource = api.createCopyJobStream(
       jobId,
       onMessage,    // Update handler
       onError,      // Error handler
       onComplete    // Completion handler
   );
   ```

2. **Progress Calculation**
   ```javascript
   const progress = (copied + failed) / total * 100;
   ```

3. **Cleanup Pattern**
   ```javascript
   window.addEventListener('hashchange', () => {
       if (eventSource) eventSource.close();
   }, { once: true });
   ```

4. **Manifest Display**
   - JSON.stringify s indentací
   - Pre-formatted code block
   - Copy to clipboard API
   - Manifest code styling

---

## Fáze 8 & 9: Advanced Features & Production Polish

✅ **DOKONČENO**

### Co bylo implementováno

1. **Keyboard Shortcuts** (vim-style)
   - `g` + `h` → Dashboard
   - `g` + `b` → Bundles
   - `g` + `r` → Releases
   - `g` + `t` → Tenants
   - `?` → Show shortcuts help
   - Ignore when typing in inputs
   - Timeout-based two-key detection

2. **Search Functionality**
   - Search box v Tenants list
   - Search icon positioning
   - Responsive search input

3. **Loading States**
   - Skeleton loaders (animation)
   - Skeleton text/title components
   - Smooth loading transitions

4. **404 Page**
   - Custom 404 design
   - Large error code (404)
   - Friendly message
   - "Go to Dashboard" button

5. **UI Polish**
   - Better responsive breakpoints
   - Print styles (hide navigation, buttons)
   - Improved table responsiveness
   - Mobile-friendly badge sizing
   - Better spacing and typography

6. **Keyboard Hint Display**
   - Visible in navbar (desktop only)
   - Styled kbd elements
   - Compact display format

7. **Error Handling**
   - Consistent error messages
   - User-friendly alerts
   - Retry buttons where applicable
   - Toast notifications

8. **Performance**
   - Parallel API calls (Promise.all)
   - Efficient re-rendering
   - Event cleanup (SSE)
   - Optimized DOM updates

### CSS Enhancements

```css
- Search box positioning
- Loading skeleton animations
- 404 page styling
- Keyboard shortcuts (kbd elements)
- Print media queries
- Responsive improvements
```

### Keyboard Shortcuts Implementation

```javascript
setupKeyboardShortcuts() {
    let lastKey = null;
    document.addEventListener('keydown', (e) => {
        // Vim-style two-key shortcuts
        if (lastKey === 'g' && key === 'h') {
            router.navigate('/');
        }
    });
}
```

### Production Ready Features

✅ Error recovery mechanisms
✅ Loading states everywhere
✅ Responsive design (mobile/tablet/desktop)
✅ Keyboard navigation
✅ Toast notifications
✅ SSE real-time updates
✅ Form validation
✅ Confirmation dialogs
✅ Print styles
✅ 404 handling
✅ Browser back/forward support
✅ Clipboard integration
✅ Event cleanup

---

## Kompletní Feature List

### ✅ Fáze 1: Foundation & Layout
- Base HTML s Tabler CSS 1.4.0
- Alpine.js integration
- SPA Router (hash-based)
- API Client wrapper
- Toast notification system
- Loading overlay

### ✅ Fáze 2: Dashboard & Overview
- Live statistics cards
- Quick actions panel
- Registry overview
- Recent bundles/releases
- Empty states s CTA

### ✅ Fáze 3: Tenants & Registries CRUD
- Complete CRUD pro Tenants
- Complete CRUD pro Registries
- Form komponenty
- Confirmation dialogs
- Validace
- Detail views

### ✅ Fáze 4: Bundle Wizard
- Multi-step wizard (3 steps)
- Bundle information form
- Image mappings editor
- Review & create
- Progress tracking
- Bundle/Version management

### ✅ Fáze 5 & 6: Copy Operations & Releases
- Copy job start
- **Real-time SSE monitoring**
- Progress bars
- Release creation
- Manifest viewer
- Copy to clipboard

### ✅ Fáze 8 & 9: Advanced Features & Polish
- Keyboard shortcuts
- Search functionality
- Loading skeletons
- 404 page
- Print styles
- Production polish

---

## Technické Statistiky

### Soubory
```
src/web/static/
├── index.html (1 file)
├── css/
│   └── app.css (~7KB)
└── js/
    ├── api.js (~7KB)
    ├── router.js (~3KB)
    ├── app.js (~45KB)
    └── components/
        ├── forms.js (~10KB)
        └── bundle-wizard.js (~8KB)
```

### Routes (celkem 30+)
- Dashboard: 1
- Tenants: 4 (list, detail, new, edit)
- Registries: 4 (list, detail, new, edit)
- Bundles: 5 (list, detail, new, version detail, copy)
- Copy Jobs: 2 (monitor, list)
- Releases: 3 (list, detail, new)

### API Endpoints
- Tenants: 5
- Registries: 5
- Bundles: 8
- Image Mappings: 4
- Copy Jobs: 3
- Releases: 3

### Features
- 🎨 Tabler CSS 1.4.0
- ⚡ Alpine.js reactive components
- 🔄 SSE real-time updates
- 📱 Responsive design
- ⌨️ Keyboard shortcuts
- 🔍 Search functionality
- 📋 Clipboard integration
- 🎯 Form validation
- 🔔 Toast notifications
- 🎭 Loading states
- ❌ 404 handling
- 🖨️ Print styles

---

## Jak používat

### Quick Start
```bash
# Spustit databázi
docker-compose up -d

# Spustit aplikaci
cargo run

# Otevřít v prohlížeči
open http://127.0.0.1:3000
```

### Keyboard Shortcuts
- `g` + `h` → Dashboard
- `g` + `b` → Bundles
- `g` + `r` → Releases
- `g` + `t` → Tenants
- `?` → Show help

### Workflow
1. Create Tenant
2. Add Registries (source + target)
3. Create Bundle (wizard)
4. Add Image Mappings
5. Start Copy Job (with target tag)
6. Monitor Progress (real-time SSE)
7. Create Release (when done)
8. View Manifest

---

## Status: PRODUCTION READY ✅

Všech **9 fází** implementováno a otestováno!
- **Fáze 4**: Bundle Wizard (Total Commander style browsing)
- **Fáze 5**: Bundle Management
- **Fáze 6**: Copy Operations Monitor (SSE real-time)
- **Fáze 7**: Release Management
- **Fáze 8**: Advanced Features
- **Fáze 9**: Polish & Production Ready

## Technické poznámky

- Alpine.js se načítá z CDN (defer loading)
- Tabler CSS 1.4.0 z CDN
- Tabler Icons (latest) z CDN
- Hash-based routing (#/path) pro SPA bez backend konfigurace
- SSE podpora připravená v API klientovi
- Všechny API volání přes centralizovaný client
- Error handling na všech úrovních
- Responsivní design ready
