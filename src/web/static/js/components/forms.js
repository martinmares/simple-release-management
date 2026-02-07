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
 * Zobrazí dialog s výběrem z možností
 */
function showSelectDialog(title, label, options = [], confirmText = 'Select', cancelText = 'Cancel') {
    return new Promise((resolve) => {
        const dialogHtml = `
            <div class="modal modal-blur fade show" style="display: block;" id="select-modal">
                <div class="modal-dialog modal-sm modal-dialog-centered" role="document">
                    <div class="modal-content">
                        <div class="modal-body">
                            <div class="modal-title">${title}</div>
                            <div class="mt-2">
                                <label class="form-label">${label}</label>
                                <select class="form-select" id="select-input">
                                    <option value="">Select...</option>
                                    ${options.map(opt => `
                                        <option value="${opt.value}">${opt.label}</option>
                                    `).join('')}
                                </select>
                            </div>
                        </div>
                        <div class="modal-footer">
                            <button type="button" class="btn btn-link link-secondary" data-bs-dismiss="modal" id="select-cancel-btn">
                                ${cancelText}
                            </button>
                            <button type="button" class="btn btn-primary" id="select-confirm-btn">
                                ${confirmText}
                            </button>
                        </div>
                    </div>
                </div>
            </div>
            <div class="modal-backdrop fade show"></div>
        `;

        document.body.insertAdjacentHTML('beforeend', dialogHtml);

        const modal = document.getElementById('select-modal');
        const backdrop = document.querySelector('.modal-backdrop');
        const confirmBtn = document.getElementById('select-confirm-btn');
        const cancelBtn = document.getElementById('select-cancel-btn');
        const selectInput = document.getElementById('select-input');

        const cleanup = () => {
            modal.remove();
            backdrop.remove();
        };

        confirmBtn.addEventListener('click', () => {
            const value = selectInput.value;
            if (!value) {
                return;
            }
            cleanup();
            resolve(value);
        });

        cancelBtn.addEventListener('click', () => {
            cleanup();
            resolve(null);
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
function createRegistryForm(registry = null, tenants = [], environments = [], environmentPaths = [], environmentCredentials = [], environmentAccess = []) {
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

    const envPathMap = new Map((environmentPaths || []).map(item => [
        item.environment_id,
        {
            source: item.source_project_path_override || '',
            target: item.target_project_path_override || '',
        },
    ]));
    const envCredMap = new Map((environmentCredentials || []).map(item => [
        item.environment_id,
        {
            auth_type: item.auth_type || '',
            username: item.username || '',
            has_password: item.has_password,
            has_token: item.has_token,
        },
    ]));
    const envAccessMap = new Map((environmentAccess || []).map(item => [
        item.environment_id,
        item.is_enabled !== false,
    ]));

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
                    <input type="text" class="form-control" name="base_url"
                           value="${registry?.base_url || ''}"
                           placeholder="https://registry.example.com" required>
                    <small class="form-hint">Hostname or URL (e.g. registry.example.com or https://registry.example.com)</small>
                </div>

                <div class="mb-3">
                    <label class="form-label">Default project path</label>
                    <input type="text" class="form-control" name="default_project_path"
                           value="${registry?.default_project_path || ''}"
                           placeholder="project-path">
                    <small class="form-hint">Optional path prefix for Release Images targets (no leading slash)</small>
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
                    <a href="#/tenants${registry?.tenant_id ? `/${registry.tenant_id}` : ''}" class="btn btn-link">Cancel</a>
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
 * Vytvoří git repository form
 */
function createGitRepoForm(repo = null, tenants = []) {
    const isEdit = !!repo;
    const authTypes = [
        { value: 'none', label: 'None' },
        { value: 'ssh', label: 'SSH Key' },
        { value: 'token', label: 'HTTPS Token' },
    ];

    return `
        <form id="git-repo-form" class="card" x-data="{ gitAuth: '${repo?.git_auth_type || 'none'}' }">
            <div class="card-header">
                <h3 class="card-title">${isEdit ? 'Edit Git Repository' : 'New Git Repository'}</h3>
            </div>
            <div class="card-body">
                <div class="mb-3">
                    <label class="form-label required">Tenant</label>
                    <select class="form-select" name="tenant_id" required>
                        <option value="">Select tenant...</option>
                        ${tenants.map(t => `
                            <option value="${t.id}" ${isEdit && repo.tenant_id === t.id ? 'selected' : ''}>
                                ${t.name}
                            </option>
                        `).join('')}
                    </select>
                </div>

                <div class="mb-3">
                    <label class="form-label required">Name</label>
                    <input type="text" class="form-control" name="name"
                           value="${repo?.name || ''}"
                           placeholder="tsm-environments" required>
                </div>

                <div class="mb-3">
                    <label class="form-label required">Repository URL</label>
                    <input type="text" class="form-control" name="repo_url"
                           value="${repo?.repo_url || ''}"
                           placeholder="git@host:org/repo.git" required>
                </div>

                <div class="mb-3">
                    <label class="form-label">Default Branch</label>
                    <input type="text" class="form-control" name="default_branch"
                           value="${repo?.default_branch || 'main'}"
                           placeholder="main">
                </div>

                <hr class="my-4">
                <h4>Git Auth</h4>

                <div class="mb-3">
                    <label class="form-label required">Auth Type</label>
                    <select class="form-select" name="git_auth_type" x-model="gitAuth" required>
                        ${authTypes.map(type => `
                            <option value="${type.value}" ${repo?.git_auth_type === type.value ? 'selected' : ''}>
                                ${type.label}
                            </option>
                        `).join('')}
                    </select>
                </div>

                <div class="mb-3" x-show="gitAuth === 'token'">
                    <label class="form-label">Git Username</label>
                    <input type="text" class="form-control" name="git_username"
                           value="${repo?.git_username || ''}"
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
            </div>
            <div class="card-footer text-end">
                <div class="d-flex">
                    <a href="#/tenants${repo?.tenant_id ? `/${repo.tenant_id}` : ''}" class="btn btn-link">Cancel</a>
                    <button type="submit" class="btn btn-primary ms-auto">
                        <i class="ti ti-check me-2"></i>
                        ${isEdit ? 'Update Git Repo' : 'Create Git Repo'}
                    </button>
                </div>
            </div>
        </form>
    `;
}

/**
 * Vytvoří environment form
 */
function createEnvironmentForm(environment = null, tenants = [], registries = [], gitRepos = []) {
    const isEdit = !!environment;
    const sourceRegistries = registries.filter(r => r.role === 'source' || r.role === 'both');
    const targetRegistries = registries.filter(r => r.role === 'target' || r.role === 'both');
    const authTypes = [
        { value: '', label: 'Use registry default' },
        { value: 'basic', label: 'Basic (user:pass)' },
        { value: 'token', label: 'Token (user:token)' },
        { value: 'bearer', label: 'Bearer (token)' },
    ];
    const releaseModes = [
        { value: 'match_digest', label: 'Match digest (recommended)' },
        { value: 'match_tag', label: 'Match tag' },
        { value: 'strict_digest', label: 'Strict digest only' },
        { value: 'strict_tag', label: 'Strict tag only' },
    ];
    const envVarRows = environment?.release_env_var_mappings
        ? Object.entries(environment.release_env_var_mappings).map(([source_key, target_key]) => ({ source_key, target_key }))
        : [];
    const extraVarRows = environment?.extra_env_vars
        ? Object.entries(environment.extra_env_vars).map(([key, value]) => ({ key, value }))
        : [];
    const envVarList = envVarRows.length > 0 ? envVarRows : [{ source_key: '', target_key: '' }];
    const extraVarList = extraVarRows.length > 0 ? extraVarRows : [{ key: '', value: '' }];
    return `
        <form id="environment-form" class="card" data-env-mode="${isEdit ? 'edit' : 'new'}">
            <div class="card-header">
                <h3 class="card-title">${isEdit ? 'Edit Environment' : 'New Environment'}</h3>
            </div>
            <div class="card-body">
                <div class="mb-3">
                    <label class="form-label required">Tenant</label>
                    <select class="form-select" name="tenant_id" ${isEdit ? 'disabled' : ''} required>
                        <option value="">Select tenant...</option>
                        ${tenants.map(t => `
                            <option value="${t.id}" ${environment?.tenant_id === t.id ? 'selected' : ''}>
                                ${t.name}
                            </option>
                        `).join('')}
                    </select>
                    ${isEdit ? `<input type="hidden" name="tenant_id" value="${environment.tenant_id}">` : ''}
                </div>

                <div class="mb-3">
                    <label class="form-label required">Name</label>
                    <input type="text" class="form-control" name="name"
                           value="${environment?.name || ''}"
                           placeholder="dev, test, prod" required>
                </div>

                <div class="mb-3">
                    <label class="form-label">Slug</label>
                    <input type="text" class="form-control" id="env-slug-preview" name="slug"
                           value="${environment?.slug || ''}"
                           placeholder="generated-from-name">
                    <small class="form-hint">Used in paths, tags, and lookups. Edit only if needed.</small>
                </div>

                <div class="mb-3">
                    <label class="form-label">Color</label>
                    <div class="row g-2 align-items-end">
                        <div class="col-md-3">
                            <input type="color" class="form-control form-control-color env-color-input"
                                   value="${environment?.color || '#1f6feb'}"
                                   title="Pick a color">
                        </div>
                        <div class="col-md-5">
                            <input type="text" class="form-control env-color-text" name="color"
                                   value="${environment?.color || ''}"
                                   placeholder="#1f6feb">
                        </div>
                        <div class="col-md-4">
                            <div class="text-secondary small mb-1">Preview</div>
                            <span id="env-color-preview" class="badge" style="${environment?.color ? `background:${environment.color};color:#fff;` : ''}">
                                ${environment?.color ? environment.color : 'Preview'}
                            </span>
                        </div>
                    </div>
                    <small class="form-hint">Optional label color (hex). Used for environment badges.</small>
                </div>

                <hr class="my-4">
                <h4>Registries</h4>
                <div class="row g-3">
                    <div class="col-md-6">
                        <label class="form-label required">Source Registry</label>
                        <select class="form-select" name="source_registry_id" required>
                            <option value="">Select source...</option>
                            ${sourceRegistries.map(r => `
                                <option value="${r.id}" ${environment?.source_registry_id === r.id ? 'selected' : ''}>
                                    ${r.name} (${r.base_url})
                                </option>
                            `).join('')}
                        </select>
                        <div class="form-hint">Registry used as source for copy jobs.</div>
                    </div>
                    <div class="col-md-6">
                        <label class="form-label required">Target Registry</label>
                        <select class="form-select" name="target_registry_id" required>
                            <option value="">Select target...</option>
                            ${targetRegistries.map(r => `
                                <option value="${r.id}" ${environment?.target_registry_id === r.id ? 'selected' : ''}>
                                    ${r.name} (${r.base_url})
                                </option>
                            `).join('')}
                        </select>
                        <div class="form-hint">Registry used as target for copy jobs and releases.</div>
                    </div>
                </div>

                <div class="row g-3 mt-1">
                    <div class="col-md-6">
                        <label class="form-label">Source Project Path</label>
                        <input type="text" class="form-control" name="source_project_path"
                               value="${environment?.source_project_path || ''}"
                               placeholder="project/path">
                    </div>
                    <div class="col-md-6">
                        <label class="form-label">Target Project Path</label>
                        <input type="text" class="form-control" name="target_project_path"
                               value="${environment?.target_project_path || ''}"
                               placeholder="project/path">
                    </div>
                </div>

                <hr class="my-4">
                <h4>Registry Credentials (override)</h4>
                <div class="row g-3">
                    <div class="col-md-6">
                        <div class="mb-2 fw-semibold">Source credentials</div>
                        <select class="form-select mb-2" name="source_auth_type">
                            ${authTypes.map(t => `
                                <option value="${t.value}" ${environment?.source_auth_type === t.value ? 'selected' : ''}>${t.label}</option>
                            `).join('')}
                        </select>
                        <input type="text" class="form-control mb-2" name="source_username" placeholder="username"
                               value="${environment?.source_username || ''}">
                        <input type="password" class="form-control mb-2" name="source_password" placeholder="password (leave blank to keep)">
                        <input type="password" class="form-control" name="source_token" placeholder="token (leave blank to keep)">
                    </div>
                    <div class="col-md-6">
                        <div class="mb-2 fw-semibold">Target credentials</div>
                        <select class="form-select mb-2" name="target_auth_type">
                            ${authTypes.map(t => `
                                <option value="${t.value}" ${environment?.target_auth_type === t.value ? 'selected' : ''}>${t.label}</option>
                            `).join('')}
                        </select>
                        <input type="text" class="form-control mb-2" name="target_username" placeholder="username"
                               value="${environment?.target_username || ''}">
                        <input type="password" class="form-control mb-2" name="target_password" placeholder="password (leave blank to keep)">
                        <input type="password" class="form-control" name="target_token" placeholder="token (leave blank to keep)">
                    </div>
                </div>

                <hr class="my-4">
                <h4>Git Repositories</h4>
                <div class="row g-3">
                    <div class="col-md-6">
                        <label class="form-label required">Environment Repo</label>
                        <select class="form-select" name="env_repo_id" required>
                            <option value="">Select repository...</option>
                            ${gitRepos.map(repo => `
                                <option value="${repo.id}" ${environment?.env_repo_id === repo.id ? 'selected' : ''}>
                                    ${repo.name}
                                </option>
                            `).join('')}
                        </select>
                        <div class="row g-2 mt-2">
                            <div class="col-md-6">
                                <label class="form-label">Repo path</label>
                                <input type="text" class="form-control" name="env_repo_path"
                                       value="${environment?.env_repo_path || ''}"
                                       placeholder="dev / test / prod">
                            </div>
                            <div class="col-md-6">
                                <label class="form-label">Repo branch</label>
                                <input type="text" class="form-control" name="env_repo_branch"
                                       value="${environment?.env_repo_branch || ''}"
                                       placeholder="main">
                            </div>
                        </div>
                    </div>
                    <div class="col-md-6">
                        <label class="form-label required">Deploy Repo</label>
                        <select class="form-select" name="deploy_repo_id" required>
                            <option value="">Select repository...</option>
                            ${gitRepos.map(repo => `
                                <option value="${repo.id}" ${environment?.deploy_repo_id === repo.id ? 'selected' : ''}>
                                    ${repo.name}
                                </option>
                            `).join('')}
                        </select>
                        <div class="row g-2 mt-2">
                            <div class="col-md-6">
                                <label class="form-label">Repo path</label>
                                <input type="text" class="form-control" name="deploy_repo_path"
                                       value="${environment?.deploy_repo_path || ''}"
                                       placeholder="deploy/dev">
                            </div>
                            <div class="col-md-6">
                                <label class="form-label">Repo branch</label>
                                <input type="text" class="form-control" name="deploy_repo_branch"
                                       value="${environment?.deploy_repo_branch || ''}"
                                       placeholder="main">
                            </div>
                        </div>
                    </div>
                </div>

                <hr class="my-4">
                <h4>Deploy Options</h4>
                <div class="row g-3">
                    <div class="col-md-6">
                        <label class="form-check">
                            <input class="form-check-input" type="checkbox" name="allow_auto_release" ${environment?.allow_auto_release ? 'checked' : ''}>
                            <span class="form-check-label">Allow auto release</span>
                        </label>
                        <label class="form-check mt-2">
                            <input class="form-check-input" type="checkbox" name="append_env_suffix" ${environment?.append_env_suffix ? 'checked' : ''}>
                            <span class="form-check-label">Append env suffix to release tag</span>
                        </label>
                    </div>
                    <div class="col-md-6">
                        <label class="form-label">Release manifest mode</label>
                        <select class="form-select" name="release_manifest_mode">
                            ${releaseModes.map(m => `
                                <option value="${m.value}" ${environment?.release_manifest_mode === m.value ? 'selected' : ''}>${m.label}</option>
                            `).join('')}
                        </select>
                        <label class="form-label mt-2">ArgoCD poll interval</label>
                        <select class="form-select" name="argocd_poll_interval_seconds">
                            ${[
                                { value: 0, label: 'Off' },
                                { value: 10, label: '10s' },
                                { value: 30, label: '30s' },
                                { value: 60, label: '60s' },
                            ].map(o => `
                                <option value="${o.value}" ${Number(environment?.argocd_poll_interval_seconds ?? 0) === o.value ? 'selected' : ''}>${o.label}</option>
                            `).join('')}
                        </select>
                        <label class="form-label mt-2">Encjson key dir</label>
                        <input type="text" class="form-control" name="encjson_key_dir"
                               value="${environment?.encjson_key_dir || ''}"
                               placeholder="~/.config/encjson">
                    </div>
                </div>

                <hr class="my-4">
                <h4>Release Env Var Mappings</h4>
                <div id="env-var-mappings">
                    ${envVarList.map((row, idx) => `
                        <div class="row g-2 mb-2" data-env-var-index="${idx}">
                            <div class="col-md-5">
                                <input type="text" class="form-control env-var-source" placeholder="SIMPLE_RELEASE_ID"
                                       value="${row.source_key}">
                            </div>
                            <div class="col-md-5">
                                <input type="text" class="form-control env-var-target" placeholder="TSM_RELEASE_ID"
                                       value="${row.target_key}">
                            </div>
                            <div class="col-md-2">
                                <button type="button" class="btn btn-outline-danger w-100 env-var-remove">
                                    <i class="ti ti-trash"></i>
                                </button>
                            </div>
                        </div>
                    `).join('')}
                    <button type="button" class="btn btn-outline-primary btn-sm" id="env-var-add">
                        <i class="ti ti-plus"></i>
                        Add mapping
                    </button>
                </div>

                <hr class="my-4">
                <h4>Extra Env Vars</h4>
                <div id="extra-env-vars">
                    ${extraVarList.map((row, idx) => `
                        <div class="row g-2 mb-2" data-extra-var-index="${idx}">
                            <div class="col-md-5">
                                <input type="text" class="form-control extra-var-key" placeholder="KEY"
                                       value="${row.key}">
                            </div>
                            <div class="col-md-5">
                                <input type="text" class="form-control extra-var-value" placeholder="VALUE"
                                       value="${row.value}">
                            </div>
                            <div class="col-md-2">
                                <button type="button" class="btn btn-outline-danger w-100 extra-var-remove">
                                    <i class="ti ti-trash"></i>
                                </button>
                            </div>
                        </div>
                    `).join('')}
                    <button type="button" class="btn btn-outline-primary btn-sm" id="extra-var-add">
                        <i class="ti ti-plus"></i>
                        Add env var
                    </button>
                </div>
            </div>
            <div class="card-footer text-end">
                <div class="d-flex">
                    <a href="#/tenants${environment?.tenant_id ? `/${environment.tenant_id}` : ''}" class="btn btn-link">Cancel</a>
                    <button type="submit" class="btn btn-primary ms-auto">
                        <i class="ti ti-check me-2"></i>
                        ${isEdit ? 'Update Environment' : 'Create Environment'}
                    </button>
                </div>
            </div>
        </form>
    `;
}

/**
 * Vytvoří ArgoCD instance form
 */
function createArgocdInstanceForm(instance = null, tenants = []) {
    const isEdit = !!instance;
    const authTypes = [
        { value: 'basic', label: 'Username + Password' },
        { value: 'token', label: 'Token' },
    ];
    return `
        <form id="argocd-instance-form" class="card" data-argocd-mode="${isEdit ? 'edit' : 'new'}">
            <div class="card-header">
                <h3 class="card-title">${isEdit ? 'Edit ArgoCD Instance' : 'New ArgoCD Instance'}</h3>
            </div>
            <div class="card-body">
                <div class="mb-3">
                    <label class="form-label required">Tenant</label>
                    <select class="form-select" name="tenant_id" ${isEdit ? 'disabled' : ''} required>
                        <option value="">Select tenant...</option>
                        ${tenants.map(t => `
                            <option value="${t.id}" ${instance?.tenant_id === t.id ? 'selected' : ''}>
                                ${t.name}
                            </option>
                        `).join('')}
                    </select>
                    ${isEdit ? `<input type="hidden" name="tenant_id" value="${instance.tenant_id}">` : ''}
                </div>

                <div class="mb-3">
                    <label class="form-label required">Name</label>
                    <input type="text" class="form-control" name="name"
                           value="${instance?.name || ''}"
                           placeholder="ArgoCD (prod)" required>
                </div>

                <div class="mb-3">
                    <label class="form-label required">Base URL</label>
                    <input type="url" class="form-control" name="base_url"
                           value="${instance?.base_url || ''}"
                           placeholder="https://argocd.example.com" required>
                </div>

                <div class="mb-3">
                    <label class="form-label required">Auth Type</label>
                    <select class="form-select" name="auth_type" required>
                        ${authTypes.map(t => `
                            <option value="${t.value}" ${instance?.auth_type === t.value ? 'selected' : ''}>${t.label}</option>
                        `).join('')}
                    </select>
                </div>

                <div class="row g-3">
                    <div class="col-md-6">
                        <label class="form-label">Username</label>
                        <input type="text" class="form-control" name="username"
                               value="${instance?.username || ''}"
                               placeholder="admin">
                    </div>
                    <div class="col-md-6">
                        <label class="form-label">Password</label>
                        <input type="password" class="form-control" name="password"
                               placeholder="${isEdit ? 'Leave blank to keep' : ''}">
                    </div>
                </div>

                <div class="mt-3">
                    <label class="form-label">Token</label>
                    <input type="password" class="form-control" name="token"
                           placeholder="${isEdit ? 'Leave blank to keep' : ''}">
                </div>

                <div class="mt-3">
                    <label class="form-check">
                        <input class="form-check-input" type="checkbox" name="verify_tls" ${instance?.verify_tls !== false ? 'checked' : ''}>
                        <span class="form-check-label">Verify TLS certificates</span>
                    </label>
                </div>
            </div>
            <div class="card-footer text-end">
                <div class="d-flex">
                    <a href="#/tenants${instance?.tenant_id ? `/${instance.tenant_id}` : ''}" class="btn btn-link">Cancel</a>
                    <button type="submit" class="btn btn-primary ms-auto">
                        <i class="ti ti-check me-2"></i>
                        ${isEdit ? 'Update Instance' : 'Create Instance'}
                    </button>
                </div>
            </div>
        </form>
    `;
}

/**
 * Vytvoří ArgoCD app form
 */
function createArgocdAppForm(app = null, instances = [], environment = null) {
    const isEdit = !!app;
    return `
        <form id="argocd-app-form" class="card" data-argocd-app-mode="${isEdit ? 'edit' : 'new'}">
            <div class="card-header">
                <h3 class="card-title">${isEdit ? 'Edit ArgoCD App' : 'New ArgoCD App'}</h3>
            </div>
            <div class="card-body">
                ${environment ? `
                    <div class="mb-3">
                        <label class="form-label">Environment</label>
                        <div class="form-control-plaintext">
                            <span class="badge" style="${environment.color ? `background:${environment.color};color:#fff;` : ''}">${environment.name}</span>
                            <span class="text-secondary small ms-2">${environment.slug}</span>
                        </div>
                        <input type="hidden" name="environment_id" value="${environment.id}">
                    </div>
                ` : ''}

                <div class="mb-3">
                    <label class="form-label required">ArgoCD Instance</label>
                    <select class="form-select" name="argocd_instance_id" required>
                        <option value="">Select instance...</option>
                        ${instances.map(i => `
                            <option value="${i.id}" ${app?.argocd_instance_id === i.id ? 'selected' : ''}>
                                ${i.name} (${i.base_url})
                            </option>
                        `).join('')}
                    </select>
                </div>

                <div class="mb-3">
                    <label class="form-label required">Application Name</label>
                    <input type="text" class="form-control" name="application_name"
                           value="${app?.application_name || ''}"
                           placeholder="nac-prod" required>
                </div>

                <div class="mb-3">
                    <label class="form-check">
                        <input class="form-check-input" type="checkbox" name="is_active" ${app?.is_active !== false ? 'checked' : ''}>
                        <span class="form-check-label">Active</span>
                    </label>
                </div>
            </div>
            <div class="card-footer text-end">
                <div class="d-flex">
                    <a href="#/environments/${environment?.id || app?.environment_id}" class="btn btn-link">Cancel</a>
                    <button type="submit" class="btn btn-primary ms-auto">
                        <i class="ti ti-check me-2"></i>
                        ${isEdit ? 'Update App' : 'Create App'}
                    </button>
                </div>
            </div>
        </form>
    `;
}

/**
 * Vytvoří deploy target form
 */
function createDeployTargetForm(target = null, tenants = [], gitRepos = [], environments = [], encjsonKeys = [], envVars = [], extraEnvVars = [], options = {}) {
    const isEdit = options.isEdit ?? !!target;
    const title = options.title || (isEdit ? 'Edit Deploy Target' : 'New Deploy Target');
    const submitLabel = options.submitLabel || (isEdit ? 'Update Deploy Target' : 'Create Deploy Target');
    const copyLink = options.copyLink || '';
    const keys = encjsonKeys.length > 0 ? encjsonKeys : [{ public_key: '', has_private: false }];
    const vars = envVars.length > 0 ? envVars : [{ source_key: '', target_key: '' }];
    const extraVars = extraEnvVars.length > 0 ? extraEnvVars : [{ key: '', value: '' }];
    const selectedTenantId = target?.tenant_id || '';

    return `
        <form id="deploy-target-form" class="card">
            <div class="card-header">
                <h3 class="card-title">${title}</h3>
                ${copyLink ? `
                    <div class="card-actions">
                        <a href="${copyLink}" class="btn btn-sm btn-outline-primary">
                            <i class="ti ti-copy"></i>
                            Copy
                        </a>
                    </div>
                ` : ''}
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
                </div>

                <hr class="my-4">
                <h4>Environments</h4>
                ${environments.length === 0 ? `
                    <div class="alert alert-info">
                        No environments configured for this tenant yet.
                    </div>
                ` : `
                    <ul class="nav nav-tabs nav-fill">
                        ${environments.map((env, index) => `
                            <li class="nav-item">
                                <button class="nav-link ${index === 0 ? 'active' : ''}"
                                        data-bs-toggle="tab"
                                        data-bs-target="#deploy-env-pane-${env.id}"
                                        type="button">
                                    <span class="badge env-badge" style="${env.color ? `background:${env.color};color:#fff;` : ''}">${env.name}</span>
                                    <span class="text-secondary small ms-2">${env.slug}</span>
                                </button>
                            </li>
                        `).join('')}
                    </ul>
                    <div class="tab-content border border-top-0 rounded-bottom p-3">
                        ${environments.map((env, index) => {
                    const envConfig = (target?.envs || []).find(e => e.environment_id === env.id) || {};
                    const envRepoMode = envConfig.env_repo_branch ? 'branch' : 'path';
                    const deployRepoMode = envConfig.deploy_repo_branch ? 'branch' : 'path';
                    return `
                        <div class="tab-pane fade ${index === 0 ? 'show active' : ''}" id="deploy-env-pane-${env.id}" data-deploy-env data-env-id="${env.id}">
                            <div class="card">
                                <div class="card-body">
                                <div class="row g-3">
                                    <div class="col-md-6">
                                        <label class="form-label required">Environment Repository</label>
                                        <select class="form-select env-repo-id">
                                            <option value="">Select repository...</option>
                                            ${gitRepos.map(repo => `
                                                <option value="${repo.id}" ${envConfig.env_repo_id === repo.id ? 'selected' : ''}>
                                                    ${repo.name}
                                                </option>
                                            `).join('')}
                                        </select>
                                        <small class="form-hint">Manage repositories in Git Repositories</small>
                                    </div>
                                    <div class="col-md-6">
                                        <label class="form-label required">Deploy Repository</label>
                                        <select class="form-select deploy-repo-id">
                                            <option value="">Select repository...</option>
                                            ${gitRepos.map(repo => `
                                                <option value="${repo.id}" ${envConfig.deploy_repo_id === repo.id ? 'selected' : ''}>
                                                    ${repo.name}
                                                </option>
                                            `).join('')}
                                        </select>
                                        <small class="form-hint">Manage repositories in Git Repositories</small>
                                    </div>
                                </div>

                                <div class="row g-3 mt-1">
                                    <div class="col-md-6">
                                        <label class="form-label">Environment Repo Mode</label>
                                        <select class="form-select env-repo-mode">
                                            <option value="path" ${envRepoMode === 'path' ? 'selected' : ''}>Path</option>
                                            <option value="branch" ${envRepoMode === 'branch' ? 'selected' : ''}>Branch</option>
                                        </select>
                                    </div>
                                    <div class="col-md-6">
                                        <label class="form-label">Deploy Repo Mode</label>
                                        <select class="form-select deploy-repo-mode">
                                            <option value="path" ${deployRepoMode === 'path' ? 'selected' : ''}>Path</option>
                                            <option value="branch" ${deployRepoMode === 'branch' ? 'selected' : ''}>Branch</option>
                                        </select>
                                    </div>
                                </div>

                                <div class="row g-3 mt-1">
                                    <div class="col-md-6">
                                        <label class="form-label env-repo-path-label">Env Repo Path</label>
                                        <input type="text" class="form-control env-repo-path" value="${envConfig.env_repo_path || ''}" placeholder="${env.slug}">
                                        <input type="text" class="form-control env-repo-branch d-none mt-2" value="${envConfig.env_repo_branch || ''}" placeholder="branch name">
                                    </div>
                                    <div class="col-md-6">
                                        <label class="form-label deploy-repo-path-label">Deploy Repo Path</label>
                                        <input type="text" class="form-control deploy-repo-path" value="${envConfig.deploy_repo_path || ''}" placeholder="deploy/${env.slug}">
                                        <input type="text" class="form-control deploy-repo-branch d-none mt-2" value="${envConfig.deploy_repo_branch || ''}" placeholder="branch name">
                                        <small class="form-hint">Path is relative to repo root.</small>
                                    </div>
                                </div>

                                <div class="row g-3 mt-2">
                                    <div class="col-md-4">
                                        <label class="form-label">Encjson Key Dir</label>
                                        <input type="text" class="form-control encjson-key-dir" value="${envConfig.encjson_key_dir || ''}" placeholder="(optional)">
                                    </div>
                                    <div class="col-md-4">
                                        <label class="form-label">Release manifest mode</label>
                                        <select class="form-select release-manifest-mode">
                                            <option value="match_digest" ${(envConfig.release_manifest_mode || 'match_digest') === 'match_digest' ? 'selected' : ''}>
                                                Match entries (digest preferred)
                                            </option>
                                            <option value="match_tag" ${envConfig.release_manifest_mode === 'match_tag' ? 'selected' : ''}>
                                                Match entries (tag only)
                                            </option>
                                            <option value="strict_digest" ${envConfig.release_manifest_mode === 'strict_digest' ? 'selected' : ''}>
                                                Strict (digest required)
                                            </option>
                                            <option value="strict_tag" ${envConfig.release_manifest_mode === 'strict_tag' ? 'selected' : ''}>
                                                Strict (tag required)
                                            </option>
                                        </select>
                                    </div>
                                    <div class="col-md-4">
                                        <label class="form-label">Options</label>
                                        <div class="form-check">
                                            <input class="form-check-input allow-auto-release" type="checkbox" ${envConfig.allow_auto_release ? 'checked' : ''}>
                                            <label class="form-check-label">Allow Dev/Test deploy from Copy Job (auto release)</label>
                                        </div>
                                        <div class="form-check">
                                            <input class="form-check-input append-env-suffix" type="checkbox" ${envConfig.append_env_suffix ? 'checked' : ''}>
                                            <label class="form-check-label">Append env suffix to git tag (e.g. -test)</label>
                                        </div>
                                        <div class="form-check">
                                            <input class="form-check-input is-active" type="checkbox" ${envConfig.is_active !== false ? 'checked' : ''}>
                                            <label class="form-check-label">Active</label>
                                        </div>
                                    </div>
                                </div>
                                </div>
                            </div>
                        </div>
                    `;
                        }).join('')}
                    </div>
                `}

                <hr class="my-4">
                <h4>Encjson</h4>

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
                    <label class="form-label">Release env var mappings</label>
                    <div class="text-secondary small mb-2">
                        SRM exports <code>SIMPLE_RELEASE_ID</code>. Map it to your product variable (e.g. <code>TSM_RELEASE_ID</code>).
                        Examples: <code>SIMPLE_RELEASE_ID → TSM_RELEASE_ID</code>, <code>SIMPLE_RELEASE_ID → APP_RELEASE</code>.
                    </div>
                    <div id="deploy-env-vars-list">
                        ${vars.map((item, index) => `
                            <div class="row g-2 align-items-end mb-2" data-env-var-index="${index}">
                                <div class="col-md-5">
                                    <input type="text" class="form-control form-control-sm env-var-source"
                                           value="${item.source_key || ''}"
                                           placeholder="SIMPLE_RELEASE_ID">
                                </div>
                                <div class="col-md-5">
                                    <input type="text" class="form-control form-control-sm env-var-target"
                                           value="${item.target_key || ''}"
                                           placeholder="TSM_RELEASE_ID">
                                </div>
                                <div class="col-md-2">
                                    <button type="button" class="btn btn-sm btn-outline-danger w-100 env-var-remove" ${vars.length === 1 ? 'disabled' : ''}>
                                        <i class="ti ti-trash"></i>
                                    </button>
                                </div>
                            </div>
                        `).join('')}
                    </div>
                    <button type="button" class="btn btn-sm btn-outline-secondary" id="deploy-env-var-add-btn">
                        <i class="ti ti-plus"></i>
                        Add mapping
                    </button>
                </div>

                <div class="mb-3">
                    <label class="form-label">Extra env vars (override)</label>
                    <div class="text-secondary small mb-2">
                        These values are appended to the generated env file and override any existing keys.
                        Example: <code>REGISTRY_URL=https://example.com/dev</code>
                    </div>
                    <div id="deploy-extra-env-vars-list">
                        ${extraVars.map((item, index) => `
                            <div class="row g-2 align-items-end mb-2" data-extra-env-var-index="${index}">
                                <div class="col-md-5">
                                    <input type="text" class="form-control form-control-sm extra-env-var-key"
                                           value="${item.key || ''}"
                                           placeholder="REGISTRY_URL">
                                </div>
                                <div class="col-md-5">
                                    <input type="text" class="form-control form-control-sm extra-env-var-value"
                                           value="${item.value || ''}"
                                           placeholder="https://registry.example.com/project">
                                </div>
                                <div class="col-md-2">
                                    <button type="button" class="btn btn-sm btn-outline-danger w-100 extra-env-var-remove" ${extraVars.length === 1 ? 'disabled' : ''}>
                                        <i class="ti ti-trash"></i>
                                    </button>
                                </div>
                            </div>
                        `).join('')}
                    </div>
                    <button type="button" class="btn btn-sm btn-outline-secondary" id="deploy-extra-env-var-add-btn">
                        <i class="ti ti-plus"></i>
                        Add env var
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
                <div class="d-flex gap-2">
                    <a href="#/tenants${target?.tenant_id ? `/${target.tenant_id}` : ''}" class="btn btn-link">Cancel</a>
                    ${isEdit ? `
                        ${target?.is_archived ? `
                            <button type="button" class="btn btn-outline-success" id="unarchive-deploy-target-btn">
                                <i class="ti ti-archive"></i>
                                Unarchive
                            </button>
                        ` : `
                            <button type="button" class="btn btn-outline-danger" id="delete-deploy-target-btn">
                                <i class="ti ti-trash"></i>
                                ${target?.has_jobs ? 'Archive' : 'Delete'}
                            </button>
                        `}
                    ` : ''}
                    <button type="submit" class="btn btn-primary ms-auto">
                        <i class="ti ti-check me-2"></i>
                        ${submitLabel}
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
    if (data.allow_auto_release !== undefined) {
        data.allow_auto_release = formData.get('allow_auto_release') === 'on';
    }
    if (data.append_env_suffix !== undefined) {
        data.append_env_suffix = formData.get('append_env_suffix') === 'on';
    }
    const verifyTlsInput = form.querySelector('input[name="verify_tls"]');
    if (verifyTlsInput) {
        data.verify_tls = verifyTlsInput.checked === true;
    }

    // Clean up empty optional fields (convert empty strings to null or remove them)
    Object.keys(data).forEach(key => {
        if (typeof data[key] === 'string' && data[key].trim() === '') {
            // For optional fields like password, token, description - set to null
            if (['password', 'token', 'description', 'git_token', 'git_ssh_key', 'encjson_private_key', 'encjson_key_dir', 'source_password', 'source_token', 'target_password', 'target_token'].includes(key)) {
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
