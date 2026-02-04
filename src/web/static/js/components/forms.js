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

                ${isEdit ? `
                    <div class="mb-3">
                        <label class="form-label">Slug</label>
                        <div class="form-control-plaintext">
                            <code>${tenant.slug}</code>
                        </div>
                        <small class="form-hint text-muted">Slug cannot be changed after creation</small>
                    </div>
                ` : `
                    <div class="mb-3">
                        <label class="form-label required">Slug</label>
                        <input type="text" class="form-control" name="slug" id="tenant-slug"
                               placeholder="production"
                               pattern="[a-z0-9\\-]+"
                               required>
                        <small class="form-hint">Lowercase alphanumeric and dashes only</small>
                    </div>
                `}

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

    const authTypes = [
        { value: 'none', label: 'None (Public registry)' },
        { value: 'basic', label: 'Basic Auth (Username + Password)' },
        { value: 'token', label: 'Token Auth (Robot accounts)' },
        { value: 'bearer', label: 'Bearer Token (Service accounts)' },
    ];

    return `
        <form id="registry-form" class="card" x-data="{ authType: '${registry?.auth_type || 'none'}' }">
            <div class="card-header">
                <h3 class="card-title">${isEdit ? 'Edit Registry' : 'New Registry'}</h3>
            </div>
            <div class="card-body">
                <div class="mb-3">
                    <label class="form-label required">Tenant</label>
                    <select class="form-select" name="tenant_id" required>
                        <option value="">Select tenant...</option>
                        ${tenants.map(t => `
                            <option value="${t.id}" ${isEdit && registry.tenant_id === t.id ? 'selected' : ''}>
                                ${t.name}
                            </option>
                        `).join('')}
                    </select>
                </div>

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

                <hr class="my-4">
                <h4>Authentication</h4>

                <div class="mb-3">
                    <label class="form-label required">Auth Type</label>
                    <select class="form-select" name="auth_type" x-model="authType" required>
                        ${authTypes.map(type => `
                            <option value="${type.value}" ${registry?.auth_type === type.value ? 'selected' : ''}>
                                ${type.label}
                            </option>
                        `).join('')}
                    </select>
                    <small class="form-hint">
                        Basic: Docker Hub, generic registries (user:pass) |
                        Token: Harbor/Quay robots (user:token) |
                        Bearer: GCR/ECR (pure token)
                    </small>
                </div>

                <!-- Username field (shown for basic and token) -->
                <div class="mb-3" x-show="authType === 'basic' || authType === 'token'">
                    <label class="form-label">Username</label>
                    <input type="text" class="form-control" name="username"
                           value="${registry?.username || ''}"
                           placeholder="username or robot-account-name"
                           :required="authType === 'basic' || authType === 'token'">
                </div>

                <!-- Password field (shown for basic) -->
                <div class="mb-3" x-show="authType === 'basic'">
                    <label class="form-label">Password</label>
                    <input type="password" class="form-control" name="password"
                           placeholder="${isEdit ? 'Leave empty to keep current password' : 'Enter password'}"
                           :required="authType === 'basic' && ${!isEdit}">
                    ${isEdit ? '<small class="form-hint">Leave empty to keep current password</small>' : ''}
                </div>

                <!-- Token field (shown for token and bearer) -->
                <div class="mb-3" x-show="authType === 'token' || authType === 'bearer'">
                    <label class="form-label">Token</label>
                    <input type="password" class="form-control" name="token"
                           placeholder="${isEdit ? 'Leave empty to keep current token' : 'Enter token'}"
                           :required="(authType === 'token' || authType === 'bearer') && ${!isEdit}">
                    ${isEdit ? '<small class="form-hint">Leave empty to keep current token</small>' : ''}
                </div>

                <hr class="my-4">

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
 * Vytvoří deploy target form
 */
function createDeployTargetForm(target = null, tenants = [], encjsonKeys = []) {
    const isEdit = !!target;
    const gitAuthTypes = [
        { value: 'none', label: 'None' },
        { value: 'ssh', label: 'SSH Key' },
        { value: 'token', label: 'HTTPS Token' },
    ];

    const keys = encjsonKeys.length > 0 ? encjsonKeys : [{ public_key: '', has_private: false }];

    return `
        <form id="deploy-target-form" class="card" x-data="{ gitAuth: '${target?.git_auth_type || 'none'}' }">
            <div class="card-header">
                <h3 class="card-title">${isEdit ? 'Edit Deploy Target' : 'New Deploy Target'}</h3>
            </div>
            <div class="card-body">
                <div class="mb-3">
                    <label class="form-label required">Tenant</label>
                    <select class="form-select" name="tenant_id" required>
                        <option value="">Select tenant...</option>
                        ${tenants.map(t => `
                            <option value="${t.id}" ${isEdit && target.tenant_id === t.id ? 'selected' : ''}>
                                ${t.name}
                            </option>
                        `).join('')}
                    </select>
                </div>

                <div class="row">
                    <div class="col-md-6">
                        <div class="mb-3">
                            <label class="form-label required">Name</label>
                            <input type="text" class="form-control" name="name"
                                   value="${target?.name || ''}"
                                   placeholder="Deploy to Test" required>
                        </div>
                    </div>
                    <div class="col-md-6">
                        <div class="mb-3">
                            <label class="form-label required">Environment</label>
                            <input type="text" class="form-control" name="env_name"
                                   value="${target?.env_name || ''}"
                                   placeholder="test" required>
                        </div>
                    </div>
                </div>

                <hr class="my-4">
                <h4>Repositories</h4>

                <div class="mb-3">
                    <label class="form-label required">Environments Repo URL</label>
                    <input type="text" class="form-control" name="environments_repo_url"
                           value="${target?.environments_repo_url || ''}"
                           placeholder="git@host:org/tsm-environments.git" required>
                </div>

                <div class="mb-3">
                    <label class="form-label">Environments Branch</label>
                    <input type="text" class="form-control" name="environments_branch"
                           value="${target?.environments_branch || 'main'}"
                           placeholder="main">
                </div>

                <div class="mb-3">
                    <label class="form-label required">Deploy Repo URL</label>
                    <input type="text" class="form-control" name="deploy_repo_url"
                           value="${target?.deploy_repo_url || ''}"
                           placeholder="git@host:org/tsm-deploy.git" required>
                </div>

                <div class="mb-3">
                    <label class="form-label">Deploy Branch</label>
                    <input type="text" class="form-control" name="deploy_branch"
                           value="${target?.deploy_branch || 'main'}"
                           placeholder="main">
                </div>

                <div class="mb-3">
                    <label class="form-label">Deploy Path</label>
                    <input type="text" class="form-control" name="deploy_path"
                           value="${target?.deploy_path || ''}"
                           placeholder="deploy/${target?.env_name || ''}">
                    <small class="form-hint">Defaults to deploy/&lt;env&gt;</small>
                </div>

                <hr class="my-4">
                <h4>Git Auth</h4>

                <div class="mb-3">
                    <label class="form-label required">Auth Type</label>
                    <select class="form-select" name="git_auth_type" x-model="gitAuth" required>
                        ${gitAuthTypes.map(type => `
                            <option value="${type.value}" ${target?.git_auth_type === type.value ? 'selected' : ''}>
                                ${type.label}
                            </option>
                        `).join('')}
                    </select>
                </div>

                <div class="mb-3" x-show="gitAuth === 'token'">
                    <label class="form-label">Git Username</label>
                    <input type="text" class="form-control" name="git_username"
                           value="${target?.git_username || ''}"
                           placeholder="git username">
                </div>

                <div class="mb-3" x-show="gitAuth === 'token'">
                    <label class="form-label">Git Token</label>
                    <input type="password" class="form-control" name="git_token"
                           placeholder="${isEdit ? 'Leave blank to keep existing token' : 'token'}">
                </div>

                <div class="mb-3" x-show="gitAuth === 'ssh'">
                    <label class="form-label">SSH Private Key</label>
                    <textarea class="form-control" name="git_ssh_key" rows="5"
                              placeholder="${isEdit ? 'Leave blank to keep existing key' : '-----BEGIN OPENSSH PRIVATE KEY-----'}"></textarea>
                </div>

                <hr class="my-4">
                <h4>Encjson</h4>

                <div class="mb-3">
                    <label class="form-label">ENCJSON Key Dir</label>
                    <input type="text" class="form-control" name="encjson_key_dir"
                           value="${target?.encjson_key_dir || ''}"
                           placeholder="(optional)">
                </div>

                <div class="mb-3">
                    <label class="form-label">ENCJSON Keys</label>
                    <div id="encjson-keys-list">
                        ${keys.map((key, index) => `
                            <div class="card mb-2" data-encjson-index="${index}">
                                <div class="card-body p-3">
                                    <div class="row g-2">
                                        <div class="col-md-5">
                                            <input type="text" class="form-control form-control-sm encjson-public-key"
                                                   value="${key.public_key || ''}"
                                                   placeholder="public key (hex)">
                                        </div>
                                        <div class="col-md-6">
                                            <input type="password" class="form-control form-control-sm encjson-private-key"
                                                   placeholder="${key.has_private ? 'Leave blank to keep existing key' : 'private key (hex)'}">
                                        </div>
                                        <div class="col-md-1 d-flex align-items-center">
                                            <button type="button" class="btn btn-sm btn-ghost-danger encjson-remove">
                                                <i class="ti ti-trash"></i>
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        `).join('')}
                    </div>
                    <button type="button" class="btn btn-sm btn-outline-primary" id="encjson-add-btn">
                        <i class="ti ti-plus"></i>
                        Add Key Pair
                    </button>
                </div>

                <div class="mb-3">
                    <label class="form-check">
                        <input class="form-check-input" type="checkbox" name="is_active"
                               ${target?.is_active !== false ? 'checked' : ''}>
                        <span class="form-check-label">Active</span>
                    </label>
                </div>
            </div>
            <div class="card-footer text-end">
                <div class="d-flex">
                    <a href="#/tenants" class="btn btn-link">Cancel</a>
                    <button type="submit" class="btn btn-primary ms-auto">
                        <i class="ti ti-check me-2"></i>
                        ${isEdit ? 'Update Deploy Target' : 'Create Deploy Target'}
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

    // Convert checkbox values
    if (data.is_active !== undefined) {
        data.is_active = formData.get('is_active') === 'on';
    }

    // Clean up empty optional fields (convert empty strings to null or remove them)
    Object.keys(data).forEach(key => {
        if (typeof data[key] === 'string' && data[key].trim() === '') {
            // For optional fields like password, token, description - set to null
            if (['password', 'token', 'description', 'git_token', 'git_ssh_key', 'encjson_private_key', 'encjson_key_dir'].includes(key)) {
                data[key] = null;
            }
        }
    });

    // Debug log
    console.log('Form data (cleaned):', data);

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
