/**
 * Hlavní aplikační logika s Alpine.js
 */

// Global helper pro přístup k app komponentě
window.getApp = function() {
    const appElement = document.querySelector('[x-data="app"]');
    return appElement ? Alpine.$data(appElement) : null;
};

function escapeHtml(value) {
    return String(value)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}

function ansiToHtml(line) {
    const ansiRegex = /\x1b\[([0-9;]*)m/g;
    const colors = {
        30: '#111827',
        31: '#ef4444',
        32: '#22c55e',
        33: '#f59e0b',
        34: '#3b82f6',
        35: '#a855f7',
        36: '#06b6d4',
        37: '#e5e7eb',
        90: '#6b7280',
        91: '#f87171',
        92: '#4ade80',
        93: '#fbbf24',
        94: '#60a5fa',
        95: '#c084fc',
        96: '#22d3ee',
        97: '#f9fafb',
    };

    let out = '';
    let lastIndex = 0;
    let currentStyle = '';
    let match;

    while ((match = ansiRegex.exec(line)) !== null) {
        const chunk = line.slice(lastIndex, match.index);
        if (chunk) {
            out += currentStyle ? `<span style="${currentStyle}">${escapeHtml(chunk)}</span>` : escapeHtml(chunk);
        }

        const codes = match[1]
            .split(';')
            .filter(Boolean)
            .map((c) => Number(c));

        if (codes.length === 0) {
            currentStyle = '';
        } else {
            for (const code of codes) {
                if (code === 0) {
                    currentStyle = '';
                } else if (code === 1) {
                    currentStyle = `${currentStyle}font-weight:bold;`;
                } else if (code === 39) {
                    currentStyle = currentStyle.replace(/color:[^;]+;?/g, '');
                } else if (colors[code]) {
                    currentStyle = currentStyle.replace(/color:[^;]+;?/g, '');
                    currentStyle = `${currentStyle}color:${colors[code]};`;
                }
            }
        }

        lastIndex = ansiRegex.lastIndex;
    }

    const tail = line.slice(lastIndex);
    if (tail) {
        out += currentStyle ? `<span style="${currentStyle}">${escapeHtml(tail)}</span>` : escapeHtml(tail);
    }

    return out;
}

function attachEnvironmentColorPreview() {
    const colorInput = document.querySelector('.env-color-input');
    const textInput = document.querySelector('.env-color-text');
    const preview = document.getElementById('env-color-preview');
    if (!colorInput || !textInput || !preview) return;

    const normalize = (value) => {
        const trimmed = value.trim();
        if (!trimmed) return '';
        if (trimmed.startsWith('#')) return trimmed;
        const hex = trimmed.replace(/[^0-9a-fA-F]/g, '');
        if (hex.length === 3 || hex.length === 6) {
            return `#${hex}`;
        }
        return trimmed;
    };

    const updatePreview = (value) => {
        const color = normalize(value);
        if (color) {
            preview.style.background = color;
            preview.style.color = '#fff';
            preview.textContent = color;
        } else {
            preview.style.background = '';
            preview.style.color = '';
            preview.textContent = 'Preview';
        }
    };

    colorInput.addEventListener('input', () => {
        textInput.value = colorInput.value;
        updatePreview(colorInput.value);
    });

    textInput.addEventListener('input', () => {
        const normalized = normalize(textInput.value);
        if (normalized.startsWith('#')) {
            colorInput.value = normalized;
        }
        updatePreview(textInput.value);
    });

    updatePreview(textInput.value || colorInput.value);
}

function attachEnvironmentSlugPreview() {
    const nameInput = document.querySelector('input[name="name"]');
    const slugInput = document.querySelector('input[name="slug"]');
    if (!nameInput || !slugInput) return;

    let manuallyEdited = false;
    slugInput.addEventListener('input', () => {
        manuallyEdited = true;
    });

    nameInput.addEventListener('input', (e) => {
        if (manuallyEdited) return;
        if (typeof slugify === 'function') {
            slugInput.value = slugify(e.target.value);
        }
    });
}

function attachEnvironmentVarHandlers() {
    const mappings = document.getElementById('env-var-mappings');
    const addMappingBtn = document.getElementById('env-var-add');
    const extraVars = document.getElementById('extra-env-vars');
    const addExtraBtn = document.getElementById('extra-var-add');

    const attachRemoveHandlers = () => {
        mappings?.querySelectorAll('.env-var-remove').forEach(btn => {
            btn.addEventListener('click', () => {
                const rows = mappings.querySelectorAll('[data-env-var-index]');
                const row = btn.closest('[data-env-var-index]');
                if (!row) return;
                if (rows.length <= 1) {
                    row.querySelector('.env-var-source').value = '';
                    row.querySelector('.env-var-target').value = '';
                    return;
                }
                row.remove();
            });
        });
        extraVars?.querySelectorAll('.extra-var-remove').forEach(btn => {
            btn.addEventListener('click', () => {
                const rows = extraVars.querySelectorAll('[data-extra-var-index]');
                const row = btn.closest('[data-extra-var-index]');
                if (!row) return;
                if (rows.length <= 1) {
                    row.querySelector('.extra-var-key').value = '';
                    row.querySelector('.extra-var-value').value = '';
                    return;
                }
                row.remove();
            });
        });
    };

    if (addMappingBtn && mappings) {
        addMappingBtn.addEventListener('click', () => {
            const index = mappings.querySelectorAll('[data-env-var-index]').length;
            const row = document.createElement('div');
            row.className = 'row g-2 mb-2';
            row.setAttribute('data-env-var-index', index.toString());
            row.innerHTML = `
                <div class="col-md-5">
                    <input type="text" class="form-control env-var-source" placeholder="SIMPLE_RELEASE_ID">
                </div>
                <div class="col-md-5">
                    <input type="text" class="form-control env-var-target" placeholder="TSM_RELEASE_ID">
                </div>
                <div class="col-md-2">
                    <button type="button" class="btn btn-outline-danger w-100 env-var-remove">
                        <i class="ti ti-trash"></i>
                    </button>
                </div>
            `;
            mappings.insertBefore(row, addMappingBtn);
            attachRemoveHandlers();
        });
    }

    if (addExtraBtn && extraVars) {
        addExtraBtn.addEventListener('click', () => {
            const index = extraVars.querySelectorAll('[data-extra-var-index]').length;
            const row = document.createElement('div');
            row.className = 'row g-2 mb-2';
            row.setAttribute('data-extra-var-index', index.toString());
            row.innerHTML = `
                <div class="col-md-5">
                    <input type="text" class="form-control extra-var-key" placeholder="KEY">
                </div>
                <div class="col-md-5">
                    <input type="text" class="form-control extra-var-value" placeholder="VALUE">
                </div>
                <div class="col-md-2">
                    <button type="button" class="btn btn-outline-danger w-100 extra-var-remove">
                        <i class="ti ti-trash"></i>
                    </button>
                </div>
            `;
            extraVars.insertBefore(row, addExtraBtn);
            attachRemoveHandlers();
        });
    }

    attachRemoveHandlers();
}

document.addEventListener('alpine:init', () => {
    Alpine.data('app', () => ({
        // State
        currentRoute: '/',
        mobileMenuOpen: false,
        loading: false,
        loadingMessage: '',
        toasts: [],


        // Inicializace
        init() {
            console.log('Simple Release Management initialized');

            // Watch for route changes
            this.$watch('currentRoute', (value) => {
                console.log('Route changed to:', value);
            });

            const versionBadge = document.getElementById('app-version');
            if (versionBadge) {
                api.getVersion()
                    .then((res) => {
                        if (res?.version) {
                            versionBadge.textContent = `v${res.version}`;
                        }
                    })
                    .catch(() => {});
            }

            // no keyboard shortcuts
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
            const key = (role || '').toString().trim().toLowerCase();
            const badgeMap = {
                'source': 'bg-blue text-blue-fg',
                'target': 'bg-green text-green-fg',
                'both': 'bg-purple text-purple-fg',
            };
            return badgeMap[key] || 'bg-secondary text-secondary-fg';
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
        const [tenants, bundles, releases, registries, copyJobs, deployJobs, gitRepos] = await Promise.all([
            api.getTenants(),
            api.getBundles(),
            api.getReleases(),
            api.getRegistries(),
            api.getCopyJobs(),
            api.getDeployments(),
            api.getGitRepos(),
        ]);

        const environmentsByTenant = await Promise.all(
            tenants.map(async tenant => ({
                tenant_id: tenant.id,
                environments: await api.getEnvironments(tenant.id),
            }))
        );
        const environments = environmentsByTenant.flatMap(entry => entry.environments);

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

        const registryMap = {};
        registries.forEach(r => {
            registryMap[r.id] = r;
        });

        const recentCopyJobs = copyJobs
            .filter(job => job.started_at)
            .sort((a, b) => new Date(b.started_at) - new Date(a.started_at))
            .slice(0, 5);

        const recentDeployJobs = deployJobs
            .filter(job => job.started_at)
            .sort((a, b) => new Date(b.started_at) - new Date(a.started_at))
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
                                    <div class="text-secondary">Image Releases</div>
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

            <div class="row row-deck row-cards mb-4">
                <div class="col-sm-6 col-lg-3">
                    <a href="#/copy-jobs" class="card card-sm card-link">
                        <div class="card-body">
                            <div class="row align-items-center">
                                <div class="col-auto">
                                    <span class="bg-azure text-white avatar">
                                        <i class="ti ti-copy"></i>
                                    </span>
                                </div>
                                <div class="col">
                                    <div class="font-weight-medium">${copyJobs.length}</div>
                                    <div class="text-secondary">Copy Jobs</div>
                                </div>
                            </div>
                        </div>
                    </a>
                </div>

                <div class="col-sm-6 col-lg-3">
                    <a href="#/deployments" class="card card-sm card-link">
                        <div class="card-body">
                            <div class="row align-items-center">
                                <div class="col-auto">
                                    <span class="bg-indigo text-white avatar">
                                        <i class="ti ti-rocket"></i>
                                    </span>
                                </div>
                                <div class="col">
                                    <div class="font-weight-medium">${deployJobs.length}</div>
                                    <div class="text-secondary">Deploy Jobs</div>
                                </div>
                            </div>
                        </div>
                    </a>
                </div>

                <div class="col-sm-6 col-lg-3">
                    <a href="#/git-repos" class="card card-sm card-link">
                        <div class="card-body">
                            <div class="row align-items-center">
                                <div class="col-auto">
                                    <span class="bg-orange text-white avatar">
                                        <i class="ti ti-brand-git"></i>
                                    </span>
                                </div>
                                <div class="col">
                                    <div class="font-weight-medium">${gitRepos.length}</div>
                                    <div class="text-secondary">Git Repos</div>
                                </div>
                            </div>
                        </div>
                    </a>
                </div>

                <div class="col-sm-6 col-lg-3">
                    <a href="#/tenants" class="card card-sm card-link">
                        <div class="card-body">
                            <div class="row align-items-center">
                                <div class="col-auto">
                                    <span class="bg-teal text-white avatar">
                                        <i class="ti ti-planet"></i>
                                    </span>
                                </div>
                                <div class="col">
                                    <div class="font-weight-medium">${environments.length}</div>
                                    <div class="text-secondary">Environments</div>
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
                                        View Image Releases
                                    </a>
                                </div>
                                <div class="col-6 col-md-4">
                                    <a href="#/copy-jobs" class="btn btn-outline-cyan w-100">
                                        <i class="ti ti-copy me-2"></i>
                                        Copy Jobs
                                    </a>
                                </div>
                                <div class="col-6 col-md-4">
                                    <a href="#/deployments" class="btn btn-outline-indigo w-100">
                                        <i class="ti ti-rocket me-2"></i>
                                        Deployments
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
                            <h3 class="card-title">Recent Image Releases</h3>
                            <div class="card-actions">
                                <a href="#/releases" class="btn btn-sm btn-outline-primary">View All</a>
                            </div>
                        </div>
                        <div class="card-body p-0">
                            ${recentReleases.length === 0 ? `
                                <div class="empty p-4">
                                    <p class="empty-title">No image releases yet</p>
                                    <p class="empty-subtitle text-secondary">Create a bundle and release images</p>
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

            <div class="row mt-3">
                <div class="col-md-6">
                    <div class="card">
                        <div class="card-header">
                            <h3 class="card-title">
                                <i class="ti ti-copy me-2"></i>
                                Recent Copy Jobs
                            </h3>
                            <div class="card-actions">
                                <a href="#/copy-jobs" class="btn btn-sm btn-outline-primary">View All</a>
                            </div>
                        </div>
                        <div class="card-body p-0">
                            ${recentCopyJobs.length === 0 ? `
                                <div class="empty p-4">
                                    <p class="empty-title">No copy jobs yet</p>
                                    <p class="empty-subtitle text-secondary">Start one from a bundle version</p>
                                </div>
                            ` : `
                                <div class="list-group list-group-flush">
                                    ${recentCopyJobs.map(job => {
                                        const sourceReg = job.source_registry_id ? registryMap[job.source_registry_id] : null;
                                        const targetReg = job.target_registry_id ? registryMap[job.target_registry_id] : null;
                                        const sourceText = sourceReg?.base_url
                                            ? `${sourceReg.base_url}${sourceReg.default_project_path ? ` (path: ${sourceReg.default_project_path})` : ''}`
                                            : '-';
                                        const targetText = targetReg?.base_url
                                            ? `${targetReg.base_url}${targetReg.default_project_path ? ` (path: ${targetReg.default_project_path})` : ''}`
                                            : '-';
                                        return `
                                        <a href="#/copy-jobs/${job.job_id}" class="list-group-item list-group-item-action">
                                            <div class="row align-items-center">
                                                <div class="col text-truncate">
                                                    <div class="text-reset d-block">${job.bundle_name}</div>
                                                    <div class="text-secondary text-truncate mt-n1">
                                                        ${job.target_tag}
                                                    </div>
                                                    <div class="text-secondary small mt-1">
                                                        Source: <code class="small">${sourceText}</code>
                                                    </div>
                                                    <div class="text-secondary small">
                                                        Target: <code class="small">${targetText}</code>
                                                    </div>
                                                </div>
                                                <div class="col-auto">
                                                    <span class="badge ${
                                                        job.status === 'success' ? 'bg-success text-success-fg' :
                                                        job.status === 'failed' ? 'bg-danger text-danger-fg' :
                                                        job.status === 'in_progress' ? 'bg-info text-info-fg' :
                                                        'bg-secondary text-secondary-fg'
                                                    }">${job.status}</span>
                                                </div>
                                            </div>
                                        </a>
                                    `;
                                    }).join('')}
                                </div>
                            `}
                        </div>
                    </div>
                </div>

                <div class="col-md-6">
                    <div class="card">
                        <div class="card-header">
                            <h3 class="card-title">
                                <i class="ti ti-rocket me-2"></i>
                                Recent Deploy Jobs
                            </h3>
                            <div class="card-actions">
                                <a href="#/deployments" class="btn btn-sm btn-outline-primary">View All</a>
                            </div>
                        </div>
                        <div class="card-body p-0">
                            ${recentDeployJobs.length === 0 ? `
                                <div class="empty p-4">
                                    <p class="empty-title">No deployments yet</p>
                                    <p class="empty-subtitle text-secondary">Run a deploy job from a release</p>
                                </div>
                            ` : `
                                <div class="list-group list-group-flush">
                                    ${recentDeployJobs.map(job => `
                                        <a href="#/deploy-jobs/${job.id}" class="list-group-item list-group-item-action">
                                            <div class="row align-items-center">
                                                <div class="col text-truncate">
                                                    <div class="text-reset d-block">${job.release_id}</div>
                                                    <div class="text-secondary text-truncate mt-n1">
                                                        ${job.target_name} (${job.env_name})
                                                    </div>
                                                </div>
                                                <div class="col-auto">
                                                    <span class="badge ${
                                                        job.status === 'success' ? 'bg-success text-success-fg' :
                                                        job.status === 'failed' ? 'bg-danger text-danger-fg' :
                                                        job.status === 'in_progress' ? 'bg-info text-info-fg' :
                                                        'bg-secondary text-secondary-fg'
                                                    }">${job.status}</span>
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

        window._exportMappings = [];

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
        const [tenants, registries, bundles] = await Promise.all([
            api.getTenants(),
            api.getRegistries(),
            api.getBundles(),
        ]);

        const registriesByTenant = new Map();
        registries.forEach(reg => {
            if (!registriesByTenant.has(reg.tenant_id)) registriesByTenant.set(reg.tenant_id, []);
            registriesByTenant.get(reg.tenant_id).push(reg);
        });

        const bundlesByTenant = new Map();
        bundles.forEach(bundle => {
            if (!bundlesByTenant.has(bundle.tenant_id)) bundlesByTenant.set(bundle.tenant_id, []);
            bundlesByTenant.get(bundle.tenant_id).push(bundle);
        });

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
                                <th>Registries</th>
                                <th>Bundles</th>
                                <th>Created</th>
                            </tr>
                        </thead>
                        <tbody>
                            ${rows.length === 0 ? `
                                <tr>
                                    <td colspan="5" class="text-center text-secondary py-5">
                                        No tenants found. Create your first tenant to get started.
                                    </td>
                                </tr>
                            ` : rows.map(tenant => `
                                ${(() => {
                                    const regs = registriesByTenant.get(tenant.id) || [];
                                    const bnds = bundlesByTenant.get(tenant.id) || [];
                                    const regList = regs.length === 0
                                        ? '<span class="text-secondary">-</span>'
                                        : regs.map(reg => `
                                            <div class="small">
                                                <a href="#/registries/${reg.id}">${reg.name}</a>
                                                <span class="text-secondary">•</span>
                                                <span class="text-secondary">${reg.username || '-'}</span>
                                                <span class="text-secondary">•</span>
                                                <code class="small">${reg.base_url}${reg.default_project_path ? ` (path: ${reg.default_project_path})` : ''}</code>
                                            </div>
                                        `).join('');
                                    const bundleList = bnds.length === 0
                                        ? '<span class="text-secondary">-</span>'
                                        : bnds.map(bundle => `
                                            <div class="small">
                                                <a href="#/bundles/${bundle.id}">${bundle.name}</a>
                                                <span class="text-secondary">•</span>
                                                <span class="badge bg-blue text-blue-fg">v${bundle.current_version || 1}</span>
                                                <span class="text-secondary">•</span>
                                                <span class="text-secondary">${bundle.image_count || '-'}</span>
                                            </div>
                                        `).join('');
                                    return `
                                <tr>
                                    <td><a href="#/tenants/${tenant.id}"><strong>${tenant.name}</strong></a></td>
                                    <td><span class="badge">${tenant.slug}</span></td>
                                    <td>${regList}</td>
                                    <td>${bundleList}</td>
                                    <td>${new Date(tenant.created_at).toLocaleDateString('cs-CZ')}</td>
                                </tr>
                                    `;
                                })()}
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
        const [tenant, registries, bundles, gitRepos, environments, argocdInstances, kubernetesInstances] = await Promise.all([
            api.getTenant(params.id),
            api.getRegistries(params.id),
            api.getBundles(params.id),
            api.getGitRepos(params.id),
            api.getEnvironments(params.id),
            api.getArgocdInstances(params.id),
            api.getKubernetesInstances(params.id),
        ]);
        const argocdAppsByEnv = await Promise.all(
            environments.map(async env => ({
                env,
                apps: await api.getArgocdApps(env.id).catch(() => []),
            }))
        );
        const argocdApps = argocdAppsByEnv.flatMap(entry =>
            entry.apps.map(app => ({
                ...app,
                environment: entry.env,
            }))
        );
        const k8sNamespacesByEnv = await Promise.all(
            environments.map(async env => ({
                env,
                namespaces: await api.getKubernetesNamespaces(env.id).catch(() => []),
            }))
        );
        const k8sNamespaces = k8sNamespacesByEnv.flatMap(entry =>
            entry.namespaces.map(ns => ({
                ...ns,
                environment: entry.env,
            }))
        );

        const gitRepoById = new Map(gitRepos.map(repo => [repo.id, repo]));
        const registryById = new Map(registries.map(reg => [reg.id, reg]));

        content.innerHTML = `
            <style>
                .log-panel pre {
                    margin: 0;
                    max-height: 320px;
                    overflow: auto;
                    font-family: "SFMono-Regular", "Consolas", "Liberation Mono", monospace;
                    font-size: 12px;
                    line-height: 1.4;
                    background: #0b0f14;
                    color: #cfe3f4;
                    padding: 12px;
                    border-radius: 6px;
                    white-space: pre-wrap;
                }
                .terminal-header {
                    display: flex;
                    align-items: center;
                    gap: 6px;
                    padding: 10px 12px;
                    background: #12171d;
                    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
                    border-top-left-radius: 8px;
                    border-top-right-radius: 8px;
                }
                .terminal-dot {
                    width: 10px;
                    height: 10px;
                    border-radius: 999px;
                    display: inline-block;
                }
                .terminal-dot.red { background: #ff5f57; }
                .terminal-dot.yellow { background: #febc2e; }
                .terminal-dot.green { background: #28c840; }
            </style>
            <div class="row mb-3">
                <div class="col">
                    <a href="#/tenants" class="btn btn-ghost-secondary">
                        <i class="ti ti-arrow-left"></i>
                        Back to Tenants
                    </a>
                </div>
            </div>

            <div class="row">
                <div class="col-12">
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

                    <div class="row">
                        <div class="col-md-6">
                            <div class="card mb-3">
                                <div class="card-header">
                                    <h3 class="card-title">Environments</h3>
                                    <div class="card-actions">
                                        <a href="#/environments/new?tenant_id=${tenant.id}" class="btn btn-primary btn-sm">
                                            <i class="ti ti-plus"></i>
                                            Add
                                        </a>
                                    </div>
                                </div>
                                <div class="list-group list-group-flush">
                                    ${environments.length === 0 ? `
                                        <div class="list-group-item text-center text-secondary py-4">
                                            No environments yet
                                        </div>
                                    ` : environments.map(env => {
                                        const envColor = env.color || '';
                                        const sourceRegistry = env.source_registry_id ? registryById.get(env.source_registry_id) : null;
                                        const targetRegistry = env.target_registry_id ? registryById.get(env.target_registry_id) : null;
                                        const envRepo = env.env_repo_id ? gitRepoById.get(env.env_repo_id) : null;
                                        const deployRepo = env.deploy_repo_id ? gitRepoById.get(env.deploy_repo_id) : null;
                                        return `
                                        <a href="#/environments/${env.id}/edit" class="list-group-item list-group-item-action">
                                            <div class="d-flex align-items-start">
                                                <span class="avatar avatar-sm me-2" style="background-color:${envColor || 'transparent'} !important;border:1px solid rgba(98,105,118,0.4);" title="${envColor || 'no color'}"></span>
                                                <div class="flex-fill">
                                                    <div class="d-flex align-items-center gap-2">
                                                        <span class="badge" style="${envColor ? `background:${envColor};color:#fff;` : ''}">${env.name}</span>
                                                        <span class="text-secondary small">${env.slug}</span>
                                                    </div>
                                                    <div class="text-secondary small mt-2">
                                                        <div class="d-flex flex-wrap gap-2">
                                                            <span class="text-secondary small">Source Reg</span>
                                                            <code class="small text-wrap">${sourceRegistry?.base_url || '-'}</code>
                                                            <span class="text-secondary small">path:</span>
                                                            <code class="small text-wrap">${env.source_project_path || '-'}</code>
                                                        </div>
                                                        <div class="d-flex flex-wrap gap-2 mt-1">
                                                            <span class="text-secondary small">Target Reg</span>
                                                            <code class="small text-wrap">${targetRegistry?.base_url || '-'}</code>
                                                            <span class="text-secondary small">path:</span>
                                                            <code class="small text-wrap">${env.target_project_path || '-'}</code>
                                                        </div>
                                                        <div class="d-flex flex-wrap gap-2 mt-1">
                                                            <span class="text-secondary small">Env Git</span>
                                                            <code class="small text-wrap">${envRepo?.repo_url || '-'}</code>
                                                            <span class="text-secondary small">path:</span>
                                                            <code class="small text-wrap">${env.env_repo_path || '-'}</code>
                                                        </div>
                                                        <div class="d-flex flex-wrap gap-2 mt-1">
                                                            <span class="text-secondary small">Deploy Git</span>
                                                            <code class="small text-wrap">${deployRepo?.repo_url || '-'}</code>
                                                            <span class="text-secondary small">path:</span>
                                                            <code class="small text-wrap">${env.deploy_repo_path || '-'}</code>
                                                        </div>
                                                    </div>
                                                </div>
                                            </div>
                                        </a>
                                    `;
                                    }).join('')}
                                </div>
                            </div>
                        </div>

                        <div class="col-md-6">
                            <div class="card mb-3">
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
                                    ` : registries.map(reg => {
                                        const linkedEnvs = environments.filter(env =>
                                            env.source_registry_id === reg.id || env.target_registry_id === reg.id
                                        );
                                        return `
                                        <a href="#/registries/${reg.id}/edit" class="list-group-item list-group-item-action">
                                            <div class="d-flex align-items-start gap-2">
                                                <span class="avatar avatar-sm">
                                                    <i class="ti ${window.Alpine?.$data?.app?.getRegistryTypeIcon(reg.registry_type) || 'ti-database'}"></i>
                                                </span>
                                                <div class="flex-fill">
                                                    <div class="d-flex align-items-center justify-content-between gap-2">
                                                        <div>
                                                            <div class="fw-semibold">${reg.name}</div>
                                                            <div class="text-secondary small">${reg.registry_type}</div>
                                                        </div>
                                                        <span class="badge ${getApp().getRegistryRoleBadge(reg.role) || 'bg-secondary text-secondary-fg'}">${reg.role}</span>
                                                    </div>
                                                    <div class="row g-2 mt-2">
                                                        <div class="col-md-6">
                                                            <div class="text-secondary small">Base URL</div>
                                                            <div><code class="small">${reg.base_url || '-'}</code></div>
                                                            <div class="text-secondary small mt-1">Default project path</div>
                                                            <div><code class="small">${reg.default_project_path || '-'}</code></div>
                                                        </div>
                                                        <div class="col-md-6">
                                                            ${linkedEnvs.length > 0 ? `
                                                                <div class="text-secondary small mb-1">Environment paths</div>
                                                                <div class="d-flex flex-column gap-1">
                                                                    ${linkedEnvs.map(env => {
                                                                        const src = env.source_registry_id === reg.id ? (env.source_project_path || '-') : '-';
                                                                        const trg = env.target_registry_id === reg.id ? (env.target_project_path || '-') : '-';
                                                                        return `
                                                                            <div class="d-flex align-items-center gap-2 small">
                                                                                <span class="badge" style="${env.color ? `background:${env.color};color:#fff;` : ''}">${env.name}</span>
                                                                                <span class="text-secondary">src:</span>
                                                                                <code class="small">${src}</code>
                                                                                <span class="text-secondary">trg:</span>
                                                                                <code class="small">${trg}</code>
                                                                            </div>
                                                                        `;
                                                                    }).join('')}
                                                                </div>
                                                            ` : `
                                                                <div class="text-secondary small">No environments</div>
                                                            `}
                                                        </div>
                                                    </div>
                                                </div>
                                            </div>
                                        </a>
                                    `;
                                    }).join('')}
                                </div>
                            </div>
                        </div>
                    </div>

                    <div class="row">
                        <div class="col-md-6">
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

                        <div class="col-md-6">
                            <div class="card mb-3">
                                <div class="card-header">
                                    <h3 class="card-title">Git Repos</h3>
                                    <div class="card-actions">
                                        <a href="#/git-repos/new?tenant_id=${tenant.id}" class="btn btn-primary btn-sm">
                                            <i class="ti ti-plus"></i>
                                            Add
                                        </a>
                                    </div>
                                </div>
                                <div class="list-group list-group-flush">
                                    ${gitRepos.length === 0 ? `
                                        <div class="list-group-item text-center text-secondary py-4">
                                            No git repositories yet
                                        </div>
                                    ` : gitRepos.map(repo => `
                                        <a href="#/git-repos/${repo.id}/edit" class="list-group-item list-group-item-action">
                                            <div class="d-flex align-items-center">
                                                <div class="flex-fill">
                                                    <div>${repo.name}</div>
                                                    <div class="text-secondary small"><code class="small">${repo.repo_url}</code></div>
                                                </div>
                                                <span class="badge bg-secondary-lt text-secondary-fg">${repo.default_branch || 'main'}</span>
                                            </div>
                                        </a>
                                    `).join('')}
                                </div>
                            </div>
                        </div>
                    </div>

                    <div class="row">
                        <div class="col-md-6">
                            <div class="card mb-3">
                                <div class="card-header">
                                    <h3 class="card-title">ArgoCD Instances</h3>
                                    <div class="card-actions">
                                        <a href="#/argocd/new?tenant_id=${tenant.id}" class="btn btn-primary btn-sm">
                                            <i class="ti ti-plus"></i>
                                            Add
                                        </a>
                                    </div>
                                </div>
                                <div class="list-group list-group-flush">
                                    ${argocdInstances.length === 0 ? `
                                        <div class="list-group-item text-center text-secondary py-4">
                                            No ArgoCD instances yet
                                        </div>
                                    ` : argocdInstances.map(inst => `
                                        <div class="list-group-item">
                                            <div class="d-flex align-items-center justify-content-between">
                                                <div>
                                                    <div class="fw-semibold">${inst.name}</div>
                                                    <div class="text-secondary small"><code class="small">${inst.base_url}</code></div>
                                                </div>
                                                <div class="d-flex align-items-center gap-2">
                                                    <span class="badge ${inst.verify_tls === false ? 'bg-yellow-lt text-yellow-fg' : 'bg-green-lt text-green-fg'}">
                                                        ${inst.verify_tls === false ? 'insecure' : 'tls'}
                                                    </span>
                                                    <a href="#/argocd/${inst.id}/edit" class="btn btn-outline-primary btn-sm">
                                                        Edit
                                                    </a>
                                                </div>
                                            </div>
                                        </div>
                                    `).join('')}
                                </div>
                            </div>
                        </div>
                        <div class="col-md-6">
                            <div class="card mb-3">
                                <div class="card-header">
                                    <h3 class="card-title">ArgoCD Apps</h3>
                                </div>
                                <div class="list-group list-group-flush">
                                    ${argocdApps.length === 0 ? `
                                        <div class="list-group-item text-center text-secondary py-4">
                                            No ArgoCD apps yet
                                        </div>
                                    ` : argocdApps.map(app => {
                                        const env = app.environment;
                                        const inactiveClass = app.is_active ? '' : 'opacity-75';
                                        return `
                                            <div class="list-group-item ${inactiveClass}">
                                                <div class="d-flex align-items-center justify-content-between">
                                                    <div>
                                                        <div>
                                                            ${app.is_active
                                                                ? `<a href="#/argocd-apps/${app.id}" class="fw-semibold text-reset">${app.application_name}</a>`
                                                                : `<span class="fw-semibold text-secondary">${app.application_name}</span>`
                                                            }
                                                        </div>
                                                        <div class="text-secondary small">
                                                            Env:
                                                            <span class="badge ms-1" style="${env?.color ? `background:${env.color};color:#fff;` : ''}">
                                                                ${env?.name || '-'}
                                                            </span>
                                                        </div>
                                                        <div class="text-secondary small">
                                                            Project: <code class="small">${app.project_name || 'default'}</code>
                                                        </div>
                                                    </div>
                                                    <div class="d-flex align-items-center gap-2">
                                                        <span class="badge ${app.is_active ? 'bg-green-lt text-green-fg' : 'bg-yellow-lt text-yellow-fg'}">
                                                            ${app.is_active ? 'active' : 'inactive'}
                                                        </span>
                                                        <a href="#/argocd-apps/${app.id}/edit" class="btn btn-outline-primary btn-sm">
                                                            Edit
                                                        </a>
                                                    </div>
                                                </div>
                                            </div>
                                        `;
                                    }).join('')}
                                </div>
                            </div>
                        </div>
                    </div>

                    <div class="row">
                        <div class="col-md-6">
                            <div class="card mb-3">
                                <div class="card-header">
                                    <h3 class="card-title">Kubernetes Instances</h3>
                                    <div class="card-actions">
                                        <a href="#/kubernetes/new?tenant_id=${tenant.id}" class="btn btn-primary btn-sm">
                                            <i class="ti ti-plus"></i>
                                            Add
                                        </a>
                                    </div>
                                </div>
                                <div class="list-group list-group-flush">
                                    ${kubernetesInstances.length === 0 ? `
                                        <div class="list-group-item text-center text-secondary py-4">
                                            No Kubernetes instances yet
                                        </div>
                                    ` : kubernetesInstances.map(inst => `
                                        <div class="list-group-item">
                                            <div class="d-flex align-items-center justify-content-between">
                                                <div>
                                                    <div class="fw-semibold">${inst.name}</div>
                                                    <div class="text-secondary small"><code class="small">${inst.base_url}</code></div>
                                                </div>
                                                <div class="d-flex align-items-center gap-2">
                                                    <span class="badge ${inst.verify_tls === false ? 'bg-yellow-lt text-yellow-fg' : 'bg-green-lt text-green-fg'}">
                                                        ${inst.verify_tls === false ? 'insecure' : 'tls'}
                                                    </span>
                                                    <a href="#/kubernetes/${inst.id}/edit" class="btn btn-outline-primary btn-sm">
                                                        Edit
                                                    </a>
                                                </div>
                                            </div>
                                        </div>
                                    `).join('')}
                                </div>
                            </div>
                        </div>
                        <div class="col-md-6">
                            <div class="card mb-3">
                                <div class="card-header">
                                    <h3 class="card-title">Kubernetes Namespaces</h3>
                                </div>
                                <div class="list-group list-group-flush">
                                    ${k8sNamespaces.length === 0 ? `
                                        <div class="list-group-item text-center text-secondary py-4">
                                            No Kubernetes namespaces yet
                                        </div>
                                    ` : k8sNamespaces.map(ns => {
                                        const env = ns.environment;
                                        const inactiveClass = ns.is_active ? '' : 'opacity-75';
                                        return `
                                            <div class="list-group-item ${inactiveClass}">
                                                <div class="d-flex align-items-center justify-content-between">
                                                    <div>
                                                        <div>
                                                            ${ns.is_active
                                                                ? `<a href="#/kubernetes-namespaces/${ns.id}" class="fw-semibold text-reset">${ns.namespace}</a>`
                                                                : `<span class="fw-semibold text-secondary">${ns.namespace}</span>`
                                                            }
                                                        </div>
                                                        <div class="text-secondary small">
                                                            Env:
                                                            <span class="badge ms-1" style="${env?.color ? `background:${env.color};color:#fff;` : ''}">
                                                                ${env?.name || '-'}
                                                            </span>
                                                        </div>
                                                        <div class="text-secondary small">
                                                            Instance: <code class="small">${ns.instance_name || '-'}</code>
                                                        </div>
                                                    </div>
                                                    <div class="d-flex align-items-center gap-2">
                                                        <span class="badge ${ns.is_active ? 'bg-green-lt text-green-fg' : 'bg-yellow-lt text-yellow-fg'}">
                                                            ${ns.is_active ? 'active' : 'inactive'}
                                                        </span>
                                                        <a href="#/kubernetes-namespaces/${ns.id}/edit" class="btn btn-outline-primary btn-sm">
                                                            Edit
                                                        </a>
                                                    </div>
                                                </div>
                                            </div>
                                        `;
                                    }).join('')}
                                </div>
                            </div>
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
                                <th>Project Path</th>
                                <th>Username</th>
                                <th>Role</th>
                                <th>Status</th>
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
                                            <a :href="'#/registries/' + reg.id"><strong x-text="reg.name"></strong></a>
                                        </div>
                                    </td>
                                    <td>
                                        <span x-text="tenantMap[reg.tenant_id]?.name || 'Unknown'"></span>
                                    </td>
                                    <td>
                                        <span class="badge bg-azure text-azure-fg" x-text="reg.registry_type"></span>
                                    </td>
                                    <td>
                                        <code class="small" x-text="reg.base_url"></code>
                                    </td>
                                    <td>
                                        <code class="small" x-text="reg.default_project_path || '-'"></code>
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

// ==================== GIT REPOSITORIES ROUTES ====================

router.on('/git-repos', async () => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const [repos, tenants] = await Promise.all([
            api.getGitRepos(),
            api.getTenants(),
        ]);

        const tenantMap = {};
        tenants.forEach(t => tenantMap[t.id] = t);

        const renderRepos = (rows, searchQuery = '', selectedTenant = '') => `
            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">Git Repositories</h3>
                    <div class="card-actions">
                        <a href="#/git-repos/new" class="btn btn-primary">
                            <i class="ti ti-plus"></i>
                            New Git Repo
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
                                <input type="text" class="form-control" placeholder="Search by name or url..."
                                       id="git-repos-search" value="${searchQuery}">
                            </div>
                        </div>
                        <div class="col-md-3">
                            <select class="form-select" id="git-repos-tenant">
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
                    <table class="table table-vcenter card-table table-hover">
                        <thead>
                            <tr>
                                <th>Name</th>
                                <th>Tenant</th>
                                <th>URL</th>
                                <th>Branch</th>
                            </tr>
                        </thead>
                        <tbody>
                            ${rows.length === 0 ? `
                                <tr>
                                    <td colspan="4" class="text-center text-secondary py-5">
                                        No git repositories yet.
                                    </td>
                                </tr>
                            ` : rows.map(repo => `
                                <tr>
                                    <td><a href="#/git-repos/${repo.id}/edit"><strong>${repo.name}</strong></a></td>
                                    <td>${tenantMap[repo.tenant_id]?.name || 'Unknown'}</td>
                                    <td><code class="small">${repo.repo_url}</code></td>
                                    <td><span class="badge bg-secondary-lt text-secondary-fg">${repo.default_branch || 'main'}</span></td>
                                </tr>
                            `).join('')}
                        </tbody>
                    </table>
                </div>
            </div>
        `;

        content.innerHTML = renderRepos(repos);
        const searchEl = document.getElementById('git-repos-search');

        const applyFilters = () => {
            const q = searchEl.value.trim().toLowerCase();
            const tenantId = document.getElementById('git-repos-tenant').value;
            const filtered = repos.filter(r => {
                const nameOk = !q || r.name.toLowerCase().includes(q) || r.repo_url.toLowerCase().includes(q);
                const tenantOk = !tenantId || r.tenant_id === tenantId;
                return nameOk && tenantOk;
            });
            content.innerHTML = renderRepos(filtered, q, tenantId);
            document.getElementById('git-repos-search').addEventListener('input', applyFilters);
            document.getElementById('git-repos-tenant').addEventListener('change', applyFilters);
        };

        searchEl.addEventListener('input', applyFilters);
        document.getElementById('git-repos-tenant').addEventListener('change', applyFilters);
    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load git repositories: ${error.message}
            </div>
        `;
    }
});

router.on('/git-repos/new', async (params, query) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const tenants = await api.getTenants();
        content.innerHTML = createGitRepoForm(null, tenants);

        if (query.tenant_id) {
            const select = document.querySelector('select[name="tenant_id"]');
            if (select) {
                select.value = query.tenant_id;
                select.disabled = true;
                const hidden = document.createElement('input');
                hidden.type = 'hidden';
                hidden.name = 'tenant_id';
                hidden.value = query.tenant_id;
                select.parentElement.appendChild(hidden);
            }
        }

        document.getElementById('git-repo-form').addEventListener('submit', async (e) => {
            await handleFormSubmit(e, async (data) => {
                const tenantId = data.tenant_id;
                delete data.tenant_id;
                await api.createGitRepo(tenantId, data);
                getApp().showSuccess('Git repository created successfully');
                router.navigate('/git-repos');
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

router.on('/git-repos/:id/edit', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const [repo, tenants] = await Promise.all([
            api.getGitRepo(params.id),
            api.getTenants(),
        ]);
        content.innerHTML = `
            <style>
                .terminal-window { background: #1e1e1e; border-radius: 8px; box-shadow: 0 4px 16px rgba(0,0,0,0.3); overflow: hidden; }
                .terminal-header { background: linear-gradient(180deg, #3c3c3c 0%, #2c2c2c 100%); color: #ccc; padding: 8px 12px; display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid #1a1a1a; }
                .terminal-controls { display: flex; gap: 6px; }
                .terminal-dot { width: 12px; height: 12px; border-radius: 50%; display: inline-block; }
                .terminal-dot-red { background: #ff5f56; }
                .terminal-dot-yellow { background: #ffbd2e; }
                .terminal-dot-green { background: #27c93f; }
                .terminal-title { flex: 1; text-align: center; font-size: 13px; font-weight: 500; }
                .terminal-body { background: #1e1e1e; color: #c9d1d9; font-family: Monaco, Menlo, ui-monospace, SFMono-Regular, Consolas, "Liberation Mono", "Courier New", monospace; font-size: 13px; padding: 12px; height: 360px; overflow-y: auto; white-space: pre-wrap; word-break: break-word; }
            </style>
            <div class="row mb-3">
                <div class="col">
                    <a href="#/tenants/${repo.tenant_id}" class="btn btn-ghost-secondary">
                        <i class="ti ti-arrow-left"></i>
                        Back to Tenant
                    </a>
                </div>
            </div>
            ${createGitRepoForm(repo, tenants)}
        `;

        const tenantSelect = document.querySelector('select[name="tenant_id"]');
        if (tenantSelect) {
            tenantSelect.disabled = true;
            const hidden = document.createElement('input');
            hidden.type = 'hidden';
            hidden.name = 'tenant_id';
            hidden.value = repo.tenant_id;
            tenantSelect.parentElement.appendChild(hidden);
        }

        document.getElementById('git-repo-form').addEventListener('submit', async (e) => {
            await handleFormSubmit(e, async (data) => {
                await api.updateGitRepo(params.id, data);
                getApp().showSuccess('Git repository updated successfully');
                router.navigate(`/tenants/${repo.tenant_id}`);
            });
        });
    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load git repository: ${error.message}
            </div>
        `;
    }
});

// ArgoCD instances
router.on('/argocd/new', async (params, query) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';
    try {
        const tenants = await api.getTenants();
        const form = createArgocdInstanceForm(null, tenants);
        content.innerHTML = form;

        const formEl = document.getElementById('argocd-instance-form');
        if (query.tenant_id) {
            const tenantSelect = formEl.querySelector('select[name="tenant_id"]');
            if (tenantSelect) {
                tenantSelect.value = query.tenant_id;
            }
        }

        formEl.addEventListener('submit', async (e) => {
            await handleFormSubmit(e, async (data) => {
                await api.createArgocdInstance(data.tenant_id, data);
                router.navigate(`/tenants/${data.tenant_id}`);
            });
        });
    } catch (error) {
        console.error(error);
        content.innerHTML = `<div class="alert alert-danger">Failed to load form</div>`;
    }
});

router.on('/argocd/:id/edit', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';
    try {
        const instance = await api.getArgocdInstance(params.id);
        const tenants = await api.getTenants();
        const form = createArgocdInstanceForm(instance, tenants);
        content.innerHTML = `
            <div class="row mb-3">
                <div class="col">
                    <a href="#/tenants/${instance.tenant_id}" class="btn btn-ghost-secondary">
                        <i class="ti ti-arrow-left"></i>
                        Back to Tenant
                    </a>
                </div>
            </div>
        ` + form;
        const formEl = document.getElementById('argocd-instance-form');
        formEl.addEventListener('submit', async (e) => {
            await handleFormSubmit(e, async (data) => {
                await api.updateArgocdInstance(params.id, data);
                router.navigate(`/tenants/${instance.tenant_id}`);
            });
        });
        document.getElementById('delete-argocd-instance-btn')?.addEventListener('click', async () => {
            const confirmed = await showConfirmDialog(
                'Delete ArgoCD Instance?',
                `Are you sure you want to delete \"${instance.name}\"?`,
                'Delete',
                'Cancel'
            );
            if (!confirmed) return;
            try {
                await api.deleteArgocdInstance(instance.id);
                getApp().showSuccess('ArgoCD instance deleted');
                router.navigate(`/tenants/${instance.tenant_id}`);
            } catch (error) {
                getApp().showError(error.message);
            }
        });
    } catch (error) {
        console.error(error);
        content.innerHTML = `<div class="alert alert-danger">Failed to load instance</div>`;
    }
});

// ArgoCD apps
router.on('/argocd-apps/new', async (params, query) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';
    try {
        const environment = await api.getEnvironment(query.environment_id);
        const instances = await api.getArgocdInstances(environment.tenant_id);
        content.innerHTML = createArgocdAppForm(null, instances, environment);

        const formEl = document.getElementById('argocd-app-form');
        formEl.addEventListener('submit', async (e) => {
            await handleFormSubmit(e, async (data) => {
                const envId = data.environment_id;
                delete data.environment_id;
                if (typeof data.ignore_resources === 'string') {
                    data.ignore_resources = data.ignore_resources
                        .split('\n')
                        .map(v => v.trim())
                        .filter(v => v.length > 0);
                }
                await api.createArgocdApp(envId, data);
                router.navigate(`/environments/${envId}/edit`);
            });
        });
    } catch (error) {
        console.error(error);
        content.innerHTML = `<div class="alert alert-danger">Failed to load form</div>`;
    }
});

router.on('/argocd-apps/:id', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';
    try {
        const app = await api.getArgocdApp(params.id);
        const instance = await api.getArgocdInstance(app.argocd_instance_id);
        const env = await api.getEnvironment(app.environment_id);

        content.innerHTML = `
            <div class="row mb-3">
                <div class="col">
                    <a href="#/tenants/${env.tenant_id}" class="btn btn-ghost-secondary">
                        <i class="ti ti-arrow-left"></i>
                        Back to Tenant
                    </a>
                </div>
                <div class="col-auto">
                    <a href="${instance.base_url}/applications/${encodeURIComponent(app.application_name)}" target="_blank" class="btn btn-outline-primary">
                        <i class="ti ti-external-link"></i>
                        Open ArgoCD
                    </a>
                </div>
            </div>

            <div class="card mb-3">
                <div class="card-header">
                    <h3 class="card-title">${app.application_name}</h3>
                    <div class="card-actions">
                        <a href="#/argocd-apps/${app.id}/edit" class="btn btn-primary btn-sm">
                            <i class="ti ti-pencil"></i>
                            Edit
                        </a>
                    </div>
                </div>
                <div class="card-body">
                    <div class="row">
                        <div class="col-md-6">
                            <div class="text-secondary">Instance</div>
                            <div>${instance.name}</div>
                            <div class="text-secondary small"><code class="small">${instance.base_url}</code></div>
                        </div>
                        <div class="col-md-6">
                            <div class="text-secondary">Environment</div>
                            <div><span class="badge" style="${env.color ? `background:${env.color};color:#fff;` : ''}">${env.name}</span> <span class="text-secondary small">${env.slug}</span></div>
                        </div>
                    </div>
                    <div class="row mt-2">
                        <div class="col-md-6">
                            <div class="text-secondary">Project</div>
                            <div><code class="small">${app.project_name || 'default'}</code></div>
                        </div>
                    </div>
                    <div class="mt-3">
                        <div class="text-secondary small">Target Revision</div>
                        <div class="d-flex align-items-center gap-2">
                            <code class="small" id="argocd-target-revision">Loading...</code>
                            <button class="btn btn-outline-primary btn-sm" id="argocd-change-revision-btn">
                                <i class="ti ti-pencil"></i>
                                Change Target Revision
                            </button>
                        </div>
                    </div>
                    <div class="mt-3" id="argocd-status-card">
                        <div class="text-secondary small">Status</div>
                        <div class="text-secondary">Loading...</div>
                    </div>
                    <div class="mt-2">
                        <label class="form-label small text-secondary mb-1">Auto refresh</label>
                        <select class="form-select form-select-sm w-auto" id="argocd-poll-select">
                            <option value="0">Off</option>
                            <option value="10">10s</option>
                            <option value="15">15s</option>
                            <option value="30">30s</option>
                            <option value="60">60s</option>
                        </select>
                    </div>
                    <div class="mt-3">
                        <div class="card log-panel" style="background: transparent; border: none;">
                            <div class="terminal-header" style="display:flex;align-items:center;gap:6px;padding:10px 12px;background:#12171d;border-bottom:1px solid rgba(255,255,255,0.08);border-top-left-radius:8px;border-top-right-radius:8px;">
                                <span class="terminal-dot red" style="width:10px;height:10px;border-radius:999px;display:inline-block;background:#ff5f57;"></span>
                                <span class="terminal-dot yellow" style="width:10px;height:10px;border-radius:999px;display:inline-block;background:#febc2e;"></span>
                                <span class="terminal-dot green" style="width:10px;height:10px;border-radius:999px;display:inline-block;background:#28c840;"></span>
                                <span class="ms-2 text-secondary" style="font-size:12px;">ArgoCD Live Events</span>
                            </div>
                            <div class="card-body p-0">
                                <pre id="argocd-event-log" style="margin:0;max-height:320px;overflow:auto;font-family:SFMono-Regular,Consolas,'Liberation Mono',monospace;font-size:12px;line-height:1.4;background:#0b0f14;color:#cfe3f4;padding:12px;border-bottom-left-radius:8px;border-bottom-right-radius:8px;white-space:pre-wrap;"></pre>
                            </div>
                            <div class="card-footer" style="border-top:1px solid rgba(255,255,255,0.08); background:#12171d;">
                                <div class="d-flex gap-2">
                                    <button class="btn btn-outline-secondary" id="argocd-refresh-btn"><i class="ti ti-refresh"></i> Refresh</button>
                                    <button class="btn btn-success" id="argocd-sync-btn"><i class="ti ti-rocket"></i> Sync</button>
                                    <button class="btn btn-outline-danger" id="argocd-terminate-btn"><i class="ti ti-circle-x"></i> Terminate</button>
                                </div>
                            </div>
                        </div>
                    </div>
                    <hr class="my-4">
                    <ul class="nav nav-tabs" id="argocd-resource-tabs">
                        <li class="nav-item"><button class="nav-link active" data-kind="Deployment">Deployments</button></li>
                        <li class="nav-item"><button class="nav-link" data-kind="Pod">Pods</button></li>
                        <li class="nav-item"><button class="nav-link" data-kind="Route">Routes</button></li>
                        <li class="nav-item"><button class="nav-link" data-kind="Ingress">Ingresses</button></li>
                        <li class="nav-item"><button class="nav-link" data-kind="ConfigMap">ConfigMaps</button></li>
                    </ul>
                    <div class="table-responsive mt-2">
                        <table class="table table-vcenter card-table">
                            <thead>
                                <tr>
                                    <th>Kind</th>
                                    <th>Name</th>
                                    <th>Namespace</th>
                                    <th>Health</th>
                                    <th>Sync</th>
                                </tr>
                            </thead>
                            <tbody id="argocd-resource-body">
                                <tr><td colspan="5" class="text-secondary">Loading...</td></tr>
                            </tbody>
                        </table>
                    </div>
                </div>
                <div class="card-footer d-none"></div>
            </div>
        `;

        const statusEl = document.getElementById('argocd-status-card');
        const targetRevisionEl = document.getElementById('argocd-target-revision');
        const renderStatus = (status) => {
            if (!status) {
                statusEl.innerHTML = '<div class="text-secondary small">Status</div><div class="text-secondary">-</div>';
                if (targetRevisionEl) targetRevisionEl.textContent = '-';
                return;
            }
            if (targetRevisionEl) {
                targetRevisionEl.textContent = status?.target_revision || status?.revision || '-';
            }
            const conditions = Array.isArray(status.conditions) ? status.conditions : [];
            const issues = Array.isArray(status.resource_issues) ? status.resource_issues : [];
            const issueItems = issues.slice(0, 50).map(i => {
                const label = `${i.kind || ''}/${i.name || ''}`.replace(/^\/|\/$/g, '');
                const msg = i.message ? ` — ${i.message}` : '';
                return `<li><strong>${label}</strong>${msg}</li>`;
            }).join('');
            const conditionItems = conditions.map(c => {
                const label = c.type || 'Condition';
                const msg = c.message ? ` — ${c.message}` : '';
                return `<li><strong>${label}</strong>${msg}</li>`;
            }).join('');
            statusEl.innerHTML = `
                <div class="text-secondary small">Status</div>
                <div class="d-flex align-items-center gap-2">
                    <span class="badge ${status?.sync_status === 'Synced' ? 'bg-green-lt text-green-fg' : 'bg-yellow-lt text-yellow-fg'}">${status?.sync_status || 'unknown'}</span>
                    <span class="badge ${status?.health_status === 'Healthy' ? 'bg-green-lt text-green-fg' : 'bg-red-lt text-red-fg'}">${status?.health_status || 'unknown'}</span>
                    <span class="text-secondary small">${status?.operation_phase || ''}</span>
                </div>
                <div class="text-secondary small mt-1">${status?.operation_message || ''}</div>
                ${status?.operation_sync_message ? `<div class="text-secondary small">${status.operation_sync_message}</div>` : ''}
                ${status?.operation_started_at || status?.operation_finished_at ? `
                    <div class="text-secondary small mt-1">
                        ${status.operation_started_at ? `Started: ${status.operation_started_at}` : ''}
                        ${status.operation_finished_at ? ` · Finished: ${status.operation_finished_at}` : ''}
                    </div>
                ` : ''}
                ${status?.revision ? `<div class="text-secondary small">Revision: <code class="small">${status.revision}</code></div>` : ''}
                ${status?.last_deployed_at || status?.last_deployed_revision || status?.last_deployed_message ? `
                    <div class="text-secondary small mt-1">
                        ${status.last_deployed_at ? `Last deploy: ${status.last_deployed_at}` : ''}
                        ${status.last_deployed_revision ? ` · ${status.last_deployed_revision}` : ''}
                    </div>
                    ${status.last_deployed_message ? `<div class="text-secondary small">${status.last_deployed_message}</div>` : ''}
                ` : ''}
                ${conditions.length > 0 || issues.length > 0 ? `
                    <div class="alert alert-warning mt-2 mb-0 p-2">
                        <div class="small mb-1"><strong>Issues:</strong> ${issues.length} ${issues.length > 50 ? '(showing first 50)' : ''}</div>
                        <ul class="mb-0 ps-3 small" style="max-height:140px; overflow:auto;">
                            ${issueItems}
                            ${conditionItems}
                        </ul>
                    </div>
                ` : ''}
            `;
        };

        let issueMap = new Map();
        let resources = [];
        const resourceBody = document.getElementById('argocd-resource-body');
        const loadResources = async () => {
            try {
                const list = await api.getArgocdAppResources(app.id);
                resources = Array.isArray(list) ? list : [];
                const activeKind = document.querySelector('#argocd-resource-tabs .nav-link.active')?.getAttribute('data-kind') || 'Deployment';
                renderResources(activeKind);
            } catch {
                resourceBody.innerHTML = `<tr><td colspan="5" class="text-danger">Failed to load resources</td></tr>`;
            }
        };
        const renderResources = (kind) => {
            const rows = resources.filter(r => r.kind === kind);
            if (rows.length === 0) {
                resourceBody.innerHTML = `<tr><td colspan="5" class="text-secondary">No resources</td></tr>`;
                return;
            }
            resourceBody.innerHTML = rows.map(r => `
                <tr>
                    <td>${r.kind}</td>
                    <td>
                        <code class="small">${r.name}</code>
                        ${issueMap.has(`${r.kind}|${r.namespace || ''}|${r.name}`) ? '<span class="badge bg-yellow-lt text-yellow-fg ms-2" title="Resource issue">issue</span>' : ''}
                    </td>
                    <td>${r.namespace || '-'}</td>
                    <td><span class="badge ${r.health === 'Healthy' ? 'bg-green-lt text-green-fg' : 'bg-yellow-lt text-yellow-fg'}">${r.health || '-'}</span></td>
                    <td><span class="badge ${r.sync === 'Synced' ? 'bg-green-lt text-green-fg' : 'bg-yellow-lt text-yellow-fg'}">${r.sync || '-'}</span></td>
                </tr>
            `).join('');
        };
        const loadStatus = async () => {
            try {
                const status = await api.getArgocdAppStatus(app.id);
                renderStatus(status);
                issueMap = new Map((status?.resource_issues || []).map(i => [`${i.kind}|${i.namespace || ''}|${i.name}`, i]));
                const activeKind = document.querySelector('#argocd-resource-tabs .nav-link.active')?.getAttribute('data-kind') || 'Deployment';
                renderResources(activeKind);
                await loadResources();
            } catch (err) {
                console.error('Failed to load ArgoCD status:', err);
                statusEl.innerHTML = `
                    <div class="text-secondary small">Status</div>
                    <div class="text-danger small">Failed to load status</div>
                `;
            }
        };
        // Defer to ensure DOM is ready
        setTimeout(loadStatus, 0);
        loadResources();

        document.querySelectorAll('#argocd-resource-tabs .nav-link').forEach(btn => {
            btn.addEventListener('click', () => {
                document.querySelectorAll('#argocd-resource-tabs .nav-link').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');
                renderResources(btn.getAttribute('data-kind'));
            });
        });

        const poll = env.argocd_poll_interval_seconds || 0;
        let pollTimer = null;
        const pollSelect = document.getElementById('argocd-poll-select');
        const setPoll = (seconds) => {
            if (pollTimer) {
                clearInterval(pollTimer);
                pollTimer = null;
            }
            if (seconds > 0) {
                pollTimer = setInterval(loadStatus, seconds * 1000);
            }
        };
        const defaultPoll = Number.isFinite(poll) && poll > 0 ? poll : 10;
        if (pollSelect) {
            pollSelect.value = String(defaultPoll);
            pollSelect.addEventListener('change', () => {
                const seconds = parseInt(pollSelect.value, 10);
                setPoll(Number.isFinite(seconds) ? seconds : 0);
            });
        }
        setPoll(defaultPoll);

        const refreshBtn = document.getElementById('argocd-refresh-btn');
        const syncBtn = document.getElementById('argocd-sync-btn');
        const termBtn = document.getElementById('argocd-terminate-btn');

        const runAction = async (btn, actionLabel, fn) => {
            const original = btn.innerHTML;
            btn.disabled = true;
            btn.innerHTML = `<span class="spinner-border spinner-border-sm me-2"></span>${actionLabel}`;
            try {
                await fn();
                getApp().showSuccess(`${actionLabel} requested`);
            } catch (err) {
                getApp().showError(`${actionLabel} failed: ${err.message}`);
            } finally {
                btn.disabled = false;
                btn.innerHTML = original;
                await loadStatus();
            }
        };

        refreshBtn.addEventListener('click', async () => {
            await runAction(refreshBtn, 'Refreshing', () => api.refreshArgocdApp(app.id));
        });
        syncBtn.addEventListener('click', async () => {
            await runAction(syncBtn, 'Syncing', () => api.syncArgocdApp(app.id));
        });
        termBtn.addEventListener('click', async () => {
            await runAction(termBtn, 'Terminating', () => api.terminateArgocdApp(app.id));
        });

        const eventLog = document.getElementById('argocd-event-log');
        const logLines = [];
        const renderLog = () => {
            eventLog.textContent = logLines.join('\n');
            eventLog.scrollTop = eventLog.scrollHeight;
        };
        const appendEvent = (ev) => {
            const ts = ev.last_timestamp || ev.first_timestamp || '';
            const reason = ev.reason || '';
            const kind = ev.kind || '';
            const name = ev.name || '';
            const msg = ev.message || '';
            const line = `[${ts}] ${reason} ${kind}/${name} ${msg}`.trim();
            logLines.push(line);
            renderLog();
        };
        api.getArgocdAppEvents(app.id).then(events => {
            if (Array.isArray(events)) {
                events.forEach(appendEvent);
            }
        }).catch(() => {});
        let eventSource = null;
        try {
            eventSource = new EventSource(`${api.baseUrl}/argocd-apps/${app.id}/events/stream`);
            eventSource.onmessage = (evt) => {
                try {
                    const ev = JSON.parse(evt.data);
                    appendEvent(ev);
                } catch {}
            };
        } catch {}

        if (pollTimer) {
            window.addEventListener('hashchange', () => clearInterval(pollTimer), { once: true });
        }
        if (eventSource) {
            window.addEventListener('hashchange', () => eventSource.close(), { once: true });
        }

        const changeRevisionBtn = document.getElementById('argocd-change-revision-btn');
        const showChangeRevisionDialog = async () => {
            let tags = [];
            try {
                tags = await api.getArgocdDeployTags(app.id);
            } catch {}

            const modalHtml = `
                <div class="modal modal-blur fade show" style="display: block;" id="argocd-revision-modal">
                    <div class="modal-dialog modal-md modal-dialog-centered" role="document">
                        <div class="modal-content">
                            <div class="modal-header">
                                <h5 class="modal-title">Change Target Revision</h5>
                                <button type="button" class="btn-close" id="argocd-revision-close"></button>
                            </div>
                            <div class="modal-body">
                                <div class="mb-3">
                                    <label class="form-label">Recent successful tags</label>
                                    <select class="form-select" id="argocd-revision-select">
                                        <option value="">Select tag...</option>
                                        ${Array.isArray(tags) ? tags.map(t => `<option value="${t}">${t}</option>`).join('') : ''}
                                    </select>
                                </div>
                                <div class="mb-3">
                                    <label class="form-label">Or enter revision</label>
                                    <input type="text" class="form-control" id="argocd-revision-input" placeholder="2026.01.29.01-tst">
                                </div>
                                <div class="text-secondary small">Current: <code class="small">${targetRevisionEl?.textContent || '-'}</code></div>
                            </div>
                            <div class="modal-footer">
                                <button type="button" class="btn btn-link" id="argocd-revision-cancel">Cancel</button>
                                <button type="button" class="btn btn-primary" id="argocd-revision-apply" disabled>Apply</button>
                            </div>
                        </div>
                    </div>
                </div>
            `;
            document.body.insertAdjacentHTML('beforeend', modalHtml);
            const modal = document.getElementById('argocd-revision-modal');
            const closeModal = () => {
                modal?.remove();
            };
            const selectEl = document.getElementById('argocd-revision-select');
            const inputEl = document.getElementById('argocd-revision-input');
            const applyBtn = document.getElementById('argocd-revision-apply');
            const updateApplyState = () => {
                const value = (inputEl?.value || '').trim() || (selectEl?.value || '').trim();
                applyBtn.disabled = value.length === 0;
            };
            selectEl?.addEventListener('change', () => {
                if (selectEl.value) {
                    inputEl.value = selectEl.value;
                }
                updateApplyState();
            });
            inputEl?.addEventListener('input', updateApplyState);
            updateApplyState();

            document.getElementById('argocd-revision-close')?.addEventListener('click', closeModal);
            document.getElementById('argocd-revision-cancel')?.addEventListener('click', closeModal);

            applyBtn?.addEventListener('click', async () => {
                const value = (inputEl?.value || '').trim() || (selectEl?.value || '').trim();
                if (!value) return;
                applyBtn.disabled = true;
                try {
                    await api.updateArgocdTargetRevision(app.id, value);
                    getApp().showSuccess('Target revision updated');
                    closeModal();
                    await loadStatus();
                } catch (err) {
                    getApp().showError(`Failed to update revision: ${err.message}`);
                    applyBtn.disabled = false;
                }
            });
        };

        changeRevisionBtn?.addEventListener('click', showChangeRevisionDialog);
    } catch (error) {
        console.error(error);
        content.innerHTML = `<div class="alert alert-danger">Failed to load ArgoCD app</div>`;
    }
});

router.on('/argocd-apps/:id/edit', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';
    try {
        const app = await api.getArgocdApp(params.id);
        const environment = await api.getEnvironment(app.environment_id);
        const instances = await api.getArgocdInstances(environment.tenant_id);
        content.innerHTML = `
            <div class="row mb-3">
                <div class="col">
                    <a href="#/tenants/${environment.tenant_id}" class="btn btn-ghost-secondary">
                        <i class="ti ti-arrow-left"></i>
                        Back to Tenant
                    </a>
                </div>
            </div>
        ` + createArgocdAppForm(app, instances, environment);
        const formEl = document.getElementById('argocd-app-form');
        formEl.addEventListener('submit', async (e) => {
            await handleFormSubmit(e, async (data) => {
                delete data.environment_id;
                if (typeof data.ignore_resources === 'string') {
                    data.ignore_resources = data.ignore_resources
                        .split('\n')
                        .map(v => v.trim())
                        .filter(v => v.length > 0);
                }
                await api.updateArgocdApp(params.id, data);
                router.navigate(`/environments/${environment.id}/edit`);
            });
        });
        document.getElementById('delete-argocd-app-btn')?.addEventListener('click', async () => {
            const confirmed = await showConfirmDialog(
                'Delete ArgoCD App?',
                `Are you sure you want to delete "${app.application_name}"?`,
                'Delete',
                'Cancel'
            );
            if (!confirmed) return;
            try {
                await api.deleteArgocdApp(app.id);
                getApp().showSuccess('ArgoCD app deleted');
                router.navigate(`/tenants/${environment.tenant_id}`);
            } catch (error) {
                getApp().showError(error.message);
            }
        });
    } catch (error) {
        console.error(error);
        content.innerHTML = `<div class="alert alert-danger">Failed to load app</div>`;
    }
});

// Kubernetes instances
router.on('/kubernetes/new', async (params, query) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';
    try {
        const tenants = await api.getTenants();
        const form = createKubernetesInstanceForm(null, tenants);
        content.innerHTML = form;
        const tenantSelect = document.querySelector('select[name="tenant_id"]');
        if (query?.tenant_id && tenantSelect) {
            tenantSelect.value = query.tenant_id;
        }
        const formEl = document.getElementById('kubernetes-instance-form');
        formEl.addEventListener('submit', async (e) => {
            await handleFormSubmit(e, async (data) => {
                await api.createKubernetesInstance(data.tenant_id, data);
                router.navigate(`/tenants/${data.tenant_id}`);
            });
        });
    } catch (error) {
        console.error(error);
        content.innerHTML = `<div class="alert alert-danger">Failed to load form</div>`;
    }
});

router.on('/kubernetes/:id/edit', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';
    try {
        const instance = await api.getKubernetesInstance(params.id);
        const tenants = await api.getTenants();
        const form = createKubernetesInstanceForm(instance, tenants);
        content.innerHTML = `
            <div class="row mb-3">
                <div class="col">
                    <a href="#/tenants/${instance.tenant_id}" class="btn btn-ghost-secondary">
                        <i class="ti ti-arrow-left"></i>
                        Back to Tenant
                    </a>
                </div>
            </div>
        ` + form;
        const formEl = document.getElementById('kubernetes-instance-form');
        formEl.addEventListener('submit', async (e) => {
            await handleFormSubmit(e, async (data) => {
                await api.updateKubernetesInstance(params.id, data);
                router.navigate(`/tenants/${instance.tenant_id}`);
            });
        });
        document.getElementById('delete-kubernetes-instance-btn')?.addEventListener('click', async () => {
            const confirmed = await showConfirmDialog(
                'Delete Kubernetes Instance?',
                `Are you sure you want to delete \"${instance.name}\"?`,
                'Delete',
                'Cancel'
            );
            if (!confirmed) return;
            try {
                await api.deleteKubernetesInstance(instance.id);
                getApp().showSuccess('Kubernetes instance deleted');
                router.navigate(`/tenants/${instance.tenant_id}`);
            } catch (error) {
                getApp().showError(error.message);
            }
        });
    } catch (error) {
        console.error(error);
        content.innerHTML = `<div class="alert alert-danger">Failed to load instance</div>`;
    }
});

// Kubernetes namespaces
router.on('/kubernetes-namespaces/new', async (params, query) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';
    try {
        const environment = await api.getEnvironment(query.environment_id);
        const instances = await api.getKubernetesInstances(environment.tenant_id);
        content.innerHTML = createKubernetesNamespaceForm(null, instances, environment);

        const formEl = document.getElementById('kubernetes-namespace-form');
        formEl.addEventListener('submit', async (e) => {
            await handleFormSubmit(e, async (data) => {
                const envId = data.environment_id;
                delete data.environment_id;
                await api.createKubernetesNamespace(envId, data);
                router.navigate(`/environments/${envId}/edit`);
            });
        });
    } catch (error) {
        console.error(error);
        content.innerHTML = `<div class="alert alert-danger">Failed to load form</div>`;
    }
});

router.on('/kubernetes-namespaces/:id', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';
    try {
        const entry = await api.getKubernetesNamespace(params.id);
        const instance = await api.getKubernetesInstance(entry.kubernetes_instance_id);
        const env = await api.getEnvironment(entry.environment_id);

        content.innerHTML = `
            <div class="row mb-3">
                <div class="col">
                    <a href="#/tenants/${env.tenant_id}" class="btn btn-ghost-secondary">
                        <i class="ti ti-arrow-left"></i>
                        Back to Tenant
                    </a>
                </div>
            </div>

            <div class="card mb-3">
                <div class="card-header">
                    <h3 class="card-title">${entry.namespace}</h3>
                    <div class="card-actions">
                        <a href="#/kubernetes-namespaces/${entry.id}/edit" class="btn btn-primary btn-sm">
                            <i class="ti ti-pencil"></i>
                            Edit
                        </a>
                    </div>
                </div>
                <div class="card-body">
                    <div class="row">
                        <div class="col-md-6">
                            <div class="text-secondary">Instance</div>
                            <div>${instance.name}</div>
                            <div class="text-secondary small"><code class="small">${instance.base_url}</code></div>
                        </div>
                        <div class="col-md-6">
                            <div class="text-secondary">Environment</div>
                            <div><span class="badge" style="${env.color ? `background:${env.color};color:#fff;` : ''}">${env.name}</span> <span class="text-secondary small">${env.slug}</span></div>
                        </div>
                    </div>
                    <div class="mt-3">
                        <div class="text-secondary small mb-2">Cluster Version</div>
                        <div class="table-responsive">
                            <table class="table table-vcenter card-table table-sm small">
                                <tbody id="k8s-version-table">
                                    <tr><td class="text-secondary">Loading...</td></tr>
                                </tbody>
                            </table>
                        </div>
                    </div>
                    <div class="mt-3">
                        <div class="card log-panel" style="background: transparent; border: none;">
                            <div class="terminal-header" style="display:flex;align-items:center;gap:6px;padding:10px 12px;background:#12171d;border-bottom:1px solid rgba(255,255,255,0.08);border-top-left-radius:8px;border-top-right-radius:8px;">
                                <span class="terminal-dot red" style="width:10px;height:10px;border-radius:999px;display:inline-block;background:#ff5f57;"></span>
                                <span class="terminal-dot yellow" style="width:10px;height:10px;border-radius:999px;display:inline-block;background:#febc2e;"></span>
                                <span class="terminal-dot green" style="width:10px;height:10px;border-radius:999px;display:inline-block;background:#28c840;"></span>
                                <span class="ms-2 text-secondary" style="font-size:12px;">Kubernetes Live Events</span>
                            </div>
                            <div class="card-body p-0">
                                <pre id="kubernetes-event-log" style="margin:0;max-height:320px;overflow:auto;font-family:SFMono-Regular,Consolas,'Liberation Mono',monospace;font-size:12px;line-height:1.4;background:#0b0f14;color:#cfe3f4;padding:12px;border-bottom-left-radius:8px;border-bottom-right-radius:8px;white-space:pre-wrap;"></pre>
                            </div>
                        </div>
                    </div>
                    <div class="mt-3">
                        <button class="btn btn-outline-primary btn-sm" id="k8s-skopeo-open">
                            <i class="ti ti-wand"></i>
                            Skopeo Helper
                        </button>
                    </div>
                    <hr class="my-4">
                    <ul class="nav nav-tabs" id="kubernetes-resource-tabs">
                        <li class="nav-item"><button class="nav-link active" data-kind="namespaces">Namespaces</button></li>
                        <li class="nav-item"><button class="nav-link" data-kind="deployments">Deployments</button></li>
                        <li class="nav-item"><button class="nav-link" data-kind="pods">Pods</button></li>
                        <li class="nav-item"><button class="nav-link" data-kind="services">Services</button></li>
                        <li class="nav-item"><button class="nav-link" data-kind="routes">Routes</button></li>
                        <li class="nav-item"><button class="nav-link" data-kind="configmaps">ConfigMaps</button></li>
                        <li class="nav-item"><button class="nav-link" data-kind="images">Docker Images</button></li>
                    </ul>
                    <div class="table-responsive mt-2">
                        <table class="table table-vcenter card-table">
                            <thead id="k8s-resource-head"></thead>
                            <tbody id="k8s-resource-body">
                                <tr><td class="text-secondary">Loading...</td></tr>
                            </tbody>
                        </table>
                    </div>
                </div>
                <div class="card-footer d-none"></div>
            </div>
        `;

        const versionTable = document.getElementById('k8s-version-table');
        const renderVersion = (data) => {
            if (!data || typeof data !== 'object') {
                versionTable.innerHTML = '<tr><td class="text-secondary">No data</td></tr>';
                return;
            }
            const rows = [
                ['Major', data.major],
                ['Minor', data.minor],
                ['Git Version', data.gitVersion],
                ['Git Commit', data.gitCommit],
                ['Git Tree State', data.gitTreeState],
                ['Build Date', data.buildDate],
                ['Go Version', data.goVersion],
                ['Compiler', data.compiler],
                ['Platform', data.platform],
            ].filter(([, v]) => v !== undefined);
            versionTable.innerHTML = rows.map(([k, v]) => `
                <tr>
                    <td class="text-secondary">${k}</td>
                    <td><code class="small">${v ?? '-'}</code></td>
                </tr>
            `).join('');
        };

        const loadVersion = async () => {
            try {
                const data = await api.getKubernetesNamespaceStatus(entry.id);
                renderVersion(data);
            } catch (err) {
                versionTable.innerHTML = `<tr><td class="text-danger">Failed to load version</td></tr>`;
            }
        };

        const resourceHead = document.getElementById('k8s-resource-head');
        const resourceBody = document.getElementById('k8s-resource-body');
        const resourceCache = new Map();

        const formatAge = (ts) => {
            if (!ts) return '-';
            const diffMs = Date.now() - new Date(ts).getTime();
            if (!Number.isFinite(diffMs) || diffMs < 0) return '-';
            const mins = Math.floor(diffMs / 60000);
            if (mins < 60) return `${mins}m`;
            const hours = Math.floor(mins / 60);
            if (hours < 24) return `${hours}h`;
            const days = Math.floor(hours / 24);
            return `${days}d`;
        };

        const renderResources = (kind, data) => {
            if (kind === 'namespaces') {
                resourceHead.innerHTML = `
                    <tr><th>Name</th><th>Phase</th><th>Labels</th><th>Age</th></tr>
                `;
                const meta = data?.metadata || {};
                const status = data?.status || {};
                const labels = meta.labels ? Object.entries(meta.labels).map(([k, v]) => `${k}=${v}`).join(', ') : '-';
                resourceBody.innerHTML = `
                    <tr>
                        <td><code class="small">${meta.name || entry.namespace}</code></td>
                        <td>${status.phase || '-'}</td>
                        <td>${labels || '-'}</td>
                        <td>${formatAge(meta.creationTimestamp)}</td>
                    </tr>
                `;
                return;
            }
            if (kind === 'deployments') {
                resourceHead.innerHTML = `
                    <tr><th>Name</th><th>Ready</th><th>Up-to-date</th><th>Available</th><th>Age</th></tr>
                `;
                const items = Array.isArray(data?.items) ? data.items : [];
                if (items.length === 0) {
                    resourceBody.innerHTML = `<tr><td colspan="5" class="text-secondary">No deployments</td></tr>`;
                    return;
                }
                resourceBody.innerHTML = items.map(item => {
                    const meta = item.metadata || {};
                    const spec = item.spec || {};
                    const status = item.status || {};
                    const ready = `${status.readyReplicas || 0}/${spec.replicas || 0}`;
                    return `
                        <tr>
                            <td><code class="small">${meta.name || '-'}</code></td>
                            <td>${ready}</td>
                            <td>${status.updatedReplicas || 0}</td>
                            <td>${status.availableReplicas || 0}</td>
                            <td>${formatAge(meta.creationTimestamp)}</td>
                        </tr>
                    `;
                }).join('');
                return;
            }
            if (kind === 'pods') {
                resourceHead.innerHTML = `
                    <tr><th>Name</th><th>Phase</th><th>Ready</th><th>Restarts</th><th>Age</th></tr>
                `;
                const items = Array.isArray(data?.items) ? data.items : [];
                if (items.length === 0) {
                    resourceBody.innerHTML = `<tr><td colspan="5" class="text-secondary">No pods</td></tr>`;
                    return;
                }
                resourceBody.innerHTML = items.map(item => {
                    const meta = item.metadata || {};
                    const status = item.status || {};
                    const containers = Array.isArray(status.containerStatuses) ? status.containerStatuses : [];
                    const ready = containers.filter(c => c.ready).length;
                    const restarts = containers.reduce((acc, c) => acc + (c.restartCount || 0), 0);
                    return `
                        <tr>
                            <td><code class="small">${meta.name || '-'}</code></td>
                            <td>${status.phase || '-'}</td>
                            <td>${ready}/${containers.length}</td>
                            <td>${restarts}</td>
                            <td>${formatAge(meta.creationTimestamp)}</td>
                        </tr>
                    `;
                }).join('');
                return;
            }
            if (kind === 'services') {
                resourceHead.innerHTML = `
                    <tr><th>Name</th><th>Type</th><th>Cluster IP</th><th>Ports</th><th>Age</th></tr>
                `;
                const items = Array.isArray(data?.items) ? data.items : [];
                if (items.length === 0) {
                    resourceBody.innerHTML = `<tr><td colspan="5" class="text-secondary">No services</td></tr>`;
                    return;
                }
                resourceBody.innerHTML = items.map(item => {
                    const meta = item.metadata || {};
                    const spec = item.spec || {};
                    const ports = Array.isArray(spec.ports) ? spec.ports.map(p => `${p.port}/${p.protocol || 'TCP'}`).join(', ') : '-';
                    return `
                        <tr>
                            <td><code class="small">${meta.name || '-'}</code></td>
                            <td>${spec.type || '-'}</td>
                            <td>${spec.clusterIP || '-'}</td>
                            <td>${ports || '-'}</td>
                            <td>${formatAge(meta.creationTimestamp)}</td>
                        </tr>
                    `;
                }).join('');
                return;
            }
            if (kind === 'routes') {
                resourceHead.innerHTML = `
                    <tr><th>Name</th><th>Host</th><th>Target</th><th>TLS</th><th>Age</th></tr>
                `;
                const items = Array.isArray(data?.items) ? data.items : [];
                if (items.length === 0) {
                    resourceBody.innerHTML = `<tr><td colspan="5" class="text-secondary">No routes</td></tr>`;
                    return;
                }
                resourceBody.innerHTML = items.map(item => {
                    const meta = item.metadata || {};
                    const spec = item.spec || {};
                    return `
                        <tr>
                            <td><code class="small">${meta.name || '-'}</code></td>
                            <td>${spec.host || '-'}</td>
                            <td>${spec.to?.name || '-'}</td>
                            <td>${spec.tls ? 'yes' : 'no'}</td>
                            <td>${formatAge(meta.creationTimestamp)}</td>
                        </tr>
                    `;
                }).join('');
                return;
            }
            if (kind === 'configmaps') {
                resourceHead.innerHTML = `
                    <tr><th>Name</th><th>Keys</th><th>Age</th></tr>
                `;
                const items = Array.isArray(data?.items) ? data.items : [];
                if (items.length === 0) {
                    resourceBody.innerHTML = `<tr><td colspan="3" class="text-secondary">No config maps</td></tr>`;
                    return;
                }
                resourceBody.innerHTML = items.map(item => {
                    const meta = item.metadata || {};
                    const dataKeys = item.data ? Object.keys(item.data).length : 0;
                    return `
                        <tr>
                            <td><code class="small">${meta.name || '-'}</code></td>
                            <td>${dataKeys}</td>
                            <td>${formatAge(meta.creationTimestamp)}</td>
                        </tr>
                    `;
                }).join('');
                return;
            }
            if (kind === 'images') {
                resourceHead.innerHTML = `
                    <tr><th>Deployment</th><th>Container</th><th>Docker Image</th></tr>
                `;
                const items = Array.isArray(data?.items) ? data.items : [];
                const rows = [];
                items.forEach(item => {
                    const meta = item.metadata || {};
                    const containers = item?.spec?.template?.spec?.containers || [];
                    containers.forEach(c => {
                        rows.push(`
                            <tr>
                                <td><code class="small">${meta.name || '-'}</code></td>
                                <td>${c.name || '-'}</td>
                                <td><code class="small">${c.image || '-'}</code></td>
                            </tr>
                        `);
                    });
                });
                resourceBody.innerHTML = rows.length > 0
                    ? rows.join('')
                    : `<tr><td colspan="3" class="text-secondary">No images</td></tr>`;
            }
        };

        const loadResources = async (kind) => {
            const fetchKind = kind === 'images' ? 'deployments' : kind;
            if (!resourceCache.has(fetchKind)) {
                resourceBody.innerHTML = `<tr><td class="text-secondary">Loading...</td></tr>`;
                try {
                    const data = await api.getKubernetesNamespaceResources(entry.id, fetchKind);
                    resourceCache.set(fetchKind, data);
                } catch (err) {
                    resourceBody.innerHTML = `<tr><td class="text-danger">Failed to load resources</td></tr>`;
                    return;
                }
            }
            renderResources(kind, resourceCache.get(fetchKind));
        };

        const openSkopeoModal = () => {
            const modalHtml = `
                <div class="modal modal-blur fade show" style="display: block;" id="k8s-skopeo-modal">
                    <div class="modal-dialog modal-xl modal-dialog-centered" role="document">
                        <div class="modal-content">
                            <div class="modal-header">
                                <h5 class="modal-title">Skopeo Helper</h5>
                                <button type="button" class="btn-close" id="k8s-skopeo-close"></button>
                            </div>
                            <div class="modal-body">
                                <div class="row g-3">
                                    <div class="col-md-6">
                                        <label class="form-label">Image filter (substring, case-insensitive)</label>
                                        <input type="text" class="form-control" id="k8s-skopeo-filter" placeholder="/nac-test/">
                                    </div>
                                    <div class="col-md-6">
                                        <label class="form-label">Replacements</label>
                                        <div id="k8s-skopeo-replacements"></div>
                                        <button type="button" class="btn btn-outline-secondary btn-sm mt-2" id="k8s-skopeo-add">
                                            <i class="ti ti-plus"></i>
                                            Add replacement
                                        </button>
                                    </div>
                                </div>
                                <div class="mt-3">
                                    <div class="d-flex align-items-center justify-content-between">
                                        <label class="form-label mb-0">Generated commands</label>
                                        <div class="d-flex align-items-center gap-2">
                                            <button class="btn btn-outline-primary btn-sm" id="k8s-skopeo-generate">
                                                <i class="ti ti-wand"></i>
                                                Generate
                                            </button>
                                            <button class="btn btn-outline-secondary btn-sm" id="k8s-skopeo-copy">
                                                <i class="ti ti-copy"></i>
                                                Copy
                                            </button>
                                        </div>
                                    </div>
                                    <pre class="mt-2" id="k8s-skopeo-output" style="margin:0;max-height:260px;overflow:auto;font-family:SFMono-Regular,Consolas,'Liberation Mono',monospace;font-size:12px;line-height:1.4;background:#0b0f14;color:#cfe3f4;padding:12px;border-radius:6px;white-space:pre-wrap;"></pre>
                                </div>
                            </div>
                            <div class="modal-footer">
                                <button type="button" class="btn btn-link" id="k8s-skopeo-cancel">Close</button>
                            </div>
                        </div>
                    </div>
                </div>
            `;
            document.body.insertAdjacentHTML('beforeend', modalHtml);
            const modal = document.getElementById('k8s-skopeo-modal');
            const closeModal = () => modal?.remove();

            const replacementsEl = document.getElementById('k8s-skopeo-replacements');
            const addReplacementRow = (fromVal = '', toVal = '') => {
                const row = document.createElement('div');
                row.className = 'row g-2 mb-2';
                row.innerHTML = `
                    <div class="col-5">
                        <input type="text" class="form-control form-control-sm k8s-replace-from" placeholder="from" value="${fromVal}">
                    </div>
                    <div class="col-5">
                        <input type="text" class="form-control form-control-sm k8s-replace-to" placeholder="to" value="${toVal}">
                    </div>
                    <div class="col-2">
                        <button type="button" class="btn btn-outline-danger btn-sm w-100 k8s-replace-remove">
                            <i class="ti ti-trash"></i>
                        </button>
                    </div>
                `;
                row.querySelector('.k8s-replace-remove')?.addEventListener('click', () => row.remove());
                replacementsEl?.appendChild(row);
            };
            addReplacementRow();
            document.getElementById('k8s-skopeo-add')?.addEventListener('click', () => addReplacementRow());

            const skopeoOutput = document.getElementById('k8s-skopeo-output');
            let skopeoTimer = null;
            const scheduleGenerate = () => {
                if (skopeoTimer) {
                    clearTimeout(skopeoTimer);
                }
                skopeoTimer = setTimeout(() => {
                    generateSkopeo();
                }, 300);
            };
            const generateSkopeo = async () => {
                await loadResources('deployments');
                const filter = (document.getElementById('k8s-skopeo-filter')?.value || '').trim().toLowerCase();
                const deployments = resourceCache.get('deployments');
                const items = Array.isArray(deployments?.items) ? deployments.items : [];
                const images = [];
                items.forEach(item => {
                    const containers = item?.spec?.template?.spec?.containers || [];
                    const initContainers = item?.spec?.template?.spec?.initContainers || [];
                    [...containers, ...initContainers].forEach(c => {
                        if (c?.image) images.push(c.image);
                    });
                });
                const uniqueImages = Array.from(new Set(images));
                const filtered = filter
                    ? uniqueImages.filter(img => img.toLowerCase().includes(filter))
                    : uniqueImages;

                const rules = Array.from(document.querySelectorAll('#k8s-skopeo-replacements .row')).map(row => {
                    const from = row.querySelector('.k8s-replace-from')?.value || '';
                    const to = row.querySelector('.k8s-replace-to')?.value || '';
                    return { find: from, replace: to };
                });

                const commands = filtered.map(src => {
                    const dst = applyReplaceRules(src, rules);
                    return `skopeo copy docker://${src} docker://${dst}`;
                });

                skopeoOutput.textContent = commands.join('\n\n');
            };
            document.getElementById('k8s-skopeo-generate')?.addEventListener('click', generateSkopeo);
            document.getElementById('k8s-skopeo-filter')?.addEventListener('input', scheduleGenerate);

            const copySkopeo = async () => {
                const text = skopeoOutput.textContent || '';
                if (!text.trim()) {
                    getApp().showError('Nothing to copy');
                    return;
                }
                try {
                    if (navigator.clipboard?.writeText) {
                        await navigator.clipboard.writeText(text);
                        getApp().showSuccess('Commands copied to clipboard');
                        return;
                    }
                } catch (error) {
                    console.warn('Clipboard API failed, falling back to execCommand:', error);
                }

                try {
                    const textarea = document.createElement('textarea');
                    textarea.value = text;
                    textarea.setAttribute('readonly', '');
                    textarea.style.position = 'absolute';
                    textarea.style.left = '-9999px';
                    document.body.appendChild(textarea);
                    textarea.select();
                    const success = document.execCommand('copy');
                    textarea.remove();
                    if (success) {
                        getApp().showSuccess('Commands copied to clipboard');
                    } else {
                        window.prompt('Copy commands:', text);
                        getApp().showError('Clipboard unavailable. Copied to prompt.');
                    }
                } catch (error) {
                    window.prompt('Copy commands:', text);
                    getApp().showError('Clipboard unavailable. Copied to prompt.');
                }
            };
            document.getElementById('k8s-skopeo-copy')?.addEventListener('click', copySkopeo);

            document.getElementById('k8s-skopeo-close')?.addEventListener('click', closeModal);
            document.getElementById('k8s-skopeo-cancel')?.addEventListener('click', closeModal);

            replacementsEl?.addEventListener('input', scheduleGenerate);
        };

        document.getElementById('k8s-skopeo-open')?.addEventListener('click', openSkopeoModal);

        document.querySelectorAll('#kubernetes-resource-tabs .nav-link').forEach(btn => {
            btn.addEventListener('click', async () => {
                document.querySelectorAll('#kubernetes-resource-tabs .nav-link').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');
                await loadResources(btn.getAttribute('data-kind'));
            });
        });

        const eventLog = document.getElementById('kubernetes-event-log');
        const logLines = [];
        const renderLog = () => {
            const tail = logLines.slice(-100);
            eventLog.textContent = tail.join('\\n');
            eventLog.scrollTop = eventLog.scrollHeight;
        };
        const appendEvent = (ev) => {
            const ts = ev.timestamp || '';
            const type = ev.type || ev.event_type || '';
            const reason = ev.reason || '';
            const kind = ev.kind || '';
            const name = ev.name || '';
            const msg = ev.message || '';
            const line = `${ts}  ${type}  ${reason}  ${kind}/${name}: ${msg}`.trim();
            logLines.push(line);
            renderLog();
        };

        api.getKubernetesNamespaceEvents(entry.id).then(events => {
            if (Array.isArray(events)) {
                events.reverse().forEach(appendEvent);
            }
        }).catch(() => {});

        let eventSource = null;
        try {
            eventSource = new EventSource(`${api.baseUrl}/kubernetes-namespaces/${entry.id}/events/stream`);
            eventSource.onmessage = (evt) => {
                try {
                    const ev = JSON.parse(evt.data);
                    appendEvent(ev);
                } catch {}
            };
        } catch {}

        setTimeout(loadVersion, 0);
        loadResources('namespaces');

        if (eventSource) {
            window.addEventListener('hashchange', () => eventSource.close(), { once: true });
        }
    } catch (error) {
        console.error(error);
        content.innerHTML = `<div class="alert alert-danger">Failed to load Kubernetes namespace</div>`;
    }
});

router.on('/kubernetes-namespaces/:id/edit', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';
    try {
        const entry = await api.getKubernetesNamespace(params.id);
        const environment = await api.getEnvironment(entry.environment_id);
        const instances = await api.getKubernetesInstances(environment.tenant_id);
        content.innerHTML = createKubernetesNamespaceForm(entry, instances, environment);
        const formEl = document.getElementById('kubernetes-namespace-form');
        formEl.addEventListener('submit', async (e) => {
            await handleFormSubmit(e, async (data) => {
                delete data.environment_id;
                await api.updateKubernetesNamespace(params.id, data);
                router.navigate(`/environments/${environment.id}/edit`);
            });
        });
        document.getElementById('delete-kubernetes-namespace-btn')?.addEventListener('click', async () => {
            const confirmed = await showConfirmDialog(
                'Delete Kubernetes Namespace?',
                `Are you sure you want to delete "${entry.namespace}"?`,
                'Delete',
                'Cancel'
            );
            if (!confirmed) return;
            try {
                await api.deleteKubernetesNamespace(entry.id);
                getApp().showSuccess('Kubernetes namespace deleted');
                router.navigate(`/tenants/${environment.tenant_id}`);
            } catch (error) {
                getApp().showError(error.message);
            }
        });
    } catch (error) {
        console.error(error);
        content.innerHTML = `<div class="alert alert-danger">Failed to load namespace</div>`;
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
                    <a href="#/tenants/${registry.tenant_id}" class="btn btn-ghost-secondary">
                        <i class="ti ti-arrow-left"></i>
                        Back to Tenant
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
                                <div class="text-secondary mb-1">Default Project Path</div>
                                <code>${registry.default_project_path || '-'}</code>
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
        const environments = [];
        content.innerHTML = `
            <div class="row mb-3">
                <div class="col">
                    <a href="#/tenants/${registry.tenant_id}" class="btn btn-ghost-secondary">
                        <i class="ti ti-arrow-left"></i>
                        Back to Tenant
                    </a>
                </div>
            </div>
            ${createRegistryForm(registry, tenants, environments, [], [], [])}
        `;

        document.getElementById('registry-form').addEventListener('submit', async (e) => {
            await handleFormSubmit(e, async (data) => {
                await api.updateRegistry(params.id, data);
                getApp().showSuccess('Registry updated successfully');
                router.navigate(`/tenants/${registry.tenant_id}`);
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

// Environment New/Edit
router.on('/environments/new', async (params, query) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const tenants = await api.getTenants();
        const registries = query.tenant_id ? await api.getRegistries(query.tenant_id).catch(() => []) : await api.getRegistries().catch(() => []);
        const gitRepos = query.tenant_id ? await api.getGitRepos(query.tenant_id).catch(() => []) : await api.getGitRepos().catch(() => []);
        content.innerHTML = createEnvironmentForm(null, tenants, registries, gitRepos);

        if (query.tenant_id) {
            const select = document.querySelector('select[name="tenant_id"]');
            if (select) select.value = query.tenant_id;
        }
        attachEnvironmentColorPreview();
        attachEnvironmentSlugPreview();
        attachEnvironmentVarHandlers();

        document.getElementById('environment-form').addEventListener('submit', async (e) => {
            await handleFormSubmit(e, async (data) => {
                const tenantId = data.tenant_id;
                delete data.tenant_id;
                if (data.argocd_poll_interval_seconds !== undefined) {
                    const parsed = parseInt(data.argocd_poll_interval_seconds, 10);
                    data.argocd_poll_interval_seconds = Number.isFinite(parsed) ? parsed : 0;
                }
                data.release_env_var_mappings = collectEnvironmentVarMappings();
                data.extra_env_vars = collectEnvironmentExtraVars();
                await api.createEnvironment(tenantId, data);
                getApp().showSuccess('Environment created successfully');
                router.navigate(`/tenants/${tenantId}`);
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

router.on('/environments/:id/edit', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const environment = await api.getEnvironment(params.id);
        const [tenants, argocdApps, argocdInstances, kubernetesNamespaces] = await Promise.all([
            api.getTenants(),
            api.getArgocdApps(params.id),
            api.getArgocdInstances(environment.tenant_id),
            api.getKubernetesNamespaces(params.id),
        ]);
        const registries = environment?.tenant_id ? await api.getRegistries(environment.tenant_id).catch(() => []) : [];
        const gitRepos = environment?.tenant_id ? await api.getGitRepos(environment.tenant_id).catch(() => []) : [];
        const appList = `
            <div class="card mt-3">
                <div class="card-header">
                    <h3 class="card-title">ArgoCD Apps</h3>
                    <div class="card-actions">
                        <a href="#/argocd-apps/new?environment_id=${environment.id}" class="btn btn-primary btn-sm">
                            <i class="ti ti-plus"></i>
                            Add
                        </a>
                    </div>
                </div>
                <div class="list-group list-group-flush">
                    ${argocdApps.length === 0 ? `
                        <div class="list-group-item text-center text-secondary py-4">
                            No ArgoCD apps yet
                        </div>
                    ` : argocdApps.map(app => {
                        const instance = argocdInstances.find(i => i.id === app.argocd_instance_id);
                        return `
                            <div class="list-group-item">
                                <div class="d-flex align-items-center justify-content-between">
                                    <div>
                                        <div><a href="#/argocd-apps/${app.id}" class="fw-semibold text-reset">${app.application_name}</a></div>
                                        <div class="text-secondary small">
                                            ${instance ? `${instance.name} · ${instance.base_url}` : 'instance'}
                                        </div>
                                        <div class="text-secondary small">
                                            Project: <code class="small">${app.project_name || 'default'}</code>
                                        </div>
                                    </div>
                                    <div class="d-flex align-items-center gap-2">
                                        <span class="badge ${app.is_active ? 'bg-green-lt text-green-fg' : 'bg-yellow-lt text-yellow-fg'}">
                                            ${app.is_active ? 'active' : 'inactive'}
                                        </span>
                                        <a href="#/argocd-apps/${app.id}/edit" class="btn btn-outline-primary btn-sm">
                                            Edit
                                        </a>
                                    </div>
                                </div>
                            </div>
                        `;
                    }).join('')}
                </div>
            </div>
        `;
        const namespaceList = `
            <div class="card mt-3">
                <div class="card-header">
                    <h3 class="card-title">Kubernetes Namespaces</h3>
                    <div class="card-actions">
                        <a href="#/kubernetes-namespaces/new?environment_id=${environment.id}" class="btn btn-primary btn-sm">
                            <i class="ti ti-plus"></i>
                            Add
                        </a>
                    </div>
                </div>
                <div class="list-group list-group-flush">
                    ${kubernetesNamespaces.length === 0 ? `
                        <div class="list-group-item text-center text-secondary py-4">
                            No Kubernetes namespaces yet
                        </div>
                    ` : kubernetesNamespaces.map(entry => `
                        <a href="#/kubernetes-namespaces/${entry.id}" class="list-group-item list-group-item-action">
                            <div class="d-flex align-items-center justify-content-between">
                                <div>
                                    <div class="fw-semibold">${entry.namespace}</div>
                                    <div class="text-secondary small">
                                        ${entry.instance_name} · ${entry.instance_base_url}
                                    </div>
                                </div>
                                <span class="badge ${entry.is_active ? 'bg-green-lt text-green-fg' : 'bg-yellow-lt text-yellow-fg'}">
                                    ${entry.is_active ? 'active' : 'inactive'}
                                </span>
                            </div>
                        </a>
                    `).join('')}
                </div>
            </div>
        `;
        content.innerHTML = `
            <div class="row mb-3">
                <div class="col">
                    <a href="#/tenants/${environment.tenant_id}" class="btn btn-ghost-secondary">
                        <i class="ti ti-arrow-left"></i>
                        Back to Tenant
                    </a>
                </div>
            </div>
        ` + createEnvironmentForm(environment, tenants, registries, gitRepos) + appList + namespaceList;
        attachEnvironmentColorPreview();
        attachEnvironmentSlugPreview();
        attachEnvironmentVarHandlers();

        document.getElementById('environment-form').addEventListener('submit', async (e) => {
            await handleFormSubmit(e, async (data) => {
                delete data.tenant_id;
                if (data.argocd_poll_interval_seconds !== undefined) {
                    const parsed = parseInt(data.argocd_poll_interval_seconds, 10);
                    data.argocd_poll_interval_seconds = Number.isFinite(parsed) ? parsed : 0;
                }
                data.release_env_var_mappings = collectEnvironmentVarMappings();
                data.extra_env_vars = collectEnvironmentExtraVars();
                await api.updateEnvironment(params.id, data);
                getApp().showSuccess('Environment updated successfully');
                router.navigate(`/tenants/${environment.tenant_id}`);
            });
        });
    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load environment: ${error.message}
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
                                return `
                                <tr>
                                    <td>
                                        <div><a href="#/bundles/${bundle.id}"><strong>${bundle.name}</strong></a></div>
                                        <div class="small text-secondary" style="line-height: 1.2;">
                                            <div class="mt-1">
                                                <i class="ti ti-download" style="font-size: 0.8em;"></i>
                                                <span style="font-size: 0.85em;">${sourceReg?.base_url || 'Unknown'}${sourceReg?.default_project_path ? ` (path: ${sourceReg.default_project_path})` : ''}</span>
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

            const exportBtn = document.getElementById('export-mappings-btn');
            if (exportBtn) {
                exportBtn.addEventListener('click', async () => {
                    wizard.collectStep2Data();
                    await copyMappingsToClipboard(wizard.data.imageMappings);
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

            // Duplicate mapping buttons
            document.querySelectorAll('.mapping-duplicate').forEach((btn, index) => {
                btn.addEventListener('click', () => {
                    wizard.collectStep2Data();
                    wizard.duplicateMapping(index);
                    renderWizard();
                });
            });

            const importBtn = document.getElementById('import-mappings-btn');
            if (importBtn) {
                importBtn.addEventListener('click', async () => {
                    wizard.collectStep2Data();
                    await showMappingImportModal({
                        onApply: (rows) => {
                            wizard.data.imageMappings = rows;
                            renderWizard();
                        },
                    });
                });
            }

            const clearBtn = document.getElementById('clear-mappings-btn');
            if (clearBtn) {
                clearBtn.addEventListener('click', async () => {
                    const confirmed = await showConfirmDialog(
                        'Clear all mappings?',
                        'This will remove all image mappings from this bundle.',
                        'Clear',
                        'Cancel'
                    );
                    if (!confirmed) return;
                    wizard.data.imageMappings = [];
                    renderWizard();
                });
            }
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
        const [versions, copyJobs, releases, deployments, tenant, sourceRegistry, registries, environments] = await Promise.all([
            api.getBundleVersions(params.id),
            api.getBundleCopyJobs(params.id),
            api.getReleases(),
            api.getBundleDeployments(params.id),
            bundle.tenant_id ? api.getTenant(bundle.tenant_id).catch(() => null) : null,
            bundle.source_registry_id ? api.getRegistry(bundle.source_registry_id).catch(() => null) : null,
            api.getRegistries().catch(() => []),
            bundle.tenant_id ? api.getEnvironments(bundle.tenant_id).catch(() => []) : Promise.resolve([]),
        ]);

        const registryMap = {};
        (registries || []).forEach(r => {
            registryMap[r.id] = r;
        });
        const environmentMap = new Map((environments || []).map(env => [env.id, env]));

        const latestVersion = versions.length > 0
            ? Math.max(...versions.map(v => v.version))
            : null;
        const latestSuccessJob = copyJobs
            .filter(job => job.status === 'success' && !job.is_release_job)
            .sort((a, b) => new Date(b.started_at).getTime() - new Date(a.started_at).getTime())[0];
        const releasesForBundle = (releases || []).filter(r => r.bundle_id === bundle.id);
        const releaseByCopyJobId = new Map();
        releasesForBundle.forEach(r => {
            if (!releaseByCopyJobId.has(r.copy_job_id)) {
                releaseByCopyJobId.set(r.copy_job_id, r);
            }
        });

        content.innerHTML = `
            ${tenant?.id ? `
                <div class="row mb-3">
                    <div class="col">
                        <a href="#/tenants/${tenant.id}" class="btn btn-ghost-secondary">
                            <i class="ti ti-arrow-left"></i>
                            Back to Tenant
                        </a>
                    </div>
                </div>
            ` : `
                <div class="row mb-3">
                    <div class="col">
                        <a href="#/bundles" class="btn btn-ghost-secondary">
                            <i class="ti ti-arrow-left"></i>
                            Back to Bundles
                        </a>
                    </div>
                </div>
            `}

            <div class="row">
                <div class="col-12">
                    <div class="card mb-3">
                        <div class="card-header">
                            <div>
                                <h3 class="card-title mb-1">${bundle.name}</h3>
                                <div class="text-secondary small">
                                    <div>${tenant?.name ? `Tenant: <strong>${tenant.name}</strong>` : 'Tenant: -'}</div>
                                    <div>${sourceRegistry?.base_url ? `Source: <code>${sourceRegistry.base_url}${sourceRegistry.default_project_path ? ` (path: ${sourceRegistry.default_project_path})` : ''}</code>` : 'Source: -'}</div>
                                </div>
                            </div>
                            <div class="card-actions">
                                <a href="#/bundles/${bundle.id}/versions/new" class="btn btn-primary btn-sm">
                                    <i class="ti ti-plus"></i>
                                    New Version
                                </a>
                                    <a href="#/bundles/${bundle.id}/copy" class="btn btn-success btn-sm">
                                        <i class="ti ti-copy"></i>
                                        Duplicate Bundle
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

                    <div class="card mb-3">
                        <div class="card-header">
                            <h3 class="card-title">How It Works</h3>
                        </div>
                        <div class="card-body">
                            <ol class="mb-0">
                                <li class="mb-2">
                                    Start a copy job from the latest bundle version.
                                    ${latestVersion ? `
                                        <a href="#/bundles/${bundle.id}/versions/${latestVersion}/copy" class="btn btn-sm btn-outline-primary ms-2">
                                            Start Copy Job
                                        </a>
                                    ` : ''}
                                </li>
                                <li class="mb-2">
                                    From a successful copy job, create a release.
                                    ${latestSuccessJob ? `
                                        <a href="#/releases/new?copy_job_id=${latestSuccessJob.job_id}" class="btn btn-sm btn-outline-success ms-2">
                                            Release Images
                                        </a>
                                    ` : `
                                        <span class="text-secondary ms-2">No successful copy jobs yet</span>
                                    `}
                                </li>
                                <li>
                                    Build deploy from the release to regenerate <code>tsm-deploy/deploy/&lt;env&gt;</code>.
                                    ${releasesForBundle.length > 0 ? `
                                        <a href="#/releases/${releasesForBundle[0].id}" class="btn btn-sm btn-outline-secondary ms-2">
                                            View Image Release
                                        </a>
                                    ` : ''}
                                </li>
                            </ol>
                        </div>
                    </div>

                    <div class="card">
                        <div class="card-header">
                            <h3 class="card-title">
                                <i class="ti ti-layers me-2"></i>
                                Versions
                            </h3>
                            ${versions.some(v => v.is_archived) ? `
                                <div class="card-actions">
                                    <button class="btn btn-ghost-secondary btn-sm" id="toggle-archived-versions">
                                        Show archived (${versions.filter(v => v.is_archived).length})
                                    </button>
                                </div>
                            ` : ''}
                        </div>
                        <div class="list-group list-group-flush">
                            ${versions.filter(v => !v.is_archived).map(version => `
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
                        ${versions.some(v => v.is_archived) ? `
                            <div class="list-group list-group-flush d-none" id="archived-versions">
                                ${versions.filter(v => v.is_archived).map(version => `
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
                                                <span class="badge bg-secondary text-secondary-fg">archived</span>
                                            </div>
                                        </div>
                                    </a>
                                `).join('')}
                            </div>
                        ` : ''}
                    </div>

                    <div class="card mt-3">
                        <div class="card-header">
                            <h3 class="card-title">
                                <i class="ti ti-copy me-2"></i>
                                Copy History
                            </h3>
                        </div>
                        <div class="table-responsive">
                            <table class="table table-vcenter card-table">
                                <thead>
                                    <tr>
                                        <th>Version</th>
                                        <th>Target Tag</th>
                                        <th>Release</th>
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
                                            <td colspan="8" class="text-center text-secondary py-4">
                                                No copy jobs yet.
                                            </td>
                                        </tr>
                                    ` : copyJobs.map(job => `
                                        <tr>
                                            <td>
                                                <span class="badge bg-blue text-blue-fg">v${job.version}</span>
                                                ${job.validate_only ? '<span class="badge bg-azure-lt text-azure-fg ms-2">validate</span>' : ''}
                                                ${job.is_selective ? '<span class="badge bg-purple-lt text-purple-fg ms-2">selective</span>' : ''}
                                            </td>
                                            <td>
                                                <a href="#/copy-jobs/${job.job_id}"><span class="badge bg-azure-lt">${job.target_tag}</span></a>
                                                <div class="text-secondary small mt-1">
                                                    ${
                                                        job.environment_id && environmentMap.get(job.environment_id)
                                                            ? `Env: <span class="badge" style="${environmentMap.get(job.environment_id).color ? `background:${environmentMap.get(job.environment_id).color};color:#fff;` : ''}">${environmentMap.get(job.environment_id).name}</span>`
                                                            : 'Env: -'
                                                    }
                                                </div>
                                                <div class="text-secondary small">
                                                    <div>${job.source_registry_id ? `Source: <code>${registryMap[job.source_registry_id]?.base_url || '-'}${registryMap[job.source_registry_id]?.default_project_path ? ` (path: ${registryMap[job.source_registry_id]?.default_project_path})` : ''}</code>` : 'Source: -'}</div>
                                                    <div>${job.target_registry_id ? `Target: <code>${registryMap[job.target_registry_id]?.base_url || '-'}${registryMap[job.target_registry_id]?.default_project_path ? ` (path: ${registryMap[job.target_registry_id]?.default_project_path})` : ''}</code>` : 'Target: -'}</div>
                                                </div>
                                            </td>
                                            <td>
                                                ${(() => {
                                                    const release = releaseByCopyJobId.get(job.job_id);
                                                    if (!release) {
                                                        return '<span class="text-secondary">-</span>';
                                                    }
                                                    return `
                                                        <a href="#/releases/${release.id}"><strong>${release.release_id}</strong></a>
                                                        ${release.is_auto ? '<span class="badge bg-azure-lt text-azure-fg ms-2">auto</span>' : ''}
                                                    `;
                                                })()}
                                            </td>
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
                                                    <span class="badge bg-purple-lt text-purple-fg">image release</span>
                                                ` : job.status === 'success' ? `
                                                    <div class="d-flex flex-column gap-1">
                                                        <a href="#/releases/new?copy_job_id=${job.job_id}" class="btn btn-sm btn-success">
                                                            <i class="ti ti-rocket"></i>
                                                            Release Images
                                                        </a>
                                                        <button type="button" class="btn btn-sm btn-outline-secondary selective-copy-btn" data-job-id="${job.job_id}">
                                                            <i class="ti ti-adjustments"></i>
                                                            Selective Copy
                                                        </button>
                                                        <button type="button" class="btn btn-sm btn-outline-primary auto-deploy-btn" data-job-id="${job.job_id}" data-target-tag="${job.target_tag}">
                                                            <i class="ti ti-rocket"></i>
                                                            Deploy Action
                                                        </button>
                                                    </div>
                                                ` : ''}
                                            </td>
                                        </tr>
                                    `).join('')}
                                </tbody>
                            </table>
                        </div>
                    </div>

                    <div class="card mt-3">
                        <div class="card-header">
                            <h3 class="card-title">
                                <i class="ti ti-rocket me-2"></i>
                                Deployments
                            </h3>
                        </div>
                        <div class="table-responsive">
                            <table class="table table-vcenter card-table">
                                <thead>
                                    <tr>
                                        <th>Job</th>
                                        <th>Release</th>
                                        <th>Target</th>
                                        <th>Status</th>
                                        <th>Started</th>
                                        <th>Completed</th>
                                        <th>Tag</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    ${deployments.length === 0 ? `
                                        <tr>
                                            <td colspan="7" class="text-center text-secondary py-4">
                                                No deployments yet.
                                            </td>
                                        </tr>
                                    ` : deployments.map(row => `
                                        <tr>
                                            <td>
                                                <a href="#/deploy-jobs/${row.id}"><code class="small">${row.id}</code></a>
                                            </td>
                                            <td>
                                                <a href="#/releases/${row.release_db_id}"><strong>${row.release_id}</strong></a>
                                                ${row.is_auto ? '<span class="badge bg-azure-lt text-azure-fg ms-2">auto</span>' : ''}
                                            </td>
                                            <td>${row.target_name} (${row.env_name})</td>
                                            <td>
                                                <span class="badge ${
                                                    row.status === 'success' ? 'bg-success text-success-fg' :
                                                    row.status === 'failed' ? 'bg-danger text-danger-fg' :
                                                    row.status === 'in_progress' ? 'bg-info text-info-fg' :
                                                    'bg-secondary text-secondary-fg'
                                                }">${row.status}</span>
                                            </td>
                                            <td>${new Date(row.started_at).toLocaleString('cs-CZ')}</td>
                                            <td>${row.completed_at ? new Date(row.completed_at).toLocaleString('cs-CZ') : '-'}</td>
                                            <td>${row.tag_name ? `<code class="small">${row.tag_name}</code>` : '-'}</td>
                                        </tr>
                                    `).join('')}
                                </tbody>
                            </table>
                        </div>
                    </div>
                </div>

            </div>
        `;

        const archivedVersionsToggle = document.getElementById('toggle-archived-versions');
        if (archivedVersionsToggle) {
            archivedVersionsToggle.addEventListener('click', () => {
                const container = document.getElementById('archived-versions');
                if (!container) return;
                const isHidden = container.classList.contains('d-none');
                container.classList.toggle('d-none');
                archivedVersionsToggle.textContent = isHidden
                    ? 'Hide archived'
                    : `Show archived (${versions.filter(v => v.is_archived).length})`;
            });
        }

        document.querySelectorAll('.auto-deploy-btn').forEach(btn => {
            btn.addEventListener('click', async () => {
                const jobId = btn.getAttribute('data-job-id');
                const targetTag = btn.getAttribute('data-target-tag');
                await runAutoDeployFromCopyJob(jobId, tenant?.id, targetTag);
            });
        });

        document.querySelectorAll('.selective-copy-btn').forEach(btn => {
            btn.addEventListener('click', async () => {
                const jobId = btn.getAttribute('data-job-id');
                await runSelectiveCopyFromJob(jobId, bundle);
            });
        });

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
                            <label class="form-label">Source Registry</label>
                            <div class="form-control-plaintext">
                                <code class="small">${sourceRegistry?.base_url || '-'}</code>
                                ${sourceRegistry?.default_project_path ? `<span class="text-secondary small ms-1">(path: ${sourceRegistry.default_project_path})</span>` : ''}
                            </div>
                        </div>

                        <div class="mb-3">
                            <label class="form-label">Description</label>
                            <textarea class="form-control" name="description" rows="3">${bundle.description || ''}</textarea>
                        </div>

                        <div class="mb-3">
                            <label class="form-check">
                                <input type="checkbox" class="form-check-input" name="auto_tag_enabled" ${bundle.auto_tag_enabled ? 'checked' : ''}>
                                <span class="form-check-label">Auto-generate target tag (YYYY.MM.DD.COUNTER)</span>
                            </label>
                            <small class="form-hint">Locks target tag input when starting copy jobs</small>
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
                    auto_tag_enabled: data.auto_tag_enabled === 'on' || data.auto_tag_enabled === true,
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
            title: 'Duplicate Bundle',
            createLabel: 'Create Duplicate',
            tenantLocked: true,
            enableReplaceRules: false,
            showRegistrySelectors: false,
        });

        wizard.data.bundle.tenant_id = bundle.tenant_id;
        wizard.data.bundle.name = `${bundle.name} Copy`;
        wizard.data.bundle.description = bundle.description || '';
        wizard.data.bundle.source_registry_id = bundle.source_registry_id;
        wizard.data.bundle.auto_tag_enabled = bundle.auto_tag_enabled;
        wizard.data.imageMappings = mappings.map(m => ({
            source_image: m.source_image,
            source_tag: m.source_tag,
            target_image: m.target_image,
            app_name: m.app_name,
            container_name: m.container_name,
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
                            app_name: m.app_name,
                            container_name: m.container_name,
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

            const exportBtn = document.getElementById('export-mappings-btn');
            if (exportBtn) {
                exportBtn.addEventListener('click', async () => {
                    wizard.collectStep2Data();
                    await copyMappingsToClipboard(wizard.data.imageMappings);
                });
            }

            document.querySelectorAll('.mapping-remove').forEach((btn, index) => {
                btn.addEventListener('click', () => {
                    wizard.collectStep2Data();
                    wizard.removeMapping(index);
                    renderWizard();
                });
            });

            document.querySelectorAll('.mapping-duplicate').forEach((btn, index) => {
                btn.addEventListener('click', () => {
                    wizard.collectStep2Data();
                    wizard.duplicateMapping(index);
                    renderWizard();
                });
            });

            const replaceAddBtn = document.getElementById('replace-add-btn');
            if (replaceAddBtn) {
                replaceAddBtn.addEventListener('click', () => {
                    wizard.collectReplaceRules();
                    wizard.data.replaceRules.push({ find: '', replace: '' });
                    renderWizard();
                });
            }

            document.querySelectorAll('.replace-remove').forEach((btn, index) => {
                btn.addEventListener('click', () => {
                    wizard.collectReplaceRules();
                    if (wizard.data.replaceRules.length > 1) {
                        wizard.data.replaceRules.splice(index, 1);
                        renderWizard();
                    }
                });
            });

            const applyReplaceBtn = document.getElementById('apply-replace-btn');
            if (applyReplaceBtn) {
                applyReplaceBtn.addEventListener('click', async () => {
                    const confirmed = await showConfirmDialog(
                        'Apply replace rules?',
                        'This will update all target image paths.',
                        'Apply',
                        'Cancel'
                    );
                    if (!confirmed) return;
                    wizard.collectReplaceRules();
                    wizard.collectStep2Data();
                    wizard.applyReplaceRulesToMappings();
                    renderWizard();
                });
            }

            const importBtn = document.getElementById('import-mappings-btn');
            if (importBtn) {
                importBtn.addEventListener('click', async () => {
                    wizard.collectStep2Data();
                    await showMappingImportModal({
                        onApply: (rows) => {
                            wizard.data.imageMappings = rows;
                            renderWizard();
                        },
                    });
                });
            }

            const clearBtn = document.getElementById('clear-mappings-btn');
            if (clearBtn) {
                clearBtn.addEventListener('click', async () => {
                    const confirmed = await showConfirmDialog(
                        'Clear all mappings?',
                        'This will remove all image mappings from this bundle.',
                        'Clear',
                        'Cancel'
                    );
                    if (!confirmed) return;
                    wizard.data.imageMappings = [];
                    renderWizard();
                });
            }
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
                app_name: m.app_name,
                container_name: m.container_name,
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
                                                <div class="d-flex flex-column gap-1 w-100">
                                                    <button type="button" class="btn btn-sm btn-ghost-primary w-100 mapping-duplicate">
                                                        <i class="ti ti-copy"></i>
                                                    </button>
                                                    <button type="button" class="btn btn-sm btn-ghost-danger w-100 mapping-remove">
                                                        <i class="ti ti-trash"></i>
                                                    </button>
                                                </div>
                                            </div>
                                        </div>
                                        <div class="row g-2 mt-2">
                                            <div class="col-md-6">
                                                <label class="form-label">App Name</label>
                                                <input type="text" class="form-control form-control-sm mapping-app-name"
                                                       value="${mapping.app_name || ''}"
                                                       placeholder="app name">
                                            </div>
                                            <div class="col-md-5">
                                                <label class="form-label">Container Name</label>
                                                <input type="text" class="form-control form-control-sm mapping-container-name"
                                                       value="${mapping.container_name || ''}"
                                                       placeholder="container name (optional)">
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            `).join('')}
                        </div>

                        <div class="d-flex flex-wrap gap-2">
                            <button type="button" class="btn btn-primary" id="add-mapping-btn">
                                <i class="ti ti-plus"></i>
                                Add Image Mapping
                            </button>
                            <button type="button" class="btn btn-outline-primary" id="import-mappings-btn">
                                <i class="ti ti-file-import"></i>
                                Import from CSV
                            </button>
                            <button type="button" class="btn btn-outline-danger" id="clear-mappings-btn">
                                <i class="ti ti-trash"></i>
                                Clear All
                            </button>
                        </div>
                        <div class="text-secondary small mt-2">
                            Import format: <code>source_image;source_tag;target_image;app_name;container_name</code>
                        </div>

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
                    let sourceTag = card.querySelector('.mapping-source-tag')?.value || '';
                    if (!sourceTag) sourceTag = 'latest';
                    const targetImage = card.querySelector('.mapping-target-image')?.value || '';
                    let appName = card.querySelector('.mapping-app-name')?.value || '';
                    const containerName = card.querySelector('.mapping-container-name')?.value || '';
                    if (!appName && targetImage) {
                        const parts = targetImage.split('/');
                        appName = parts[parts.length - 1] || '';
                    }
                    mappings.push({
                        source_image: sourceImage,
                        source_tag: sourceTag,
                        target_image: targetImage,
                        app_name: appName,
                        container_name: containerName,
                    });
                });
                state.mappings = mappings;
            };

            const addBtn = document.getElementById('add-mapping-btn');
            if (addBtn) {
                addBtn.addEventListener('click', () => {
                    collectMappings();
                    state.mappings.push({
                        source_image: '',
                        source_tag: '',
                        target_image: '',
                        app_name: '',
                        container_name: '',
                    });
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

            document.querySelectorAll('.mapping-duplicate').forEach((btn, index) => {
                btn.addEventListener('click', () => {
                    collectMappings();
                    const current = state.mappings[index];
                    if (current) {
                        state.mappings.splice(index + 1, 0, { ...current });
                        render();
                    }
                });
            });

            const importBtn = document.getElementById('import-mappings-btn');
            if (importBtn) {
                importBtn.addEventListener('click', async () => {
                    collectMappings();
                    await showMappingImportModal({
                        onApply: (rows) => {
                            state.mappings = rows;
                            render();
                        },
                    });
                });
            }

            const clearBtn = document.getElementById('clear-mappings-btn');
            if (clearBtn) {
                clearBtn.addEventListener('click', async () => {
                    const confirmed = await showConfirmDialog(
                        'Clear all mappings?',
                        'This will remove all image mappings from this version.',
                        'Clear',
                        'Cancel'
                    );
                    if (!confirmed) return;
                    state.mappings = [];
                    render();
                });
            }

            const createBtn = document.getElementById('create-version-btn');
            createBtn.addEventListener('click', async () => {
                collectMappings();
                const validMappings = state.mappings.filter(m =>
                    m.source_image && (m.source_tag || 'latest') && m.target_image && m.app_name
                );
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
        const [bundle, version, mappings, copyJobs, registries] = await Promise.all([
            api.getBundle(params.id),
            api.getBundleVersion(params.id, params.version),
            api.getImageMappings(params.id, params.version),
            api.getBundleCopyJobs(params.id),
            api.getRegistries().catch(() => []),
        ]);

        const registryMap = {};
        (registries || []).forEach(r => {
            registryMap[r.id] = r;
        });

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
                            Start Copy Job
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
                    <div class="card-actions">
                        <button class="btn btn-sm btn-outline-primary" id="export-mappings-btn">
                            <i class="ti ti-clipboard-copy"></i>
                            Export to clipboard
                        </button>
                    </div>
                </div>
                <div class="table-responsive">
                    <table class="table table-vcenter card-table">
                        <thead>
                            <tr>
                                <th>Source Image</th>
                                <th>Target Image</th>
                                <th>App</th>
                                <th>Container</th>
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
                                    <td>${mapping.app_name || '-'}</td>
                                    <td>${mapping.container_name || '-'}</td>
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
                                    <td>
                                        <a href="#/copy-jobs/${job.job_id}"><span class="badge bg-azure-lt">${job.target_tag}</span></a>
                                        ${job.validate_only ? '<span class="badge bg-azure-lt text-azure-fg ms-2">validate</span>' : ''}
                                        ${job.is_selective ? '<span class="badge bg-purple-lt text-purple-fg ms-2">selective</span>' : ''}
                                        <div class="text-secondary small mt-1">
                                            ${job.source_registry_id ? `Source: <code class="small">${registryMap[job.source_registry_id]?.base_url || '-'}${registryMap[job.source_registry_id]?.default_project_path ? ` (path: ${registryMap[job.source_registry_id]?.default_project_path})` : ''}</code>` : 'Source: -'}
                                        </div>
                                        <div class="text-secondary small">
                                            ${job.target_registry_id ? `Target: <code class="small">${registryMap[job.target_registry_id]?.base_url || '-'}${registryMap[job.target_registry_id]?.default_project_path ? ` (path: ${registryMap[job.target_registry_id]?.default_project_path})` : ''}</code>` : 'Target: -'}
                                        </div>
                                    </td>
                                    <td>
                                        <span class="badge ${
                                            job.status === 'success' ? 'bg-success text-success-fg' :
                                            job.status === 'failed' ? 'bg-danger text-danger-fg' :
                                            job.status === 'in_progress' ? 'bg-info text-info-fg' :
                                            job.status === 'pending' ? 'bg-warning text-warning-fg' :
                                            job.status === 'cancelled' || job.status === 'canceled' ? 'bg-warning text-warning-fg' :
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
                                            <div class="d-flex flex-column gap-1">
                                                <a href="#/releases/new?copy_job_id=${job.job_id}" class="btn btn-sm btn-success">
                                                    <i class="ti ti-rocket"></i>
                                                    Release Images
                                                </a>
                                                <button type="button" class="btn btn-sm btn-outline-secondary selective-copy-btn" data-job-id="${job.job_id}">
                                                    <i class="ti ti-adjustments"></i>
                                                    Selective Copy
                                                </button>
                                                <button type="button" class="btn btn-sm btn-outline-primary auto-deploy-btn" data-job-id="${job.job_id}" data-target-tag="${job.target_tag}">
                                                    <i class="ti ti-rocket"></i>
                                                    Deploy Action
                                                </button>
                                            </div>
                                        ` : job.is_release_job ? `
                                            <span class="badge bg-purple-lt text-purple-fg">image release</span>
                                        ` : ''}
                                    </td>
                                </tr>
                            `).join('')}
                        </tbody>
                    </table>
                </div>
            </div>
        `;

        document.querySelectorAll('.auto-deploy-btn').forEach(btn => {
            btn.addEventListener('click', async () => {
                const jobId = btn.getAttribute('data-job-id');
                const targetTag = btn.getAttribute('data-target-tag');
                await runAutoDeployFromCopyJob(jobId, bundle?.tenant_id, targetTag);
            });
        });

        document.querySelectorAll('.selective-copy-btn').forEach(btn => {
            btn.addEventListener('click', async () => {
                const jobId = btn.getAttribute('data-job-id');
                await runSelectiveCopyFromJob(jobId, bundle);
            });
        });

        const exportBtn = document.getElementById('export-mappings-btn');
        if (exportBtn) {
            exportBtn.addEventListener('click', async () => {
                await copyMappingsToClipboard(mappings);
            });
        }

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
        const [releases, tenants, bundles] = await Promise.all([
            api.getReleases(),
            api.getTenants(),
            api.getBundles(),
        ]);

        const tenantIds = [...new Set(bundles.map(b => b.tenant_id).filter(Boolean))];
        const environmentLists = await Promise.all(
            tenantIds.map(id => api.getEnvironments(id).catch(() => []))
        );
        const environmentMap = new Map();
        environmentLists.flat().forEach(env => {
            environmentMap.set(env.id, env);
        });

        let selectedReleases = new Set();
        const releaseById = new Map(releases.map(r => [r.id, r]));

        const renderReleases = (rows, searchQuery = '', selectedTenant = '', selectedBundle = '', selectedEnv = '') => {
            const filteredBundles = bundles.filter(b => !selectedTenant || b.tenant_id === selectedTenant);
            const filteredEnvs = Array.from(environmentMap.values())
                .filter(env => !selectedTenant || env.tenant_id === selectedTenant)
                .sort((a, b) => a.name.localeCompare(b.name));
            return `
                <div class="card">
                    <div class="card-header">
                        <h3 class="card-title">Image Releases</h3>
                        <div class="card-actions">
                            <button class="btn btn-outline-secondary" id="releases-compare" ${selectedReleases.size === 2 ? '' : 'disabled'}>
                                <i class="ti ti-arrows-diff"></i>
                                <span id="releases-compare-label">Compare (${selectedReleases.size}/2)</span>
                            </button>
                            <a href="#/releases/new" class="btn btn-primary">
                                <i class="ti ti-plus"></i>
                                New Image Release
                            </a>
                        </div>
                    </div>
                    <div class="card-body border-bottom py-3">
                        <div class="row g-2">
                        <div class="col-md-3">
                            <div class="input-group">
                                <span class="input-group-text">
                                    <i class="ti ti-search"></i>
                                </span>
                                <input class="form-control" id="releases-search" placeholder="Search by release id..."
                                       value="${searchQuery}">
                            </div>
                        </div>
                        <div class="col-md-3">
                            <select class="form-select" id="releases-tenant">
                                <option value="">All Tenants</option>
                                ${tenants.map(t => `
                                    <option value="${t.id}" ${t.id === selectedTenant ? 'selected' : ''}>${t.name}</option>
                                `).join('')}
                            </select>
                        </div>
                        <div class="col-md-3">
                            <select class="form-select" id="releases-env">
                                <option value="">All Environments</option>
                                ${filteredEnvs.map(env => `
                                    <option value="${env.id}" ${env.id === selectedEnv ? 'selected' : ''}>${env.name}</option>
                                `).join('')}
                            </select>
                        </div>
                        <div class="col-md-3">
                            <select class="form-select" id="releases-bundle">
                                <option value="">All Bundles</option>
                                ${filteredBundles.map(b => `
                                    <option value="${b.id}" ${b.id === selectedBundle ? 'selected' : ''}>${b.name}</option>
                                `).join('')}
                            </select>
                        </div>
                    </div>
                </div>
                    <div class="table-responsive">
                        <table class="table table-vcenter card-table table-hover">
                            <thead>
                                <tr>
                                    <th class="w-1"></th>
                                    <th>Image Release ID</th>
                                    <th>Tenant</th>
                                    <th>Bundle</th>
                                    <th>
                                        Deploy Jobs
                                        <i class="ti ti-info-circle text-secondary ms-1" title="Counts of deploy jobs (success/failed/running/pending). Image release stays draft until first successful deploy."></i>
                                    </th>
                                    <th>Created</th>
                                </tr>
                            </thead>
                            <tbody>
                                ${rows.length === 0 ? `
                                    <tr>
                                        <td colspan="6" class="text-center text-secondary py-5">
                                            No image releases yet. Create one from a successful copy job.
                                        </td>
                                    </tr>
                                ` : rows.map(release => {
                                    const total = Number(release.deploy_total || 0);
                                    const parts = [];
                                    if (release.deploy_success) parts.push(`<span class="badge bg-success-lt text-success-fg">success ${release.deploy_success}</span>`);
                                    if (release.deploy_failed) parts.push(`<span class="badge bg-danger-lt text-danger-fg">failed ${release.deploy_failed}</span>`);
                                    if (release.deploy_in_progress) parts.push(`<span class="badge bg-info-lt text-info-fg">running ${release.deploy_in_progress}</span>`);
                                    if (release.deploy_pending) parts.push(`<span class="badge bg-secondary-lt text-secondary-fg">pending ${release.deploy_pending}</span>`);
                                    const summary = total === 0
                                        ? `<span class="text-secondary">No deploys</span>`
                                        : `<span class="badge bg-azure-lt text-azure-fg me-1">total ${total}</span> ${parts.join(' ')}`;
                                    const isAuto = release.is_auto === true
                                        || release.is_auto === 1
                                        || release.is_auto === 't'
                                        || release.is_auto === 'true'
                                        || release.isAuto === true
                                        || release.auto === true;
                                    return `
                                        <tr>
                                            <td>
                                                <input class="form-check-input release-select" type="checkbox" value="${release.id}" ${selectedReleases.has(release.id) ? 'checked' : ''}>
                                            </td>
                                            <td>
                                                <a href="#/releases/${release.id}"><strong>${release.release_id}</strong></a>
                                                ${isAuto ? '<span class="badge bg-azure-lt text-azure-fg ms-2">auto</span>' : ''}
                                                <span class="badge bg-azure-lt text-azure-fg ms-2">${release.source_ref_mode || 'tag'}</span>
                                                <div class="text-secondary small mt-1">
                                                    ${
                                                        release.environment_id && environmentMap.get(release.environment_id)
                                                            ? `Env: <span class="badge" style="${environmentMap.get(release.environment_id).color ? `background:${environmentMap.get(release.environment_id).color};color:#fff;` : ''}">${environmentMap.get(release.environment_id).name}</span>`
                                                            : 'Env: -'
                                                    }
                                                </div>
                                            </td>
                                            <td>
                                                ${release.tenant_id ? `<a href="#/tenants/${release.tenant_id}">${release.tenant_name || 'Unknown'}</a>` : (release.tenant_name || 'Unknown')}
                                            </td>
                                            <td>
                                                ${release.bundle_id ? `<a href="#/bundles/${release.bundle_id}">${release.bundle_name || '-'}</a>` : (release.bundle_name || '-')}
                                            </td>
                                            <td>${summary}</td>
                                            <td>${new Date(release.created_at).toLocaleDateString('cs-CZ')}</td>
                                        </tr>
                                    `;
                                }).join('')}
                            </tbody>
                        </table>
                    </div>
                </div>
            `;
        };

        content.innerHTML = renderReleases(releases);

        const showReleaseCompareModal = (releaseA, releaseB, rows) => {
            const modalHtml = `
                <div class="modal modal-blur fade show" style="display: block;" id="releases-compare-modal">
                    <div class="modal-dialog modal-xl modal-dialog-centered" role="document">
                        <div class="modal-content">
                            <div class="modal-header">
                                <h5 class="modal-title">Release Digest Comparison</h5>
                                <button type="button" class="btn-close" data-modal-close></button>
                            </div>
                            <div class="modal-body">
                                <div class="text-secondary small mb-3">
                                    Release A: <code>${releaseA.release_id}</code><br>
                                    Release B: <code>${releaseB.release_id}</code>
                                </div>
                                <div class="text-secondary small mb-3" id="release-compare-summary"></div>
                                <div class="row g-2 mb-3">
                                    <div class="col-auto">
                                        <label class="form-check form-check-inline">
                                            <input class="form-check-input" type="checkbox" id="release-compare-changes">
                                            <span class="form-check-label">Changes</span>
                                        </label>
                                    </div>
                                    <div class="col-auto">
                                        <label class="form-check form-check-inline">
                                            <input class="form-check-input" type="checkbox" id="release-compare-missing">
                                            <span class="form-check-label">Missing</span>
                                        </label>
                                    </div>
                                    <div class="col-auto">
                                        <label class="form-check form-check-inline">
                                            <input class="form-check-input" type="checkbox" id="release-compare-new">
                                            <span class="form-check-label">New</span>
                                        </label>
                                    </div>
                                </div>
                                <div class="table-responsive">
                                    <table class="table table-vcenter">
                                        <thead>
                                            <tr>
                                                <th>App</th>
                                                <th>Container</th>
                                                <th>Digest A</th>
                                                <th>Digest B</th>
                                                <th>Status</th>
                                            </tr>
                                        </thead>
                                        <tbody id="release-compare-body"></tbody>
                                    </table>
                                </div>
                            </div>
                            <div class="modal-footer">
                                <button type="button" class="btn btn-secondary" data-modal-close>Close</button>
                            </div>
                        </div>
                    </div>
                </div>
                <div class="modal-backdrop fade show"></div>
            `;

            document.body.insertAdjacentHTML('beforeend', modalHtml);
            const modal = document.getElementById('releases-compare-modal');
            const backdrop = document.querySelector('.modal-backdrop');
            const closeModal = () => {
                modal?.remove();
                backdrop?.remove();
            };
            modal.querySelectorAll('[data-modal-close]').forEach(btn => btn.addEventListener('click', closeModal));

            const shortDigest = (value) => {
                if (!value) return '';
                const cleaned = value.startsWith('sha256:') ? value.slice(7) : value;
                return cleaned.slice(0, 7);
            };
            const digestCell = (value) => {
                if (!value) return '-';
                const short = shortDigest(value);
                return `
                    <div class="d-flex align-items-center gap-2">
                        <code class="small" title="${value}">${short}</code>
                        <button type="button" class="btn btn-ghost-secondary btn-sm copy-digest" data-digest="${value}">
                            <i class="ti ti-copy"></i>
                        </button>
                    </div>
                `;
            };
            const renderRows = (filtered) => {
                const body = modal.querySelector('#release-compare-body');
                if (!body) return;
                if (!filtered.length) {
                    body.innerHTML = '<tr><td colspan="5" class="text-center text-secondary">No data</td></tr>';
                    return;
                }
                body.innerHTML = filtered.map(row => {
                    const status = row.status;
                    const badgeClass =
                        status === 'same' ? 'bg-success text-success-fg' :
                        status === 'changed' ? 'bg-warning text-warning-fg' :
                        status === 'missing_in_a' ? 'bg-danger text-danger-fg' :
                        status === 'missing_in_b' ? 'bg-danger text-danger-fg' :
                        'bg-secondary text-secondary-fg';
                    const label = status.replaceAll('_', ' ');
                    return `
                        <tr>
                            <td><code class="small">${row.app_name}</code></td>
                            <td><code class="small">${row.container_name}</code></td>
                            <td>${digestCell(row.digest_a)}</td>
                            <td>${digestCell(row.digest_b)}</td>
                            <td><span class="badge ${badgeClass}">${label}</span></td>
                        </tr>
                    `;
                }).join('');
            };
            const renderSummary = (allRows, filteredRows) => {
                const summaryEl = modal.querySelector('#release-compare-summary');
                if (!summaryEl) return;
                const countBy = (rows, status) => rows.filter(r => r.status === status).length;
                const total = allRows.length;
                const same = countBy(filteredRows, 'same');
                const changed = countBy(filteredRows, 'changed');
                const missingA = countBy(filteredRows, 'missing_in_a');
                const missingB = countBy(filteredRows, 'missing_in_b');
                summaryEl.innerHTML = `
                    Showing <strong>${filteredRows.length}</strong> / ${total} &nbsp;|&nbsp;
                    <span class="text-success">same: ${same}</span>
                    &nbsp;|&nbsp;
                    <span class="text-warning">changes: ${changed}</span>
                    &nbsp;|&nbsp;
                    <span class="text-danger">missing: ${missingB}</span>
                    &nbsp;|&nbsp;
                    <span class="text-info">new: ${missingA}</span>
                `;
            };
            const applyFilters = () => {
                const onlyChanges = modal.querySelector('#release-compare-changes')?.checked;
                const onlyMissing = modal.querySelector('#release-compare-missing')?.checked;
                const onlyNew = modal.querySelector('#release-compare-new')?.checked;
                let filtered = rows.slice();
                const active = [];
                if (onlyChanges) active.push('changed');
                if (onlyMissing) active.push('missing_in_b');
                if (onlyNew) active.push('missing_in_a');
                if (active.length > 0) {
                    filtered = filtered.filter(r => active.includes(r.status));
                }
                renderRows(filtered);
                renderSummary(rows, filtered);
                modal.querySelectorAll('.copy-digest').forEach(btn => {
                    btn.addEventListener('click', async () => {
                        const value = btn.getAttribute('data-digest') || '';
                        if (!value) return;
                        try {
                            await navigator.clipboard.writeText(value);
                            app.showSuccess('Digest copied to clipboard');
                        } catch (_) {
                            app.showError('Failed to copy digest');
                        }
                    });
                });
            };

            ['release-compare-changes', 'release-compare-missing', 'release-compare-new'].forEach(id => {
                const el = modal.querySelector(`#${id}`);
                if (el) el.addEventListener('change', applyFilters);
            });

            renderRows(rows);
            renderSummary(rows, rows);
            modal.querySelectorAll('.copy-digest').forEach(btn => {
                btn.addEventListener('click', async () => {
                    const value = btn.getAttribute('data-digest') || '';
                    if (!value) return;
                    try {
                        await navigator.clipboard.writeText(value);
                        app.showSuccess('Digest copied to clipboard');
                    } catch (_) {
                        app.showError('Failed to copy digest');
                    }
                });
            });
        };

        const applyFilters = () => {
            const searchEl = document.getElementById('releases-search');
            const tenantEl = document.getElementById('releases-tenant');
            const envEl = document.getElementById('releases-env');
            const bundleEl = document.getElementById('releases-bundle');

            const q = searchEl.value.trim().toLowerCase();
            const tenantId = tenantEl.value;
            const envId = envEl.value;
            let bundleId = bundleEl.value;

            if (tenantId) {
                const validBundles = new Set(bundles.filter(b => b.tenant_id === tenantId).map(b => b.id));
                if (bundleId && !validBundles.has(bundleId)) {
                    bundleId = '';
                }
            }

            const filtered = releases.filter(r => {
                const nameOk = !q || r.release_id.toLowerCase().includes(q);
                const tenantOk = !tenantId || r.tenant_id === tenantId;
                const envOk = !envId || r.environment_id === envId;
                const bundleOk = !bundleId || r.bundle_id === bundleId;
                return nameOk && tenantOk && envOk && bundleOk;
            });

            content.innerHTML = renderReleases(filtered, q, tenantId, bundleId, envId);
            document.getElementById('releases-search').addEventListener('input', applyFilters);
            document.getElementById('releases-tenant').addEventListener('change', applyFilters);
            document.getElementById('releases-env').addEventListener('change', applyFilters);
            document.getElementById('releases-bundle').addEventListener('change', applyFilters);

            const compareBtn = document.getElementById('releases-compare');
            document.querySelectorAll('.release-select').forEach(cb => {
                cb.addEventListener('change', () => {
                    if (cb.checked) {
                        if (selectedReleases.size >= 2) {
                            cb.checked = false;
                            alert('Select only two releases to compare.');
                            return;
                        }
                        selectedReleases.add(cb.value);
                    } else {
                        selectedReleases.delete(cb.value);
                    }
                    compareBtn.disabled = selectedReleases.size !== 2;
                    const label = document.getElementById('releases-compare-label');
                    if (label) label.textContent = `Compare (${selectedReleases.size}/2)`;
                });
            });

            compareBtn.addEventListener('click', async () => {
                if (selectedReleases.size !== 2) return;
                const [releaseAId, releaseBId] = Array.from(selectedReleases);
                try {
                    const rows = await api.compareReleases(releaseAId, releaseBId);
                    const releaseA = releaseById.get(releaseAId) || { id: releaseAId, release_id: '-' };
                    const releaseB = releaseById.get(releaseBId) || { id: releaseBId, release_id: '-' };
                    showReleaseCompareModal(releaseA, releaseB, rows);
                } catch (error) {
                    alert(`Failed to compare releases: ${error.message}`);
                }
            });
        };

        document.getElementById('releases-search').addEventListener('input', applyFilters);
        document.getElementById('releases-tenant').addEventListener('change', applyFilters);
        document.getElementById('releases-env').addEventListener('change', applyFilters);
        document.getElementById('releases-bundle').addEventListener('change', applyFilters);
        applyFilters();
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
        const [release, manifest, deployJobs] = await Promise.all([
            api.getRelease(params.id),
            api.getReleaseManifest(params.id),
            api.getReleaseDeployJobs(params.id),
        ]);
        const copyJob = release.copy_job_id ? await api.getCopyJobStatus(release.copy_job_id).catch(() => null) : null;
        const bundle = copyJob?.bundle_id ? await api.getBundle(copyJob.bundle_id).catch(() => null) : null;
        const tenant = bundle?.tenant_id ? await api.getTenant(bundle.tenant_id).catch(() => null) : null;
        const environment = copyJob?.environment_id ? await api.getEnvironment(copyJob.environment_id).catch(() => null) : null;
        const environments = tenant?.id ? await api.getEnvironments(tenant.id).catch(() => []) : [];
        if (copyJob?.environment_id) {
            release.environment_id = copyJob.environment_id;
        }
        const [sourceRegistry, targetRegistry] = await Promise.all([
            copyJob?.source_registry_id ? api.getRegistry(copyJob.source_registry_id).catch(() => null) : null,
            copyJob?.target_registry_id ? api.getRegistry(copyJob.target_registry_id).catch(() => null) : null,
        ]);

        content.innerHTML = `
            <div class="row mb-3">
                <div class="col">
                    <a href="#/releases" class="btn btn-ghost-secondary">
                        <i class="ti ti-arrow-left"></i>
                        Back to Image Releases
                    </a>
                </div>
            </div>

            <div class="card mb-3">
                <div class="card-header">
                    <h3 class="card-title">
                        <i class="ti ti-rocket me-2"></i>
                        ${release.release_id}
                        ${release.is_auto ? '<span class="badge bg-azure-lt text-azure-fg ms-2">auto</span>' : ''}
                    </h3>
                </div>
                <div class="card-body">
                    <div class="text-secondary small mb-3">
                        <div>${tenant?.name ? `Tenant: <strong>${tenant.name}</strong>` : 'Tenant: -'}</div>
                        <div>${bundle?.name ? `Bundle: <strong>${bundle.name}</strong>` : 'Bundle: -'}</div>
                        <div>${sourceRegistry?.base_url ? `Source: <code>${sourceRegistry.base_url}${sourceRegistry.default_project_path ? ` (path: ${sourceRegistry.default_project_path})` : ''}</code>` : 'Source: -'}</div>
                        <div>${targetRegistry?.base_url ? `Target: <code>${targetRegistry.base_url}${targetRegistry.default_project_path ? ` (path: ${targetRegistry.default_project_path})` : ''}</code>` : 'Target: -'}</div>
                        <div>Environment: ${environment ? `<span class="badge" style="${environment.color ? `background:${environment.color};color:#fff;` : ''}">${environment.name}</span>` : '-'}</div>
                        <div>${release.copy_job_id ? `Copy Job: <a href="#/copy-jobs/${release.copy_job_id}"><code>${release.copy_job_id}</code></a>` : 'Copy Job: -'}</div>
                    </div>
                    <dl class="row mb-0">
                        <dt class="col-4">Notes:</dt>
                        <dd class="col-8">${release.notes || '-'}</dd>

                        <dt class="col-4">Source Ref:</dt>
                        <dd class="col-8">
                            <span class="badge bg-azure-lt text-azure-fg">${release.source_ref_mode || 'tag'}</span>
                        </dd>

                        <dt class="col-4">Created:</dt>
                        <dd class="col-8">${new Date(release.created_at).toLocaleString('cs-CZ')}</dd>

                    </dl>
                </div>
            </div>

            <div class="card mb-3">
                <div class="card-header">
                    <h3 class="card-title">Image Release Manifest</h3>
                    <div class="card-actions">
                        <button class="btn btn-sm btn-primary" id="copy-manifest-btn">
                            <i class="ti ti-copy"></i>
                            Copy Manifest
                        </button>
                    </div>
                </div>
                <div class="card-body">
                    <pre class="manifest-code" id="manifest-content"></pre>
                </div>
            </div>

            <div class="alert alert-info">
                <i class="ti ti-info-circle"></i>
                Build Deploy regenerates <code>tsm-deploy/deploy/&lt;env&gt;</code> for this release.
            </div>

            <div class="card mb-3">
                <div class="card-header">
                    <h3 class="card-title">Build Deploy</h3>
                </div>
                <div class="card-body">
                    <button class="btn btn-primary" id="build-deploy-btn">
                        <i class="ti ti-rocket"></i>
                        Build Deploy
                    </button>
                    ${release.tenant_id ? `
                        <div class="text-secondary small mt-2">
                            Manage environments in <a href="#/tenants/${release.tenant_id}">Tenant detail</a>.
                        </div>
                    ` : ''}
                    ${environments.length === 0 ? `
                        <div class="alert alert-info mt-3">
                            <i class="ti ti-info-circle"></i>
                            No environments configured for this tenant.
                        </div>
                    ` : ''}
                </div>
            </div>

            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">Deploy Jobs</h3>
                </div>
                <div class="table-responsive">
                    <table class="table table-vcenter card-table">
                        <thead>
                            <tr>
                                <th>Target</th>
                                <th>Status</th>
                                <th>Started</th>
                                <th>Completed</th>
                                <th>Commit</th>
                                <th class="w-1"></th>
                            </tr>
                        </thead>
                        <tbody>
                            ${deployJobs.length === 0 ? `
                                <tr>
                                    <td colspan="6" class="text-center text-secondary py-4">
                                        No deploy jobs yet.
                                    </td>
                                </tr>
                            ` : deployJobs.map(job => `
                                <tr>
                                    <td>
                                        ${job.target_name} (${job.env_name})
                                        ${job.dry_run ? '<span class="badge bg-azure-lt text-azure-fg ms-2">dry-run</span>' : ''}
                                    </td>
                                    <td>
                                        <span class="badge ${
                                            job.status === 'success' ? 'bg-success text-success-fg' :
                                            job.status === 'failed' ? 'bg-danger text-danger-fg' :
                                            job.status === 'in_progress' ? 'bg-info text-info-fg' :
                                            job.status === 'pending' ? 'bg-warning text-warning-fg' :
                                            job.status === 'cancelled' || job.status === 'canceled' ? 'bg-warning text-warning-fg' :
                                            'bg-secondary text-secondary-fg'
                                        }">${job.status}</span>
                                    </td>
                                    <td>${new Date(job.started_at).toLocaleString('cs-CZ')}</td>
                                    <td>${job.completed_at ? new Date(job.completed_at).toLocaleString('cs-CZ') : '-'}</td>
                                    <td>${job.commit_sha ? `<code class="small">${job.commit_sha.slice(0, 8)}</code>` : '-'}</td>
                                    <td>
                                        <a href="#/deploy-jobs/${job.id}" class="btn btn-sm btn-outline-primary">
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

        document.getElementById('manifest-content').textContent = manifest;

        // Copy manifest handler
        document.getElementById('copy-manifest-btn').addEventListener('click', () => {
            const text = document.getElementById('manifest-content').textContent;
            navigator.clipboard.writeText(text).then(() => {
                getApp().showSuccess('Manifest copied to clipboard');
            });
        });

        const buildDeployBtn = document.getElementById('build-deploy-btn');
        if (buildDeployBtn) {
            buildDeployBtn.addEventListener('click', async () => {
                await runDeployFromRelease(release, environments);
            });
        }

    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load release: ${error.message}
            </div>
        `;
    }
});

// Deploy Job Monitor
router.on('/deploy-jobs/:id', async (params) => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const [job, logHistory, diffInfo, imageRows] = await Promise.all([
            api.getDeployJob(params.id),
            api.getDeployJobLogHistory(params.id),
            api.getDeployJobDiff(params.id),
            api.getDeployJobImages(params.id),
        ]);
        const environment = job.environment_id
            ? await api.getEnvironment(job.environment_id).catch(() => null)
            : null;

        content.innerHTML = `
            <div class="row mb-3">
                <div class="col">
                    ${job.is_auto && job.copy_job_id ? `
                        <a href="#/copy-jobs/${job.copy_job_id}" class="btn btn-ghost-secondary">
                            <i class="ti ti-arrow-left"></i>
                            Back to Copy Job
                        </a>
                    ` : `
                        <a href="#/releases/${job.release_id}" class="btn btn-ghost-secondary">
                            <i class="ti ti-arrow-left"></i>
                            Back to Image Release
                        </a>
                    `}
                    ${job.is_auto && job.bundle_id ? `
                        <a href="#/bundles/${job.bundle_id}" class="btn btn-link btn-sm ms-2">
                            Bundle detail
                        </a>
                    ` : ''}
                </div>
            </div>

            <div class="card mb-3">
                <div class="card-header">
                    <h3 class="card-title">
                        <i class="ti ti-rocket me-2"></i>
                        <i class="ti ti-cloud-upload me-2"></i>
                        Deploy Job Monitor
                        ${job.is_auto ? '<span class="badge bg-azure-lt text-azure-fg ms-2">auto</span>' : ''}
                    </h3>
                    ${job.status === 'pending' ? `
                        <div class="card-actions">
                            <button class="btn btn-primary btn-sm" id="start-deploy-job-btn">
                                <i class="ti ti-player-play"></i>
                                Start Deploy
                            </button>
                        </div>
                    ` : ''}
                </div>
                <div class="card-body">
                    <dl class="row mb-0">
                        <dt class="col-4">Target:</dt>
                        <dd class="col-8">${formatTargetWithEnv(job.target_name, job.env_name)}</dd>

                        <dt class="col-4">Status:</dt>
                        <dd class="col-8"><span class="badge ${
                            job.status === 'success' ? 'bg-success text-success-fg' :
                            job.status === 'failed' ? 'bg-danger text-danger-fg' :
                            job.status === 'in_progress' ? 'bg-info text-info-fg' :
                            'bg-secondary text-secondary-fg'
                        }">${job.status}</span></dd>

                        <dt class="col-4">Environment:</dt>
                        <dd class="col-8">${environment ? `<span class="badge" style="${environment.color ? `background:${environment.color};color:#fff;` : ''}">${environment.name}</span>` : '-'}</dd>

                        <dt class="col-4">Dry run:</dt>
                        <dd class="col-8">${job.dry_run ? '<span class="badge bg-azure-lt text-azure-fg">enabled</span>' : '<span class="badge bg-yellow-lt text-yellow-fg">disabled</span>'}</dd>

                        <dt class="col-4">Started:</dt>
                        <dd class="col-8">${new Date(job.started_at).toLocaleString('cs-CZ')}</dd>

                        <dt class="col-4">Completed:</dt>
                        <dd class="col-8">${job.completed_at ? new Date(job.completed_at).toLocaleString('cs-CZ') : '-'}</dd>

                        <dt class="col-4">Commit:</dt>
                        <dd class="col-8">${job.commit_sha ? `<code>${job.commit_sha}</code>` : '-'}</dd>
                    </dl>
                </div>
            </div>

            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">${job.status === 'in_progress' ? 'Live Logs' : 'Audit Logs'}</h3>
                </div>
                <div class="card-body">
                    <div class="terminal" id="deploy-log-terminal">
                        <div class="terminal-header">
                            <span class="terminal-dot red"></span>
                            <span class="terminal-dot yellow"></span>
                            <span class="terminal-dot green"></span>
                        </div>
                        <pre class="terminal-body" id="deploy-log-output"></pre>
                    </div>
                </div>
            </div>

            ${diffInfo ? `
            <div class="card mt-3">
                <div class="card-header">
                    <h3 class="card-title">Deploy Diff</h3>
                </div>
                <div class="card-body">
                    <div class="mb-3">
                        <div class="text-secondary small mb-1">Files changed</div>
                        <pre class="terminal-body" style="max-height: 200px;">${escapeHtml(diffInfo.files_changed || '')}</pre>
                    </div>
                    <div>
                        <div class="text-secondary small mb-1">Diff</div>
                        <div id="deploy-diff-content"></div>
                    </div>
                </div>
            </div>
            ` : ''}

            ${Array.isArray(imageRows) && imageRows.length ? `
            <div class="card mt-3">
                <div class="card-header">
                    <h3 class="card-title">Deployed Images</h3>
                </div>
                <div class="table-responsive">
                    <table class="table table-vcenter card-table">
                        <thead>
                            <tr>
                                <th>File</th>
                                <th>Container</th>
                                <th>Image</th>
                            </tr>
                        </thead>
                        <tbody>
                            ${imageRows.map(row => `
                                <tr>
                                    <td><code class="small">${escapeHtml(row.file_path)}</code></td>
                                    <td>${escapeHtml(row.container_name)}</td>
                                    <td><code class="small">${escapeHtml(row.image)}</code></td>
                                </tr>
                            `).join('')}
                        </tbody>
                    </table>
                </div>
            </div>
            ` : ''}
        `;

        const logOutput = document.getElementById('deploy-log-output');
        const deployLines = [];
        const renderDeployLogs = () => {
            logOutput.innerHTML = deployLines.map(ansiToHtml).join('\n');
            logOutput.scrollTop = logOutput.scrollHeight;
        };

        if (Array.isArray(logHistory) && logHistory.length > 0) {
            deployLines.push(...logHistory);
            renderDeployLogs();
        }

        let refreshScheduled = false;
        if (job.status === 'in_progress') {
            api.createDeployJobStream(params.id, (msg) => {
                deployLines.push(msg);
                renderDeployLogs();
                if (!refreshScheduled && /Deploy job completed successfully|Deploy job failed/i.test(msg)) {
                    refreshScheduled = true;
                    setTimeout(() => {
                        router.navigate(`/deploy-jobs/${params.id}`);
                        router.handleRoute();
                    }, 800);
                }
            }, (err) => {
                deployLines.push(`[Log stream error] ${err}`);
                renderDeployLogs();
            });
        }

        const startBtn = document.getElementById('start-deploy-job-btn');
        if (startBtn) {
            startBtn.addEventListener('click', async () => {
                startBtn.disabled = true;
                try {
                    await api.startDeployJob(params.id);
                    getApp().showSuccess('Deploy job started');
                    router.navigate(`/deploy-jobs/${params.id}`);
                    router.handleRoute();
                } catch (error) {
                    startBtn.disabled = false;
                    getApp().showError(error.message);
                }
            });
        }

        if (diffInfo && diffInfo.diff_patch) {
            const diffEl = document.getElementById('deploy-diff-content');
            if (diffEl && window.Diff2Html) {
                diffEl.innerHTML = window.Diff2Html.html(diffInfo.diff_patch, {
                    drawFileList: false,
                    matching: 'lines',
                    outputFormat: 'line-by-line',
                    colorScheme: 'auto',
                });
            } else if (diffEl) {
                diffEl.innerHTML = `<pre class="terminal-body" style="max-height: 320px; white-space: pre;">${escapeHtml(diffInfo.diff_patch || '')}</pre>`;
            }
        }
    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load deploy job: ${error.message}
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
                <a href="#/copy-jobs/${job.job_id}" class="btn btn-ghost-secondary mb-3">
                    <i class="ti ti-arrow-left"></i>
                    Back to Copy Job
                </a>
                <div class="card">
                    <div class="card-header">
                        <h3 class="card-title">Start Release Images</h3>
                    </div>
                    <div class="card-body">
                        <div class="alert alert-info">
                            <i class="ti ti-info-circle"></i>
                            Create an image release from a successful copy job.
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

        const [job, images] = await Promise.all([
            api.getCopyJobStatus(query.copy_job_id),
            api.getCopyJobImages(query.copy_job_id),
        ]);
        const bundle = await api.getBundle(job.bundle_id).catch(() => null);
        const registries = bundle?.tenant_id
            ? await api.getRegistries(bundle.tenant_id).catch(() => [])
            : await api.getRegistries().catch(() => []);
        const environments = bundle?.tenant_id
            ? await api.getEnvironments(bundle.tenant_id).catch(() => [])
            : [];

        const sourceRegistry = registries.find(r => r.id === job.target_registry_id);
        const sourceBase = sourceRegistry?.base_url || '';

        const state = {
            releaseId: job.target_tag ? `RE_${job.target_tag}` : '',
            notes: '',
            targetRegistryId: '',
            environmentId: job.environment_id || '',
            sourceRefMode: 'tag',
            sourceTagOverride: '',
            validateOnly: true,
            extraTagsInput: '',
            renameRules: [{ find: '', replace: '' }],
            overrides: images.map(img => ({ copy_job_image_id: img.id, override_name: '' })),
        };
        if (state.environmentId) {
            const selectedEnv = environments.find(env => env.id === state.environmentId);
            if (!selectedEnv?.target_registry_id) {
                getApp().showError('Selected environment has no target registry');
            }
        }

        const applyRules = (path) => {
            let out = path;
            state.renameRules.forEach(rule => {
                if (rule.find) {
                    out = out.split(rule.find).join(rule.replace);
                }
            });
            return out;
        };

        const applyProjectPath = (path, env, role = 'target') => {
            const rawDefault = role === 'source' ? env?.source_project_path : env?.target_project_path;
            const defaultPath = (rawDefault || '').trim().replace(/^\/+|\/+$/g, '');
            if (!defaultPath) return path;
            const trimmed = (path || '').replace(/^\/+|\/+$/g, '');
            if (!trimmed) return defaultPath;
            const slashIndex = trimmed.indexOf('/');
            const rest = slashIndex === -1 ? trimmed : trimmed.slice(slashIndex + 1);
            return `${defaultPath}/${rest}`;
        };

        const applyOverride = (path, overrideName) => {
            if (!overrideName) return path;
            const idx = path.lastIndexOf('/');
            if (idx === -1) return overrideName;
            return `${path.slice(0, idx + 1)}${overrideName}`;
        };

        const render = () => {
            const sourceRegistry = registries.find(r => r.id === job.target_registry_id);
            const selectedEnv = environments.find(env => env.id === state.environmentId);
            if (selectedEnv?.target_registry_id) {
                state.targetRegistryId = selectedEnv.target_registry_id;
            }
            const targetRegistry = registries.find(r => r.id === state.targetRegistryId);
            const targetBase = targetRegistry?.base_url || '';
            const environmentId = state.environmentId;
            const sourceEnv = environments.find(env => env.id === job.environment_id);
            const envMismatch = sourceEnv && selectedEnv && sourceEnv.id !== selectedEnv.id;
            const sourceEnvBadge = sourceEnv
                ? `<span class="badge" style="${sourceEnv.color ? `background:${sourceEnv.color};color:#fff;` : ''}">${sourceEnv.name}</span>`
                : '<span class="badge bg-secondary-lt text-secondary-fg">unknown</span>';
            const targetEnvBadge = selectedEnv
                ? `<span class="badge" style="${selectedEnv.color ? `background:${selectedEnv.color};color:#fff;` : ''}">${selectedEnv.name}</span>`
                : '<span class="badge bg-secondary-lt text-secondary-fg">not selected</span>';

            const sourceTag = state.sourceTagOverride.trim() || job.target_tag;
            content.innerHTML = `
                <div class="card">
                    <div class="card-header">
                        <h3 class="card-title">Start Release Images</h3>
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
                                    <div class="form-hint">
                                        ${sourceRegistry?.default_project_path ? `Project path: ${sourceRegistry.default_project_path}` : 'Project path: -'}
                                    </div>
                                    <div class="mt-2">
                                        <span class="text-secondary small me-2">Source env:</span>
                                        ${sourceEnvBadge}
                                    </div>
                                    <div class="mt-2">
                                        <label class="form-label">Source Reference</label>
                                        <div class="form-selectgroup">
                                            <label class="form-selectgroup-item">
                                                <input type="radio" name="source-ref-mode" value="tag" class="form-selectgroup-input" ${state.sourceRefMode === 'tag' ? 'checked' : ''}>
                                                <span class="form-selectgroup-label">Tag</span>
                                            </label>
                                            <label class="form-selectgroup-item">
                                                <input type="radio" name="source-ref-mode" value="digest" class="form-selectgroup-input" ${state.sourceRefMode === 'digest' ? 'checked' : ''}>
                                                <span class="form-selectgroup-label">SHA digest</span>
                                            </label>
                                        </div>
                                        <div class="text-secondary small mt-1">
                                            Tag uses <code>:tag</code>, digest uses <code>@sha256</code>.
                                        </div>
                                        ${state.sourceRefMode === 'digest' && images.some(img => !img.target_sha256) ? `
                                            <div class="alert alert-warning mt-2 mb-0">
                                                Some images are missing digests. Digest mode cannot be used.
                                            </div>
                                        ` : ''}
                                        <div class="mt-3 p-3 border rounded">
                                            <div class="d-flex align-items-center gap-2 mb-2">
                                                <i class="ti ti-alert-triangle text-warning"></i>
                                                <strong>Advanced: Source Tag Override</strong>
                                                <span class="text-secondary small">(Tag mode only)</span>
                                            </div>
                                            <label class="form-label">Source Tag Override</label>
                                                        <input type="text" class="form-control" id="release-source-tag-override"
                                                               value="${state.sourceTagOverride}"
                                                               placeholder="2026.01.29.01"
                                                               ${state.sourceRefMode === 'tag' ? '' : 'disabled'}>
                                            <div class="form-hint">
                                                Overrides the source tag for all images. Available only in Tag mode.
                                            </div>
                                            ${state.sourceRefMode !== 'tag' ? `
                                                <div class="text-warning small mt-2">
                                                    Switch Source Reference to <strong>Tag</strong> to enable this field.
                                                </div>
                                            ` : ''}
                                        </div>
                                    </div>
                                </div>
                            </div>
                            <div class="col-md-6">
                                <div class="mb-3">
                                    <label class="form-label">Target Registry</label>
                                    <div class="form-control-plaintext">
                                        ${targetRegistry ? `${targetRegistry.name} (${targetRegistry.base_url})` : '-'}
                                    </div>
                                    <div class="form-hint">
                                        ${targetRegistry ? `Project path: ${applyProjectPath('', selectedEnv, 'target') || '-'}` : 'Project path: -'}
                                    </div>
                                    <div class="mt-2">
                                        <span class="text-secondary small me-2">Target env:</span>
                                        ${targetEnvBadge}
                                    </div>
                                    ${envMismatch ? `
                                        <div class="alert alert-warning mt-3 mb-0">
                                            <i class="ti ti-alert-triangle me-2"></i>
                                            Source and target environments differ. Ensure source credentials are valid for the source registry.
                                        </div>
                                    ` : ''}
                                </div>
                                ${environments.length > 0 ? `
                                    <div class="mb-3">
                                        <label class="form-label required">Environment</label>
                                        <select class="form-select" id="release-environment" required>
                                            <option value="">Select environment...</option>
                                            ${environments.map(env => `
                                                <option value="${env.id}" ${state.environmentId === env.id ? 'selected' : ''}>
                                                    ${env.name} (${env.slug})
                                                </option>
                                            `).join('')}
                                        </select>
                                        <div class="form-hint">Required for environment-specific project path overrides.</div>
                                    </div>
                                ` : ''}
                            </div>
                        </div>

                        <div class="mb-3">
                            <label class="form-label required">Release ID (target tag)</label>
                            <input type="text" class="form-control" id="release-id" value="${state.releaseId}"
                                   placeholder="2026.02.04.01">
                        </div>

                        <div class="mb-3">
                            <label class="form-label">Extra tags (optional)</label>
                            <textarea class="form-control" id="release-extra-tags" rows="2" placeholder="latest, latest-prod, 2026">${state.extraTagsInput}</textarea>
                            <small class="form-hint">Comma or newline separated tags. Applied to the same target images after the main release tag.</small>
                        </div>

                        <div class="mb-3">
                            <label class="form-label">Notes</label>
                            <textarea class="form-control" id="release-notes" rows="3">${state.notes || ''}</textarea>
                        </div>

                        <div class="mb-3">
                            <label class="form-check">
                                <input class="form-check-input" type="checkbox" id="release-validate-only" ${state.validateOnly ? 'checked' : ''}>
                                <span class="form-check-label">Validate only (no copy, no tag write)</span>
                            </label>
                            <small class="form-hint">Runs source validation and digest checks without copying to target.</small>
                        </div>

                        <div class="mb-3" id="release-precheck-result"></div>

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
                                        const basePath = applyProjectPath(img.target_image, selectedEnv, 'target');
                                        const renamed = applyRules(basePath);
                                        const override = state.overrides[idx]?.override_name || '';
                                        const finalPath = applyOverride(renamed, override);
                                        const sourceFull = state.sourceRefMode === 'digest'
                                            ? (img.target_sha256
                                                ? `${sourceBase}/${img.target_image}@${img.target_sha256}`
                                                : `${sourceBase}/${img.target_image}@<missing-digest>`)
                                            : `${sourceBase}/${img.target_image}:${sourceTag}`;
                                        const targetFull = targetBase ? `${targetBase}/${finalPath}:${state.releaseId || '<release_id>'}` : '-';
                                        return `
                                            <tr>
                                                <td>
                                                    <code class="small"
                                                          data-source-preview
                                                          data-source-path="${img.target_image}"
                                                          data-source-sha="${img.target_sha256 || ''}">
                                                        ${sourceFull}
                                                    </code>
                                                </td>
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
                    <div class="card-footer">
                        <div id="release-precheck-result-footer" class="mb-3"></div>
                        <div class="d-flex gap-2">
                            <button type="button" class="btn btn-outline-primary w-100" id="release-precheck-btn">
                                <i class="ti ti-search"></i>
                                Pre-check Images
                            </button>
                            <button type="button" class="btn btn-success w-100" id="release-create">
                                <i class="ti ti-rocket"></i>
                                Release Images
                            </button>
                        </div>
                    </div>
                </div>
            `;

            const envSelect = document.getElementById('release-environment');
            if (envSelect) {
                envSelect.addEventListener('change', (e) => {
                    state.environmentId = e.target.value;
                    render();
                });
            }
            document.querySelectorAll('input[name="source-ref-mode"]').forEach(input => {
                input.addEventListener('change', (e) => {
                    state.sourceRefMode = e.target.value;
                    render();
                });
            });
            const sourceOverrideInput = document.getElementById('release-source-tag-override');
            if (sourceOverrideInput) {
                sourceOverrideInput.addEventListener('input', (e) => {
                    state.sourceTagOverride = e.target.value;
                    updatePreview();
                });
            }
            document.getElementById('release-id').addEventListener('input', (e) => {
                state.releaseId = e.target.value;
                updatePreview();
            });
            document.getElementById('release-notes').addEventListener('input', (e) => {
                state.notes = e.target.value;
            });
            document.getElementById('release-extra-tags').addEventListener('input', (e) => {
                state.extraTagsInput = e.target.value;
            });
            document.getElementById('release-validate-only').addEventListener('change', (e) => {
                state.validateOnly = e.target.checked;
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

            const renderPrecheck = (result) => {
                const box = document.getElementById('release-precheck-result-footer');
                if (!box) return;
                const overrideInfo = state.sourceRefMode === 'tag' && state.sourceTagOverride.trim()
                    ? `<div class="text-secondary small mb-2">Using source tag override: <code class="small">${state.sourceTagOverride.trim()}</code></div>`
                    : '';
                if (result.failed && result.failed.length > 0) {
                    box.innerHTML = `
                        <div class="alert alert-danger">
                            ${overrideInfo}
                            <div class="d-flex align-items-center mb-2">
                                <i class="ti ti-alert-triangle me-2"></i>
                                <strong>Pre-check failed (${result.failed.length}/${result.total})</strong>
                            </div>
                            <div class="list-group">
                                ${result.failed.map(f => `
                                    <div class="list-group-item">
                                        <div><code class="small">${f.source_image}:${f.source_tag}</code></div>
                                        <div class="text-secondary small">${f.error}</div>
                                    </div>
                                `).join('')}
                            </div>
                        </div>
                    `;
                } else {
                    box.innerHTML = `
                        <div class="alert alert-success">
                            ${overrideInfo}
                            <i class="ti ti-check me-2"></i>
                            All ${result.total} images found.
                        </div>
                    `;
                }
            };

            const runReleasePrecheck = async () => {
                if (!state.targetRegistryId) {
                    getApp().showError('Selected environment has no target registry');
                    return null;
                }
                if (environments.length > 0 && !state.environmentId) {
                    getApp().showError('Please select environment');
                    return null;
                }
                if (state.sourceRefMode === 'digest' && images.some(img => !img.target_sha256)) {
                    getApp().showError('Digest mode is not available because some images are missing digests');
                    return null;
                }
                if (state.sourceRefMode === 'digest' && state.sourceTagOverride.trim()) {
                    getApp().showError('Source tag override is only available in Tag mode');
                    return null;
                }

                const payload = {
                    source_copy_job_id: job.job_id,
                    target_registry_id: state.targetRegistryId,
                    environment_id: state.environmentId || null,
                    release_id: state.releaseId.trim() || '',
                    notes: state.notes || null,
                    source_ref_mode: state.sourceRefMode,
                    source_tag_override: state.sourceTagOverride.trim() || null,
                    validate_only: state.validateOnly,
                    extra_tags: state.extraTagsInput
                        .split(/[,\n]/)
                        .map(t => t.trim())
                        .filter(t => t && t !== state.releaseId.trim()),
                    rename_rules: state.renameRules.filter(r => r.find),
                    overrides: state.overrides.filter(o => o.override_name),
                };

                const btn = document.getElementById('release-precheck-btn');
                const box = document.getElementById('release-precheck-result-footer');
                if (box) {
                    box.innerHTML = `
                        <div class="alert alert-info">
                            <i class="ti ti-search me-2"></i>
                            Running pre-check...
                        </div>
                    `;
                }
                if (btn) {
                    btn.disabled = true;
                    btn.innerHTML = '<span class="spinner-border spinner-border-sm me-2"></span>Checking...';
                }
                try {
                    const result = await api.precheckReleaseCopy(payload);
                    renderPrecheck(result);
                    return result;
                } catch (error) {
                    getApp().showError(error.message);
                    return null;
                } finally {
                    if (btn) {
                        btn.disabled = false;
                        btn.innerHTML = '<i class="ti ti-search"></i> Pre-check Images';
                    }
                }
            };

            const precheckBtn = document.getElementById('release-precheck-btn');
            if (precheckBtn) {
                precheckBtn.addEventListener('click', runReleasePrecheck);
            }

            document.getElementById('release-create').addEventListener('click', async () => {
                if (!state.targetRegistryId) {
                    getApp().showError('Selected environment has no target registry');
                    return;
                }
                if (environments.length > 0 && !state.environmentId) {
                    getApp().showError('Please select environment');
                    return;
                }
                const releaseId = state.releaseId.trim();
                if (!releaseId) {
                    getApp().showError('Release ID cannot be empty');
                    return;
                }
                if (state.sourceRefMode === 'digest' && images.some(img => !img.target_sha256)) {
                    getApp().showError('Digest mode is not available because some images are missing digests');
                    return;
                }
                if (state.sourceRefMode === 'digest' && state.sourceTagOverride.trim()) {
                    getApp().showError('Source tag override is only available in Tag mode');
                    return;
                }

                const precheck = await runReleasePrecheck();
                if (precheck && precheck.failed && precheck.failed.length > 0) {
                    getApp().showError('Pre-check failed. Fix missing images before releasing.');
                    return;
                }

                const payload = {
                    source_copy_job_id: job.job_id,
                    target_registry_id: state.targetRegistryId,
                    environment_id: state.environmentId || null,
                    release_id: releaseId,
                    notes: state.notes || null,
                    source_ref_mode: state.sourceRefMode,
                    source_tag_override: state.sourceTagOverride.trim() || null,
                    validate_only: state.validateOnly,
                    extra_tags: state.extraTagsInput
                        .split(/[,\n]/)
                        .map(t => t.trim())
                        .filter(t => t && t !== releaseId),
                    rename_rules: state.renameRules.filter(r => r.find),
                    overrides: state.overrides.filter(o => o.override_name),
                };

                try {
                    const response = await api.startReleaseCopyJob(payload);
                    getApp().showSuccess('Image release copy job started');
                    router.navigate(`/copy-jobs/${response.job_id}`);
                } catch (error) {
                    getApp().showError(error.message);
                }
            });
        };

        const updatePreview = () => {
            const selectedEnv = environments.find(env => env.id === state.environmentId);
            const targetRegistry = registries.find(r => r.id === state.targetRegistryId);
            const targetBase = targetRegistry?.base_url || '';
            const sourceTag = state.sourceTagOverride.trim() || job.target_tag;
            document.querySelectorAll('[data-preview]').forEach(el => {
                const idx = parseInt(el.getAttribute('data-index'), 10);
                const sourcePath = el.getAttribute('data-source-path') || '';
                const basePath = applyProjectPath(sourcePath, selectedEnv, 'target');
                const renamed = applyRules(basePath);
                const override = state.overrides[idx]?.override_name || '';
                const finalPath = applyOverride(renamed, override);
                const releaseId = state.releaseId.trim();
                const targetFull = targetBase
                    ? `${targetBase}/${finalPath}:${releaseId || '<release_id>'}`
                    : '-';
                el.textContent = targetFull;
            });
            document.querySelectorAll('[data-source-preview]').forEach(el => {
                const sourcePath = el.getAttribute('data-source-path') || '';
                const sourceSha = el.getAttribute('data-source-sha') || '';
                const sourceFull = state.sourceRefMode === 'digest'
                    ? (sourceSha
                        ? `${sourceBase}/${sourcePath}@${sourceSha}`
                        : `${sourceBase}/${sourcePath}@<missing-digest>`)
                    : `${sourceBase}/${sourcePath}:${sourceTag}`;
                el.textContent = sourceFull;
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
        const bundle = await api.getBundle(params.id);
        const [mappings, environments, registries] = await Promise.all([
            api.getImageMappings(params.id, params.version),
            bundle.tenant_id ? api.getEnvironments(bundle.tenant_id) : Promise.resolve([]),
            api.getRegistries(bundle.tenant_id).catch(() => []),
        ]);
        const registryMap = new Map((registries || []).map(r => [r.id, r]));

        const autoTagEnabled = !!bundle.auto_tag_enabled;

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
                        <label class="form-label required">
                            Target Tag
                            ${autoTagEnabled ? '<span class="badge bg-azure-lt text-azure-fg ms-2">auto</span>' : ''}
                        </label>
                        <input type="text" class="form-control" id="target-tag"
                               placeholder="YYYY.MM.DD.COUNTER" required ${autoTagEnabled ? 'disabled' : ''}>
                        <small class="form-hint">
                            ${autoTagEnabled ? 'Tag is auto-generated (YYYY.MM.DD.COUNTER)' : 'Tag to use for all target images'}
                        </small>
                    </div>

                    <div class="mb-3">
                        <label class="form-label required">Environment</label>
                        <select class="form-select" id="environment-select" required>
                            <option value="">Select environment</option>
                            ${environments.map(env => `
                                <option value="${env.id}">${env.name}</option>
                            `).join('')}
                        </select>
                        <small class="form-hint">Required for environment-specific registry paths.</small>
                    </div>

                    <div class="list-group mb-3" id="copy-preview-list">
                        <div class="list-group-item">
                            <strong>Images to copy:</strong>
                        </div>
                        ${mappings.slice(0, 5).map(m => `
                            <div class="list-group-item">
                                <code class="small" data-preview-source="${m.source_image}" data-preview-source-tag="${m.source_tag}">${m.source_image}:${m.source_tag}</code>
                                <i class="ti ti-arrow-right mx-2"></i>
                                <code class="small" data-preview-target="${m.target_image}">${m.target_image}:<span class="text-primary">[tag]</span></code>
                            </div>
                        `).join('')}
                        ${mappings.length > 5 ? `
                            <div class="list-group-item text-secondary">
                                ... and ${mappings.length - 5} more
                            </div>
                        ` : ''}
                    </div>

                    <div class="mb-3" id="precheck-result"></div>

                    <div class="d-flex gap-2">
                        <button type="button" class="btn btn-outline-primary w-100" id="precheck-btn">
                            <i class="ti ti-search"></i>
                            Pre-check Images
                        </button>
                        <button type="button" class="btn btn-primary w-100" id="start-copy-btn">
                            <i class="ti ti-copy"></i>
                            Start Copy Job
                        </button>
                    </div>
                </div>
            </div>
        `;

        const applyProjectPath = (path, projectPath) => {
            const defaultPath = (projectPath || '').trim().replace(/^\/+|\/+$/g, '');
            if (!defaultPath) return path;
            const trimmed = (path || '').replace(/^\/+|\/+$/g, '');
            if (!trimmed) return defaultPath;
            const slashIndex = trimmed.indexOf('/');
            const rest = slashIndex === -1 ? trimmed : trimmed.slice(slashIndex + 1);
            return `${defaultPath}/${rest}`;
        };

        const targetTagInput = document.getElementById('target-tag');
        const updateAutoTagPreview = async () => {
            if (!autoTagEnabled || !targetTagInput) return;
            const envId = document.getElementById('environment-select')?.value || '';
            if (!envId) {
                targetTagInput.value = '';
                return;
            }
            try {
                const tzOffset = new Date().getTimezoneOffset();
                const preview = await api.getNextCopyTag(params.id, params.version, tzOffset, envId);
                targetTagInput.value = preview.tag;
            } catch (error) {
                getApp().showError(`Failed to generate tag preview: ${error.message}`);
            }
        };

        const updatePreview = () => {
            const envId = document.getElementById('environment-select')?.value || '';
            const env = environments.find(e => e.id === envId);
            const sourceReg = env?.source_registry_id ? registryMap.get(env.source_registry_id) : null;
            const targetReg = env?.target_registry_id ? registryMap.get(env.target_registry_id) : null;
            content.querySelectorAll('[data-preview-source]').forEach(el => {
                const raw = el.getAttribute('data-preview-source') || '';
                const tag = el.getAttribute('data-preview-source-tag') || '';
                const path = applyProjectPath(raw, env?.source_project_path);
                el.textContent = `${path}:${tag}`;
            });
            content.querySelectorAll('[data-preview-target]').forEach(el => {
                const raw = el.getAttribute('data-preview-target') || '';
                const path = applyProjectPath(raw, env?.target_project_path);
                const suffix = el.querySelector('span')?.outerHTML || '<span class="text-primary">[tag]</span>';
                el.innerHTML = `${path}:${suffix}`;
            });
            updateAutoTagPreview();
        };

        const renderPrecheck = (result) => {
            const box = document.getElementById('precheck-result');
            if (!box) return;

            if (result.failed && result.failed.length > 0) {
                box.innerHTML = `
                    <div class="alert alert-danger">
                        <div class="d-flex align-items-center mb-2">
                            <i class="ti ti-alert-triangle me-2"></i>
                            <strong>Pre-check failed (${result.failed.length}/${result.total})</strong>
                        </div>
                        <div class="small text-secondary mb-2">Fix the missing images and create a new bundle version.</div>
                        <div class="mb-2">
                            <a href="#/bundles/${params.id}/versions/new" class="btn btn-sm btn-outline-primary">
                                Create New Version
                            </a>
                        </div>
                        <div class="list-group">
                            ${result.failed.map(f => `
                                <div class="list-group-item">
                                    <div><code class="small">${f.source_image}:${f.source_tag}</code></div>
                                    <div class="text-secondary small">${f.error}</div>
                                </div>
                            `).join('')}
                        </div>
                    </div>
                `;
            } else {
                box.innerHTML = `
                    <div class="alert alert-success">
                        <i class="ti ti-check me-2"></i>
                        All ${result.total} images found.
                    </div>
                `;
            }
        };

        const runPrecheck = async () => {
            const envId = document.getElementById('environment-select')?.value || '';
            if (!envId) {
                getApp().showError('Please select an environment');
                return null;
            }
            const btn = document.getElementById('precheck-btn');
            if (btn) {
                btn.disabled = true;
                btn.innerHTML = '<span class="spinner-border spinner-border-sm me-2"></span>Checking...';
            }
            try {
                const result = await api.precheckCopyImages(params.id, params.version, envId, null);
                renderPrecheck(result);
                return result;
            } catch (error) {
                getApp().showError(error.message);
                return null;
            } finally {
                if (btn) {
                    btn.disabled = false;
                    btn.innerHTML = '<i class="ti ti-search"></i> Pre-check Images';
                }
            }
        };

        const precheckBtn = document.getElementById('precheck-btn');
        if (precheckBtn) {
            precheckBtn.addEventListener('click', runPrecheck);
        }

        const envSelect = document.getElementById('environment-select');
        if (envSelect) {
            envSelect.addEventListener('change', () => {
                updatePreview();
                updateAutoTagPreview();
            });
            updatePreview();
            updateAutoTagPreview();
        }

        document.getElementById('start-copy-btn').addEventListener('click', async () => {
            const targetTag = document.getElementById('target-tag').value;
            if (!autoTagEnabled && !targetTag) {
                getApp().showError('Please enter a target tag');
                return;
            }
            const envId = document.getElementById('environment-select')?.value || '';
            if (!envId) {
                getApp().showError('Please select an environment');
                return;
            }

            const precheck = await runPrecheck();
            if (precheck && precheck.failed && precheck.failed.length > 0) {
                getApp().showError('Pre-check failed. Fix missing images and create a new version.');
                return;
            }

            const btn = document.getElementById('start-copy-btn');
            btn.disabled = true;
            btn.innerHTML = '<span class="spinner-border spinner-border-sm me-2"></span>Starting...';

            try {
                const tzOffset = new Date().getTimezoneOffset();
                const response = await api.startCopyJob(
                    params.id,
                    params.version,
                    targetTag,
                    tzOffset,
                    envId,
                    null,
                    null,
                );
                getApp().showSuccess('Copy job created. Click Start to run.');
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
        const [initialStatus, initialImages, logHistory, releaseList] = await Promise.all([
            api.getCopyJobStatus(params.jobId),
            api.getCopyJobImages(params.jobId),
            api.getCopyJobLogHistory(params.jobId),
            api.getReleases(),
        ]);
        const linkedRelease = releaseList.find(r => r.copy_job_id === initialStatus.job_id);
        const bundle = initialStatus.bundle_id ? await api.getBundle(initialStatus.bundle_id).catch(() => null) : null;
        const baseJob = initialStatus.base_copy_job_id
            ? await api.getCopyJobStatus(initialStatus.base_copy_job_id).catch(() => null)
            : null;

        const [tenant, sourceRegistry, targetRegistry, environment, sourceEnvironment] = await Promise.all([
            bundle?.tenant_id ? api.getTenant(bundle.tenant_id).catch(() => null) : null,
            initialStatus.source_registry_id ? api.getRegistry(initialStatus.source_registry_id).catch(() => null) : null,
            initialStatus.target_registry_id ? api.getRegistry(initialStatus.target_registry_id).catch(() => null) : null,
            initialStatus.environment_id ? api.getEnvironment(initialStatus.environment_id).catch(() => null) : null,
            baseJob?.environment_id ? api.getEnvironment(baseJob.environment_id).catch(() => null) : null,
        ]);

        const renderLogs = () => {
            const logEl = document.getElementById('copy-job-log');
            if (!logEl) return;
            logEl.innerHTML = logLines.map(ansiToHtml).join('\n');
            logEl.scrollTop = logEl.scrollHeight;
        };

        const renderJobStatus = (status, images = []) => {
            const progress = status.total_images > 0
                ? ((status.copied_images + status.failed_images) / status.total_images * 100).toFixed(0)
                : 0;

            const isComplete = status.status === 'success' || status.status === 'failed' || status.status === 'cancelled';
            const failedImages = images.filter(img => img.copy_status === 'failed');
            const skippedImages = images.filter(img => img.copy_status === 'success' && img.bytes_copied === 0);
            const autoRelease = releaseList.find(r => r.copy_job_id === status.job_id && r.is_auto);

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
                <div class="row mb-3">
                    <div class="col">
                        <a href="#/bundles/${status.bundle_id}" class="btn btn-ghost-secondary">
                            <i class="ti ti-arrow-left"></i>
                            Back to Bundle
                        </a>
                    </div>
                </div>

                <div class="card">
                    <div class="card-header d-flex align-items-start justify-content-between flex-wrap gap-2">
                        <div>
                            <h3 class="card-title mb-1">
                                <i class="ti ti-brand-docker me-2"></i>
                                <i class="ti ti-copy me-2"></i>
                                Copy Job Monitor
                                ${status.is_release_job ? `
                                    <span class="badge bg-purple-lt text-purple-fg ms-2">image release</span>
                                ` : ''}
                                ${status.is_selective ? `
                                    <span class="badge bg-purple-lt text-purple-fg ms-2">selective</span>
                                ` : ''}
                                ${status.validate_only ? `
                                    <span class="badge bg-azure-lt text-azure-fg ms-2">validate-only</span>
                                ` : ''}
                            </h3>
                        <div class="text-secondary small">
                            <div>${tenant?.name ? `Tenant: <strong>${tenant.name}</strong>` : 'Tenant: -'}</div>
                            <div>${bundle?.name ? `Bundle: <strong>${bundle.name}</strong>` : 'Bundle: -'}</div>
                            <div>${sourceRegistry?.base_url ? `Source: <code>${sourceRegistry.base_url}${sourceEnvironment?.source_project_path ? ` (path: ${sourceEnvironment.source_project_path})` : ''}</code>` : 'Source: -'}</div>
                            <div>${targetRegistry?.base_url ? `Target: <code>${targetRegistry.base_url}${environment?.target_project_path ? ` (path: ${environment.target_project_path})` : ''}</code>` : 'Target: -'}</div>
                            <div>Env Src: ${sourceEnvironment ? `<span class="badge" style="${sourceEnvironment.color ? `background:${sourceEnvironment.color};color:#fff;` : ''}">${sourceEnvironment.name}</span>` : '-'}</div>
                            <div>Env Trg: ${environment ? `<span class="badge" style="${environment.color ? `background:${environment.color};color:#fff;` : ''}">${environment.name}</span>` : '-'}</div>
                            ${linkedRelease ? `<div>Image Release: <a href="#/releases/${linkedRelease.id}"><code>${linkedRelease.release_id}</code></a></div>` : ''}
                            ${status.base_copy_job_id ? `<div>Base Job: <a href="#/copy-jobs/${status.base_copy_job_id}"><code>${status.base_copy_job_id}</code></a></div>` : ''}
                        </div>
                        </div>
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
                            ${skippedImages.length > 0 ? `
                                <div class="text-secondary small mt-2">
                                    Skipped: ${skippedImages.length}
                                </div>
                            ` : ''}
                        </div>

                        <div class="alert ${
                            status.status === 'success' ? 'alert-success' :
                            status.status === 'failed' ? 'alert-warning' :
                            status.status === 'cancelled' ? 'alert-warning' :
                            status.status === 'in_progress' ? 'alert-info pulse' :
                            'alert-secondary'
                        }">
                            <div class="d-flex align-items-center">
                                <i class="ti ${
                                    status.status === 'success' ? 'ti-check' :
                                    status.status === 'failed' ? 'ti-x' :
                                    status.status === 'cancelled' ? 'ti-ban' :
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

                        <div class="text-secondary small mb-3">
                            Auto-release: ${autoRelease ? `created (<a href="#/releases/${autoRelease.id}">${autoRelease.release_id}</a>)` : 'not created'}
                            ${status.validate_only ? ' • Validate-only run (no copy)' : ''}
                        </div>

                        ${isComplete ? `
                            <div class="d-grid gap-2">
                                ${status.status === 'success' && !status.is_release_job ? `
                                    <a href="#/releases/new?copy_job_id=${status.job_id}" class="btn btn-success">
                                        <i class="ti ti-rocket"></i>
                                        Release Images
                                    </a>
                                    <button class="btn btn-outline-primary" id="auto-deploy-btn">
                                        <i class="ti ti-rocket"></i>
                                        Deploy Action
                                    </button>
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
                                    ${status.validate_only ? 'Start Validation' : 'Start Copy Job'}
                                </button>
                                <button class="btn btn-outline-danger" id="cancel-copy-job">
                                    <i class="ti ti-x"></i>
                                    Cancel Copy Job
                                </button>
                                <a href="#/copy-jobs" class="btn btn-outline-secondary">
                                    <i class="ti ti-list"></i>
                                    Back to Copy Jobs
                                </a>
                            </div>
                        ` : status.status === 'in_progress' ? `
                            <div class="d-grid gap-2">
                                <button class="btn btn-outline-danger" id="cancel-copy-job">
                                    <i class="ti ti-x"></i>
                                    Cancel Copy Job
                                </button>
                            </div>
                        ` : ''}
                    </div>
                </div>

                <div class="card mt-3">
                    <div class="card-header">
                        <h3 class="card-title">${isComplete ? 'Audit Logs' : 'Live Logs'}</h3>
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

                ${skippedImages.length > 0 ? `
                <div class="card mt-3">
                    <div class="card-header">
                        <h3 class="card-title">Skipped Images</h3>
                    </div>
                    <div class="table-responsive">
                        <table class="table table-vcenter card-table">
                            <thead>
                                <tr>
                                    <th>Source</th>
                                    <th>Target</th>
                                    <th>Status</th>
                                </tr>
                            </thead>
                            <tbody>
                                ${skippedImages.map(img => `
                                    <tr>
                                        <td>
                                            <div><code class="small">${img.source_image}:${img.source_tag}</code></div>
                                        </td>
                                        <td>
                                            <div><code class="small">${img.target_image}:${img.target_tag}</code></div>
                                        </td>
                                        <td><span class="badge bg-azure-lt">SKIP</span></td>
                                    </tr>
                                `).join('')}
                            </tbody>
                        </table>
                    </div>
                </div>
                ` : ''}
            `;

            const autoDeployBtn = document.getElementById('auto-deploy-btn');
            if (autoDeployBtn) {
                autoDeployBtn.addEventListener('click', async () => {
                    await runAutoDeployFromCopyJob(status.job_id, tenant?.id || bundle?.tenant_id, status.target_tag);
                });
            }

            const cancelBtn = document.getElementById('cancel-copy-job');
            if (cancelBtn) {
                cancelBtn.addEventListener('click', async () => {
                    const confirmed = await showConfirmDialog(
                        'Cancel Copy Job?',
                        'This will stop further copies for this job.',
                        'Cancel Job',
                        'Keep Running'
                    );
                    if (!confirmed) return;
                    try {
                        await api.cancelCopyJob(params.jobId);
                        getApp().showSuccess('Cancel requested');
                        const [newStatus, newImages] = await Promise.all([
                            api.getCopyJobStatus(params.jobId),
                            api.getCopyJobImages(params.jobId),
                        ]);
                        renderJobStatus(newStatus, newImages);
                    } catch (error) {
                        getApp().showError(error.message);
                    }
                });
            }

            renderLogs();
        };

        if (Array.isArray(logHistory)) {
            logLines.push(...logHistory);
        }

        // Initial render
        renderJobStatus(initialStatus, initialImages);

        const attachStartHandler = () => {
            const startBtn = document.getElementById('start-copy-job');
            if (!startBtn) return;
            startBtn.addEventListener('click', async () => {
                try {
                    startBtn.disabled = true;
                    startBtn.innerHTML = '<span class="spinner-border spinner-border-sm me-2"></span>Starting...';
                    await api.startPendingCopyJob(params.jobId);
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

        if (initialStatus.status === 'in_progress') {
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
        const [jobs, registries, bundles, tenants] = await Promise.all([
            api.getCopyJobs(),
            api.getRegistries(),
            api.getBundles(),
            api.getTenants(),
        ]);
        const registryMap = {};
        registries.forEach(r => {
            registryMap[r.id] = r;
        });
        const bundleMap = new Map(bundles.map(b => [b.id, b]));
        const tenantMap = new Map(tenants.map(t => [t.id, t]));
        const tenantIds = [...new Set(bundles.map(b => b.tenant_id).filter(Boolean))];
        const environmentLists = await Promise.all(
            tenantIds.map(id => api.getEnvironments(id).catch(() => []))
        );
        const environmentMap = new Map();
        environmentLists.flat().forEach(env => {
            environmentMap.set(env.id, env);
        });

        const allEnvironments = Array.from(environmentMap.values()).sort((a, b) => a.name.localeCompare(b.name));
        let selectedJobs = new Set();
        let activeTab = 'copy';

        const renderJobs = (rows, searchQuery = '', selectedStatus = '', selectedTenant = '', selectedEnv = '', tab = 'copy') => `
            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">Copy Jobs</h3>
                    <div class="card-actions">
                        ${tab === 'copy' ? `
                            <button class="btn btn-outline-secondary" id="copy-jobs-compare" ${selectedJobs.size === 2 ? '' : 'disabled'}>
                                <i class="ti ti-arrows-diff"></i>
                                <span id="copy-jobs-compare-label">Compare (${selectedJobs.size}/2)</span>
                            </button>
                        ` : ''}
                    </div>
                </div>
                <div class="card-body border-bottom py-3">
                    <ul class="nav nav-tabs">
                        <li class="nav-item">
                            <a class="nav-link ${tab === 'copy' ? 'active' : ''}" href="#" data-tab="copy">
                                Copy Jobs
                            </a>
                        </li>
                        <li class="nav-item">
                            <a class="nav-link ${tab === 'release' ? 'active' : ''}" href="#" data-tab="release">
                                Release Jobs
                            </a>
                        </li>
                    </ul>
                    <div class="row g-2">
                        <div class="col-md-4">
                            <div class="input-group">
                                <span class="input-group-text">
                                    <i class="ti ti-search"></i>
                                </span>
                                <input class="form-control" id="copy-jobs-search" placeholder="Bundle name or job id" value="${searchQuery}">
                            </div>
                        </div>
                        <div class="col-md-3">
                            <select class="form-select" id="copy-jobs-tenant">
                                <option value="">All Tenants</option>
                                ${tenants.map(t => `
                                    <option value="${t.id}" ${t.id === selectedTenant ? 'selected' : ''}>${t.name}</option>
                                `).join('')}
                            </select>
                        </div>
                        <div class="col-md-3">
                            <select class="form-select" id="copy-jobs-env">
                                <option value="">All Environments</option>
                                ${allEnvironments.map(env => `
                                    <option value="${env.id}" ${env.id === selectedEnv ? 'selected' : ''}>${env.name}</option>
                                `).join('')}
                            </select>
                        </div>
                        <div class="col-md-2">
                            <select class="form-select" id="copy-jobs-status">
                                <option value="">All</option>
                                <option value="in_progress" ${selectedStatus === 'in_progress' ? 'selected' : ''}>in_progress</option>
                                <option value="pending" ${selectedStatus === 'pending' ? 'selected' : ''}>pending</option>
                                <option value="success" ${selectedStatus === 'success' ? 'selected' : ''}>success</option>
                                <option value="failed" ${selectedStatus === 'failed' ? 'selected' : ''}>failed</option>
                            </select>
                        </div>
                    </div>
                </div>
                <div class="table-responsive">
                    <table class="table table-vcenter card-table">
                        <thead>
                            <tr>
                                    <th class="w-1"></th>
                                    <th>Job ID</th>
                                    <th>Tenant</th>
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
                                    <td colspan="8" class="text-center text-secondary py-5">
                                        ${tab === 'copy' ? 'No copy jobs yet.' : 'No release jobs yet.'}
                                    </td>
                                </tr>
                            ` : rows.map(job => `
                                <tr>
                                    <td>
                                        <input class="form-check-input copy-job-select" type="checkbox" value="${job.job_id}" ${selectedJobs.has(job.job_id) ? 'checked' : ''}>
                                    </td>
                                    <td><a href="#/copy-jobs/${job.job_id}"><code class="small">${job.job_id}</code></a></td>
                                    <td>${job.tenant_name ? `<a href="#/tenants/${job.tenant_id}">${job.tenant_name}</a>` : '-'}</td>
                                    <td>
                                        <div><a href="#/bundles/${job.bundle_id}">${job.bundle_name}</a></div>
                                        <div class="text-secondary small"><code class="small">${job.bundle_id}</code></div>
                                    </td>
                                    <td>
                                        <span class="badge bg-blue text-blue-fg">v${job.version}</span>
                                        ${job.is_release_job ? `
                                            <span class="badge bg-purple-lt text-purple-fg ms-2">release</span>
                                        ` : ''}
                                        ${job.is_selective ? '<span class="badge bg-purple-lt text-purple-fg ms-2">selective</span>' : ''}
                                        ${job.validate_only ? '<span class="badge bg-azure-lt text-azure-fg ms-2">validate</span>' : ''}
                                    </td>
                                    <td>
                                        <span class="badge bg-azure-lt">${job.target_tag}</span>
                                        ${job.validate_only ? '<span class="badge bg-azure-lt text-azure-fg ms-2">validate</span>' : ''}
                                        <div class="text-secondary small mt-1">
                                            ${
                                                job.environment_id && environmentMap.get(job.environment_id)
                                                    ? `Env: <span class="badge" style="${environmentMap.get(job.environment_id).color ? `background:${environmentMap.get(job.environment_id).color};color:#fff;` : ''}">${environmentMap.get(job.environment_id).name}</span>`
                                                    : 'Env: -'
                                            }
                                        </div>
                                        <div class="text-secondary small mt-1">
                                            ${job.source_registry_id ? `Source: <code class="small">${registryMap[job.source_registry_id]?.base_url || '-'}${registryMap[job.source_registry_id]?.default_project_path ? ` (path: ${registryMap[job.source_registry_id]?.default_project_path})` : ''}</code>` : 'Source: -'}
                                        </div>
                                        <div class="text-secondary small">
                                            ${job.target_registry_id ? `Target: <code class="small">${registryMap[job.target_registry_id]?.base_url || '-'}${registryMap[job.target_registry_id]?.default_project_path ? ` (path: ${registryMap[job.target_registry_id]?.default_project_path})` : ''}</code>` : 'Target: -'}
                                        </div>
                                    </td>
                                    <td>
                                        <span class="badge ${
                                            job.status === 'success' ? 'bg-success text-success-fg' :
                                            job.status === 'failed' ? 'bg-danger text-danger-fg' :
                                            job.status === 'in_progress' ? 'bg-info text-info-fg' :
                                            job.status === 'pending' ? 'bg-warning text-warning-fg' :
                                            job.status === 'cancelled' || job.status === 'canceled' ? 'bg-warning text-warning-fg' :
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

        const hydratedJobs = jobs.map(job => {
            const bundle = bundleMap.get(job.bundle_id);
            const tenantId = bundle?.tenant_id || null;
            const tenantName = tenantId ? tenantMap.get(tenantId)?.name || '' : '';
            return {
                ...job,
                tenant_id: tenantId,
                tenant_name: tenantName,
            };
        });

        const jobById = new Map(hydratedJobs.map(job => [job.job_id, job]));

        const showCompareModal = (jobA, jobB, rows) => {
            const modalHtml = `
                <div class="modal modal-blur fade show" style="display: block;" id="copy-jobs-compare-modal">
                    <div class="modal-dialog modal-xl modal-dialog-centered" role="document">
                        <div class="modal-content">
                            <div class="modal-header">
                                <h5 class="modal-title">Digest Comparison</h5>
                                <button type="button" class="btn-close" data-modal-close></button>
                            </div>
                            <div class="modal-body">
                                <div class="text-secondary small mb-3">
                                    Job A: <code>${jobA.job_id}</code> (${jobA.target_tag})<br>
                                    Job B: <code>${jobB.job_id}</code> (${jobB.target_tag})
                                </div>
                                <div class="text-secondary small mb-3" id="compare-summary"></div>
                                <div class="row g-2 mb-3">
                                    <div class="col-auto">
                                        <label class="form-check form-check-inline">
                                            <input class="form-check-input" type="checkbox" id="compare-only-changes">
                                            <span class="form-check-label">Changes</span>
                                        </label>
                                    </div>
                                    <div class="col-auto">
                                        <label class="form-check form-check-inline">
                                            <input class="form-check-input" type="checkbox" id="compare-only-missing">
                                            <span class="form-check-label">Missing</span>
                                        </label>
                                    </div>
                                    <div class="col-auto">
                                        <label class="form-check form-check-inline">
                                            <input class="form-check-input" type="checkbox" id="compare-only-new">
                                            <span class="form-check-label">New</span>
                                        </label>
                                    </div>
                                </div>
                                <div class="table-responsive">
                                    <table class="table table-vcenter">
                                        <thead>
                                            <tr>
                                                <th>App</th>
                                                <th>Container</th>
                                                <th>Digest A</th>
                                                <th>Digest B</th>
                                                <th>Status</th>
                                            </tr>
                                        </thead>
                                        <tbody id="compare-results-body"></tbody>
                                    </table>
                                </div>
                            </div>
                            <div class="modal-footer">
                                <button type="button" class="btn btn-secondary" data-modal-close>Close</button>
                            </div>
                        </div>
                    </div>
                </div>
                <div class="modal-backdrop fade show"></div>
            `;

            document.body.insertAdjacentHTML('beforeend', modalHtml);
            const modal = document.getElementById('copy-jobs-compare-modal');
            const backdrop = document.querySelector('.modal-backdrop');
            const closeModal = () => {
                modal?.remove();
                backdrop?.remove();
            };
            modal.querySelectorAll('[data-modal-close]').forEach(btn => btn.addEventListener('click', closeModal));

                const renderRows = (filtered) => {
                    const body = modal.querySelector('#compare-results-body');
                    if (!body) return;
                    if (!filtered.length) {
                        body.innerHTML = '<tr><td colspan="5" class="text-center text-secondary">No data</td></tr>';
                        return;
                    }
                    body.innerHTML = filtered.map(row => {
                        const status = row.status;
                        const badgeClass =
                            status === 'same' ? 'bg-success text-success-fg' :
                            status === 'changed' ? 'bg-warning text-warning-fg' :
                            status === 'missing_in_a' ? 'bg-danger text-danger-fg' :
                            status === 'missing_in_b' ? 'bg-danger text-danger-fg' :
                            'bg-secondary text-secondary-fg';
                    const label = status.replaceAll('_', ' ');
                    const shortDigest = (value) => {
                        if (!value) return '';
                        const cleaned = value.startsWith('sha256:') ? value.slice(7) : value;
                        return cleaned.slice(0, 7);
                    };
                    const digestCell = (value) => {
                        if (!value) return '-';
                        const short = shortDigest(value);
                        return `
                            <div class="d-flex align-items-center gap-2">
                                <code class="small" title="${value}">${short}</code>
                                <button type="button" class="btn btn-ghost-secondary btn-sm copy-digest" data-digest="${value}">
                                    <i class="ti ti-copy"></i>
                                </button>
                            </div>
                        `;
                    };
                    return `
                        <tr>
                            <td><code class="small">${row.app_name}</code></td>
                            <td><code class="small">${row.container_name}</code></td>
                            <td>${digestCell(row.digest_a)}</td>
                            <td>${digestCell(row.digest_b)}</td>
                            <td><span class="badge ${badgeClass}">${label}</span></td>
                        </tr>
                    `;
                }).join('');
            };

            const renderSummary = (allRows, filteredRows) => {
                const summaryEl = modal.querySelector('#compare-summary');
                if (!summaryEl) return;
                const countBy = (rows, status) => rows.filter(r => r.status === status).length;
                const total = allRows.length;
                const same = countBy(filteredRows, 'same');
                const changed = countBy(filteredRows, 'changed');
                const missingA = countBy(filteredRows, 'missing_in_a');
                const missingB = countBy(filteredRows, 'missing_in_b');
                summaryEl.innerHTML = `
                    Showing <strong>${filteredRows.length}</strong> / ${total} &nbsp;|&nbsp;
                    <span class="text-success">same: ${same}</span>
                    &nbsp;|&nbsp;
                    <span class="text-warning">changes: ${changed}</span>
                    &nbsp;|&nbsp;
                    <span class="text-danger">missing: ${missingB}</span>
                    &nbsp;|&nbsp;
                    <span class="text-info">new: ${missingA}</span>
                `;
            };

            const applyFilters = () => {
                const onlyChanges = modal.querySelector('#compare-only-changes')?.checked;
                const onlyMissing = modal.querySelector('#compare-only-missing')?.checked;
                const onlyNew = modal.querySelector('#compare-only-new')?.checked;
                let filtered = rows.slice();
                const active = [];
                if (onlyChanges) active.push('changed');
                if (onlyMissing) active.push('missing_in_b');
                if (onlyNew) active.push('missing_in_a');
                if (active.length > 0) {
                    filtered = filtered.filter(r => active.includes(r.status));
                }
                renderRows(filtered);
                renderSummary(rows, filtered);
                modal.querySelectorAll('.copy-digest').forEach(btn => {
                    btn.addEventListener('click', async () => {
                        const value = btn.getAttribute('data-digest') || '';
                        if (!value) return;
                        try {
                            await navigator.clipboard.writeText(value);
                            app.showSuccess('Digest copied to clipboard');
                        } catch (_) {
                            app.showError('Failed to copy digest');
                        }
                    });
                });
            };

            ['compare-only-changes', 'compare-only-missing', 'compare-only-new'].forEach(id => {
                const el = modal.querySelector(`#${id}`);
                if (el) el.addEventListener('change', applyFilters);
            });

            renderRows(rows);
            renderSummary(rows, rows);
            modal.querySelectorAll('.copy-digest').forEach(btn => {
                btn.addEventListener('click', async () => {
                    const value = btn.getAttribute('data-digest') || '';
                    if (!value) return;
                    try {
                        await navigator.clipboard.writeText(value);
                        app.showSuccess('Digest copied to clipboard');
                    } catch (_) {
                        app.showError('Failed to copy digest');
                    }
                });
            });
        };

        const renderAndBind = (rows, q = '', status = '', tenantId = '', envId = '', tab = 'copy') => {
            content.innerHTML = renderJobs(rows, q, status, tenantId, envId, tab);

            const tenantEl = document.getElementById('copy-jobs-tenant');
            const envEl = document.getElementById('copy-jobs-env');
            const statusEl = document.getElementById('copy-jobs-status');
            const searchEl = document.getElementById('copy-jobs-search');
            const compareBtn = document.getElementById('copy-jobs-compare');

            const applyFilters = () => {
                const tenantValue = tenantEl.value;
                const envValue = envEl.value;
                const statusValue = statusEl.value;
                const qValue = searchEl.value.trim().toLowerCase();
                const filtered = hydratedJobs.filter(job => {
                    const tenantOk = !tenantValue || job.tenant_id === tenantValue;
                    const envOk = !envValue || job.environment_id === envValue;
                    const statusOk = !statusValue || job.status === statusValue;
                    const searchOk = !qValue || job.bundle_name.toLowerCase().includes(qValue) || job.job_id.toLowerCase().includes(qValue);
                    const tabOk = activeTab === 'release' ? job.is_release_job : !job.is_release_job;
                    return tenantOk && envOk && statusOk && searchOk && tabOk;
                });
                renderAndBind(filtered, qValue, statusValue, tenantValue, envValue, activeTab);
            };

            tenantEl.addEventListener('change', applyFilters);
            envEl.addEventListener('change', applyFilters);
            statusEl.addEventListener('change', applyFilters);
            searchEl.addEventListener('input', applyFilters);

            if (activeTab === 'copy') {
                document.querySelectorAll('.copy-job-select').forEach(cb => {
                    cb.addEventListener('change', () => {
                        if (cb.checked) {
                            if (selectedJobs.size >= 2) {
                                cb.checked = false;
                                alert('Select only two copy jobs to compare.');
                                return;
                            }
                            selectedJobs.add(cb.value);
                        } else {
                            selectedJobs.delete(cb.value);
                        }
                        if (compareBtn) {
                            compareBtn.disabled = selectedJobs.size !== 2;
                            const label = document.getElementById('copy-jobs-compare-label');
                            if (label) label.textContent = `Compare (${selectedJobs.size}/2)`;
                        }
                    });
                });

                if (compareBtn) {
                    compareBtn.addEventListener('click', async () => {
                        if (selectedJobs.size !== 2) return;
                        const [jobAId, jobBId] = Array.from(selectedJobs);
                        try {
                            const rows = await api.compareCopyJobs(jobAId, jobBId);
                            const jobA = jobById.get(jobAId) || { job_id: jobAId, target_tag: '-' };
                            const jobB = jobById.get(jobBId) || { job_id: jobBId, target_tag: '-' };
                            showCompareModal(jobA, jobB, rows);
                        } catch (error) {
                            alert(`Failed to compare copy jobs: ${error.message}`);
                        }
                    });
                }
            }

            document.querySelectorAll('[data-tab]').forEach(link => {
                link.addEventListener('click', (e) => {
                    e.preventDefault();
                    activeTab = link.getAttribute('data-tab') || 'copy';
                    selectedJobs = new Set();
                    applyFilters();
                });
            });
        };

        renderAndBind(hydratedJobs, '', '', '', '', activeTab);
    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load copy jobs: ${error.message}
            </div>
        `;
    }
});

// Deployments List
router.on('/deployments', async () => {
    const content = document.getElementById('app-content');
    content.innerHTML = '<div class="text-center py-5"><div class="spinner-border"></div></div>';

    try {
        const [deployments, tenants, bundles] = await Promise.all([
            api.getDeployments(),
            api.getTenants(),
            api.getBundles(),
        ]);

        const renderDeployments = (rows, searchQuery = '', selectedTenant = '', selectedBundle = '', selectedStatus = '') => {
            const filteredBundles = bundles.filter(b => !selectedTenant || b.tenant_id === selectedTenant);
            const q = searchQuery.trim().toLowerCase();

            const filtered = rows.filter(row => {
                if (selectedTenant && row.tenant_id !== selectedTenant) return false;
                if (selectedBundle && row.bundle_id !== selectedBundle) return false;
                if (selectedStatus && row.status !== selectedStatus) return false;
                if (q) {
                    return (
                        row.bundle_name.toLowerCase().includes(q) ||
                        row.release_id.toLowerCase().includes(q) ||
                        row.target_name.toLowerCase().includes(q)
                    );
                }
                return true;
            });

            return `
                <div class="card">
                    <div class="card-header">
                        <h3 class="card-title">Deployments</h3>
                    </div>
                    <div class="card-body border-bottom py-3">
                        <div class="row g-2">
                            <div class="col-md-4">
                                <div class="input-group">
                                    <span class="input-group-text">
                                        <i class="ti ti-search"></i>
                                    </span>
                                    <input class="form-control" id="deployments-search" placeholder="Search bundle, release, target..."
                                           value="${searchQuery}">
                                </div>
                            </div>
                            <div class="col-md-3">
                                <select class="form-select" id="deployments-tenant">
                                    <option value="">All Tenants</option>
                                    ${tenants.map(t => `
                                        <option value="${t.id}" ${t.id === selectedTenant ? 'selected' : ''}>${t.name}</option>
                                    `).join('')}
                                </select>
                            </div>
                            <div class="col-md-3">
                                <select class="form-select" id="deployments-bundle">
                                    <option value="">All Bundles</option>
                                    ${filteredBundles.map(b => `
                                        <option value="${b.id}" ${b.id === selectedBundle ? 'selected' : ''}>${b.name}</option>
                                    `).join('')}
                                </select>
                            </div>
                            <div class="col-md-2">
                                <select class="form-select" id="deployments-status">
                                    <option value="">All Status</option>
                                    <option value="in_progress" ${selectedStatus === 'in_progress' ? 'selected' : ''}>in_progress</option>
                                    <option value="pending" ${selectedStatus === 'pending' ? 'selected' : ''}>pending</option>
                                    <option value="success" ${selectedStatus === 'success' ? 'selected' : ''}>success</option>
                                    <option value="failed" ${selectedStatus === 'failed' ? 'selected' : ''}>failed</option>
                                </select>
                            </div>
                        </div>
                    </div>
                    <div class="table-responsive">
                        <table class="table table-vcenter card-table table-hover">
                            <thead>
                                <tr>
                                    <th>Release</th>
                                    <th>Bundle</th>
                                    <th>Target</th>
                                    <th>Status</th>
                                    <th>Started</th>
                                    <th>Completed</th>
                                    <th>Tag</th>
                                    <th></th>
                                </tr>
                            </thead>
                            <tbody>
                                ${filtered.length === 0 ? `
                                    <tr>
                                        <td colspan="8" class="text-center text-secondary py-5">
                                            No deployments found.
                                        </td>
                                    </tr>
                                ` : filtered.map(row => `
                                    <tr>
                                        <td>
                                            <a href="#/releases/${row.release_db_id}"><strong>${row.release_id}</strong></a>
                                            ${row.is_auto ? '<span class="badge bg-azure-lt text-azure-fg ms-2">auto</span>' : ''}
                                        </td>
                                        <td>
                                            <a href="#/bundles/${row.bundle_id}">${row.bundle_name}</a>
                                        </td>
                                        <td>
                                            ${row.target_name} (${row.env_name})
                                            ${row.dry_run ? '<span class="badge bg-azure-lt text-azure-fg ms-2">dry-run</span>' : ''}
                                        </td>
                                        <td>
                                            <span class="badge ${
                                                row.status === 'success' ? 'bg-success text-success-fg' :
                                                row.status === 'failed' ? 'bg-danger text-danger-fg' :
                                                row.status === 'in_progress' ? 'bg-info text-info-fg' :
                                                'bg-secondary text-secondary-fg'
                                            }">${row.status}</span>
                                        </td>
                                        <td>${new Date(row.started_at).toLocaleString('cs-CZ')}</td>
                                        <td>${row.completed_at ? new Date(row.completed_at).toLocaleString('cs-CZ') : '-'}</td>
                                        <td>${row.tag_name ? `<code class="small">${row.tag_name}</code>` : '-'}</td>
                                        <td>
                                            <a href="#/deploy-jobs/${row.id}" class="btn btn-sm btn-outline-primary">View</a>
                                        </td>
                                    </tr>
                                `).join('')}
                            </tbody>
                        </table>
                    </div>
                </div>
            `;
        };

        content.innerHTML = renderDeployments(deployments);

        const applyFilters = () => {
            const searchEl = document.getElementById('deployments-search');
            const tenantEl = document.getElementById('deployments-tenant');
            const bundleEl = document.getElementById('deployments-bundle');
            const statusEl = document.getElementById('deployments-status');
            content.innerHTML = renderDeployments(
                deployments,
                searchEl?.value || '',
                tenantEl?.value || '',
                bundleEl?.value || '',
                statusEl?.value || ''
            );
            attachFilterHandlers();
        };

        const attachFilterHandlers = () => {
            document.getElementById('deployments-search')?.addEventListener('input', applyFilters);
            document.getElementById('deployments-tenant')?.addEventListener('change', applyFilters);
            document.getElementById('deployments-bundle')?.addEventListener('change', applyFilters);
            document.getElementById('deployments-status')?.addEventListener('change', applyFilters);
        };

        attachFilterHandlers();
    } catch (error) {
        content.innerHTML = `
            <div class="alert alert-danger">
                Failed to load deployments: ${error.message}
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
            const key = (role || '').toString().trim().toLowerCase();
            const badges = {
                'source': 'bg-blue text-blue-fg',
                'target': 'bg-green text-green-fg',
                'both': 'bg-purple text-purple-fg'
            };
            return badges[key] || 'bg-secondary text-secondary-fg';
        }
    };
}

async function runAutoDeployFromCopyJob(copyJobId, tenantId, targetTag) {
    if (!tenantId) {
        getApp().showError('Tenant not resolved for this copy job');
        return;
    }

    let copyJobEnvId = null;
    try {
        const status = await api.getCopyJobStatus(copyJobId);
        copyJobEnvId = status.environment_id || null;
    } catch (error) {
        getApp().showError(`Failed to load copy job: ${error.message}`);
        return;
    }

    let environments = [];
    try {
        environments = await api.getEnvironments(tenantId);
    } catch (error) {
        getApp().showError(`Failed to load environments: ${error.message}`);
        return;
    }

    const eligible = environments.filter(env => env.allow_auto_release);
    if (eligible.length === 0) {
        getApp().showError('No environments allow auto release');
        return;
    }

    const dialogHtml = `
        <div class="modal modal-blur fade show" style="display: block;" id="auto-deploy-modal">
            <div class="modal-dialog modal-sm modal-dialog-centered" role="document">
                <div class="modal-content">
                    <div class="modal-body">
                        <div class="modal-title" id="auto-deploy-title">Environment</div>
                        <div class="mt-2">
                            <label class="form-label">Environment</label>
                            <select class="form-select" id="auto-deploy-select">
                                <option value="">Select...</option>
                                ${eligible.map(env => `
                                    <option value="${env.id}">${env.name} (${env.slug})</option>
                                `).join('')}
                            </select>
                        </div>
                        <div class="form-check mt-3">
                            <input class="form-check-input" type="checkbox" id="auto-deploy-dry-run" checked>
                            <label class="form-check-label" for="auto-deploy-dry-run">
                                Dry run (no git commit/push/tag)
                            </label>
                            <div class="text-warning small mt-1 d-none" id="auto-deploy-warning">
                                Dry run disabled: changes will be committed and pushed to git.
                            </div>
                        </div>
                    </div>
                    <div class="modal-footer">
                        <button type="button" class="btn btn-link link-secondary" id="auto-deploy-cancel">
                            Cancel
                        </button>
                        <button type="button" class="btn btn-primary" id="auto-deploy-confirm" disabled>
                            Create Deploy Job
                        </button>
                    </div>
                </div>
            </div>
        </div>
        <div class="modal-backdrop fade show"></div>
    `;

    document.body.insertAdjacentHTML('beforeend', dialogHtml);

    const modal = document.getElementById('auto-deploy-modal');
    const backdrop = document.querySelector('.modal-backdrop');
    const select = document.getElementById('auto-deploy-select');
    const titleEl = document.getElementById('auto-deploy-title');
    const labelEl = modal.querySelector('label.form-label');
    const confirmBtn = document.getElementById('auto-deploy-confirm');
    const cancelBtn = document.getElementById('auto-deploy-cancel');
    const dryRunCheckbox = document.getElementById('auto-deploy-dry-run');
    const dryRunWarning = document.getElementById('auto-deploy-warning');

    const cleanup = () => {
        modal.remove();
        backdrop.remove();
    };

    const updateTitle = (target) => {
        if (!target) {
            titleEl.textContent = 'Environment';
            if (labelEl) {
                labelEl.textContent = 'Environment';
            }
            return;
        }
        const tagSuffix = target.slug || target.name;
        const tagPreview = targetTag
            ? (target.append_env_suffix ? `${targetTag}-${tagSuffix}` : targetTag)
            : null;
        titleEl.textContent = 'Environment';
        if (labelEl) {
            labelEl.textContent = tagPreview ? `Environment (tag: ${tagPreview})` : 'Environment';
        }
    };

    if (copyJobEnvId) {
        const match = eligible.find(t => t.id === copyJobEnvId);
        if (match) {
            select.value = match.id;
            confirmBtn.disabled = false;
            updateTitle(match);
        }
    }

    select.addEventListener('change', () => {
        const target = eligible.find(t => t.id === select.value);
        confirmBtn.disabled = !target;
        updateTitle(target);
    });

    if (dryRunCheckbox && dryRunWarning) {
        const syncWarning = () => {
            dryRunWarning.classList.toggle('d-none', dryRunCheckbox.checked);
        };
        dryRunCheckbox.addEventListener('change', syncWarning);
        syncWarning();
    }

    cancelBtn.addEventListener('click', () => {
        cleanup();
    });

    confirmBtn.addEventListener('click', async () => {
        const targetEnvId = select.value;
        const dryRun = dryRunCheckbox?.checked ?? true;
        if (!targetEnvId) return;
        cleanup();
        try {
            const response = await api.startAutoDeployFromCopyJob(copyJobId, targetEnvId, dryRun);
            getApp().showSuccess('Deploy job created');
            router.navigate(`/deploy-jobs/${response.job_id}`);
        } catch (error) {
            getApp().showError(error.message);
        }
    });
}

async function runSelectiveCopyFromJob(copyJobId, bundle) {
    const autoTagEnabled = !!bundle?.auto_tag_enabled;
    let images = [];

    try {
        images = await api.getCopyJobImages(copyJobId);
    } catch (error) {
        getApp().showError(`Failed to load copy job images: ${error.message}`);
        return;
    }

    if (!images.length) {
        getApp().showError('No images available for selective copy');
        return;
    }

    const dialogHtml = `
        <div class="modal modal-blur fade show" style="display: block;" id="selective-copy-modal">
            <div class="modal-dialog modal-lg modal-dialog-centered" role="document">
                <div class="modal-content">
                    <div class="modal-body">
                        <div class="modal-title">Selective Copy</div>
                        <div class="text-secondary small mt-1">
                            Selected images will be copied from source. Unselected images will keep their digest and be retagged.
                        </div>
                        ${autoTagEnabled ? `
                            <div class="text-secondary small mt-2">
                                Target tag will be auto-generated (YYYY.MM.DD.COUNTER).
                            </div>
                        ` : `
                            <div class="mt-3">
                                <label class="form-label required">Target Tag</label>
                                <input type="text" class="form-control" id="selective-target-tag" placeholder="2026.06.02.02">
                            </div>
                        `}
                        <div class="mt-3 d-flex align-items-center justify-content-between">
                            <div class="form-check">
                                <input class="form-check-input" type="checkbox" id="selective-select-all">
                                <label class="form-check-label" for="selective-select-all">Select all</label>
                            </div>
                            <div class="text-secondary small">${images.length} images</div>
                        </div>
                        <div class="table-responsive mt-3" style="max-height: 380px;">
                            <table class="table table-vcenter card-table">
                                <thead>
                                    <tr>
                                        <th class="w-1"></th>
                                        <th>Image</th>
                                        <th>Current Tag</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    ${images.map(img => `
                                        <tr>
                                            <td>
                                                <input class="form-check-input selective-image-checkbox" type="checkbox" value="${img.id}">
                                            </td>
                                            <td><code class="small">${img.target_image}</code></td>
                                            <td><span class="badge bg-azure-lt">${img.target_tag}</span></td>
                                        </tr>
                                    `).join('')}
                                </tbody>
                            </table>
                        </div>
                    </div>
                    <div class="modal-footer">
                        <button type="button" class="btn btn-link link-secondary" id="selective-copy-cancel">
                            Cancel
                        </button>
                        <button type="button" class="btn btn-primary" id="selective-copy-confirm">
                            Create Selective Copy
                        </button>
                    </div>
                </div>
            </div>
        </div>
        <div class="modal-backdrop fade show"></div>
    `;

    document.body.insertAdjacentHTML('beforeend', dialogHtml);

    const modal = document.getElementById('selective-copy-modal');
    const backdrop = document.querySelector('.modal-backdrop');
    const cancelBtn = document.getElementById('selective-copy-cancel');
    const confirmBtn = document.getElementById('selective-copy-confirm');
    const selectAll = document.getElementById('selective-select-all');
    const tagInput = document.getElementById('selective-target-tag');
    const checkboxes = Array.from(document.querySelectorAll('.selective-image-checkbox'));

    const cleanup = () => {
        modal.remove();
        backdrop.remove();
    };

    selectAll.addEventListener('change', () => {
        checkboxes.forEach(cb => {
            cb.checked = selectAll.checked;
        });
    });

    cancelBtn.addEventListener('click', cleanup);

    confirmBtn.addEventListener('click', async () => {
        const selectedIds = checkboxes.filter(cb => cb.checked).map(cb => cb.value);
        if (selectedIds.length === 0) {
            getApp().showError('Select at least one image');
            return;
        }
        const payload = {
            base_copy_job_id: copyJobId,
            selected_image_ids: selectedIds,
            timezone_offset_minutes: new Date().getTimezoneOffset(),
        };
        if (!autoTagEnabled) {
            const targetTag = tagInput?.value.trim() || '';
            if (!targetTag) {
                getApp().showError('Target tag is required');
                return;
            }
            payload.target_tag = targetTag;
        }

        cleanup();
        try {
            const response = await api.startSelectiveCopyJob(payload);
            getApp().showSuccess('Selective copy job created');
            router.navigate(`/copy-jobs/${response.job_id}`);
        } catch (error) {
            getApp().showError(error.message);
        }
    });
}

async function runDeployFromRelease(release, environments) {
    const eligible = environments || [];
    if (eligible.length === 0) {
        getApp().showError('No environments available for this release');
        return;
    }

    const dialogHtml = `
        <div class="modal modal-blur fade show" style="display: block;" id="release-deploy-modal">
            <div class="modal-dialog modal-sm modal-dialog-centered" role="document">
                <div class="modal-content">
                    <div class="modal-body">
                        <div class="modal-title" id="release-deploy-title">Environment</div>
                        <div class="mt-2">
                            <label class="form-label">Environment</label>
                            <select class="form-select" id="release-deploy-select">
                                <option value="">Select...</option>
                                ${eligible.map(env => `
                                    <option value="${env.id}">${env.name} (${env.slug})</option>
                                `).join('')}
                            </select>
                        </div>
                        <div class="form-check mt-3">
                            <input class="form-check-input" type="checkbox" id="release-deploy-dry-run" checked>
                            <label class="form-check-label" for="release-deploy-dry-run">
                                Dry run (no git commit/push/tag)
                            </label>
                            <div class="text-warning small mt-1 d-none" id="release-deploy-warning">
                                Dry run disabled: changes will be committed and pushed to git.
                            </div>
                        </div>
                    </div>
                    <div class="modal-footer">
                        <button type="button" class="btn btn-link link-secondary" id="release-deploy-cancel">
                            Cancel
                        </button>
                        <button type="button" class="btn btn-primary" id="release-deploy-confirm" disabled>
                            Create Deploy Job
                        </button>
                    </div>
                </div>
            </div>
        </div>
        <div class="modal-backdrop fade show"></div>
    `;

    document.body.insertAdjacentHTML('beforeend', dialogHtml);

    const modal = document.getElementById('release-deploy-modal');
    const backdrop = document.querySelector('.modal-backdrop');
    const select = document.getElementById('release-deploy-select');
    const titleEl = document.getElementById('release-deploy-title');
    const labelEl = modal.querySelector('label.form-label');
    const confirmBtn = document.getElementById('release-deploy-confirm');
    const cancelBtn = document.getElementById('release-deploy-cancel');
    const dryRunCheckbox = document.getElementById('release-deploy-dry-run');
    const dryRunWarning = document.getElementById('release-deploy-warning');

    const cleanup = () => {
        modal.remove();
        backdrop.remove();
    };

    const updateTitle = (target) => {
        if (!target) {
            titleEl.textContent = 'Environment';
            if (labelEl) {
                labelEl.textContent = 'Environment';
            }
            return;
        }
        const tagSuffix = target.slug || target.name;
        const tagPreview = release?.release_id
            ? (target.append_env_suffix ? `${release.release_id}-${tagSuffix}` : release.release_id)
            : null;
        titleEl.textContent = 'Environment';
        if (labelEl) {
            labelEl.textContent = tagPreview ? `Environment (tag: ${tagPreview})` : 'Environment';
        }
    };

    if (release?.environment_id) {
        const match = eligible.find(t => t.id === release.environment_id);
        if (match) {
            select.value = match.id;
            confirmBtn.disabled = false;
            updateTitle(match);
        }
    }

    select.addEventListener('change', () => {
        const target = eligible.find(t => t.id === select.value);
        confirmBtn.disabled = !target;
        updateTitle(target);
    });

    if (dryRunCheckbox && dryRunWarning) {
        const syncWarning = () => {
            dryRunWarning.classList.toggle('d-none', dryRunCheckbox.checked);
        };
        dryRunCheckbox.addEventListener('change', syncWarning);
        syncWarning();
    }

    cancelBtn.addEventListener('click', () => {
        cleanup();
    });

    confirmBtn.addEventListener('click', async () => {
        const targetEnvId = select.value;
        const dryRun = dryRunCheckbox?.checked ?? true;
        if (!targetEnvId) return;
        cleanup();
        try {
            const response = await api.createDeployJob({
                release_id: release.id,
                environment_id: targetEnvId,
                dry_run: dryRun,
            });
            getApp().showSuccess('Deploy job created');
            router.navigate(`/deploy-jobs/${response.job_id}`);
        } catch (error) {
            getApp().showError(error.message);
        }
    });
}

async function copyMappingsToClipboard(mappings) {
    if (!Array.isArray(mappings) || mappings.length === 0) {
        getApp().showError('No mappings available to export');
        return;
    }
    const rows = mappings.map(m => [
        m.source_image || '',
        m.source_tag || 'latest',
        m.target_image || '',
        m.app_name || '',
        m.container_name || '',
    ].join(';'));
    const payload = rows.join('\n');
    try {
        if (navigator.clipboard?.writeText) {
            await navigator.clipboard.writeText(payload);
            getApp().showSuccess('Mappings copied to clipboard');
            return;
        }
    } catch (error) {
        console.warn('Clipboard API failed, falling back to execCommand:', error);
    }

    try {
        const textarea = document.createElement('textarea');
        textarea.value = payload;
        textarea.setAttribute('readonly', '');
        textarea.style.position = 'absolute';
        textarea.style.left = '-9999px';
        document.body.appendChild(textarea);
        textarea.select();
        const success = document.execCommand('copy');
        textarea.remove();
        if (success) {
            getApp().showSuccess('Mappings copied to clipboard');
        } else {
            window.prompt('Copy mappings:', payload);
            getApp().showError('Clipboard unavailable. Copied to prompt.');
        }
    } catch (error) {
        window.prompt('Copy mappings:', payload);
        getApp().showError('Clipboard unavailable. Copied to prompt.');
    }
}

function applyReplaceRules(value, rules) {
    let result = value || '';
    rules.forEach(rule => {
        const find = rule.find?.trim();
        if (!find) return;
        const replace = rule.replace ?? '';
        result = result.split(find).join(replace);
    });
    return result;
}

function formatTargetWithEnv(name, envName) {
    if (!envName) return name || '';
    const suffix = `(${envName})`;
    if ((name || '').includes(suffix)) return name;
    return `${name} (${envName})`;
}

function parseMappingCsv(input, rules = []) {
    const lines = input.split(/\r?\n/).map(line => line.trim()).filter(Boolean);
    if (lines.length === 0) {
        return { rows: [], valid: [], invalid: [] };
    }

    let startIndex = 0;
    const header = lines[0].toLowerCase();
    if (header.includes('source_image') && header.includes('target_image')) {
        startIndex = 1;
    }

    const rows = [];
    for (let i = startIndex; i < lines.length; i++) {
        const parts = lines[i].split(';').map(p => p.trim());
        const sourceImage = parts[0] || '';
        let sourceTag = parts[1] || '';
        if (!sourceTag) sourceTag = 'latest';
        const targetRaw = parts[2] || '';
        const targetImage = applyReplaceRules(targetRaw, rules);
        let appName = parts[3] || '';
        const containerName = parts[4] || '';
        if (!appName && targetImage) {
            const segs = targetImage.split('/');
            appName = segs[segs.length - 1] || '';
        }

        const valid = Boolean(sourceImage && targetImage && appName);
        rows.push({
            source_image: sourceImage,
            source_tag: sourceTag,
            target_image: targetImage,
            app_name: appName,
            container_name: containerName,
            valid,
        });
    }

    const valid = rows.filter(r => r.valid);
    const invalid = rows.filter(r => !r.valid);
    return { rows, valid, invalid };
}

async function showMappingImportModal({ onApply }) {
    const modalHtml = `
        <div class="modal modal-blur fade show" style="display: block;" id="import-mappings-modal">
            <div class="modal-dialog modal-xl modal-dialog-centered" role="document">
                <div class="modal-content">
                    <div class="modal-header">
                        <h5 class="modal-title">Import Image Mappings</h5>
                        <button type="button" class="btn-close" aria-label="Close" id="import-mappings-close"></button>
                    </div>
                    <div class="modal-body">
                        <div class="mb-3">
                            <label class="form-label">Paste CSV (semicolon-separated)</label>
                            <textarea class="form-control" id="import-mappings-input" rows="6"
                                placeholder="source_image;source_tag;target_image;app_name;container_name"></textarea>
                            <div class="form-hint">Empty source_tag defaults to <code>latest</code>.</div>
                        </div>
                        <div class="mb-3">
                            <label class="form-label">Replace rules (target_image only)</label>
                            <div id="import-replace-rules"></div>
                            <button type="button" class="btn btn-sm btn-outline-secondary mt-2" id="import-add-rule">
                                <i class="ti ti-plus"></i>
                                Add rule
                            </button>
                        </div>
                        <div class="mb-2">
                            <div class="text-secondary small mb-1" id="import-summary">0 valid, 0 invalid</div>
                        </div>
                        <div class="table-responsive">
                            <table class="table table-sm table-vcenter">
                                <thead>
                                    <tr>
                                        <th>Source</th>
                                        <th>Tag</th>
                                        <th>Target</th>
                                        <th>App</th>
                                        <th>Container</th>
                                    </tr>
                                </thead>
                                <tbody id="import-preview-body">
                                    <tr>
                                        <td colspan="5" class="text-center text-secondary">No data yet</td>
                                    </tr>
                                </tbody>
                            </table>
                        </div>
                    </div>
                    <div class="modal-footer">
                        <button type="button" class="btn btn-link link-secondary" id="import-cancel-btn">Cancel</button>
                        <button type="button" class="btn btn-primary" id="import-apply-btn" disabled>Import</button>
                    </div>
                </div>
            </div>
        </div>
        <div class="modal-backdrop fade show"></div>
    `;

    document.body.insertAdjacentHTML('beforeend', modalHtml);

    const modal = document.getElementById('import-mappings-modal');
    const backdrop = document.querySelector('.modal-backdrop');
    const inputEl = document.getElementById('import-mappings-input');
    const rulesEl = document.getElementById('import-replace-rules');
    const addRuleBtn = document.getElementById('import-add-rule');
    const previewBody = document.getElementById('import-preview-body');
    const summaryEl = document.getElementById('import-summary');
    const applyBtn = document.getElementById('import-apply-btn');
    const cancelBtn = document.getElementById('import-cancel-btn');
    const closeBtn = document.getElementById('import-mappings-close');

    let rules = [{ find: '', replace: '' }];
    let parsed = { valid: [] };

    const cleanup = () => {
        modal.remove();
        backdrop.remove();
    };

    const renderRules = () => {
        rulesEl.innerHTML = rules.map((rule, idx) => `
            <div class="row g-2 align-items-end mb-2" data-rule-index="${idx}">
                <div class="col-md-5">
                    <input type="text" class="form-control form-control-sm rule-find" placeholder="Find"
                           value="${rule.find || ''}">
                </div>
                <div class="col-md-5">
                    <input type="text" class="form-control form-control-sm rule-replace" placeholder="Replace"
                           value="${rule.replace || ''}">
                </div>
                <div class="col-md-2">
                    <button type="button" class="btn btn-sm btn-outline-danger w-100 rule-remove" ${rules.length === 1 ? 'disabled' : ''}>
                        <i class="ti ti-trash"></i>
                    </button>
                </div>
            </div>
        `).join('');

        rulesEl.querySelectorAll('.rule-find').forEach((input, idx) => {
            input.addEventListener('input', () => {
                rules[idx].find = input.value;
                updatePreview();
            });
        });
        rulesEl.querySelectorAll('.rule-replace').forEach((input, idx) => {
            input.addEventListener('input', () => {
                rules[idx].replace = input.value;
                updatePreview();
            });
        });
        rulesEl.querySelectorAll('.rule-remove').forEach((btn, idx) => {
            btn.addEventListener('click', () => {
                if (rules.length === 1) return;
                rules.splice(idx, 1);
                renderRules();
                updatePreview();
            });
        });
    };

    const updatePreview = () => {
        parsed = parseMappingCsv(inputEl.value, rules);
        const { rows, valid, invalid } = parsed;
        summaryEl.textContent = `${valid.length} valid, ${invalid.length} invalid`;
        applyBtn.disabled = valid.length === 0;

        if (rows.length === 0) {
            previewBody.innerHTML = '<tr><td colspan="5" class="text-center text-secondary">No data yet</td></tr>';
            return;
        }

        previewBody.innerHTML = rows.map(row => `
            <tr class="${row.valid ? '' : 'table-danger'}">
                <td><code class="small">${row.source_image || '-'}</code></td>
                <td>${row.source_tag || '-'}</td>
                <td><code class="small">${row.target_image || '-'}</code></td>
                <td>${row.app_name || '-'}</td>
                <td>${row.container_name || '-'}</td>
            </tr>
        `).join('');
    };

    renderRules();
    updatePreview();

    inputEl.addEventListener('input', updatePreview);
    addRuleBtn.addEventListener('click', () => {
        rules.push({ find: '', replace: '' });
        renderRules();
    });

    const onCancel = () => cleanup();
    cancelBtn.addEventListener('click', onCancel);
    closeBtn.addEventListener('click', onCancel);

    applyBtn.addEventListener('click', () => {
        if (parsed.valid.length === 0) return;
        onApply(parsed.valid.map(row => ({
            source_image: row.source_image,
            source_tag: row.source_tag,
            target_image: row.target_image,
            app_name: row.app_name,
            container_name: row.container_name,
        })));
        cleanup();
    });
}

console.log('App.js loaded');
