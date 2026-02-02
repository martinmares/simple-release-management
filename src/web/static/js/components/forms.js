/**
 * Form komponenty a helpers
 */

/**
 * Převede text na slug (lowercase, no spaces, no diacritics)
 */
function slugify(text) {
    return text
        .toString()
        .toLowerCase()
        .trim()
        // Remove diacritics
        .normalize('NFD')
        .replace(/[\u0300-\u036f]/g, '')
        // Replace spaces and underscores with -
        .replace(/[\s_]+/g, '-')
        // Remove all non-word chars except -
        .replace(/[^\w-]+/g, '')
        // Replace multiple - with single -
        .replace(/--+/g, '-')
        // Remove leading/trailing -
        .replace(/^-+/, '')
        .replace(/-+$/, '');
}

/**
 * Setup auto-slug generation for tenant form
 */
function setupTenantSlugGeneration() {
    const nameInput = document.getElementById('tenant-name');
    const slugInput = document.getElementById('tenant-slug');

    if (!nameInput || !slugInput || slugInput.hasAttribute('readonly')) {
        return;
    }

    let manuallyEdited = false;

    // Mark as manually edited if user types in slug directly
    slugInput.addEventListener('input', () => {
        manuallyEdited = true;
    });

    // Auto-generate slug from name
    nameInput.addEventListener('input', (e) => {
        if (!manuallyEdited) {
            slugInput.value = slugify(e.target.value);
        }
    });
}

/**
 * Zobrazí confirmation dialog
 */
function showConfirmDialog(title, message, confirmText = 'Delete', cancelText = 'Cancel') {
    return new Promise((resolve) => {
        const dialogHtml = `
            <div class="modal modal-blur fade show" style="display: block;" id="confirm-modal">
                <div class="modal-dialog modal-sm modal-dialog-centered" role="document">
                    <div class="modal-content">
                        <div class="modal-body">
                            <div class="modal-title">${title}</div>
                            <div>${message}</div>
                        </div>
                        <div class="modal-footer">
                            <button type="button" class="btn btn-link link-secondary" data-bs-dismiss="modal" id="cancel-btn">
                                ${cancelText}
                            </button>
                            <button type="button" class="btn btn-danger" id="confirm-btn">
                                ${confirmText}
                            </button>
                        </div>
                    </div>
                </div>
            </div>
            <div class="modal-backdrop fade show"></div>
        `;

        document.body.insertAdjacentHTML('beforeend', dialogHtml);

        const modal = document.getElementById('confirm-modal');
        const backdrop = document.querySelector('.modal-backdrop');
        const confirmBtn = document.getElementById('confirm-btn');
        const cancelBtn = document.getElementById('cancel-btn');

        const cleanup = () => {
            modal.remove();
            backdrop.remove();
        };

        confirmBtn.addEventListener('click', () => {
            cleanup();
            resolve(true);
        });

        cancelBtn.addEventListener('click', () => {
            cleanup();
            resolve(false);
        });
    });
}

/**
 * Vytvoří tenant form
 */
function createTenantForm(tenant = null) {
    const isEdit = !!tenant;

    return `
        <form id="tenant-form" class="card">
            <div class="card-header">
                <h3 class="card-title">${isEdit ? 'Edit Tenant' : 'New Tenant'}</h3>
            </div>
            <div class="card-body">
                <div class="mb-3">
                    <label class="form-label required">Name</label>
                    <input type="text" class="form-control" name="name" id="tenant-name"
                           value="${tenant?.name || ''}"
                           placeholder="Production Environment" required>
                    <small class="form-hint">Friendly name for this tenant</small>
                </div>

                <div class="mb-3">
                    <label class="form-label required">Slug</label>
                    <input type="text" class="form-control" name="slug" id="tenant-slug"
                           value="${tenant?.slug || ''}"
                           placeholder="production"
                           pattern="^[a-z0-9-]+$"
                           ${isEdit ? 'readonly' : ''}
                           required>
                    <small class="form-hint">Lowercase alphanumeric and dashes only${isEdit ? ' (cannot be changed)' : ''}</small>
                </div>

                <div class="mb-3">
                    <label class="form-label">Description</label>
                    <textarea class="form-control" name="description" rows="3"
                              placeholder="Optional description">${tenant?.description || ''}</textarea>
                </div>
            </div>
            <div class="card-footer text-end">
                <div class="d-flex">
                    <a href="#/tenants" class="btn btn-link">Cancel</a>
                    <button type="submit" class="btn btn-primary ms-auto">
                        <i class="ti ti-check me-2"></i>
                        ${isEdit ? 'Update Tenant' : 'Create Tenant'}
                    </button>
                </div>
            </div>
        </form>
    `;
}

/**
 * Vytvoří registry form
 */
function createRegistryForm(registry = null, tenants = []) {
    const isEdit = !!registry;

    const registryTypes = [
        { value: 'harbor', label: 'Harbor' },
        { value: 'docker', label: 'Docker Registry' },
        { value: 'quay', label: 'Quay.io' },
        { value: 'gcr', label: 'Google Container Registry' },
        { value: 'ecr', label: 'AWS Elastic Container Registry' },
        { value: 'acr', label: 'Azure Container Registry' },
        { value: 'generic', label: 'Generic Registry' },
    ];

    const roles = [
        { value: 'source', label: 'Source (Pull only)' },
        { value: 'target', label: 'Target (Push only)' },
        { value: 'both', label: 'Both (Pull & Push)' },
    ];

    return `
        <form id="registry-form" class="card">
            <div class="card-header">
                <h3 class="card-title">${isEdit ? 'Edit Registry' : 'New Registry'}</h3>
            </div>
            <div class="card-body">
                ${!isEdit ? `
                    <div class="mb-3">
                        <label class="form-label required">Tenant</label>
                        <select class="form-select" name="tenant_id" required>
                            <option value="">Select tenant...</option>
                            ${tenants.map(t => `
                                <option value="${t.id}">${t.name}</option>
                            `).join('')}
                        </select>
                    </div>
                ` : ''}

                <div class="mb-3">
                    <label class="form-label required">Name</label>
                    <input type="text" class="form-control" name="name"
                           value="${registry?.name || ''}"
                           placeholder="Production Harbor" required>
                </div>

                <div class="mb-3">
                    <label class="form-label required">Base URL</label>
                    <input type="url" class="form-control" name="base_url"
                           value="${registry?.base_url || ''}"
                           placeholder="https://registry.example.com" required>
                    <small class="form-hint">Full URL to the registry (including https://)</small>
                </div>

                <div class="row">
                    <div class="col-md-6">
                        <div class="mb-3">
                            <label class="form-label required">Registry Type</label>
                            <select class="form-select" name="registry_type" required>
                                <option value="">Select type...</option>
                                ${registryTypes.map(type => `
                                    <option value="${type.value}" ${registry?.registry_type === type.value ? 'selected' : ''}>
                                        ${type.label}
                                    </option>
                                `).join('')}
                            </select>
                        </div>
                    </div>

                    <div class="col-md-6">
                        <div class="mb-3">
                            <label class="form-label required">Role</label>
                            <select class="form-select" name="role" required>
                                <option value="">Select role...</option>
                                ${roles.map(role => `
                                    <option value="${role.value}" ${registry?.role === role.value ? 'selected' : ''}>
                                        ${role.label}
                                    </option>
                                `).join('')}
                            </select>
                        </div>
                    </div>
                </div>

                <div class="mb-3">
                    <label class="form-label">Description</label>
                    <textarea class="form-control" name="description" rows="2"
                              placeholder="Optional description">${registry?.description || ''}</textarea>
                </div>

                <div class="mb-3">
                    <label class="form-check">
                        <input class="form-check-input" type="checkbox" name="is_active"
                               ${registry?.is_active !== false ? 'checked' : ''}>
                        <span class="form-check-label">Active</span>
                    </label>
                    <small class="form-hint">Inactive registries cannot be used for operations</small>
                </div>
            </div>
            <div class="card-footer text-end">
                <div class="d-flex">
                    <a href="#/registries" class="btn btn-link">Cancel</a>
                    <button type="submit" class="btn btn-primary ms-auto">
                        <i class="ti ti-check me-2"></i>
                        ${isEdit ? 'Update Registry' : 'Create Registry'}
                    </button>
                </div>
            </div>
        </form>
    `;
}

/**
 * Handle form submission s error handlingem
 */
async function handleFormSubmit(event, submitHandler) {
    event.preventDefault();

    const form = event.target;
    const formData = new FormData(form);
    const data = Object.fromEntries(formData.entries());

    // Debug log
    console.log('Form data:', data);

    // Convert checkbox values
    if (data.is_active !== undefined) {
        data.is_active = formData.get('is_active') === 'on';
    }

    // Disable form during submission
    const submitBtn = form.querySelector('button[type="submit"]');
    const originalText = submitBtn.innerHTML;
    submitBtn.disabled = true;
    submitBtn.innerHTML = '<span class="spinner-border spinner-border-sm me-2"></span>Saving...';

    try {
        await submitHandler(data);
    } catch (error) {
        console.error('Form submission error:', error);
        throw error;
    } finally {
        submitBtn.disabled = false;
        submitBtn.innerHTML = originalText;
    }
}
