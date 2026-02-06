/**
 * Bundle Wizard komponenta
 */

class BundleWizard {
    constructor(options = {}) {
        this.currentStep = 1;
        this.options = {
            title: 'Create New Bundle',
            createLabel: 'Create Bundle',
            tenantLocked: false,
            showRegistrySelectors: true,
            ...options,
        };
        this.data = {
            bundle: {
                tenant_id: '',
                name: '',
                description: '',
                source_registry_id: '',
                target_registry_id: '',
                auto_tag_enabled: false,
            },
            imageMappings: [],
            replaceRules: [{ find: '', replace: '' }]
        };
        this.replaceRulesApplied = false;
    }

    /**
     * Renderuje wizard
     */
    render(tenants = [], registries = []) {
        return `
            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">${this.options.title}</h3>
                    <div class="card-subtitle">Step ${this.currentStep} of 3</div>
                </div>

                <!-- Progress bar -->
                <div class="progress progress-sm">
                    <div class="progress-bar" style="width: ${(this.currentStep / 3) * 100}%"></div>
                </div>

                <div class="card-body" id="wizard-content">
                    ${this.renderStep(tenants, registries)}
                </div>

                <div class="card-footer">
                    <div class="d-flex">
                        ${this.currentStep > 1 ? `
                            <button type="button" class="btn btn-link" id="wizard-prev">
                                <i class="ti ti-arrow-left"></i>
                                Previous
                            </button>
                        ` : `
                            <a href="#/bundles" class="btn btn-link">Cancel</a>
                        `}

                        ${this.currentStep < 3 ? `
                            <button type="button" class="btn btn-primary ms-auto" id="wizard-next">
                                Next
                                <i class="ti ti-arrow-right"></i>
                            </button>
                        ` : `
                            <button type="button" class="btn btn-success ms-auto" id="wizard-create">
                                <i class="ti ti-check"></i>
                                ${this.options.createLabel}
                            </button>
                        `}
                    </div>
                </div>
            </div>
        `;
    }

    /**
     * Renderuje konkrétní step
     */
    renderStep(tenants, registries) {
        switch (this.currentStep) {
            case 1:
                return this.renderStep1(tenants, registries);
            case 2:
                return this.renderStep2();
            case 3:
                return this.renderStep3(registries);
            default:
                return '';
        }
    }

    /**
     * Step 1: Základní informace
     */
    renderStep1(tenants, registries) {
        // Filter registries by selected tenant (show none if no tenant selected)
        const selectedTenantId = this.data.bundle.tenant_id;
        const filteredRegistries = selectedTenantId
            ? registries.filter(r => r.tenant_id === selectedTenantId)
            : [];

        const sourceRegistries = filteredRegistries.filter(r => r.role === 'source' || r.role === 'both');
        const targetRegistries = filteredRegistries.filter(r => r.role === 'target' || r.role === 'both');

        return `
            <h3 class="mb-3">Bundle Information</h3>

            <div class="mb-3">
                <label class="form-label required">Tenant</label>
                ${this.options.tenantLocked ? `
                    <input type="text" class="form-control" value="${tenants.find(t => t.id === this.data.bundle.tenant_id)?.name || ''}" disabled>
                    <input type="hidden" id="bundle-tenant" value="${this.data.bundle.tenant_id}">
                ` : `
                    <select class="form-select" id="bundle-tenant" required>
                        <option value="">Select tenant...</option>
                        ${tenants.map(t => `
                            <option value="${t.id}" ${this.data.bundle.tenant_id === t.id ? 'selected' : ''}>
                                ${t.name}
                            </option>
                        `).join('')}
                    </select>
                    ${this.options.showRegistrySelectors ? '<small class="form-hint">Selecting a tenant will filter available registries</small>' : ''}
                `}
            </div>

            <div class="mb-3">
                <label class="form-label required">Bundle Name</label>
                <input type="text" class="form-control" id="bundle-name"
                       value="${this.data.bundle.name}"
                       placeholder="e.g., NAC Production Bundle" required>
            </div>

            <div class="mb-3">
                <label class="form-label">Description</label>
                <textarea class="form-control" id="bundle-description" rows="3"
                          placeholder="Optional description">${this.data.bundle.description}</textarea>
            </div>

            ${this.options.showRegistrySelectors ? `
                <hr>

                <div class="row">
                    <div class="col-md-6">
                        <div class="mb-3">
                            <label class="form-label required">Source Registry</label>
                            <select class="form-select" id="bundle-source-registry" required>
                                <option value="">Select source...</option>
                                ${sourceRegistries.map(r => `
                                    <option value="${r.id}" ${this.data.bundle.source_registry_id === r.id ? 'selected' : ''}>
                                        ${r.name} (${r.registry_type})
                                    </option>
                                `).join('')}
                            </select>
                            <small class="form-hint">Registry to pull images from</small>
                        </div>
                    </div>

                    <div class="col-md-6">
                        <div class="mb-3">
                            <label class="form-label required">Target Registry</label>
                            <select class="form-select" id="bundle-target-registry" required>
                                <option value="">Select target...</option>
                                ${targetRegistries.map(r => `
                                    <option value="${r.id}" ${this.data.bundle.target_registry_id === r.id ? 'selected' : ''}>
                                        ${r.name} (${r.registry_type})
                                    </option>
                                `).join('')}
                            </select>
                            <small class="form-hint">Registry to push images to</small>
                        </div>
                    </div>
                </div>
            ` : ''}

            <div class="mb-3">
                <label class="form-check">
                    <input class="form-check-input" type="checkbox" id="bundle-auto-tag" ${this.data.bundle.auto_tag_enabled ? 'checked' : ''}>
                    <span class="form-check-label">Auto-generate target tag (YYYY.MM.DD.COUNTER)</span>
                </label>
                <small class="form-hint">Locks target tag input when starting copy jobs</small>
            </div>

        `;
    }

    /**
     * Step 2: Image Mappings
     */
    renderStep2() {
        return `
            <h3 class="mb-3">Image Mappings</h3>
            <p class="text-secondary mb-3">
                Define which images to include in this bundle and how they map from source to target.
            </p>

            ${this.options.enableReplaceRules ? `
                <hr class="my-3">
                <h4 class="mb-2">Replace Rules (target image only)</h4>
                <div class="text-secondary small mb-2">
                    Use rules to update target image paths (e.g. project prefix). Click "Apply" to update all rows.
                </div>
                ${this.renderReplaceRules()}
                <div class="d-flex flex-wrap gap-2 mt-2">
                    <button type="button" class="btn btn-outline-secondary" id="apply-replace-btn">
                        <i class="ti ti-repeat"></i>
                        Apply replace rules
                    </button>
                </div>
            ` : ''}

            <div id="mappings-list" class="mb-3">
                ${this.data.imageMappings.map((mapping, index) => `
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
                                    <small class="form-hint">Kubernetes app name (used for release manifest)</small>
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
                <button type="button" class="btn btn-outline-secondary" id="export-mappings-btn">
                    <i class="ti ti-clipboard-copy"></i>
                    Export
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

            ${this.data.imageMappings.length === 0 ? `
                <div class="alert alert-info mt-3">
                    <i class="ti ti-info-circle"></i>
                    Add at least one image mapping to continue
                </div>
            ` : ''}
        `;
    }

    renderReplaceRules() {
        return `
            <div id="replace-rules-list">
                ${this.data.replaceRules.map((rule, index) => `
                    <div class="row g-2 align-items-end mb-2" data-replace-index="${index}">
                        <div class="col-md-5">
                            <input type="text" class="form-control form-control-sm replace-find"
                                   placeholder="Find" value="${rule.find || ''}">
                        </div>
                        <div class="col-md-5">
                            <input type="text" class="form-control form-control-sm replace-replace"
                                   placeholder="Replace" value="${rule.replace || ''}">
                        </div>
                        <div class="col-md-2">
                            <button type="button" class="btn btn-sm btn-outline-danger w-100 replace-remove" ${this.data.replaceRules.length === 1 ? 'disabled' : ''}>
                                <i class="ti ti-trash"></i>
                            </button>
                        </div>
                    </div>
                `).join('')}
            </div>
            <button type="button" class="btn btn-sm btn-outline-secondary" id="replace-add-btn">
                <i class="ti ti-plus"></i>
                Add rule
            </button>
        `;
    }

    /**
     * Step 3: Review
     */
    renderStep3(registries) {
        const sourceRegistry = registries.find(r => r.id === this.data.bundle.source_registry_id);
        const targetRegistry = registries.find(r => r.id === this.data.bundle.target_registry_id);

        return `
            <h3 class="mb-3">Review Bundle</h3>

            <div class="card mb-3">
                <div class="card-header">
                    <h4 class="card-title">Bundle Information</h4>
                </div>
                <div class="card-body">
                    <dl class="row mb-0">
                        <dt class="col-4">Name:</dt>
                        <dd class="col-8">${this.data.bundle.name}</dd>

                        <dt class="col-4">Description:</dt>
                        <dd class="col-8">${this.data.bundle.description || '-'}</dd>

                        <dt class="col-4">Source Registry:</dt>
                        <dd class="col-8">${sourceRegistry?.name || 'N/A'}</dd>

                        <dt class="col-4">Target Registry:</dt>
                        <dd class="col-8">${targetRegistry?.name || 'N/A'}</dd>

                        <dt class="col-4">Auto Tag:</dt>
                        <dd class="col-8">${this.data.bundle.auto_tag_enabled ? 'Enabled' : 'Disabled'}</dd>
                    </dl>
                </div>
            </div>

            <div class="card">
                <div class="card-header">
                    <h4 class="card-title">Image Mappings (${this.data.imageMappings.length})</h4>
                </div>
                <div class="table-responsive">
                    <table class="table table-vcenter card-table">
                        <thead>
                            <tr>
                                <th>Source Image</th>
                                <th>Source Tag</th>
                                <th>→</th>
                                <th>Target Image</th>
                                <th>App</th>
                                <th>Container</th>
                            </tr>
                        </thead>
                        <tbody>
                            ${this.data.imageMappings.map(mapping => `
                                <tr>
                                    <td><code>${mapping.source_image}</code></td>
                                    <td><span class="badge">${mapping.source_tag}</span></td>
                                    <td class="text-center"><i class="ti ti-arrow-right"></i></td>
                                    <td><code>${mapping.target_image}</code></td>
                                    <td>${mapping.app_name || '-'}</td>
                                    <td>${mapping.container_name || '-'}</td>
                                </tr>
                            `).join('')}
                        </tbody>
                    </table>
                </div>
            </div>
        `;
    }

    /**
     * Validuje a uloží data z Step 1
     */
    saveStep1() {
        this.data.bundle.tenant_id = document.getElementById('bundle-tenant').value;
        this.data.bundle.name = document.getElementById('bundle-name').value;
        this.data.bundle.description = document.getElementById('bundle-description').value;
        const sourceSelect = document.getElementById('bundle-source-registry');
        const targetSelect = document.getElementById('bundle-target-registry');
        if (sourceSelect) {
            this.data.bundle.source_registry_id = sourceSelect.value;
        }
        if (targetSelect) {
            this.data.bundle.target_registry_id = targetSelect.value;
        }
        this.data.bundle.auto_tag_enabled = document.getElementById('bundle-auto-tag')?.checked || false;
        if (!this.data.bundle.tenant_id || !this.data.bundle.name) {
            throw new Error('Please fill in all required fields');
        }
        if (this.options.showRegistrySelectors) {
            if (!this.data.bundle.source_registry_id || !this.data.bundle.target_registry_id) {
                throw new Error('Please fill in all required fields');
            }
        }
    }

    /**
     * Uloží aktuální data z Step 2 formulářů (bez validace)
     */
    collectStep2Data() {
        const mappingCards = document.querySelectorAll('[data-mapping-index]');
        const mappings = [];

        mappingCards.forEach((card) => {
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
                container_name: containerName
            });
        });

        this.data.imageMappings = mappings;
    }

    /**
     * Validuje a uloží data z Step 2
     */
    saveStep2() {
        this.collectStep2Data();

        // Validate that we have at least one complete mapping
        const validMappings = this.data.imageMappings.filter(m =>
            m.source_image && (m.source_tag || 'latest') && m.target_image && m.app_name
        );

        if (validMappings.length === 0) {
            throw new Error('Please add at least one complete image mapping');
        }

        // Keep only valid mappings
        this.data.imageMappings = validMappings;
    }

    collectReplaceRules() {
        const rows = document.querySelectorAll('[data-replace-index]');
        const rules = [];
        rows.forEach(row => {
            const find = row.querySelector('.replace-find')?.value || '';
            const replace = row.querySelector('.replace-replace')?.value || '';
            if (find.trim()) {
                rules.push({ find, replace });
            }
        });
        this.data.replaceRules = rules.length > 0 ? rules : [{ find: '', replace: '' }];
    }

    applyReplaceRulesToMappings() {
        if (!this.options.enableReplaceRules) return;
        const rules = this.data.replaceRules || [];
        if (!rules.length) return;
        this.data.imageMappings = this.data.imageMappings.map(mapping => {
            let target = mapping.target_image || '';
            rules.forEach(rule => {
                const find = rule.find?.trim();
                if (!find) return;
                const replace = rule.replace ?? '';
                target = target.split(find).join(replace);
            });
            let appName = mapping.app_name || '';
            if (!appName && target) {
                const parts = target.split('/');
                appName = parts[parts.length - 1] || '';
            }
            return {
                ...mapping,
                target_image: target,
                app_name: appName,
            };
        });
    }

    /**
     * Přidá nový mapping
     */
    addMapping() {
        this.data.imageMappings.push({
            source_image: '',
            source_tag: '',
            target_image: '',
            app_name: '',
            container_name: ''
        });
    }

    /**
     * Duplikuje mapping
     */
    duplicateMapping(index) {
        const current = this.data.imageMappings[index];
        if (!current) return;
        this.data.imageMappings.splice(index + 1, 0, { ...current });
    }

    /**
     * Odebere mapping
     */
    removeMapping(index) {
        this.data.imageMappings.splice(index, 1);
    }

    /**
     * Vytvoří bundle přes API
     */
    async createBundle() {
        // Nejprve vytvoříme bundle
        const bundle = await api.createBundle(this.data.bundle.tenant_id, {
            name: this.data.bundle.name,
            description: this.data.bundle.description,
            source_registry_id: this.data.bundle.source_registry_id,
            target_registry_id: this.data.bundle.target_registry_id,
            auto_tag_enabled: this.data.bundle.auto_tag_enabled,
        });

        // Pak přidáme image mappings do verze 1
        for (const mapping of this.data.imageMappings) {
            await api.addImageMapping(bundle.id, 1, mapping);
        }

        return bundle;
    }
}
