/**
 * API Client pro komunikaci s backend REST API
 */

// Získat BASE_PATH z window nebo použít default
const BASE_PATH = window.BASE_PATH || '';
const API_BASE = `${BASE_PATH}/api/v1`;

class ApiClient {
    constructor() {
        this.baseUrl = API_BASE;
    }

    /**
     * Generický HTTP request s error handlingem
     */
    async request(endpoint, options = {}) {
        const url = `${this.baseUrl}${endpoint}`;
        const config = {
            headers: {
                'Content-Type': 'application/json',
                ...options.headers,
            },
            ...options,
        };

        try {
            const response = await fetch(url, config);

            // Handle non-JSON responses (např. prázdné 204)
            if (response.status === 204) {
                return null;
            }

            const data = await response.json();

            if (!response.ok) {
                throw new ApiError(
                    data.error || 'Request failed',
                    response.status,
                    data
                );
            }

            return data;
        } catch (error) {
            if (error instanceof ApiError) {
                throw error;
            }

            // Network error nebo jiný problém
            throw new ApiError(
                error.message || 'Network error',
                0,
                null
            );
        }
    }

    /**
     * GET request returning text (non-JSON)
     */
    async getText(endpoint) {
        const url = `${this.baseUrl}${endpoint}`;
        const response = await fetch(url);

        if (!response.ok) {
            let message = 'Request failed';
            try {
                const data = await response.json();
                message = data.error || message;
            } catch (_) {
                const text = await response.text();
                if (text) message = text;
            }
            throw new ApiError(message, response.status, null);
        }

        return response.text();
    }

    /**
     * GET request
     */
    async get(endpoint) {
        return this.request(endpoint, { method: 'GET' });
    }

    /**
     * POST request
     */
    async post(endpoint, data) {
        return this.request(endpoint, {
            method: 'POST',
            body: JSON.stringify(data),
        });
    }

    /**
     * PUT request
     */
    async put(endpoint, data) {
        return this.request(endpoint, {
            method: 'PUT',
            body: JSON.stringify(data),
        });
    }

    /**
     * DELETE request
     */
    async delete(endpoint) {
        return this.request(endpoint, { method: 'DELETE' });
    }

    // ==================== TENANTS ====================

    async getTenants() {
        return this.get('/tenants');
    }

    async getTenant(id) {
        return this.get(`/tenants/${id}`);
    }

    async createTenant(data) {
        return this.post('/tenants', data);
    }

    async updateTenant(id, data) {
        return this.put(`/tenants/${id}`, data);
    }

    async deleteTenant(id) {
        return this.delete(`/tenants/${id}`);
    }

    // ==================== REGISTRIES ====================

    async getRegistries(tenantId = null) {
        if (tenantId) {
            return this.get(`/tenants/${tenantId}/registries`);
        }
        // Pokud není tenant_id, můžeme mít endpoint pro všechny
        return this.get('/registries');
    }

    async getRegistry(id) {
        return this.get(`/registries/${id}`);
    }

    async createRegistry(tenantId, data) {
        return this.post(`/tenants/${tenantId}/registries`, data);
    }

    async updateRegistry(id, data) {
        return this.put(`/registries/${id}`, data);
    }

    async getRegistryEnvironmentPaths(id) {
        return this.get(`/registries/${id}/environment-paths`);
    }

    async getRegistryEnvironmentCredentials(id) {
        return this.get(`/registries/${id}/environment-credentials`);
    }

    async getRegistryEnvironmentAccess(id) {
        return this.get(`/registries/${id}/environment-access`);
    }

    async deleteRegistry(id) {
        return this.delete(`/registries/${id}`);
    }

    // ==================== GIT REPOSITORIES ====================

    async getGitRepos(tenantId = null) {
        if (tenantId) {
            return this.get(`/tenants/${tenantId}/git-repos`);
        }
        return this.get('/git-repos');
    }

    async getGitRepo(id) {
        return this.get(`/git-repos/${id}`);
    }

    async createGitRepo(tenantId, data) {
        return this.post(`/tenants/${tenantId}/git-repos`, data);
    }

    async updateGitRepo(id, data) {
        return this.put(`/git-repos/${id}`, data);
    }

    async deleteGitRepo(id) {
        return this.delete(`/git-repos/${id}`);
    }

    // ==================== BUNDLES ====================

    async getBundles(tenantId = null) {
        if (tenantId) {
            return this.get(`/tenants/${tenantId}/bundles`);
        }
        return this.get('/bundles');
    }

    async getBundle(id) {
        return this.get(`/bundles/${id}`);
    }

    async getVersion() {
        return this.get('/version');
    }

    async createBundle(tenantId, data) {
        return this.post(`/tenants/${tenantId}/bundles`, data);
    }

    async updateBundle(id, data) {
        return this.put(`/bundles/${id}`, data);
    }

    async deleteBundle(id) {
        return this.delete(`/bundles/${id}`);
    }

    // Bundle versions
    async getBundleVersions(bundleId) {
        return this.get(`/bundles/${bundleId}/versions`);
    }

    async getBundleVersion(bundleId, version) {
        return this.get(`/bundles/${bundleId}/versions/${version}`);
    }

    async createBundleVersion(bundleId, data = {}) {
        return this.post(`/bundles/${bundleId}/versions`, data);
    }

    async setBundleVersionArchived(bundleId, version, isArchived) {
        return this.put(`/bundles/${bundleId}/versions/${version}/archive`, { is_archived: isArchived });
    }

    async getBundleCopyJobs(bundleId) {
        return this.get(`/bundles/${bundleId}/copy-jobs`);
    }

    async getBundleReleases(bundleId) {
        return this.get(`/bundles/${bundleId}/releases`);
    }

    // Image mappings
    async getImageMappings(bundleId, version) {
        return this.get(`/bundles/${bundleId}/versions/${version}/images`);
    }

    async addImageMapping(bundleId, version, data) {
        return this.post(`/bundles/${bundleId}/versions/${version}/images`, data);
    }

    async updateImageMapping(bundleId, version, mappingId, data) {
        return this.put(`/bundles/${bundleId}/versions/${version}/images/${mappingId}`, data);
    }

    async deleteImageMapping(bundleId, version, mappingId) {
        return this.delete(`/bundles/${bundleId}/versions/${version}/images/${mappingId}`);
    }

    // ==================== COPY OPERATIONS ====================

    async startCopyJob(bundleId, version, targetTag, timezoneOffsetMinutes = null, environmentId = null, sourceRegistryId = null, targetRegistryId = null) {
        return this.post(`/bundles/${bundleId}/versions/${version}/copy`, {
            target_tag: targetTag,
            timezone_offset_minutes: timezoneOffsetMinutes,
            environment_id: environmentId,
            source_registry_id: sourceRegistryId,
            target_registry_id: targetRegistryId,
        });
    }

    async getNextCopyTag(bundleId, version, timezoneOffsetMinutes = null) {
        const params = timezoneOffsetMinutes === null ? '' : `?tz_offset_minutes=${encodeURIComponent(timezoneOffsetMinutes)}`;
        return this.get(`/bundles/${bundleId}/versions/${version}/next-tag${params}`);
    }

    async precheckCopyImages(bundleId, version, environmentId = null, sourceRegistryId = null) {
        return this.post(`/bundles/${bundleId}/versions/${version}/precheck`, {
            environment_id: environmentId,
            source_registry_id: sourceRegistryId,
        });
    }

    async getCopyJobStatus(jobId) {
        return this.get(`/copy/jobs/${jobId}`);
    }

    async getCopyJobImages(jobId) {
        return this.get(`/copy/jobs/${jobId}/images`);
    }

    async cancelCopyJob(jobId) {
        return this.post(`/copy/jobs/${jobId}/cancel`, {});
    }

    async getCopyJobLogHistory(jobId) {
        const url = `${this.baseUrl}/copy/jobs/${jobId}/logs/history`;
        const response = await fetch(url);

        if (response.status === 404) {
            return [];
        }

        if (!response.ok) {
            let message = 'Request failed';
            try {
                const data = await response.json();
                message = data.error || message;
            } catch (_) {
                const text = await response.text();
                if (text) message = text;
            }
            throw new ApiError(message, response.status, null);
        }

        const text = await response.text();
        if (!text) return [];
        return JSON.parse(text);
    }

    async getCopyJobs() {
        return this.get('/copy/jobs');
    }

    async getDeployments() {
        return this.get('/deploy/jobs');
    }

    async getBundleDeployments(bundleId) {
        return this.get(`/bundles/${bundleId}/deployments`);
    }

    async startReleaseCopyJob(payload) {
        return this.post('/copy/jobs/release', payload);
    }

    async startSelectiveCopyJob(payload) {
        return this.post('/copy/jobs/selective', payload);
    }

    async startPendingCopyJob(jobId) {
        return this.post(`/copy/jobs/${jobId}/start`, {});
    }

    /**
     * Vytvoří SSE stream pro sledování copy job progress
     */
    createCopyJobStream(jobId, onMessage, onError, onComplete) {
        const url = `${this.baseUrl}/copy/jobs/${jobId}/progress`;
        const eventSource = new EventSource(url);

        eventSource.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);

                if (data.error) {
                    if (onError) onError(data.error);
                    eventSource.close();
                    return;
                }

                if (onMessage) onMessage(data);

                // Zavřít stream když je job dokončený
                if (data.status === 'success' || data.status === 'failed') {
                    eventSource.close();
                    if (onComplete) onComplete(data);
                }
            } catch (error) {
                console.error('Failed to parse SSE message:', error);
                if (onError) onError(error.message);
            }
        };

        eventSource.onerror = (error) => {
            console.error('SSE error:', error);
            eventSource.close();
            if (onError) onError('Connection lost');
        };

        return eventSource;
    }

    /**
     * Jednoduchý SSE stream pro textové logy
     */
    createEventSource(path, onMessage, onError) {
        const url = `${this.baseUrl}${path}`;
        const eventSource = new EventSource(url);

        eventSource.onmessage = (event) => {
            if (onMessage) onMessage(event.data);
        };

        eventSource.onerror = (error) => {
            console.error('SSE error:', error);
            eventSource.close();
            if (onError) onError('Connection lost');
        };

        return eventSource;
    }

    // ==================== RELEASES ====================

    async getReleases(tenantId = null) {
        if (tenantId) {
            return this.get(`/tenants/${tenantId}/releases`);
        }
        return this.get('/releases');
    }

    async getRelease(id) {
        return this.get(`/releases/${id}`);
    }

    async createRelease(data) {
        return this.post('/releases', data);
    }

    async getReleaseManifest(id) {
        return this.getText(`/releases/${id}/manifest`);
    }

    // ==================== DEPLOY TARGETS ====================

    async getDeployTargets(tenantId) {
        return this.get(`/tenants/${tenantId}/deploy-targets`);
    }

    async getEnvironments(tenantId) {
        return this.get(`/tenants/${tenantId}/environments`);
    }

    async createEnvironment(tenantId, data) {
        return this.post(`/tenants/${tenantId}/environments`, data);
    }

    async updateEnvironment(id, data) {
        return this.put(`/environments/${id}`, data);
    }

    async deleteEnvironment(id) {
        return this.delete(`/environments/${id}`);
    }

    async getEnvironment(id) {
        return this.get(`/environments/${id}`);
    }

    async getReleaseDeployTargets(releaseId) {
        return this.get(`/releases/${releaseId}/deploy-targets`);
    }

    async createDeployTarget(tenantId, data) {
        return this.post(`/tenants/${tenantId}/deploy-targets`, data);
    }

    async startAutoDeployFromCopyJob(copyJobId, environmentId, dryRun = true) {
        return this.post(`/deploy/jobs/from-copy`, {
            copy_job_id: copyJobId,
            environment_id: environmentId,
            dry_run: dryRun,
        });
    }

    async updateDeployTarget(id, data) {
        return this.put(`/deploy-targets/${id}`, data);
    }

    async deleteDeployTarget(id) {
        return this.delete(`/deploy-targets/${id}`);
    }

    async archiveDeployTarget(id) {
        return this.post(`/deploy-targets/${id}/archive`, {});
    }

    async unarchiveDeployTarget(id) {
        return this.post(`/deploy-targets/${id}/unarchive`, {});
    }

    async getDeployTarget(id) {
        return this.get(`/deploy-targets/${id}`);
    }

    // ==================== DEPLOY JOBS ====================

    async createDeployJob(data) {
        return this.post(`/deploy/jobs`, data);
    }

    async startDeployJob(id) {
        return this.post(`/deploy/jobs/${id}/start`, {});
    }

    async getDeployJob(id) {
        return this.get(`/deploy/jobs/${id}`);
    }

    async getReleaseDeployJobs(releaseId) {
        return this.get(`/releases/${releaseId}/deploy-jobs`);
    }

    createDeployJobStream(jobId, onMessage, onError) {
        return this.createEventSource(`/deploy/jobs/${jobId}/logs`, onMessage, onError);
    }

    async getDeployJobLogHistory(jobId) {
        try {
            const response = await fetch(`${this.baseUrl}/deploy/jobs/${jobId}/logs/history`);
            if (!response.ok) return [];
            const text = await response.text();
            if (!text) return [];
            return JSON.parse(text);
        } catch (e) {
            return [];
        }
    }

    async getDeployJobDiff(jobId) {
        try {
            const response = await fetch(`${this.baseUrl}/deploy/jobs/${jobId}/diff`);
            if (!response.ok) return null;
            const text = await response.text();
            if (!text) return null;
            return JSON.parse(text);
        } catch (e) {
            return null;
        }
    }

    async getDeployJobImages(jobId) {
        try {
            const response = await fetch(`${this.baseUrl}/deploy/jobs/${jobId}/images`);
            if (!response.ok) return [];
            const text = await response.text();
            if (!text) return [];
            return JSON.parse(text);
        } catch (e) {
            return [];
        }
    }
}

/**
 * Custom error class pro API chyby
 */
class ApiError extends Error {
    constructor(message, status, data) {
        super(message);
        this.name = 'ApiError';
        this.status = status;
        this.data = data;
    }
}

// Export singleton instance
const api = new ApiClient();
