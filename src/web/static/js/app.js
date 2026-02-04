/**
 * Hlavní aplikační logika s Alpine.js
 */

// Global helper pro přístup k app komponentě
window.getApp = function() {
    const appElement = document.querySelector('[x-data="app"]');
    return appElement ? Alpine.$data(appElement) : null;
};

document.addEventListener('alpine:init', () => {
    Alpine.data('app', () => ({
        // State
        currentRoute: '/',
        loading: false,
        loadingMessage: '',
        toasts: [],

        // Page header state
        pageHeader: {
            show: true,
            pretitle: '',
            title: '',
            actions: '',
        },

        // Inicializace
        init() {
            console.log('Simple Release Management initialized');

            // Watch for route changes
            this.$watch('currentRoute', (value) => {
                console.log('Route changed to:', value);
            });

            // Keyboard shortcuts
            this.setupKeyboardShortcuts();
        },

        // Keyboard shortcuts
        setupKeyboardShortcuts() {
            let lastKey = null;
            let timeout = null;

            document.addEventListener('keydown', (e) => {
                // Ignore if typing in input/textarea
                if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') {
                    return;
                }

                const key = e.key.toLowerCase();

                // Single key shortcuts
                if (key === '?' && !lastKey) {
                    this.showInfo('Keyboard shortcuts: gh=Dashboard, gb=Bundles, gr=Releases, gt=Tenants');
                    return;
                }

                // Two-key shortcuts (vim-style)
                if (lastKey === 'g') {
                    switch (key) {
                        case 'h':
                            router.navigate('/');
                            break;
                        case 'b':
                            router.navigate('/bundles');
                            break;
                        case 'r':
                            router.navigate('/releases');
                            break;
                        case 't':
                            router.navigate('/tenants');
                            break;
                    }
                    lastKey = null;
                    clearTimeout(timeout);
                } else if (key === 'g') {
                    lastKey = 'g';
                    timeout = setTimeout(() => {
                        lastKey = null;
                    }, 1000);
                }
            });
        },

        // ==================== LOADING ====================

        showLoading(message = 'Loading...') {
            this.loading = true;
            this.loadingMessage = message;
        },

        hideLoading() {
            this.loading = false;
            this.loadingMessage = '';
        },

        // ==================== TOASTS ====================

        showToast(type, title, message, duration = 5000) {
            const id = Date.now() + Math.random();
            const toast = { id, type, title, message };

            this.toasts.push(toast);

            // Auto-remove po duration
            if (duration > 0) {
                setTimeout(() => {
                    this.removeToast(id);
                }, duration);
            }

            return id;
        },

        removeToast(id) {
            this.toasts = this.toasts.filter(t => t.id !== id);
        },

        showSuccess(message, title = 'Success') {
            this.showToast('success', title, message);
        },

        showError(message, title = 'Error') {
            this.showToast('danger', title, message);
        },

        showWarning(message, title = 'Warning') {
            this.showToast('warning', title, message);
        },

        showInfo(message, title = 'Info') {
            this.showToast('info', title, message);
        },

        // ==================== PAGE HEADER ====================

        setPageHeader(title, pretitle = '', actions = '') {
            this.pageHeader.show = true;
            this.pageHeader.title = title;
            this.pageHeader.pretitle = pretitle;
            this.pageHeader.actions = actions;
        },

        hidePageHeader() {
            this.pageHeader.show = false;
        },

        // ==================== HELPER METHODS ====================

        formatDate(dateString) {
            if (!dateString) return '-';
            const date = new Date(dateString);
            return date.toLocaleDateString('cs-CZ', {
                year: 'numeric',
                month: '2-digit',
                day: '2-digit',
                hour: '2-digit',
                minute: '2-digit',
            });
        },

        getStatusBadgeClass(status) {
            const statusMap = {
                'pending': 'badge-pending',
                'in_progress': 'badge-in-progress',
                'success': 'badge-success',
                'failed': 'badge-failed',
                'completed': 'badge-completed',
                'completed_with_errors': 'badge-completed-with-errors',
                'starting': 'badge-info',
            };
            return statusMap[status] || 'badge-secondary';
        },

        getRegistryTypeIcon(type) {
            const iconMap = {
                'harbor': 'ti-anchor',
                'docker': 'ti-brand-docker',
                'quay': 'ti-box',
                'gcr': 'ti-brand-google',
                'ecr': 'ti-brand-aws',
                'acr': 'ti-brand-azure',
                'generic': 'ti-database',
            };
            return iconMap[type] || 'ti-database';
        },

        getRegistryRoleBadge(role) {
            const badgeMap = {
                'source': 'bg-blue text-blue-fg',
                'target': 'bg-green text-green-fg',
                'both': 'bg-purple text-purple-fg',
            };
            return badgeMap[role] || 'bg-secondary text-secondary-fg';
        },

        // ==================== API HELPERS ====================

        async handleApiCall(apiCall, successMessage = null) {
            try {
                this.showLoading();
                const result = await apiCall();
                this.hideLoading();

                if (successMessage) {
                    this.showSuccess(successMessage);
                }

                return result;
            } catch (error) {
                this.hideLoading();
                console.error('API call failed:', error);

                if (error instanceof ApiError) {
                    this.showError(error.message);
                } else {
                    this.showError('An unexpected error occurred');
                }

                throw error;
            }
        },
    }));
});

// ==================== ROUTE HANDLERS ====================

// Dashboard
router.on('/', async () => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border text-blue"></div><p class="mt-3 text-secondary">Loading dashboard...</p></div>';

    try {
        // Načíst všechna data paralelně
        const [tenants, bundles, releases, registries] = await Promise.all([
            api.getTenants(),
            api.getBundles(),
            api.getReleases(),
            api.getRegistries(),
        ]);

        // Spočítat registry podle rolí
        const sourceRegistries = registries.filter(r => r.role === 'source' || r.role === 'both').length;
        const targetRegistries = registries.filter(r => r.role === 'target' || r.role === 'both').length;

        // Získat poslední releases (top 5)
        const recentReleases = releases
            .sort((a, b) => new Date(b.created_at) - new Date(a.created_at))
            .slice(0, 5);

        // Získat poslední bundles (top 5) s detaily
        const recentBundles = bundles
            .sort((a, b) => new Date(b.created_at) - new Date(a.created_at))
            .slice(0, 5);

        content.innerHTML = `
            <!-- Stats Row -->
            <div class="row row-deck row-cards mb-4">
                <div class="col-sm-6 col-lg-3">
                    <a href="#/tenants" class="card card-sm card-link">
                        <div class="card-body">
                            <div class="row align-items-center">
                                <div class="col-auto">
                                    <span class="bg-blue text-white avatar">
                                        <i class="ti ti-building"></i>
                                    </span>
                                </div>
                                <div class="col">
                                    <div class="font-weight-medium">${tenants.length}</div>
                                    <div class="text-secondary">Tenants</div>
                                </div>
                            </div>
                        </div>
                    </a>
                </div>

                <div class="col-sm-6 col-lg-3">
                    <a href="#/bundles" class="card card-sm card-link">
                        <div class="card-body">
                            <div class="row align-items-center">
                                <div class="col-auto">
                                    <span class="bg-green text-white avatar">
                                        <i class="ti ti-package"></i>
                                    </span>
                                </div>
                                <div class="col">
                                    <div class="font-weight-medium">${bundles.length}</div>
                                    <div class="text-secondary">Bundles</div>
                                </div>
                            </div>
                        </div>
                    </a>
                </div>

                <div class="col-sm-6 col-lg-3">
                    <a href="#/releases" class="card card-sm card-link">
                        <div class="card-body">
                            <div class="row align-items-center">
                                <div class="col-auto">
                                    <span class="bg-purple text-white avatar">
                                        <i class="ti ti-rocket"></i>
                                    </span>
                                </div>
                                <div class="col">
                                    <div class="font-weight-medium">${releases.length}</div>
                                    <div class="text-secondary">Releases</div>
                                </div>
                            </div>
                        </div>
                    </a>
                </div>

                <div class="col-sm-6 col-lg-3">
                    <a href="#/registries" class="card card-sm card-link">
                        <div class="card-body">
                            <div class="row align-items-center">
                                <div class="col-auto">
                                    <span class="bg-cyan text-white avatar">
                                        <i class="ti ti-database"></i>
                                    </span>
                                </div>
                                <div class="col">
                                    <div class="font-weight-medium">${registries.length}</div>
                                    <div class="text-secondary">Registries</div>
                                </div>
                            </div>
                        </div>
                    </a>
                </div>
            </div>

            <!-- Quick Actions & Registry Stats -->
            <div class="row mb-4">
                <div class="col-md-8">
                    <div class="card">
                        <div class="card-header">
                            <h3 class="card-title">Quick Actions</h3>
                        </div>
                        <div class="card-body">
                            <div class="row g-3">
                                <div class="col-6 col-md-4">
                                    <a href="#/tenants/new" class="btn btn-primary w-100">
                                        <i class="ti ti-plus me-2"></i>
                                        New Tenant
                                    </a>
                                </div>
                                <div class="col-6 col-md-4">
                                    <a href="#/bundles/new" class="btn btn-success w-100">
                                        <i class="ti ti-package me-2"></i>
                                        Create Bundle
                                    </a>
                                </div>
                                <div class="col-6 col-md-4">
                                    <a href="#/registries/new" class="btn btn-cyan w-100">
                                        <i class="ti ti-database me-2"></i>
                                        Add Registry
                                    </a>
                                </div>
                                <div class="col-6 col-md-4">
                                    <a href="#/bundles" class="btn btn-outline-primary w-100">
                                        <i class="ti ti-list me-2"></i>
                                        View Bundles
                                    </a>
                                </div>
                                <div class="col-6 col-md-4">
                                    <a href="#/releases" class="btn btn-outline-success w-100">
                                        <i class="ti ti-rocket me-2"></i>
                                        View Releases
                                    </a>
                                </div>
                                <div class="col-6 col-md-4">
                                    <a href="#/copy-jobs" class="btn btn-outline-cyan w-100">
                                        <i class="ti ti-copy me-2"></i>
                                        Copy Jobs
                                    </a>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>

                <div class="col-md-4">
                    <div class="card">
                        <div class="card-header">
                            <h3 class="card-title">Registry Overview</h3>
                        </div>
                        <div class="card-body">
                            <div class="mb-3">
                                <div class="row align-items-center">
                                    <div class="col-auto">
                                        <i class="ti ti-arrow-down text-blue"></i>
                                    </div>
                                    <div class="col">
                                        <div class="text-secondary">Source Registries</div>
                                        <div class="h3 mb-0">${sourceRegistries}</div>
                                    </div>
                                </div>
                            </div>
                            <div class="mb-3">
                                <div class="row align-items-center">
                                    <div class="col-auto">
                                        <i class="ti ti-arrow-up text-green"></i>
                                    </div>
                                    <div class="col">
                                        <div class="text-secondary">Target Registries</div>
                                        <div class="h3 mb-0">${targetRegistries}</div>
                                    </div>
                                </div>
                            </div>
                            <div>
                                <a href="#/registries" class="btn btn-outline-primary w-100">
                                    Manage Registries
                                </a>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <!-- Recent Activity -->
            <div class="row">
                <div class="col-md-6">
                    <div class="card">
                        <div class="card-header">
                            <h3 class="card-title">Recent Bundles</h3>
                            <div class="card-actions">
                                <a href="#/bundles" class="btn btn-sm btn-outline-primary">View All</a>
                            </div>
                        </div>
                        <div class="card-body p-0">
                            ${recentBundles.length === 0 ? `
                                <div class="empty p-4">
                                    <p class="empty-title">No bundles yet</p>
                                    <p class="empty-subtitle text-secondary">Create your first bundle to get started</p>
                                    <div class="empty-action">
                                        <a href="#/bundles/new" class="btn btn-primary">
                                            <i class="ti ti-plus"></i>
                                            Create Bundle
                                        </a>
                                    </div>
                                </div>
                            ` : `
                                <div class="list-group list-group-flush">
                                    ${recentBundles.map(bundle => `
                                        <a href="#/bundles/${bundle.id}" class="list-group-item list-group-item-action">
                                            <div class="row align-items-center">
                                                <div class="col-auto">
                                                    <span class="avatar bg-blue-lt">
                                                        <i class="ti ti-package"></i>
                                                    </span>
                                                </div>
                                                <div class="col text-truncate">
                                                    <div class="text-reset d-block">${bundle.name}</div>
                                                    <div class="text-secondary text-truncate mt-n1">
                                                        ${bundle.description || 'No description'}
                                                    </div>
                                                </div>
                                                <div class="col-auto">
                                                    <div class="badge bg-blue text-blue-fg">${bundle.current_version || 'v1'}</div>
                                                </div>
                                            </div>
                                        </a>
                                    `).join('')}
                                </div>
                            `}
                        </div>
                    </div>
                </div>

                <div class="col-md-6">
                    <div class="card">
                        <div class="card-header">
                            <h3 class="card-title">Recent Releases</h3>
                            <div class="card-actions">
                                <a href="#/releases" class="btn btn-sm btn-outline-primary">View All</a>
                            </div>
                        </div>
                        <div class="card-body p-0">
                            ${recentReleases.length === 0 ? `
                                <div class="empty p-4">
                                    <p class="empty-title">No releases yet</p>
                                    <p class="empty-subtitle text-secondary">Create a bundle and release it</p>
                                </div>
                            ` : `
                                <div class="list-group list-group-flush">
                                    ${recentReleases.map(release => `
                                        <a href="#/releases/${release.id}" class="list-group-item list-group-item-action">
                                            <div class="row align-items-center">
                                                <div class="col-auto">
                                                    <span class="avatar bg-purple-lt">
                                                        <i class="ti ti-rocket"></i>
                                                    </span>
                                                </div>
                                                <div class="col text-truncate">
                                                    <div class="text-reset d-block">${release.release_id}</div>
                                                    <div class="text-secondary text-truncate mt-n1">
                                                        <small>${new Date(release.created_at).toLocaleString('cs-CZ')}</small>
                                                    </div>
                                                </div>
                                                <div class="col-auto">
                                                    <span class="badge bg-success text-success-fg">Released</span>
                                                </div>
                                            </div>
                                        </a>
                                    `).join('')}
                                </div>
                            `}
                        </div>
                    </div>
                </div>
            </div>
        `;

    } catch (error) {
        console.error('Failed to load dashboard:', error);
        content.innerHTML = `
            <div class="empty">
                <div class="empty-icon">
                    <i class="ti ti-alert-circle text-danger"></i>
                </div>
                <p class="empty-title">Failed to load dashboard</p>
                <p class="empty-subtitle text-secondary">${error.message}</p>
                <div class="empty-action">
                    <button class="btn btn-primary" onclick="router.handleRoute()">
                        <i class="ti ti-reload"></i>
                        Retry
                    </button>
                </div>
            </div>
        `;
    }
});

// ==================== TENANTS ROUTES ====================

// Tenants List
router.on('/tenants', async () => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const tenants = await api.getTenants();

        const renderTenants = (rows, searchQuery = '') => `
            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">Tenants</h3>
                    <div class="card-actions">
                        <a href="#/tenants/new" class="btn btn-primary">
                            <i class="ti ti-plus"></i>
                            New Tenant
                        </a>
                    </div>
                </div>
                <div class="card-body border-bottom py-3">
                    <div class="row g-2">
                        <div class="col-md-4">
                            <div class="input-group">
                                <span class="input-group-text">
                                    <i class="ti ti-search"></i>
                                </span>
                                <input type="text" class="form-control" placeholder="Search by name or slug..."
                                       id="tenants-search" value="${searchQuery}">
                            </div>
                        </div>
                    </div>
                </div>
                <div class="table-responsive">
                    <table class="table table-vcenter card-table table-hover">
                        <thead>
                            <tr>
                                <th>Name</th>
                                <th>Slug</th>
                                <th>Description</th>
                                <th>Created</th>
                            </tr>
                        </thead>
                        <tbody>
                            ${rows.length === 0 ? `
                                <tr>
                                    <td colspan="4" class="text-center text-secondary py-5">
                                        No tenants found. Create your first tenant to get started.
                                    </td>
                                </tr>
                            ` : rows.map(tenant => `
                                <tr>
                                    <td><a href="#/tenants/${tenant.id}"><strong>${tenant.name}</strong></a></td>
                                    <td><span class="badge">${tenant.slug}</span></td>
                                    <td>${tenant.description || '-'}</td>
                                    <td>${new Date(tenant.created_at).toLocaleDateString('cs-CZ')}</td>
                                </tr>
                            `).join('')}
                        </tbody>
                    </table>
                </div>
            </div>
        `;

        content.innerHTML = renderTenants(tenants);
        const searchEl = document.getElementById('tenants-search');

        const applyFilters = () => {
            const q = searchEl.value.trim().toLowerCase();
            const filtered = tenants.filter(t =>
                !q || t.name.toLowerCase().includes(q) || t.slug.toLowerCase().includes(q)
            );
            content.innerHTML = renderTenants(filtered, q);
            document.getElementById('tenants-search').value = q;
            document.getElementById('tenants-search').addEventListener('input', applyFilters);
        };

        searchEl.addEventListener('input', applyFilters);
    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                <i class="ti ti-alert-circle"></i>
                Failed to load tenants: ${error.message}
            </div>
        `;
    }
});

// Tenant Detail
router.on('/tenants/:id', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const [tenant, registries, bundles] = await Promise.all([
            api.getTenant(params.id),
            api.getRegistries(params.id),
            api.getBundles(params.id),
        ]);

        content.innerHTML = `
            <div class="row mb-3">
                <div class="col">
                    <a href="#/tenants" class="btn btn-ghost-secondary">
                        <i class="ti ti-arrow-left"></i>
                        Back to Tenants
                    </a>
                </div>
            </div>

            <div class="row">
                <div class="col-md-8">
                    <div class="card mb-3">
                        <div class="card-header">
                            <h3 class="card-title">${tenant.name}</h3>
                            <div class="card-actions">
                                <a href="#/tenants/${tenant.id}/edit" class="btn btn-primary btn-sm">
                                    <i class="ti ti-pencil"></i>
                                    Edit
                                </a>
                                <button class="btn btn-danger btn-sm" id="delete-tenant-btn">
                                    <i class="ti ti-trash"></i>
                                    Delete
                                </button>
                            </div>
                        </div>
                        <div class="card-body">
                            <div class="row mb-3">
                                <div class="col-5 text-secondary">Slug:</div>
                                <div class="col-7"><code>${tenant.slug}</code></div>
                            </div>
                            <div class="row mb-3">
                                <div class="col-5 text-secondary">Description:</div>
                                <div class="col-7">${tenant.description || '-'}</div>
                            </div>
                            <div class="row">
                                <div class="col-5 text-secondary">Created:</div>
                                <div class="col-7">${new Date(tenant.created_at).toLocaleString('cs-CZ')}</div>
                            </div>
                        </div>
                    </div>

                    <div class="card mb-3">
                        <div class="card-header">
                            <h3 class="card-title">Bundles</h3>
                            <div class="card-actions">
                                <a href="#/bundles/new?tenant_id=${tenant.id}" class="btn btn-primary btn-sm">
                                    <i class="ti ti-plus"></i>
                                    New Bundle
                                </a>
                            </div>
                        </div>
                        <div class="list-group list-group-flush">
                            ${bundles.length === 0 ? `
                                <div class="list-group-item text-center text-secondary py-4">
                                    No bundles yet
                                </div>
                            ` : bundles.map(bundle => `
                                <a href="#/bundles/${bundle.id}" class="list-group-item list-group-item-action">
                                    <div class="row align-items-center">
                                        <div class="col">
                                            <strong>${bundle.name}</strong>
                                            <div class="text-secondary small">${bundle.description || ''}</div>
                                        </div>
                                        <div class="col-auto">
                                            <span class="badge bg-blue text-blue-fg">${bundle.current_version || 'v1'}</span>
                                        </div>
                                    </div>
                                </a>
                            `).join('')}
                        </div>
                    </div>
                </div>

                <div class="col-md-4">
                    <div class="card">
                        <div class="card-header">
                            <h3 class="card-title">Registries</h3>
                            <div class="card-actions">
                                <a href="#/registries/new?tenant_id=${tenant.id}" class="btn btn-primary btn-sm">
                                    <i class="ti ti-plus"></i>
                                    Add
                                </a>
                            </div>
                        </div>
                        <div class="list-group list-group-flush">
                            ${registries.length === 0 ? `
                                <div class="list-group-item text-center text-secondary py-4">
                                    No registries yet
                                </div>
                            ` : registries.map(reg => `
                                <a href="#/registries/${reg.id}" class="list-group-item list-group-item-action">
                                    <div class="d-flex align-items-center">
                                        <span class="avatar avatar-sm me-2">
                                            <i class="ti ${window.Alpine?.$data?.app?.getRegistryTypeIcon(reg.registry_type) || 'ti-database'}"></i>
                                        </span>
                                        <div class="flex-fill">
                                            <div>${reg.name}</div>
                                            <div class="text-secondary small">${reg.registry_type}</div>
                                        </div>
                                        <span class="badge ${window.Alpine?.$data?.app?.getRegistryRoleBadge(reg.role) || 'bg-secondary text-secondary-fg'}">${reg.role}</span>
                                    </div>
                                </a>
                            `).join('')}
                        </div>
                    </div>
                </div>
            </div>
        `;

        // Delete handler
        document.getElementById('delete-tenant-btn').addEventListener('click', async () => {
            const confirmed = await showConfirmDialog(
                'Delete Tenant?',
                `Are you sure you want to delete "${tenant.name}"? This will also delete all associated registries and bundles.`,
                'Delete',
                'Cancel'
            );

            if (confirmed) {
                try {
                    await api.deleteTenant(tenant.id);
                    getApp().showSuccess('Tenant deleted successfully');
                    router.navigate('/tenants');
                } catch (error) {
                    getApp().showError(error.message);
                }
            }
        });

    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                <i class="ti ti-alert-circle"></i>
                Failed to load tenant: ${error.message}
            </div>
        `;
    }
});

// Tenant New/Edit
router.on('/tenants/new', async () => {
    const content = document.getElementById('app-content');
    content.innerHTML = createTenantForm();

    // Setup auto-slug generation
    setupTenantSlugGeneration();

    document.getElementById('tenant-form').addEventListener('submit', async (e) => {
        await handleFormSubmit(e, async (data) => {
            await api.createTenant(data);
            getApp().showSuccess('Tenant created successfully');
            router.navigate('/tenants');
        });
    });
});

router.on('/tenants/:id/edit', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const tenant = await api.getTenant(params.id);
        content.innerHTML = createTenantForm(tenant);

        document.getElementById('tenant-form').addEventListener('submit', async (e) => {
            await handleFormSubmit(e, async (data) => {
                await api.updateTenant(params.id, data);
                getApp().showSuccess('Tenant updated successfully');
                router.navigate(`/tenants/${params.id}`);
            });
        });
    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load tenant: ${error.message}
            </div>
        `;
    }
});

// ==================== REGISTRIES ROUTES ====================

// Registries List
router.on('/registries', async () => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const [registries, tenants] = await Promise.all([
            api.getRegistries(),
            api.getTenants()
        ]);

        // Create tenant lookup map
        const tenantMap = {};
        tenants.forEach(t => tenantMap[t.id] = t);

        // Store data globally for Alpine to pick up
        window._registryListData = { registries, tenantMap };

        content.innerHTML = `
            <div class="card" x-data="registryList()">
                <div class="card-header">
                    <h3 class="card-title">Registries</h3>
                    <div class="card-actions">
                        <a href="#/registries/new" class="btn btn-primary">
                            <i class="ti ti-plus"></i>
                            New Registry
                        </a>
                    </div>
                </div>

                <!-- Filters -->
                <div class="card-body border-bottom py-3">
                    <div class="row g-2">
                        <div class="col-md-4">
                            <div class="input-group">
                                <span class="input-group-text">
                                    <i class="ti ti-search"></i>
                                </span>
                                <input type="text" class="form-control" placeholder="Search by name..."
                                       x-model="searchQuery">
                            </div>
                        </div>
                        <div class="col-md-3">
                            <select class="form-select" x-model="selectedTenant">
                                <option value="">All Tenants</option>
                                ${tenants.map(t => `
                                    <option value="${t.id}">${t.name}</option>
                                `).join('')}
                            </select>
                        </div>
                        <div class="col-md-auto ms-auto">
                            <span class="text-secondary" x-text="filteredRegistries.length + ' registries'"></span>
                        </div>
                    </div>
                </div>

                <div class="table-responsive">
                    <table class="table table-vcenter card-table">
                        <thead>
                            <tr>
                                <th>Name</th>
                                <th>Tenant</th>
                                <th>Type</th>
                                <th>Base URL</th>
                                <th>Username</th>
                                <th>Role</th>
                                <th>Status</th>
                            </tr>
                        </thead>
                        <tbody>
                            <template x-if="filteredRegistries.length === 0">
                                <tr>
                                    <td colspan="7" class="text-center text-secondary py-5">
                                        <div>
                                            <i class="ti ti-database-off" style="font-size: 3rem; opacity: 0.3;"></i>
                                            <div class="mt-2">No registries found</div>
                                        </div>
                                    </td>
                                </tr>
                            </template>
                            <template x-for="reg in filteredRegistries" :key="reg.id">
                                <tr>
                                    <td>
                                        <div class="d-flex align-items-center">
                                            <span class="avatar avatar-sm me-2">
                                                <i class="ti" :class="getRegistryTypeIcon(reg.registry_type)"></i>
                                            </span>
                                            <a :href="'#/registries/' + reg.id"><strong x-text="reg.name"></strong></a>
                                        </div>
                                    </td>
                                    <td>
                                        <span class="badge bg-blue text-blue-fg" x-text="tenantMap[reg.tenant_id]?.name || 'Unknown'"></span>
                                    </td>
                                    <td>
                                        <span class="badge bg-azure text-azure-fg" x-text="reg.registry_type"></span>
                                    </td>
                                    <td>
                                        <code class="small" x-text="reg.base_url"></code>
                                    </td>
                                    <td>
                                        <span class="text-secondary" x-text="reg.username || '-'"></span>
                                    </td>
                                    <td>
                                        <span class="badge" :class="getRegistryRoleBadge(reg.role)" x-text="reg.role"></span>
                                    </td>
                                    <td>
                                        <span class="badge" :class="reg.is_active ? 'bg-success text-success-fg' : 'bg-secondary text-secondary-fg'" x-text="reg.is_active ? 'Active' : 'Inactive'"></span>
                                    </td>
                                </tr>
                            </template>
                        </tbody>
                    </table>
                </div>
            </div>
        `;
    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                <i class="ti ti-alert-circle"></i>
                Failed to load registries: ${error.message}
            </div>
        `;
    }
});

// Registry Detail
router.on('/registries/:id', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const registry = await api.getRegistry(params.id);

        content.innerHTML = `
            <div class="row mb-3">
                <div class="col">
                    <a href="#/registries" class="btn btn-ghost-secondary">
                        <i class="ti ti-arrow-left"></i>
                        Back to Registries
                    </a>
                </div>
            </div>

            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">
                        <span class="avatar avatar-sm me-2">
                            <i class="ti ${window.Alpine?.$data?.app?.getRegistryTypeIcon(registry.registry_type) || 'ti-database'}"></i>
                        </span>
                        ${registry.name}
                    </h3>
                    <div class="card-actions">
                        <a href="#/registries/${registry.id}/edit" class="btn btn-primary btn-sm">
                            <i class="ti ti-pencil"></i>
                            Edit
                        </a>
                        <button class="btn btn-danger btn-sm" id="delete-registry-btn">
                            <i class="ti ti-trash"></i>
                            Delete
                        </button>
                    </div>
                </div>
                <div class="card-body">
                    <div class="row">
                        <div class="col-md-6">
                            <div class="mb-3">
                                <div class="text-secondary mb-1">Base URL</div>
                                <code>${registry.base_url}</code>
                            </div>
                            <div class="mb-3">
                                <div class="text-secondary mb-1">Registry Type</div>
                                <span class="badge bg-azure text-azure-fg">${registry.registry_type}</span>
                            </div>
                            <div class="mb-3">
                                <div class="text-secondary mb-1">Role</div>
                                <span class="badge ${window.Alpine?.$data?.app?.getRegistryRoleBadge(registry.role) || 'bg-secondary text-secondary-fg'}">${registry.role}</span>
                            </div>
                        </div>
                        <div class="col-md-6">
                            <div class="mb-3">
                                <div class="text-secondary mb-1">Status</div>
                                ${registry.is_active !== false ?
                                    '<span class="badge bg-success text-success-fg">Active</span>' :
                                    '<span class="badge bg-secondary text-secondary-fg">Inactive</span>'}
                            </div>
                            <div class="mb-3">
                                <div class="text-secondary mb-1">Description</div>
                                <div>${registry.description || '-'}</div>
                            </div>
                            <div class="mb-3">
                                <div class="text-secondary mb-1">Created</div>
                                <div>${new Date(registry.created_at).toLocaleString('cs-CZ')}</div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        `;

        // Delete handler
        document.getElementById('delete-registry-btn').addEventListener('click', async () => {
            const confirmed = await showConfirmDialog(
                'Delete Registry?',
                `Are you sure you want to delete "${registry.name}"?`,
                'Delete',
                'Cancel'
            );

            if (confirmed) {
                try {
                    await api.deleteRegistry(registry.id);
                    getApp().showSuccess('Registry deleted successfully');
                    router.navigate('/registries');
                } catch (error) {
                    getApp().showError(error.message);
                }
            }
        });

    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                <i class="ti ti-alert-circle"></i>
                Failed to load registry: ${error.message}
            </div>
        `;
    }
});

// Registry New/Edit
router.on('/registries/new', async (params, query) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const tenants = await api.getTenants();
        content.innerHTML = createRegistryForm(null, tenants);

        // Pre-select tenant if provided in query
        if (query.tenant_id) {
            const select = document.querySelector('select[name="tenant_id"]');
            if (select) select.value = query.tenant_id;
        }

        document.getElementById('registry-form').addEventListener('submit', async (e) => {
            await handleFormSubmit(e, async (data) => {
                const tenantId = data.tenant_id;
                delete data.tenant_id;
                await api.createRegistry(tenantId, data);
                getApp().showSuccess('Registry created successfully');
                router.navigate('/registries');
            });
        });
    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load form: ${error.message}
            </div>
        `;
    }
});

router.on('/registries/:id/edit', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const [registry, tenants] = await Promise.all([
            api.getRegistry(params.id),
            api.getTenants(),
        ]);
        content.innerHTML = createRegistryForm(registry, tenants);

        document.getElementById('registry-form').addEventListener('submit', async (e) => {
            await handleFormSubmit(e, async (data) => {
                await api.updateRegistry(params.id, data);
                getApp().showSuccess('Registry updated successfully');
                router.navigate(`/registries/${params.id}`);
            });
        });
    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load registry: ${error.message}
            </div>
        `;
    }
});

// ==================== BUNDLES ROUTES ====================

// Bundles List
router.on('/bundles', async () => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const [bundles, tenants, registries] = await Promise.all([
            api.getBundles(),
            api.getTenants(),
            api.getRegistries()
        ]);

        // Create lookup maps
        const tenantMap = {};
        tenants.forEach(t => tenantMap[t.id] = t);
        const registryMap = {};
        registries.forEach(r => registryMap[r.id] = r);

        const renderBundles = (rows, searchQuery = '', selectedTenant = '') => `
            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">Bundles</h3>
                    <div class="card-actions">
                        <a href="#/bundles/new" class="btn btn-primary">
                            <i class="ti ti-plus"></i>
                            New Bundle
                        </a>
                    </div>
                </div>
                <div class="card-body border-bottom py-3">
                    <div class="row g-2">
                        <div class="col-md-4">
                            <div class="input-group">
                                <span class="input-group-text">
                                    <i class="ti ti-search"></i>
                                </span>
                                <input type="text" class="form-control" placeholder="Search by name..."
                                       id="bundles-search" value="${searchQuery}">
                            </div>
                        </div>
                        <div class="col-md-3">
                            <select class="form-select" id="bundles-tenant">
                                <option value="">All Tenants</option>
                                ${tenants.map(t => `
                                    <option value="${t.id}" ${selectedTenant === t.id ? 'selected' : ''}>
                                        ${t.name}
                                    </option>
                                `).join('')}
                            </select>
                        </div>
                    </div>
                </div>
                <div class="table-responsive">
                    <table class="table table-vcenter card-table">
                        <thead>
                            <tr>
                                <th>Name</th>
                                <th>Tenant</th>
                                <th>Description</th>
                                <th>Current Version</th>
                                <th>Images</th>
                                <th>Created</th>
                            </tr>
                        </thead>
                        <tbody>
                            ${rows.length === 0 ? `
                                <tr>
                                    <td colspan="6" class="text-center text-secondary py-5">
                                        No bundles found. Create your first bundle to get started.
                                    </td>
                                </tr>
                            ` : rows.map(bundle => {
                                const tenant = tenantMap[bundle.tenant_id];
                                const sourceReg = registryMap[bundle.source_registry_id];
                                const targetReg = registryMap[bundle.target_registry_id];

                                return `
                                <tr>
                                    <td>
                                        <div><a href="#/bundles/${bundle.id}"><strong>${bundle.name}</strong></a></div>
                                        <div class="small text-secondary" style="line-height: 1.2;">
                                            <div class="mt-1">
                                                <i class="ti ti-download" style="font-size: 0.8em;"></i>
                                                <span style="font-size: 0.85em;">${sourceReg?.base_url || 'Unknown'}</span>
                                            </div>
                                            <div>
                                                <i class="ti ti-upload" style="font-size: 0.8em;"></i>
                                                <span style="font-size: 0.85em;">${targetReg?.base_url || 'Unknown'}</span>
                                            </div>
                                        </div>
                                    </td>
                                    <td>
                                        <span class="badge bg-blue text-blue-fg">${tenant?.name || 'Unknown'}</span>
                                    </td>
                                    <td>${bundle.description || '-'}</td>
                                    <td><span class="badge bg-azure text-azure-fg">v${bundle.current_version || 1}</span></td>
                                    <td>${bundle.image_count || 0}</td>
                                    <td>${new Date(bundle.created_at).toLocaleDateString('cs-CZ')}</td>
                                </tr>
                            `}).join('')}
                        </tbody>
                    </table>
                </div>
            </div>
        `;

        content.innerHTML = renderBundles(bundles);
        const searchEl = document.getElementById('bundles-search');
        const tenantEl = document.getElementById('bundles-tenant');

        const applyFilters = () => {
            const q = searchEl.value.trim().toLowerCase();
            const tenantId = tenantEl.value;
            const filtered = bundles.filter(b => {
                const nameOk = !q || b.name.toLowerCase().includes(q);
                const tenantOk = !tenantId || b.tenant_id === tenantId;
                return nameOk && tenantOk;
            });
            content.innerHTML = renderBundles(filtered, q, tenantId);
            document.getElementById('bundles-search').value = q;
            document.getElementById('bundles-tenant').value = tenantId;
            document.getElementById('bundles-search').addEventListener('input', applyFilters);
            document.getElementById('bundles-tenant').addEventListener('change', applyFilters);
        };

        searchEl.addEventListener('input', applyFilters);
        tenantEl.addEventListener('change', applyFilters);
    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                <i class="ti ti-alert-circle"></i>
                Failed to load bundles: ${error.message}
            </div>
        `;
    }
});

// Bundle Wizard
router.on('/bundles/new', async (params, query) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const [tenants, registries] = await Promise.all([
            api.getTenants(),
            api.getRegistries(),
        ]);

        const wizard = new BundleWizard();

        // Pre-select tenant z query
        if (query.tenant_id) {
            wizard.data.bundle.tenant_id = query.tenant_id;
        }

        const renderWizard = () => {
            content.innerHTML = wizard.render(tenants, registries);
            attachWizardHandlers();
        };

        const attachWizardHandlers = () => {
            // Tenant change - re-render to filter registries
            const tenantSelect = document.getElementById('bundle-tenant');
            if (tenantSelect) {
                tenantSelect.addEventListener('change', () => {
                    wizard.data.bundle.tenant_id = tenantSelect.value;
                    // Clear registry selections when tenant changes
                    wizard.data.bundle.source_registry_id = '';
                    wizard.data.bundle.target_registry_id = '';
                    renderWizard();
                });
            }

            // Next button
            const nextBtn = document.getElementById('wizard-next');
            if (nextBtn) {
                nextBtn.addEventListener('click', async () => {
                    try {
                        if (wizard.currentStep === 1) {
                            wizard.saveStep1();
                            wizard.currentStep = 2;
                            renderWizard();
                        } else if (wizard.currentStep === 2) {
                            wizard.saveStep2();
                            wizard.currentStep = 3;
                            renderWizard();
                        }
                    } catch (error) {
                        getApp().showError(error.message);
                    }
                });
            }

            // Previous button
            const prevBtn = document.getElementById('wizard-prev');
            if (prevBtn) {
                prevBtn.addEventListener('click', () => {
                    // Save current step data before going back
                    if (wizard.currentStep === 2) {
                        wizard.collectStep2Data();
                    }
                    wizard.currentStep--;
                    renderWizard();
                });
            }

            // Create button
            const createBtn = document.getElementById('wizard-create');
            if (createBtn) {
                createBtn.addEventListener('click', async () => {
                    try {
                        createBtn.disabled = true;
                        createBtn.innerHTML = '<span class="spinner-border spinner-border-sm me-2"></span>Creating...';

                        const bundle = await wizard.createBundle();

                        getApp().showSuccess('Bundle created successfully');
                        router.navigate(`/bundles/${bundle.id}`);
                    } catch (error) {
                        getApp().showError(error.message);
                        createBtn.disabled = false;
                        createBtn.innerHTML = '<i class="ti ti-check"></i> Create Bundle';
                    }
                });
            }

            // Add mapping button
            const addMappingBtn = document.getElementById('add-mapping-btn');
            if (addMappingBtn) {
                addMappingBtn.addEventListener('click', () => {
                    // IMPORTANT: Collect current form data before adding new mapping
                    wizard.collectStep2Data();
                    wizard.addMapping();
                    renderWizard();
                });
            }

            // Remove mapping buttons
            document.querySelectorAll('.mapping-remove').forEach((btn, index) => {
                btn.addEventListener('click', () => {
                    // Collect data before removing
                    wizard.collectStep2Data();
                    wizard.removeMapping(index);
                    renderWizard();
                });
            });
        };

        renderWizard();

    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load wizard: ${error.message}
            </div>
        `;
    }
});

// Bundle Detail
router.on('/bundles/:id', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const [bundle, versions, copyJobs, releases] = await Promise.all([
            api.getBundle(params.id),
            api.getBundleVersions(params.id),
            api.getBundleCopyJobs(params.id),
            api.getBundleReleases(params.id),
        ]);

        content.innerHTML = `
            <div class="row mb-3">
                <div class="col">
                    <a href="#/bundles" class="btn btn-ghost-secondary">
                        <i class="ti ti-arrow-left"></i>
                        Back to Bundles
                    </a>
                </div>
            </div>

            <div class="row">
                <div class="col-md-8">
                    <div class="card mb-3">
                        <div class="card-header">
                            <h3 class="card-title">${bundle.name}</h3>
                            <div class="card-actions">
                                <a href="#/bundles/${bundle.id}/versions/new" class="btn btn-primary btn-sm">
                                    <i class="ti ti-plus"></i>
                                    New Version
                                </a>
                                <a href="#/bundles/${bundle.id}/copy" class="btn btn-ghost-primary btn-sm">
                                    <i class="ti ti-copy"></i>
                                    Copy Bundle
                                </a>
                                <a href="#/bundles/${bundle.id}/edit" class="btn btn-ghost-secondary btn-sm">
                                    <i class="ti ti-pencil"></i>
                                    Edit
                                </a>
                                <button class="btn btn-danger btn-sm" id="delete-bundle-btn">
                                    <i class="ti ti-trash"></i>
                                    Delete
                                </button>
                            </div>
                        </div>
                        <div class="card-body">
                            <dl class="row mb-0">
                                <dt class="col-4">Description:</dt>
                                <dd class="col-8">${bundle.description || '-'}</dd>

                                <dt class="col-4">Current Version:</dt>
                                <dd class="col-8"><span class="badge bg-blue text-blue-fg">v${bundle.current_version || 1}</span></dd>

                                <dt class="col-4">Created:</dt>
                                <dd class="col-8">${new Date(bundle.created_at).toLocaleString('cs-CZ')}</dd>
                            </dl>
                        </div>
                    </div>

                    <div class="card">
                        <div class="card-header">
                            <h3 class="card-title">Versions</h3>
                        </div>
                        <div class="list-group list-group-flush">
                            ${versions.map(version => `
                                <a href="#/bundles/${bundle.id}/versions/${version.version}" class="list-group-item list-group-item-action">
                                    <div class="row align-items-center">
                                        <div class="col-auto">
                                            <span class="badge bg-blue text-blue-fg">v${version.version}</span>
                                        </div>
                                        <div class="col">
                                            <div class="text-secondary small">
                                                Created ${new Date(version.created_at).toLocaleString('cs-CZ')}
                                            </div>
                                        </div>
                                        <div class="col-auto">
                                            <span class="badge">${version.image_count || 0} images</span>
                                        </div>
                                        <div class="col-auto">
                                            ${version.is_archived ? `
                                                <span class="badge bg-secondary text-secondary-fg">archived</span>
                                            ` : ''}
                                        </div>
                                    </div>
                                </a>
                            `).join('')}
                        </div>
                    </div>

                    <div class="card mt-3">
                        <div class="card-header">
                            <h3 class="card-title">Releases</h3>
                        </div>
                        <div class="table-responsive">
                            <table class="table table-vcenter card-table">
                                <thead>
                                    <tr>
                                        <th>Release ID</th>
                                        <th>Status</th>
                                        <th>Created</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    ${releases.length === 0 ? `
                                        <tr>
                                            <td colspan="3" class="text-center text-secondary py-4">
                                                No releases yet.
                                            </td>
                                        </tr>
                                    ` : releases.map(release => `
                                        <tr>
                                            <td><a href="#/releases/${release.id}"><strong>${release.release_id}</strong></a></td>
                                            <td><span class="badge bg-blue text-blue-fg">${release.status}</span></td>
                                            <td>${new Date(release.created_at).toLocaleDateString('cs-CZ')}</td>
                                        </tr>
                                    `).join('')}
                                </tbody>
                            </table>
                        </div>
                    </div>

                    <div class="card mt-3">
                        <div class="card-header">
                            <h3 class="card-title">Copy History</h3>
                        </div>
                        <div class="table-responsive">
                            <table class="table table-vcenter card-table">
                                <thead>
                                    <tr>
                                        <th>Version</th>
                                        <th>Target Tag</th>
                                        <th>Status</th>
                                        <th>Started</th>
                                        <th>Completed</th>
                                        <th>Duration</th>
                                        <th class="w-1"></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    ${copyJobs.length === 0 ? `
                                        <tr>
                                            <td colspan="7" class="text-center text-secondary py-4">
                                                No copy jobs yet.
                                            </td>
                                        </tr>
                                    ` : copyJobs.map(job => `
                                        <tr>
                                            <td>
                                                <span class="badge bg-blue text-blue-fg">v${job.version}</span>
                                            </td>
                                            <td><a href="#/copy-jobs/${job.job_id}"><span class="badge bg-azure-lt">${job.target_tag}</span></a></td>
                                            <td>
                                                <span class="badge ${
                                                    job.status === 'success' ? 'bg-success text-success-fg' :
                                                    job.status === 'failed' ? 'bg-danger text-danger-fg' :
                                                    job.status === 'in_progress' ? 'bg-info text-info-fg' :
                                                    'bg-secondary text-secondary-fg'
                                                }">${job.status}</span>
                                            </td>
                                            <td>${new Date(job.started_at).toLocaleString('cs-CZ')}</td>
                                            <td>${job.completed_at ? new Date(job.completed_at).toLocaleString('cs-CZ') : '-'}</td>
                                            <td>${job.completed_at ? (() => {
                                                const start = new Date(job.started_at).getTime();
                                                const end = new Date(job.completed_at).getTime();
                                                const secs = Math.max(0, Math.floor((end - start) / 1000));
                                                const mins = Math.floor(secs / 60);
                                                const rem = secs % 60;
                                                return mins > 0 ? `${mins}m ${rem}s` : `${rem}s`;
                                            })() : '-'}</td>
                                            <td>
                                                ${job.is_release_job ? `
                                                    <span class="badge bg-purple-lt text-purple-fg">release</span>
                                                ` : job.status === 'success' ? `
                                                    <a href="#/releases/new?copy_job_id=${job.job_id}" class="btn btn-sm btn-success">
                                                        <i class="ti ti-rocket"></i>
                                                        Release
                                                    </a>
                                                ` : ''}
                                            </td>
                                        </tr>
                                    `).join('')}
                                </tbody>
                            </table>
                        </div>
                    </div>
                </div>

                <div class="col-md-4">
                    <div class="card">
                        <div class="card-header">
                            <h3 class="card-title">Quick Actions</h3>
                        </div>
                        <div class="card-body">
                            <div class="d-grid gap-2">
                                <a href="#/bundles/${bundle.id}/versions/new" class="btn btn-primary">
                                    <i class="ti ti-plus"></i>
                                    New Version
                                </a>
                                <a href="#/bundles/${bundle.id}/copy" class="btn btn-success">
                                    <i class="ti ti-copy"></i>
                                    Copy Bundle
                                </a>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        `;

        // Delete handler
        document.getElementById('delete-bundle-btn').addEventListener('click', async () => {
            const confirmed = await showConfirmDialog(
                'Delete Bundle?',
                `Are you sure you want to delete "${bundle.name}"? This will delete all versions and image mappings.`,
                'Delete',
                'Cancel'
            );

            if (confirmed) {
                try {
                    await api.deleteBundle(bundle.id);
                    getApp().showSuccess('Bundle deleted successfully');
                    router.navigate('/bundles');
                } catch (error) {
                    getApp().showError(error.message);
                }
            }
        });

    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load bundle: ${error.message}
            </div>
        `;
    }
});

// Bundle Edit (name/description only)
router.on('/bundles/:id/edit', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const [bundle, versions] = await Promise.all([
            api.getBundle(params.id),
            api.getBundleVersions(params.id),
        ]);
        const latestVersion = versions.length > 0
            ? Math.max(...versions.map(v => v.version))
            : null;

        content.innerHTML = `
            <div class="row mb-3">
                <div class="col">
                    <a href="#/bundles/${bundle.id}" class="btn btn-ghost-secondary">
                        <i class="ti ti-arrow-left"></i>
                        Back to Bundle
                    </a>
                </div>
            </div>

            <div class="card mb-3">
                <div class="card-header">
                    <h3 class="card-title">Edit Bundle</h3>
                    <div class="card-subtitle">${bundle.name}</div>
                </div>
                <form id="bundle-edit-form">
                    <div class="card-body">
                        <div class="mb-3">
                            <label class="form-label required">Name</label>
                            <input type="text" class="form-control" name="name" value="${bundle.name}" required>
                        </div>

                        <div class="mb-3">
                            <label class="form-label">Description</label>
                            <textarea class="form-control" name="description" rows="3">${bundle.description || ''}</textarea>
                        </div>
                    </div>
                    <div class="card-footer text-end">
                        <div class="d-flex">
                            <a href="#/bundles/${bundle.id}" class="btn btn-link">Cancel</a>
                            <button type="submit" class="btn btn-primary ms-auto">
                                <i class="ti ti-check"></i>
                                Save
                            </button>
                        </div>
                    </div>
                </form>
            </div>

            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">Archive Versions</h3>
                </div>
                <div class="table-responsive">
                    <table class="table table-vcenter card-table">
                        <thead>
                            <tr>
                                <th>Version</th>
                                <th>Created</th>
                                <th>Status</th>
                                <th class="w-1"></th>
                            </tr>
                        </thead>
                        <tbody>
                            ${versions.map(version => `
                                <tr>
                                    <td><span class="badge bg-blue text-blue-fg">v${version.version}</span></td>
                                    <td>${new Date(version.created_at).toLocaleString('cs-CZ')}</td>
                                    <td>
                                        ${version.is_archived
                                            ? '<span class="badge bg-secondary text-secondary-fg">archived</span>'
                                            : '<span class="badge bg-success text-success-fg">active</span>'}
                                    </td>
                                    <td>
                                        ${!version.is_archived && version.version === latestVersion ? `
                                            <button type="button" class="btn btn-sm btn-outline-secondary" disabled>
                                                Latest
                                            </button>
                                        ` : `
                                            <button type="button" class="btn btn-sm ${version.is_archived ? 'btn-primary' : 'btn-outline-secondary'} archive-toggle"
                                                    data-version="${version.version}" data-archived="${version.is_archived}">
                                                ${version.is_archived ? 'Un-archive' : 'Archive'}
                                            </button>
                                        `}
                                    </td>
                                </tr>
                            `).join('')}
                        </tbody>
                    </table>
                </div>
            </div>
        `;

        document.getElementById('bundle-edit-form').addEventListener('submit', async (e) => {
            await handleFormSubmit(e, async (data) => {
                await api.updateBundle(bundle.id, {
                    name: data.name,
                    description: data.description,
                    source_registry_id: bundle.source_registry_id,
                    target_registry_id: bundle.target_registry_id,
                });
                getApp().showSuccess('Bundle updated successfully');
                router.navigate(`/bundles/${bundle.id}`);
            });
        });

        document.querySelectorAll('.archive-toggle').forEach(btn => {
            btn.addEventListener('click', async () => {
                const version = parseInt(btn.getAttribute('data-version'), 10);
                const isArchived = btn.getAttribute('data-archived') === 'true';
                try {
                    await api.setBundleVersionArchived(bundle.id, version, !isArchived);
                    getApp().showSuccess(`Version v${version} ${isArchived ? 'un-archived' : 'archived'}`);
                    router.navigate(`/bundles/${bundle.id}/edit`);
                    router.handleRoute();
                } catch (error) {
                    getApp().showError(`Failed to update archive status: ${error.message}`);
                }
            });
        });
    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load bundle: ${error.message}
            </div>
        `;
    }
});

// Copy Bundle
router.on('/bundles/:id/copy', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const [bundle, tenants, registries, versions] = await Promise.all([
            api.getBundle(params.id),
            api.getTenants(),
            api.getRegistries(),
            api.getBundleVersions(params.id),
        ]);

        const latestVersion = versions.length > 0
            ? Math.max(...versions.map(v => v.version))
            : 1;
        let selectedVersion = latestVersion;
        let mappings = await api.getImageMappings(params.id, selectedVersion);

        const wizard = new BundleWizard({
            title: 'Copy Bundle',
            createLabel: 'Create Copy',
            tenantLocked: true,
        });

        wizard.data.bundle.tenant_id = bundle.tenant_id;
        wizard.data.bundle.name = `${bundle.name} Copy`;
        wizard.data.bundle.description = bundle.description || '';
        wizard.data.bundle.source_registry_id = bundle.source_registry_id;
        wizard.data.bundle.target_registry_id = bundle.target_registry_id;
        wizard.data.imageMappings = mappings.map(m => ({
            source_image: m.source_image,
            source_tag: m.source_tag,
            target_image: m.target_image,
        }));

        const renderWizard = () => {
            content.innerHTML = `
                <div class="card mb-3">
                    <div class="card-body">
                        <div class="row g-2">
                            <div class="col-md-4">
                                <label class="form-label">Start from version</label>
                                <select class="form-select" id="copy-from-version">
                                    ${versions.map(v => `
                                        <option value="${v.version}" ${v.version === selectedVersion ? 'selected' : ''}>
                                            v${v.version}
                                        </option>
                                    `).join('')}
                                </select>
                            </div>
                        </div>
                    </div>
                </div>
                ${wizard.render(tenants, registries)}
            `;
            attachWizardHandlers();
        };

        const attachWizardHandlers = () => {
            const tenantSelect = document.getElementById('bundle-tenant');
            if (tenantSelect && !wizard.options.tenantLocked) {
                tenantSelect.addEventListener('change', () => {
                    wizard.data.bundle.tenant_id = tenantSelect.value;
                    wizard.data.bundle.source_registry_id = '';
                    wizard.data.bundle.target_registry_id = '';
                    renderWizard();
                });
            }

            const versionSelect = document.getElementById('copy-from-version');
            if (versionSelect) {
                versionSelect.addEventListener('change', async () => {
                    selectedVersion = parseInt(versionSelect.value, 10);
                    try {
                        mappings = await api.getImageMappings(params.id, selectedVersion);
                        wizard.data.imageMappings = mappings.map(m => ({
                            source_image: m.source_image,
                            source_tag: m.source_tag,
                            target_image: m.target_image,
                        }));
                        renderWizard();
                    } catch (error) {
                        getApp().showError(`Failed to load mappings for v${selectedVersion}`);
                    }
                });
            }

            const nextBtn = document.getElementById('wizard-next');
            if (nextBtn) {
                nextBtn.addEventListener('click', async () => {
                    try {
                        if (wizard.currentStep === 1) {
                            wizard.saveStep1();
                            wizard.currentStep = 2;
                            renderWizard();
                        } else if (wizard.currentStep === 2) {
                            wizard.saveStep2();
                            wizard.currentStep = 3;
                            renderWizard();
                        }
                    } catch (error) {
                        getApp().showError(error.message);
                    }
                });
            }

            const prevBtn = document.getElementById('wizard-prev');
            if (prevBtn) {
                prevBtn.addEventListener('click', () => {
                    if (wizard.currentStep === 2) {
                        wizard.collectStep2Data();
                    }
                    wizard.currentStep--;
                    renderWizard();
                });
            }

            const createBtn = document.getElementById('wizard-create');
            if (createBtn) {
                createBtn.addEventListener('click', async () => {
                    try {
                        createBtn.disabled = true;
                        createBtn.innerHTML = '<span class="spinner-border spinner-border-sm me-2"></span>Creating...';

                        const newBundle = await wizard.createBundle();
                        getApp().showSuccess('Bundle copied successfully');
                        router.navigate(`/bundles/${newBundle.id}`);
                    } catch (error) {
                        getApp().showError(error.message);
                        createBtn.disabled = false;
                        createBtn.innerHTML = '<i class="ti ti-check"></i> Create Copy';
                    }
                });
            }

            const addMappingBtn = document.getElementById('add-mapping-btn');
            if (addMappingBtn) {
                addMappingBtn.addEventListener('click', () => {
                    wizard.collectStep2Data();
                    wizard.addMapping();
                    renderWizard();
                });
            }

            document.querySelectorAll('.mapping-remove').forEach((btn, index) => {
                btn.addEventListener('click', () => {
                    wizard.collectStep2Data();
                    wizard.removeMapping(index);
                    renderWizard();
                });
            });
        };

        renderWizard();
    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load bundle: ${error.message}
            </div>
        `;
    }
});

// New Bundle Version (must be before the generic version route)
router.on('/bundles/:id/versions/new', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const bundle = await api.getBundle(params.id);
        const versions = await api.getBundleVersions(params.id);
        const latestVersion = versions.length > 0
            ? Math.max(...versions.map(v => v.version))
            : 1;
        const initialMappings = await api.getImageMappings(params.id, latestVersion);

        const state = {
            mappings: initialMappings.map(m => ({
                source_image: m.source_image,
                source_tag: m.source_tag,
                target_image: m.target_image,
            })),
        };

        const render = () => {
            content.innerHTML = `
                <div class="card">
                    <div class="card-header">
                        <h3 class="card-title">Create New Version</h3>
                        <div class="card-subtitle">Bundle: ${bundle.name}</div>
                    </div>
                    <div class="card-body">
                        <h3 class="mb-3">Image Mappings</h3>
                        <p class="text-secondary mb-3">
                            Start from the latest version and adjust the list as needed.
                        </p>

                        <div id="mappings-list" class="mb-3">
                            ${state.mappings.map((mapping, index) => `
                                <div class="card mb-2" data-mapping-index="${index}">
                                    <div class="card-body">
                                        <div class="row g-2">
                                            <div class="col-md-5">
                                                <label class="form-label">Source Image</label>
                                                <input type="text" class="form-control form-control-sm mapping-source-image"
                                                       value="${mapping.source_image}"
                                                       placeholder="project/image">
                                                <small class="form-hint">Path without registry hostname</small>
                                            </div>
                                            <div class="col-md-2">
                                                <label class="form-label">Source Tag</label>
                                                <input type="text" class="form-control form-control-sm mapping-source-tag"
                                                       value="${mapping.source_tag}"
                                                       placeholder="latest">
                                            </div>
                                            <div class="col-md-4">
                                                <label class="form-label">Target Image</label>
                                                <input type="text" class="form-control form-control-sm mapping-target-image"
                                                       value="${mapping.target_image}"
                                                       placeholder="project/image">
                                                <small class="form-hint">Path without registry hostname</small>
                                            </div>
                                            <div class="col-md-1 d-flex align-items-end">
                                                <button type="button" class="btn btn-sm btn-ghost-danger w-100 mapping-remove">
                                                    <i class="ti ti-trash"></i>
                                                </button>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            `).join('')}
                        </div>

                        <button type="button" class="btn btn-primary" id="add-mapping-btn">
                            <i class="ti ti-plus"></i>
                            Add Image Mapping
                        </button>

                        ${state.mappings.length === 0 ? `
                            <div class="alert alert-info mt-3">
                                <i class="ti ti-info-circle"></i>
                                Add at least one image mapping to continue
                            </div>
                        ` : ''}

                        <hr class="my-4">

                        <div class="mb-3">
                            <label class="form-label">Description</label>
                            <textarea class="form-control" id="change-note" rows="3"
                                      placeholder="Describe what changed in this version (optional)"></textarea>
                        </div>
                    </div>
                    <div class="card-footer text-end">
                        <div class="d-flex">
                            <a href="#/bundles/${bundle.id}" class="btn btn-link">Cancel</a>
                            <button type="button" class="btn btn-primary ms-auto" id="create-version-btn">
                                <i class="ti ti-plus"></i>
                                Create Version
                            </button>
                        </div>
                    </div>
                </div>
            `;

            const collectMappings = () => {
                const cards = document.querySelectorAll('[data-mapping-index]');
                const mappings = [];
                cards.forEach((card) => {
                    const sourceImage = card.querySelector('.mapping-source-image')?.value || '';
                    const sourceTag = card.querySelector('.mapping-source-tag')?.value || '';
                    const targetImage = card.querySelector('.mapping-target-image')?.value || '';
                    mappings.push({ source_image: sourceImage, source_tag: sourceTag, target_image: targetImage });
                });
                state.mappings = mappings;
            };

            const addBtn = document.getElementById('add-mapping-btn');
            if (addBtn) {
                addBtn.addEventListener('click', () => {
                    collectMappings();
                    state.mappings.push({ source_image: '', source_tag: '', target_image: '' });
                    render();
                });
            }

            document.querySelectorAll('.mapping-remove').forEach((btn, index) => {
                btn.addEventListener('click', () => {
                    collectMappings();
                    state.mappings.splice(index, 1);
                    render();
                });
            });

            const createBtn = document.getElementById('create-version-btn');
            createBtn.addEventListener('click', async () => {
                collectMappings();
                const validMappings = state.mappings.filter(m => m.source_image && m.source_tag && m.target_image);
                if (validMappings.length === 0) {
                    getApp().showError('Please add at least one complete image mapping');
                    return;
                }

                try {
                    createBtn.disabled = true;
                    createBtn.innerHTML = '<span class="spinner-border spinner-border-sm me-2"></span>Creating...';

                    const changeNote = document.getElementById('change-note').value || null;
                    const newVersion = await api.createBundleVersion(bundle.id, { change_note: changeNote });

                    for (const mapping of validMappings) {
                        await api.addImageMapping(bundle.id, newVersion.version, mapping);
                    }

                    getApp().showSuccess(`Version ${newVersion.version} created successfully`);
                    router.navigate(`/bundles/${bundle.id}/versions/${newVersion.version}`);
                } catch (error) {
                    getApp().showError('Failed to create version: ' + error.message);
                    createBtn.disabled = false;
                    createBtn.innerHTML = '<i class="ti ti-plus"></i> Create Version';
                }
            });
        };

        render();

    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load bundle: ${error.message}
            </div>
        `;
    }
});

// Bundle Version Detail
router.on('/bundles/:id/versions/:version', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const [bundle, version, mappings, copyJobs] = await Promise.all([
            api.getBundle(params.id),
            api.getBundleVersion(params.id, params.version),
            api.getImageMappings(params.id, params.version),
            api.getBundleCopyJobs(params.id),
        ]);

        const versionJobs = copyJobs.filter(j => j.version === Number(params.version));

        content.innerHTML = `
            <div class="row mb-3">
                <div class="col">
                    <a href="#/bundles/${params.id}" class="btn btn-ghost-secondary">
                        <i class="ti ti-arrow-left"></i>
                        Back to Bundle
                    </a>
                </div>
            </div>

            <div class="card mb-3">
                <div class="card-header">
                    <h3 class="card-title">${bundle.name} - Version ${params.version}</h3>
                    <div class="card-actions">
                        <a href="#/bundles/${params.id}/versions/${params.version}/copy" class="btn btn-primary btn-sm">
                            <i class="ti ti-copy"></i>
                            Copy Images
                        </a>
                    </div>
                </div>
                <div class="card-body">
                    ${version.is_archived ? `
                        <div class="alert alert-warning">
                            <i class="ti ti-alert-triangle"></i>
                            This version is archived. Copying from an archived version may be intentional but is not recommended.
                        </div>
                    ` : ''}
                    <div class="row">
                        <div class="col-md-12">
                            <div class="text-center">
                                <div class="h1">${mappings.length}</div>
                                <div class="text-secondary">Total</div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <div class="card mb-3">
                <div class="card-header">
                    <h3 class="card-title">Image Mappings</h3>
                </div>
                <div class="table-responsive">
                    <table class="table table-vcenter card-table">
                        <thead>
                            <tr>
                                <th>Source Image</th>
                                <th>Target Image</th>
                            </tr>
                        </thead>
                        <tbody>
                            ${mappings.map(mapping => {
                                return `
                                <tr>
                                    <td>
                                        <div><code class="small">${mapping.source_image}</code></div>
                                        <div class="small text-secondary mt-1">
                                            <span class="badge badge-sm bg-azure-lt">${mapping.source_tag}</span>
                                        </div>
                                    </td>
                                    <td>
                                        <div><code class="small">${mapping.target_image}</code></div>
                                    </td>
                                </tr>
                            `}).join('')}
                        </tbody>
                    </table>
                </div>
            </div>

            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">Copy Jobs</h3>
                </div>
                <div class="table-responsive">
                    <table class="table table-vcenter card-table">
                        <thead>
                            <tr>
                                <th>Target Tag</th>
                                <th>Status</th>
                                <th>Started</th>
                                <th>Completed</th>
                                <th>Duration</th>
                                <th class="w-1"></th>
                            </tr>
                        </thead>
                        <tbody>
                            ${versionJobs.length === 0 ? `
                                <tr>
                                    <td colspan="6" class="text-center text-secondary py-4">
                                        No copy jobs for this version.
                                    </td>
                                </tr>
                            ` : versionJobs.map(job => `
                                <tr>
                                    <td><a href="#/copy-jobs/${job.job_id}"><span class="badge bg-azure-lt">${job.target_tag}</span></a></td>
                                    <td>
                                        <span class="badge ${
                                            job.status === 'success' ? 'bg-success text-success-fg' :
                                            job.status === 'failed' ? 'bg-danger text-danger-fg' :
                                            job.status === 'in_progress' ? 'bg-info text-info-fg' :
                                            'bg-secondary text-secondary-fg'
                                        }">${job.status}</span>
                                    </td>
                                    <td>${new Date(job.started_at).toLocaleString('cs-CZ')}</td>
                                    <td>${job.completed_at ? new Date(job.completed_at).toLocaleString('cs-CZ') : '-'}</td>
                                    <td>${job.completed_at ? (() => {
                                        const start = new Date(job.started_at).getTime();
                                        const end = new Date(job.completed_at).getTime();
                                        const secs = Math.max(0, Math.floor((end - start) / 1000));
                                        const mins = Math.floor(secs / 60);
                                        const rem = secs % 60;
                                        return mins > 0 ? `${mins}m ${rem}s` : `${rem}s`;
                                    })() : '-'}</td>
                                    <td>
                                        ${!job.is_release_job && job.status === 'success' ? `
                                            <a href="#/releases/new?copy_job_id=${job.job_id}" class="btn btn-sm btn-success">
                                                <i class="ti ti-rocket"></i>
                                                Release
                                            </a>
                                        ` : job.is_release_job ? `
                                            <span class="badge bg-purple-lt text-purple-fg">release</span>
                                        ` : ''}
                                    </td>
                                </tr>
                            `).join('')}
                        </tbody>
                    </table>
                </div>
            </div>
        `;

    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load version: ${error.message}
            </div>
        `;
    }
});

// ==================== RELEASES ROUTES ====================

// Releases List
router.on('/releases', async () => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const releases = await api.getReleases();

        content.innerHTML = `
            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">Releases</h3>
                    <div class="card-actions">
                        <a href="#/releases/new" class="btn btn-primary">
                            <i class="ti ti-plus"></i>
                            New Release
                        </a>
                    </div>
                </div>
                <div class="table-responsive">
                    <table class="table table-vcenter card-table">
                        <thead>
                            <tr>
                                <th>Release ID</th>
                                <th>Status</th>
                                <th>Created</th>
                            </tr>
                        </thead>
                        <tbody>
                            ${releases.length === 0 ? `
                                <tr>
                                    <td colspan="3" class="text-center text-secondary py-5">
                                        No releases yet. Create a release from a copy job.
                                    </td>
                                </tr>
                            ` : releases.map(release => `
                                <tr>
                                    <td><a href="#/releases/${release.id}"><strong>${release.release_id}</strong></a></td>
                                    <td><span class="badge bg-blue text-blue-fg">${release.status}</span></td>
                                    <td>${new Date(release.created_at).toLocaleDateString('cs-CZ')}</td>
                                </tr>
                            `).join('')}
                        </tbody>
                    </table>
                </div>
            </div>
        `;
    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load releases: ${error.message}
            </div>
        `;
    }
});

// Release Detail
router.on('/releases/:id', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const [release, manifest] = await Promise.all([
            api.getRelease(params.id),
            api.getReleaseManifest(params.id),
        ]);

        content.innerHTML = `
            <div class="row mb-3">
                <div class="col">
                    <a href="#/releases" class="btn btn-ghost-secondary">
                        <i class="ti ti-arrow-left"></i>
                        Back to Releases
                    </a>
                </div>
            </div>

            <div class="card mb-3">
                <div class="card-header">
                    <h3 class="card-title">
                        <i class="ti ti-rocket me-2"></i>
                        ${release.release_id}
                    </h3>
                </div>
                <div class="card-body">
                    <dl class="row mb-0">
                        <dt class="col-4">Notes:</dt>
                        <dd class="col-8">${release.notes || '-'}</dd>

                        <dt class="col-4">Created:</dt>
                        <dd class="col-8">${new Date(release.created_at).toLocaleString('cs-CZ')}</dd>

                        <dt class="col-4">Images:</dt>
                        <dd class="col-8">${manifest.images.length}</dd>
                    </dl>
                </div>
            </div>

            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">Release Manifest</h3>
                    <div class="card-actions">
                        <button class="btn btn-sm btn-primary" id="copy-manifest-btn">
                            <i class="ti ti-copy"></i>
                            Copy Manifest
                        </button>
                    </div>
                </div>
                <div class="card-body">
                    <pre class="manifest-code" id="manifest-content">${JSON.stringify(manifest, null, 2)}</pre>
                </div>
            </div>
        `;

        // Copy manifest handler
        document.getElementById('copy-manifest-btn').addEventListener('click', () => {
            const text = document.getElementById('manifest-content').textContent;
            navigator.clipboard.writeText(text).then(() => {
                getApp().showSuccess('Manifest copied to clipboard');
            });
        });

    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load release: ${error.message}
            </div>
        `;
    }
});

// Create Release
router.on('/releases/new', async (params, query) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        if (!query.copy_job_id) {
            content.innerHTML = `
                <div class="card">
                    <div class="card-header">
                        <h3 class="card-title">Create Release</h3>
                    </div>
                    <div class="card-body">
                        <div class="alert alert-info">
                            <i class="ti ti-info-circle"></i>
                            Create release from a successful copy job.
                        </div>
                        <a href="#/copy-jobs" class="btn btn-primary">
                            <i class="ti ti-list"></i>
                            Choose Copy Job
                        </a>
                    </div>
                </div>
            `;
            return;
        }

        const [job, images, registries] = await Promise.all([
            api.getCopyJobStatus(query.copy_job_id),
            api.getCopyJobImages(query.copy_job_id),
            api.getRegistries(),
        ]);

        const sourceRegistry = registries.find(r => r.id === job.target_registry_id);
        const sourceBase = sourceRegistry?.base_url || '';

        const state = {
            releaseId: '',
            notes: '',
            targetRegistryId: '',
            renameRules: [{ find: '', replace: '' }],
            overrides: images.map(img => ({ copy_job_image_id: img.id, override_name: '' })),
        };

        const applyRules = (path) => {
            let out = path;
            state.renameRules.forEach(rule => {
                if (rule.find) {
                    out = out.split(rule.find).join(rule.replace);
                }
            });
            return out;
        };

        const applyOverride = (path, overrideName) => {
            if (!overrideName) return path;
            const idx = path.lastIndexOf('/');
            if (idx === -1) return overrideName;
            return `${path.slice(0, idx + 1)}${overrideName}`;
        };

        const render = () => {
            const targetRegistry = registries.find(r => r.id === state.targetRegistryId);
            const targetBase = targetRegistry?.base_url || '';

            content.innerHTML = `
                <div class="card">
                    <div class="card-header">
                        <h3 class="card-title">Create Release</h3>
                    </div>
                    <div class="card-body">
                        <div class="mb-3">
                            <label class="form-label required">Copy Job ID</label>
                            <input type="text" class="form-control" value="${job.job_id}" readonly>
                        </div>

                        <div class="row">
                            <div class="col-md-6">
                                <div class="mb-3">
                                    <label class="form-label">Source Registry</label>
                                    <select class="form-select" disabled>
                                        ${registries.map(r => `
                                            <option value="${r.id}" ${r.id === job.target_registry_id ? 'selected' : ''}>
                                                ${r.name} (${r.base_url})
                                            </option>
                                        `).join('')}
                                    </select>
                                </div>
                            </div>
                            <div class="col-md-6">
                                <div class="mb-3">
                                    <label class="form-label required">Target Registry</label>
                                    <select class="form-select" id="release-target-registry">
                                        <option value="">Select target...</option>
                                        ${registries.map(r => `
                                            <option value="${r.id}" ${state.targetRegistryId === r.id ? 'selected' : ''}>
                                                ${r.name} (${r.base_url})
                                            </option>
                                        `).join('')}
                                    </select>
                                </div>
                            </div>
                        </div>

                        <div class="mb-3">
                            <label class="form-label required">Release ID (target tag)</label>
                            <input type="text" class="form-control" id="release-id" value="${state.releaseId}"
                                   placeholder="2026.02.04.01">
                        </div>

                        <div class="mb-3">
                            <label class="form-label">Notes</label>
                            <textarea class="form-control" id="release-notes" rows="3">${state.notes || ''}</textarea>
                        </div>

                        <hr class="my-4">

                        <div class="mb-3">
                            <label class="form-label">Rename Rules</label>
                            ${state.renameRules.map((rule, idx) => `
                                <div class="row g-2 mb-2">
                                    <div class="col-md-5">
                                        <input type="text" class="form-control rename-find" data-index="${idx}"
                                               placeholder="find" value="${rule.find}">
                                    </div>
                                    <div class="col-md-5">
                                        <input type="text" class="form-control rename-replace" data-index="${idx}"
                                               placeholder="replace" value="${rule.replace}">
                                    </div>
                                    <div class="col-md-2">
                                        <button type="button" class="btn btn-outline-danger w-100 rename-remove" data-index="${idx}">
                                            <i class="ti ti-trash"></i>
                                        </button>
                                    </div>
                                </div>
                            `).join('')}
                            <button type="button" class="btn btn-primary btn-sm" id="rename-add">
                                <i class="ti ti-plus"></i>
                                Add Rule
                            </button>
                        </div>

                        <hr class="my-4">

                        <div class="table-responsive">
                            <table class="table table-vcenter card-table">
                                <thead>
                                    <tr>
                                        <th>Source Image</th>
                                        <th>Target Preview</th>
                                        <th>Override name</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    ${images.map((img, idx) => {
                                        const renamed = applyRules(img.target_image);
                                        const override = state.overrides[idx]?.override_name || '';
                                        const finalPath = applyOverride(renamed, override);
                                        const sourceFull = `${sourceBase}/${img.target_image}:${img.target_tag}`;
                                        const targetFull = targetBase ? `${targetBase}/${finalPath}:${state.releaseId || '<release_id>'}` : '-';
                                        return `
                                            <tr>
                                                <td><code class="small">${sourceFull}</code></td>
                                                <td>
                                                    <code class="small"
                                                          data-preview
                                                          data-index="${idx}"
                                                          data-source-path="${img.target_image}">
                                                        ${targetFull}
                                                    </code>
                                                </td>
                                                <td>
                                                    <input type="text" class="form-control form-control-sm override-input"
                                                           data-index="${idx}" placeholder="image name"
                                                           value="${override}">
                                                </td>
                                            </tr>
                                        `;
                                    }).join('')}
                                </tbody>
                            </table>
                        </div>
                    </div>
                    <div class="card-footer text-end">
                        <div class="d-flex">
                            <a href="#/copy-jobs/${job.job_id}" class="btn btn-link">Cancel</a>
                            <button type="button" class="btn btn-success ms-auto" id="release-create">
                                <i class="ti ti-rocket"></i>
                                Create Release
                            </button>
                        </div>
                    </div>
                </div>
            `;

            document.getElementById('release-target-registry').addEventListener('change', (e) => {
                state.targetRegistryId = e.target.value;
                updatePreview();
            });
            document.getElementById('release-id').addEventListener('input', (e) => {
                state.releaseId = e.target.value;
                updatePreview();
            });
            document.getElementById('release-notes').addEventListener('input', (e) => {
                state.notes = e.target.value;
            });

            document.querySelectorAll('.rename-find').forEach(input => {
                input.addEventListener('input', (e) => {
                    const idx = parseInt(e.target.getAttribute('data-index'), 10);
                    state.renameRules[idx].find = e.target.value;
                    updatePreview();
                });
            });
            document.querySelectorAll('.rename-replace').forEach(input => {
                input.addEventListener('input', (e) => {
                    const idx = parseInt(e.target.getAttribute('data-index'), 10);
                    state.renameRules[idx].replace = e.target.value;
                    updatePreview();
                });
            });
            document.querySelectorAll('.rename-remove').forEach(btn => {
                btn.addEventListener('click', () => {
                    const idx = parseInt(btn.getAttribute('data-index'), 10);
                    state.renameRules.splice(idx, 1);
                    render();
                });
            });
            document.getElementById('rename-add').addEventListener('click', () => {
                state.renameRules.push({ find: '', replace: '' });
                render();
            });
            document.querySelectorAll('.override-input').forEach(input => {
                input.addEventListener('input', (e) => {
                    const idx = parseInt(e.target.getAttribute('data-index'), 10);
                    state.overrides[idx].override_name = e.target.value;
                    updatePreview();
                });
            });

            document.getElementById('release-create').addEventListener('click', async () => {
                if (!state.targetRegistryId) {
                    getApp().showError('Please select target registry');
                    return;
                }
                if (!state.releaseId.trim()) {
                    getApp().showError('Release ID cannot be empty');
                    return;
                }

                const payload = {
                    source_copy_job_id: job.job_id,
                    target_registry_id: state.targetRegistryId,
                    release_id: state.releaseId,
                    notes: state.notes || null,
                    rename_rules: state.renameRules.filter(r => r.find),
                    overrides: state.overrides.filter(o => o.override_name),
                };

                try {
                    const response = await api.startReleaseCopyJob(payload);
                    getApp().showSuccess('Release copy job started');
                    router.navigate(`/copy-jobs/${response.job_id}`);
                } catch (error) {
                    getApp().showError(error.message);
                }
            });
        };

        const updatePreview = () => {
            const targetRegistry = registries.find(r => r.id === state.targetRegistryId);
            const targetBase = targetRegistry?.base_url || '';
            document.querySelectorAll('[data-preview]').forEach(el => {
                const idx = parseInt(el.getAttribute('data-index'), 10);
                const sourcePath = el.getAttribute('data-source-path') || '';
                const renamed = applyRules(sourcePath);
                const override = state.overrides[idx]?.override_name || '';
                const finalPath = applyOverride(renamed, override);
                const targetFull = targetBase
                    ? `${targetBase}/${finalPath}:${state.releaseId || '<release_id>'}`
                    : '-';
                el.textContent = targetFull;
            });
        };

        render();

    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load form: ${error.message}
            </div>
        `;
    }
});

// ==================== COPY OPERATIONS ROUTES ====================

// Start Copy Job
router.on('/bundles/:id/versions/:version/copy', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const [bundle, mappings] = await Promise.all([
            api.getBundle(params.id),
            api.getImageMappings(params.id, params.version),
        ]);

        content.innerHTML = `
            <div class="row mb-3">
                <div class="col">
                    <a href="#/bundles/${params.id}/versions/${params.version}" class="btn btn-ghost-secondary">
                        <i class="ti ti-arrow-left"></i>
                        Back to Version
                    </a>
                </div>
            </div>

            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">Start Copy Job</h3>
                </div>
                <div class="card-body">
                    <p>Start copying images for <strong>${bundle.name} v${params.version}</strong></p>

                    <div class="alert alert-info">
                        <i class="ti ti-info-circle"></i>
                        This will copy <strong>${mappings.length} images</strong> from source to target registry.
                    </div>

                    <div class="mb-3">
                        <label class="form-label required">Target Tag</label>
                        <input type="text" class="form-control" id="target-tag"
                               placeholder="2026.02.02.01" required>
                        <small class="form-hint">Tag to use for all target images</small>
                    </div>

                    <div class="list-group mb-3">
                        <div class="list-group-item">
                            <strong>Images to copy:</strong>
                        </div>
                        ${mappings.slice(0, 5).map(m => `
                            <div class="list-group-item">
                                <code class="small">${m.source_image}:${m.source_tag}</code>
                                <i class="ti ti-arrow-right mx-2"></i>
                                <code class="small">${m.target_image}:<span class="text-primary">[tag]</span></code>
                            </div>
                        `).join('')}
                        ${mappings.length > 5 ? `
                            <div class="list-group-item text-secondary">
                                ... and ${mappings.length - 5} more
                            </div>
                        ` : ''}
                    </div>

                    <button type="button" class="btn btn-primary w-100" id="start-copy-btn">
                        <i class="ti ti-copy"></i>
                        Start Copy Job
                    </button>
                </div>
            </div>
        `;

        document.getElementById('start-copy-btn').addEventListener('click', async () => {
            const targetTag = document.getElementById('target-tag').value;
            if (!targetTag) {
                getApp().showError('Please enter a target tag');
                return;
            }

            const btn = document.getElementById('start-copy-btn');
            btn.disabled = true;
            btn.innerHTML = '<span class="spinner-border spinner-border-sm me-2"></span>Starting...';

            try {
                const response = await api.startCopyJob(params.id, params.version, targetTag);
                getApp().showSuccess('Copy job started successfully');
                router.navigate(`/copy-jobs/${response.job_id}`);
            } catch (error) {
                getApp().showError(error.message);
                btn.disabled = false;
                btn.innerHTML = '<i class="ti ti-copy"></i> Start Copy Job';
            }
        });

    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load copy form: ${error.message}
            </div>
        `;
    }
});

// Copy Job Monitor
router.on('/copy-jobs/:jobId', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    let eventSource = null;
    let logSource = null;
    const logLines = [];
    const apiBase = `${window.BASE_PATH || ''}/api/v1`;

    try {
        // Initial status + images
        const [initialStatus, initialImages] = await Promise.all([
            api.getCopyJobStatus(params.jobId),
            api.getCopyJobImages(params.jobId),
        ]);

        const renderLogs = () => {
            const logEl = document.getElementById('copy-job-log');
            if (!logEl) return;
            logEl.textContent = logLines.join('\n');
            logEl.scrollTop = logEl.scrollHeight;
        };

        const renderJobStatus = (status, images = []) => {
            const progress = status.total_images > 0
                ? ((status.copied_images + status.failed_images) / status.total_images * 100).toFixed(0)
                : 0;

            const isComplete = status.status === 'success' || status.status === 'failed';
            const failedImages = images.filter(img => img.copy_status === 'failed');

            content.innerHTML = `
                <style>
                    .terminal-shell { background: #0e0f12; border-radius: 10px; overflow: hidden; border: 1px solid #1f2430; }
                    .terminal-header { display: flex; align-items: center; gap: 6px; padding: 8px 10px; background: #1b1f2a; }
                    .terminal-dot { width: 10px; height: 10px; border-radius: 50%; display: inline-block; }
                    .terminal-dot.red { background: #ff5f57; }
                    .terminal-dot.yellow { background: #febc2e; }
                    .terminal-dot.green { background: #28c840; }
                    .terminal-body { max-height: 320px; overflow: auto; margin: 0; padding: 12px; color: #c9d1d9; font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace; font-size: 12px; line-height: 1.45; white-space: pre-wrap; }
                </style>
                <div class="card">
                    <div class="card-header">
                        <h3 class="card-title">Copy Job Monitor</h3>
                        <div class="card-subtitle">Job ID: ${status.job_id}</div>
                    </div>
                    <div class="card-body">
                        <div class="row mb-4">
                            <div class="col-md-3">
                                <div class="text-center">
                                    <div class="h1 text-blue">${status.total_images}</div>
                                    <div class="text-secondary">Total Images</div>
                                </div>
                            </div>
                            <div class="col-md-3">
                                <div class="text-center">
                                    <div class="h1 text-success">${status.copied_images}</div>
                                    <div class="text-secondary">Copied</div>
                                </div>
                            </div>
                            <div class="col-md-3">
                                <div class="text-center">
                                    <div class="h1 text-danger">${status.failed_images}</div>
                                    <div class="text-secondary">Failed</div>
                                </div>
                            </div>
                            <div class="col-md-3">
                                <div class="text-center">
                                    <div class="h1">${progress}%</div>
                                    <div class="text-secondary">Progress</div>
                                </div>
                            </div>
                        </div>

                        <div class="mb-3">
                            <div class="d-flex justify-content-between mb-1">
                                <span>Overall Progress</span>
                                <span>${status.copied_images + status.failed_images} / ${status.total_images}</span>
                            </div>
                            <div class="progress">
                                <div class="progress-bar bg-success" style="width: ${(status.copied_images / status.total_images * 100).toFixed(0)}%"></div>
                                <div class="progress-bar bg-danger" style="width: ${(status.failed_images / status.total_images * 100).toFixed(0)}%"></div>
                            </div>
                        </div>

                        <div class="alert ${
                            status.status === 'success' ? 'alert-success' :
                            status.status === 'failed' ? 'alert-warning' :
                            status.status === 'in_progress' ? 'alert-info pulse' :
                            'alert-secondary'
                        }">
                            <div class="d-flex align-items-center">
                                <i class="ti ${
                                    status.status === 'success' ? 'ti-check' :
                                    status.status === 'failed' ? 'ti-x' :
                                    status.status === 'in_progress' ? 'ti-loader' :
                                    'ti-info-circle'
                                } me-2"></i>
                                <div>
                                    <strong>Status: ${status.status.replace('_', ' ').toUpperCase()}</strong>
                                    ${status.current_image ? `
                                        <div class="text-secondary small mt-1">
                                            Currently copying: <code>${status.current_image}</code>
                                        </div>
                                    ` : ''}
                                </div>
                            </div>
                        </div>

                        ${isComplete ? `
                            <div class="d-grid gap-2">
                                ${status.status === 'success' && !status.is_release_job ? `
                                    <a href="#/releases/new?copy_job_id=${status.job_id}" class="btn btn-success">
                                        <i class="ti ti-rocket"></i>
                                        Create Release
                                    </a>
                                ` : ''}
                                <a href="#/copy-jobs" class="btn btn-outline-secondary">
                                    <i class="ti ti-list"></i>
                                    Back to Copy Jobs
                                </a>
                                <a href="#/bundles/${status.bundle_id}/versions/${status.version}" class="btn btn-primary">
                                    <i class="ti ti-arrow-left"></i>
                                    Back to Bundle Version
                                </a>
                            </div>
                        ` : status.status === 'pending' ? `
                            <div class="d-grid gap-2">
                                <button class="btn btn-primary" id="start-copy-job">
                                    <i class="ti ti-play"></i>
                                    Start Copy Job
                                </button>
                                <a href="#/copy-jobs" class="btn btn-outline-secondary">
                                    <i class="ti ti-list"></i>
                                    Back to Copy Jobs
                                </a>
                            </div>
                        ` : ''}
                    </div>
                </div>

                <div class="card mt-3">
                    <div class="card-header">
                        <h3 class="card-title">Live Logs</h3>
                    </div>
                    <div class="card-body">
                        <div class="terminal-shell">
                            <div class="terminal-header">
                                <span class="terminal-dot red"></span>
                                <span class="terminal-dot yellow"></span>
                                <span class="terminal-dot green"></span>
                            </div>
                            <pre id="copy-job-log" class="terminal-body"></pre>
                        </div>
                    </div>
                </div>

                ${failedImages.length > 0 ? `
                <div class="card mt-3">
                    <div class="card-header">
                        <h3 class="card-title">Failed Images</h3>
                    </div>
                    <div class="table-responsive">
                        <table class="table table-vcenter card-table">
                            <thead>
                                <tr>
                                    <th>Source</th>
                                    <th>Target</th>
                                    <th>Error</th>
                                </tr>
                            </thead>
                            <tbody>
                                ${failedImages.map(img => `
                                    <tr>
                                        <td>
                                            <div><code class="small">${img.source_image}:${img.source_tag}</code></div>
                                        </td>
                                        <td>
                                            <div><code class="small">${img.target_image}:${img.target_tag}</code></div>
                                        </td>
                                        <td>
                                            <div class="text-danger small" style="max-width: 520px; white-space: normal;">
                                                ${img.error_message || 'Unknown error'}
                                            </div>
                                        </td>
                                    </tr>
                                `).join('')}
                            </tbody>
                        </table>
                    </div>
                </div>
                ` : ''}
            `;

            renderLogs();
        };

        // Initial render
        renderJobStatus(initialStatus, initialImages);

        const attachStartHandler = () => {
            const startBtn = document.getElementById('start-copy-job');
            if (!startBtn) return;
            startBtn.addEventListener('click', async () => {
                try {
                    startBtn.disabled = true;
                    startBtn.innerHTML = '<span class="spinner-border spinner-border-sm me-2"></span>Starting...';
                    await api.startCopyJob(params.jobId);
                    const refreshed = await api.getCopyJobStatus(params.jobId);
                    renderJobStatus(refreshed, initialImages);
                    startLogStream();
                } catch (error) {
                    getApp().showError(error.message);
                    startBtn.disabled = false;
                    startBtn.innerHTML = '<i class="ti ti-play"></i> Start Copy Job';
                }
            });
        };
        attachStartHandler();

        const startLogStream = () => {
            if (logSource) {
                logSource.close();
            }
            logSource = new EventSource(`${apiBase}/copy/jobs/${params.jobId}/logs`);
            logSource.onmessage = (event) => {
                if (!event.data) return;
                logLines.push(event.data);
                if (logLines.length > 1000) {
                    logLines.shift();
                }
                renderLogs();
            };
            logSource.addEventListener('log-end', (event) => {
                if (event?.data) {
                    logLines.push(event.data);
                    renderLogs();
                }
                logSource.close();
            });
            logSource.onerror = (error) => {
                console.error('Log SSE error:', error);
            };
        };

        if (initialStatus.status !== 'pending') {
            startLogStream();
        }

        // Start SSE stream if not complete
        if (initialStatus.status !== 'success' && initialStatus.status !== 'failed') {
            eventSource = api.createCopyJobStream(
                params.jobId,
                (data) => {
                    renderJobStatus(data, initialImages);
                    attachStartHandler();
                },
                (error) => {
                    console.error('SSE error:', error);
                    getApp().showError('Connection lost');
                },
                (data) => {
                    api.getCopyJobImages(params.jobId).then((images) => {
                        renderJobStatus(data, images);
                        attachStartHandler();
                    }).catch(() => {
                        renderJobStatus(data, initialImages);
                        attachStartHandler();
                    });
                    if (data.failed_images === 0 && data.status === 'success') {
                        getApp().showSuccess('Copy job completed successfully!');
                    } else if (data.status === 'failed') {
                        getApp().showWarning(`Copy job completed with ${data.failed_images} errors`);
                    }
                }
            );
        }

    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load copy job: ${error.message}
            </div>
        `;
    }

    // Cleanup on route change
    window.addEventListener('hashchange', () => {
        if (eventSource) {
            eventSource.close();
        }
        if (logSource) {
            logSource.close();
        }
    }, { once: true });
});

// Copy Jobs List
router.on('/copy-jobs', async () => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const jobs = await api.getCopyJobs();

        const renderJobs = (rows) => `
            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">Copy Jobs</h3>
                    <div class="card-actions">
                        <a href="#/bundles" class="btn btn-primary">
                            <i class="ti ti-package"></i>
                            New Copy Job
                        </a>
                    </div>
                </div>
                <div class="card-body">
                    <div class="row g-2 align-items-end">
                        <div class="col-sm-4">
                            <label class="form-label">Status</label>
                            <select class="form-select" id="copy-jobs-status">
                                <option value="">All</option>
                                <option value="in_progress">in_progress</option>
                                <option value="pending">pending</option>
                                <option value="success">success</option>
                                <option value="failed">failed</option>
                            </select>
                        </div>
                        <div class="col-sm-8">
                            <label class="form-label">Search</label>
                            <input class="form-control" id="copy-jobs-search" placeholder="Bundle name or job id">
                        </div>
                    </div>
                </div>
                <div class="table-responsive">
                    <table class="table table-vcenter card-table">
                        <thead>
                            <tr>
                                <th>Job ID</th>
                                <th>Bundle</th>
                                <th>Version</th>
                                <th>Target Tag</th>
                                <th>Status</th>
                                <th>Started</th>
                            </tr>
                        </thead>
                        <tbody>
                            ${rows.length === 0 ? `
                                <tr>
                                    <td colspan="6" class="text-center text-secondary py-5">
                                        No copy jobs yet. Start one from a bundle version.
                                    </td>
                                </tr>
                            ` : rows.map(job => `
                                <tr>
                                    <td><a href="#/copy-jobs/${job.job_id}"><code class="small">${job.job_id}</code></a></td>
                                    <td>
                                        <div><a href="#/bundles/${job.bundle_id}">${job.bundle_name}</a></div>
                                        <div class="text-secondary small"><code class="small">${job.bundle_id}</code></div>
                                    </td>
                                    <td>
                                        <span class="badge bg-blue text-blue-fg">v${job.version}</span>
                                        ${job.is_release_job ? `
                                            <span class="badge bg-purple-lt text-purple-fg ms-2">release</span>
                                        ` : ''}
                                    </td>
                                    <td><span class="badge bg-azure-lt">${job.target_tag}</span></td>
                                    <td>
                                        <span class="badge ${
                                            job.status === 'success' ? 'bg-success text-success-fg' :
                                            job.status === 'failed' ? 'bg-danger text-danger-fg' :
                                            job.status === 'in_progress' ? 'bg-info text-info-fg' :
                                            'bg-secondary text-secondary-fg'
                                        }">${job.status}</span>
                                    </td>
                                    <td>${new Date(job.started_at).toLocaleString('cs-CZ')}</td>
                                </tr>
                            `).join('')}
                        </tbody>
                    </table>
                </div>
            </div>
        `;

        content.innerHTML = renderJobs(jobs);
        const statusEl = document.getElementById('copy-jobs-status');
        const searchEl = document.getElementById('copy-jobs-search');

        const applyFilters = () => {
            const status = statusEl.value;
            const q = searchEl.value.trim().toLowerCase();
            const filtered = jobs.filter(job => {
                const statusOk = !status || job.status === status;
                const searchOk = !q || job.bundle_name.toLowerCase().includes(q) || job.job_id.toLowerCase().includes(q);
                return statusOk && searchOk;
            });
            content.innerHTML = renderJobs(filtered);
            document.getElementById('copy-jobs-status').value = status;
            document.getElementById('copy-jobs-search').value = q;
        };

        statusEl.addEventListener('change', applyFilters);
        searchEl.addEventListener('input', applyFilters);
    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load copy jobs: ${error.message}
            </div>
        `;
    }
});

/**
 * Registry list component with filtering
 */
function registryList() {
    const data = window._registryListData || { registries: [], tenantMap: {} };

    return {
        registries: data.registries,
        tenantMap: data.tenantMap,
        searchQuery: '',
        selectedTenant: '',

        get filteredRegistries() {
            return this.registries.filter(reg => {
                // Filter by search query
                if (this.searchQuery) {
                    const query = this.searchQuery.toLowerCase();
                    if (!reg.name.toLowerCase().includes(query) &&
                        !reg.base_url.toLowerCase().includes(query)) {
                        return false;
                    }
                }

                // Filter by tenant
                if (this.selectedTenant && reg.tenant_id !== this.selectedTenant) {
                    return false;
                }

                return true;
            });
        },

        getRegistryTypeIcon(type) {
            const icons = {
                'harbor': 'ti-anchor',
                'docker': 'ti-brand-docker',
                'quay': 'ti-box',
                'gcr': 'ti-brand-google',
                'ecr': 'ti-cloud',
                'acr': 'ti-cloud',
                'generic': 'ti-database'
            };
            return icons[type] || 'ti-database';
        },

        getRegistryRoleBadge(role) {
            const badges = {
                'source': 'bg-info text-info-fg',
                'target': 'bg-warning text-warning-fg',
                'both': 'bg-success text-success-fg'
            };
            return badges[role] || 'bg-secondary text-secondary-fg';
        }
    };
}

console.log('App.js loaded');
