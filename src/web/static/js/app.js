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
                                                    <div class="text-reset d-block">${release.name}</div>
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

        content.innerHTML = `
            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">Tenants</h3>
                    <div class="card-actions">
                        <div class="search-box me-2">
                            <i class="ti ti-search search-icon"></i>
                            <input type="text" class="form-control form-control-sm"
                                   placeholder="Search..." id="tenants-search">
                        </div>
                        <a href="#/tenants/new" class="btn btn-primary btn-sm">
                            <i class="ti ti-plus"></i>
                            New Tenant
                        </a>
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
                                <th class="w-1"></th>
                            </tr>
                        </thead>
                        <tbody>
                            ${tenants.length === 0 ? `
                                <tr>
                                    <td colspan="5" class="text-center text-secondary py-5">
                                        No tenants found. Create your first tenant to get started.
                                    </td>
                                </tr>
                            ` : tenants.map(tenant => `
                                <tr onclick="router.navigate('/tenants/${tenant.id}')" style="cursor: pointer;">
                                    <td><strong>${tenant.name}</strong></td>
                                    <td><span class="badge">${tenant.slug}</span></td>
                                    <td>${tenant.description || '-'}</td>
                                    <td>${new Date(tenant.created_at).toLocaleDateString('cs-CZ')}</td>
                                    <td>
                                        <a href="#/tenants/${tenant.id}" class="btn btn-sm btn-ghost-primary">
                                            View
                                        </a>
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
                                <th class="w-1"></th>
                            </tr>
                        </thead>
                        <tbody>
                            <template x-if="filteredRegistries.length === 0">
                                <tr>
                                    <td colspan="8" class="text-center text-secondary py-5">
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
                                            <strong x-text="reg.name"></strong>
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
                                    <td>
                                        <a :href="'#/registries/' + reg.id" class="btn btn-sm btn-ghost-primary">
                                            View
                                        </a>
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
        const bundles = await api.getBundles();

        content.innerHTML = `
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
                <div class="table-responsive">
                    <table class="table table-vcenter card-table">
                        <thead>
                            <tr>
                                <th>Name</th>
                                <th>Description</th>
                                <th>Current Version</th>
                                <th>Images</th>
                                <th>Created</th>
                                <th class="w-1"></th>
                            </tr>
                        </thead>
                        <tbody>
                            ${bundles.length === 0 ? `
                                <tr>
                                    <td colspan="6" class="text-center text-secondary py-5">
                                        No bundles found. Create your first bundle to get started.
                                    </td>
                                </tr>
                            ` : bundles.map(bundle => `
                                <tr>
                                    <td><strong>${bundle.name}</strong></td>
                                    <td>${bundle.description || '-'}</td>
                                    <td><span class="badge bg-blue text-blue-fg">v${bundle.current_version || 1}</span></td>
                                    <td>${bundle.total_images || 0}</td>
                                    <td>${new Date(bundle.created_at).toLocaleDateString('cs-CZ')}</td>
                                    <td>
                                        <a href="#/bundles/${bundle.id}" class="btn btn-sm btn-ghost-primary">
                                            View
                                        </a>
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
        const bundle = await api.getBundle(params.id);
        const versions = await api.getBundleVersions(params.id);

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
                                    </div>
                                </a>
                            `).join('')}
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
                                <a href="#/bundles/${bundle.id}/copy" class="btn btn-primary">
                                    <i class="ti ti-copy"></i>
                                    Start Copy Job
                                </a>
                                <a href="#/releases/new?bundle_id=${bundle.id}" class="btn btn-success">
                                    <i class="ti ti-rocket"></i>
                                    Create Release
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

// New Bundle Version (must be before the generic version route)
router.on('/bundles/:id/versions/new', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const bundle = await api.getBundle(params.id);

        content.innerHTML = `
            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">Create New Version</h3>
                    <div class="card-subtitle">Bundle: ${bundle.name}</div>
                </div>
                <form id="new-version-form">
                    <div class="card-body">
                        <div class="alert alert-info">
                            <i class="ti ti-info-circle"></i>
                            Creating a new version will copy all image mappings from version ${bundle.current_version}.
                            You can modify them after creation.
                        </div>

                        <div class="mb-3">
                            <label class="form-label">Change Note</label>
                            <textarea class="form-control" name="change_note" rows="3"
                                      placeholder="Describe what changed in this version (optional)"></textarea>
                        </div>

                        <div class="mb-3">
                            <label class="form-label">Created By</label>
                            <input type="text" class="form-control" name="created_by"
                                   placeholder="Your name (optional)">
                        </div>
                    </div>
                    <div class="card-footer text-end">
                        <div class="d-flex">
                            <a href="#/bundles/${bundle.id}" class="btn btn-link">Cancel</a>
                            <button type="submit" class="btn btn-primary ms-auto">
                                <i class="ti ti-plus"></i>
                                Create Version
                            </button>
                        </div>
                    </div>
                </form>
            </div>
        `;

        document.getElementById('new-version-form').addEventListener('submit', async (e) => {
            e.preventDefault();
            const formData = new FormData(e.target);
            const data = {
                change_note: formData.get('change_note') || null,
                created_by: formData.get('created_by') || null,
            };

            try {
                const newVersion = await api.createBundleVersion(bundle.id, data);
                getApp().showSuccess(`Version ${newVersion.version} created successfully`);
                router.navigate(`/bundles/${bundle.id}/versions/${newVersion.version}`);
            } catch (error) {
                getApp().showError('Failed to create version: ' + error.message);
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

// Bundle Version Detail
router.on('/bundles/:id/versions/:version', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const [bundle, version, mappings] = await Promise.all([
            api.getBundle(params.id),
            api.getBundleVersion(params.id, params.version),
            api.getImageMappings(params.id, params.version),
        ]);

        const copiedCount = mappings.filter(m => m.copy_status === 'success').length;
        const failedCount = mappings.filter(m => m.copy_status === 'failed').length;
        const pendingCount = mappings.filter(m => !m.copy_status || m.copy_status === 'pending').length;

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
                    <div class="row">
                        <div class="col-md-3">
                            <div class="text-center">
                                <div class="h1 text-success">${copiedCount}</div>
                                <div class="text-secondary">Copied</div>
                            </div>
                        </div>
                        <div class="col-md-3">
                            <div class="text-center">
                                <div class="h1 text-danger">${failedCount}</div>
                                <div class="text-secondary">Failed</div>
                            </div>
                        </div>
                        <div class="col-md-3">
                            <div class="text-center">
                                <div class="h1 text-warning">${pendingCount}</div>
                                <div class="text-secondary">Pending</div>
                            </div>
                        </div>
                        <div class="col-md-3">
                            <div class="text-center">
                                <div class="h1">${mappings.length}</div>
                                <div class="text-secondary">Total</div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">Image Mappings</h3>
                </div>
                <div class="table-responsive">
                    <table class="table table-vcenter card-table table-sm">
                        <thead>
                            <tr>
                                <th>Source Image</th>
                                <th>Source Tag</th>
                                <th>→</th>
                                <th>Target Image</th>
                                <th>Status</th>
                                <th>SHA256</th>
                            </tr>
                        </thead>
                        <tbody>
                            ${mappings.map(mapping => `
                                <tr>
                                    <td><code class="small">${mapping.source_image}</code></td>
                                    <td><span class="badge">${mapping.source_tag}</span></td>
                                    <td class="text-center"><i class="ti ti-arrow-right"></i></td>
                                    <td><code class="small">${mapping.target_image}</code></td>
                                    <td>
                                        ${mapping.copy_status === 'success' ? '<span class="badge bg-success text-success-fg">Success</span>' :
                                          mapping.copy_status === 'failed' ? '<span class="badge bg-danger text-danger-fg">Failed</span>' :
                                          mapping.copy_status === 'in_progress' ? '<span class="badge bg-info text-info-fg">In Progress</span>' :
                                          '<span class="badge bg-secondary text-secondary-fg">Pending</span>'}
                                    </td>
                                    <td>
                                        ${mapping.target_sha256 ?
                                          `<code class="small">${mapping.target_sha256.substring(0, 12)}...</code>` :
                                          '-'}
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
                                <th>Name</th>
                                <th>Bundle</th>
                                <th>Version</th>
                                <th>Images</th>
                                <th>Created</th>
                                <th class="w-1"></th>
                            </tr>
                        </thead>
                        <tbody>
                            ${releases.length === 0 ? `
                                <tr>
                                    <td colspan="6" class="text-center text-secondary py-5">
                                        No releases yet. Create a release from a bundle version.
                                    </td>
                                </tr>
                            ` : releases.map(release => `
                                <tr>
                                    <td><strong>${release.name}</strong></td>
                                    <td>${release.bundle_name || '-'}</td>
                                    <td><span class="badge bg-blue text-blue-fg">v${release.bundle_version}</span></td>
                                    <td>${release.image_count || 0}</td>
                                    <td>${new Date(release.created_at).toLocaleDateString('cs-CZ')}</td>
                                    <td>
                                        <a href="#/releases/${release.id}" class="btn btn-sm btn-ghost-primary">
                                            View
                                        </a>
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
                        ${release.name}
                    </h3>
                </div>
                <div class="card-body">
                    <dl class="row mb-0">
                        <dt class="col-4">Description:</dt>
                        <dd class="col-8">${release.description || '-'}</dd>

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
        const bundles = await api.getBundles();

        content.innerHTML = `
            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">Create Release</h3>
                </div>
                <form id="release-form">
                    <div class="card-body">
                        <div class="mb-3">
                            <label class="form-label required">Release Name</label>
                            <input type="text" class="form-control" name="name"
                                   placeholder="Production Release 2026.02.02" required>
                        </div>

                        <div class="mb-3">
                            <label class="form-label">Description</label>
                            <textarea class="form-control" name="description" rows="3"
                                      placeholder="Optional description"></textarea>
                        </div>

                        <div class="row">
                            <div class="col-md-6">
                                <div class="mb-3">
                                    <label class="form-label required">Bundle</label>
                                    <select class="form-select" name="bundle_id" id="bundle-select" required>
                                        <option value="">Select bundle...</option>
                                        ${bundles.map(b => `
                                            <option value="${b.id}" ${query.bundle_id === b.id ? 'selected' : ''}>
                                                ${b.name}
                                            </option>
                                        `).join('')}
                                    </select>
                                </div>
                            </div>

                            <div class="col-md-6">
                                <div class="mb-3">
                                    <label class="form-label required">Bundle Version</label>
                                    <select class="form-select" name="bundle_version_id" id="version-select" required>
                                        <option value="">Select bundle first...</option>
                                    </select>
                                </div>
                            </div>
                        </div>

                        <div class="alert alert-info">
                            <i class="ti ti-info-circle"></i>
                            Only bundle versions with all images successfully copied can be released.
                        </div>
                    </div>
                    <div class="card-footer text-end">
                        <div class="d-flex">
                            <a href="#/releases" class="btn btn-link">Cancel</a>
                            <button type="submit" class="btn btn-success ms-auto">
                                <i class="ti ti-rocket"></i>
                                Create Release
                            </button>
                        </div>
                    </div>
                </form>
            </div>
        `;

        // Bundle selection handler
        const bundleSelect = document.getElementById('bundle-select');
        const versionSelect = document.getElementById('version-select');

        bundleSelect.addEventListener('change', async () => {
            const bundleId = bundleSelect.value;
            if (!bundleId) {
                versionSelect.innerHTML = '<option value="">Select bundle first...</option>';
                return;
            }

            try {
                const versions = await api.getBundleVersions(bundleId);
                versionSelect.innerHTML = versions.map(v => `
                    <option value="${v.id}">Version ${v.version}</option>
                `).join('');
            } catch (error) {
                getApp().showError('Failed to load versions');
            }
        });

        // Trigger if pre-selected
        if (query.bundle_id) {
            bundleSelect.dispatchEvent(new Event('change'));
        }

        // Form submit
        document.getElementById('release-form').addEventListener('submit', async (e) => {
            await handleFormSubmit(e, async (data) => {
                const release = await api.createRelease(data);
                getApp().showSuccess('Release created successfully');
                router.navigate(`/releases/${release.id}`);
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

    try {
        // Initial status
        const initialStatus = await api.getCopyJobStatus(params.jobId);

        const renderJobStatus = (status) => {
            const progress = status.total_images > 0
                ? ((status.copied_images + status.failed_images) / status.total_images * 100).toFixed(0)
                : 0;

            const isComplete = status.status === 'completed' || status.status === 'completed_with_errors';

            content.innerHTML = `
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
                            status.status === 'completed' ? 'alert-success' :
                            status.status === 'completed_with_errors' ? 'alert-warning' :
                            status.status === 'in_progress' ? 'alert-info pulse' :
                            'alert-secondary'
                        }">
                            <div class="d-flex align-items-center">
                                <i class="ti ${
                                    status.status === 'completed' ? 'ti-check' :
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
                                <a href="#/bundles/${status.bundle_id}/versions/${status.version}" class="btn btn-primary">
                                    <i class="ti ti-arrow-left"></i>
                                    Back to Bundle Version
                                </a>
                            </div>
                        ` : ''}
                    </div>
                </div>
            `;
        };

        // Initial render
        renderJobStatus(initialStatus);

        // Start SSE stream if not complete
        if (initialStatus.status !== 'completed' && initialStatus.status !== 'completed_with_errors') {
            eventSource = api.createCopyJobStream(
                params.jobId,
                (data) => {
                    renderJobStatus(data);
                },
                (error) => {
                    console.error('SSE error:', error);
                    getApp().showError('Connection lost');
                },
                (data) => {
                    renderJobStatus(data);
                    if (data.failed_images === 0) {
                        getApp().showSuccess('Copy job completed successfully!');
                    } else {
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
    }, { once: true });
});

// Copy Jobs List
router.on('/copy-jobs', async () => {
    document.getElementById('app-content').innerHTML = `
        <div class="card">
            <div class="card-header">
                <h3 class="card-title">Copy Jobs</h3>
            </div>
            <div class="card-body">
                <div class="alert alert-info">
                    <i class="ti ti-info-circle"></i>
                    Copy jobs are started from bundle versions. Visit a bundle version to start a new copy job.
                </div>
                <a href="#/bundles" class="btn btn-primary">
                    <i class="ti ti-package"></i>
                    View Bundles
                </a>
            </div>
        </div>
    `;
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
