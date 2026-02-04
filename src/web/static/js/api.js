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

    async deleteRegistry(id) {
        return this.delete(`/registries/${id}`);
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

    async startCopyJob(bundleId, version, targetTag) {
        return this.post(`/bundles/${bundleId}/versions/${version}/copy`, {
            target_tag: targetTag,
        });
    }

    async precheckCopyImages(bundleId, version) {
        return this.post(`/bundles/${bundleId}/versions/${version}/precheck`, {});
    }

    async getCopyJobStatus(jobId) {
        return this.get(`/copy/jobs/${jobId}`);
    }

    async getCopyJobImages(jobId) {
        return this.get(`/copy/jobs/${jobId}/images`);
    }

    async getCopyJobLogHistory(jobId) {
        return this.get(`/copy/jobs/${jobId}/logs/history`);
    }

    async getCopyJobs() {
        return this.get('/copy/jobs');
    }

    async startReleaseCopyJob(payload) {
        return this.post('/copy/jobs/release', payload);
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

    async getReleaseDeployTargets(releaseId) {
        return this.get(`/releases/${releaseId}/deploy-targets`);
    }

    async createDeployTarget(tenantId, data) {
        return this.post(`/tenants/${tenantId}/deploy-targets`, data);
    }

    async updateDeployTarget(id, data) {
        return this.put(`/deploy-targets/${id}`, data);
    }

    async deleteDeployTarget(id) {
        return this.delete(`/deploy-targets/${id}`);
    }

    async getDeployTarget(id) {
        return this.get(`/deploy-targets/${id}`);
    }

    // ==================== DEPLOY JOBS ====================

    async createDeployJob(data) {
        return this.post(`/deploy/jobs`, data);
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
