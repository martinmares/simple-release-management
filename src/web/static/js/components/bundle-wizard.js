/**
 * Bundle Wizard komponenta
 */

class BundleWizard {
    constructor() {
        this.currentStep = 1;
        this.data = {
            bundle: {
                tenant_id: '',
                name: '',
                description: '',
                source_registry_id: '',
                target_registry_id: '',
            },
            imageMappings: []
        };
    }

    /**
     * Renderuje wizard
     */
    render(tenants = [], registries = []) {
        return `
            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">Create New Bundle</h3>
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
                                Create Bundle
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
        const sourceRegistries = registries.filter(r => r.role === 'source' || r.role === 'both');
        const targetRegistries = registries.filter(r => r.role === 'target' || r.role === 'both');

        return `
            <h3 class="mb-3">Bundle Information</h3>

            <div class="mb-3">
                <label class="form-label required">Tenant</label>
                <select class="form-select" id="bundle-tenant" required>
                    <option value="">Select tenant...</option>
                    ${tenants.map(t => `
                        <option value="${t.id}" ${this.data.bundle.tenant_id === t.id ? 'selected' : ''}>
                            ${t.name}
                        </option>
                    `).join('')}
                </select>
            </div>

            <div class="mb-3">
                <label class="form-label required">Bundle Name</label>
                <input type="text" class="form-control" id="bundle-name"
                       value="${this.data.bundle.name}"
                       placeholder="e.g., NAC Production Release" required>
            </div>

            <div class="mb-3">
                <label class="form-label">Description</label>
                <textarea class="form-control" id="bundle-description" rows="3"
                          placeholder="Optional description">${this.data.bundle.description}</textarea>
            </div>

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

            ${this.data.imageMappings.length === 0 ? `
                <div class="alert alert-info mt-3">
                    <i class="ti ti-info-circle"></i>
                    Add at least one image mapping to continue
                </div>
            ` : ''}
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
                            </tr>
                        </thead>
                        <tbody>
                            ${this.data.imageMappings.map(mapping => `
                                <tr>
                                    <td><code>${mapping.source_image}</code></td>
                                    <td><span class="badge">${mapping.source_tag}</span></td>
                                    <td class="text-center"><i class="ti ti-arrow-right"></i></td>
                                    <td><code>${mapping.target_image}</code></td>
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
        this.data.bundle.source_registry_id = document.getElementById('bundle-source-registry').value;
        this.data.bundle.target_registry_id = document.getElementById('bundle-target-registry').value;

        if (!this.data.bundle.tenant_id || !this.data.bundle.name ||
            !this.data.bundle.source_registry_id || !this.data.bundle.target_registry_id) {
            throw new Error('Please fill in all required fields');
        }
    }

    /**
     * Validuje a uloží data z Step 2
     */
    saveStep2() {
        const mappingCards = document.querySelectorAll('[data-mapping-index]');
        this.data.imageMappings = [];

        mappingCards.forEach((card) => {
            const sourceImage = card.querySelector('.mapping-source-image').value;
            const sourceTag = card.querySelector('.mapping-source-tag').value;
            const targetImage = card.querySelector('.mapping-target-image').value;

            if (sourceImage && sourceTag && targetImage) {
                this.data.imageMappings.push({
                    source_image: sourceImage,
                    source_tag: sourceTag,
                    target_image: targetImage
                });
            }
        });

        if (this.data.imageMappings.length === 0) {
            throw new Error('Please add at least one image mapping');
        }
    }

    /**
     * Přidá nový mapping
     */
    addMapping() {
        this.data.imageMappings.push({
            source_image: '',
            source_tag: '',
            target_image: ''
        });
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
        });

        // Pak přidáme image mappings do verze 1
        for (const mapping of this.data.imageMappings) {
            await api.addImageMapping(bundle.id, 1, mapping);
        }

        return bundle;
    }
}
