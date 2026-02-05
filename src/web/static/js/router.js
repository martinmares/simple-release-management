/**
 * Hash-based SPA Router
 */

class Router {
    constructor() {
        this.routes = {};
        this.currentRoute = '/';
        this.params = {};
        this.query = {};

        // Listen for hash changes
        window.addEventListener('hashchange', () => this.handleRoute());

        // Handle initial load
        window.addEventListener('DOMContentLoaded', () => this.handleRoute());
    }

    /**
     * Registruje route s handler funkcí
     */
    on(path, handler) {
        this.routes[path] = handler;
    }

    /**
     * Naviguje na danou cestu
     */
    navigate(path) {
        window.location.hash = path;
    }

    /**
     * Zpracuje aktuální route
     */
    async handleRoute() {
        const hash = window.location.hash.slice(1) || '/';

        // Parse query params
        const [path, queryString] = hash.split('?');
        this.query = this.parseQuery(queryString);

        // Find matching route
        const { route, params } = this.matchRoute(path);

        if (route) {
            this.currentRoute = path;
            this.params = params;
            const app = window.getApp ? window.getApp() : null;
            if (app) {
                app.currentRoute = path;
            }

            try {
                await this.routes[route](params, this.query);
            } catch (error) {
                console.error('Route handler error:', error);
                this.showError('Failed to load page');
            }
        } else {
            console.warn('No route found for:', path);
            this.navigate('/');
        }
    }

    /**
     * Najde odpovídající route s podporou parametrů
     */
    matchRoute(path) {
        // Exact match
        if (this.routes[path]) {
            return { route: path, params: {} };
        }

        // Pattern matching (např. /bundles/:id)
        for (const route in this.routes) {
            const pattern = this.routeToRegex(route);
            const match = path.match(pattern);

            if (match) {
                const params = this.extractParams(route, match);
                return { route, params };
            }
        }

        return { route: null, params: {} };
    }

    /**
     * Převede route pattern na regex
     */
    routeToRegex(route) {
        const pattern = route
            .replace(/\//g, '\\/')
            .replace(/:([^\/]+)/g, '([^/]+)');
        return new RegExp(`^${pattern}$`);
    }

    /**
     * Extrahuje parametry z route
     */
    extractParams(route, match) {
        const keys = [];
        const regex = /:([^\/]+)/g;
        let key;

        while ((key = regex.exec(route)) !== null) {
            keys.push(key[1]);
        }

        const params = {};
        keys.forEach((key, index) => {
            params[key] = match[index + 1];
        });

        return params;
    }

    /**
     * Parse query string
     */
    parseQuery(queryString) {
        if (!queryString) return {};

        const params = {};
        queryString.split('&').forEach(param => {
            const [key, value] = param.split('=');
            params[decodeURIComponent(key)] = decodeURIComponent(value || '');
        });

        return params;
    }

    /**
     * Zobrazí error stránku
     */
    showError(message) {
        const content = document.getElementById('app-content');
        content.innerHTML = `
            <div class="error-404">
                <div class="error-code">404</div>
                <div class="error-title">Page Not Found</div>
                <p class="text-secondary mb-4">${message}</p>
                <div>
                    <a href="#/" class="btn btn-primary">
                        <i class="ti ti-home"></i>
                        Go to Dashboard
                    </a>
                </div>
            </div>
        `;
    }
}

// Export singleton
const router = new Router();

/**
 * Helper pro vytvoření query string
 */
function buildQuery(params) {
    return Object.keys(params)
        .map(key => `${encodeURIComponent(key)}=${encodeURIComponent(params[key])}`)
        .join('&');
}
