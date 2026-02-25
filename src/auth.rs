use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::Engine;
use serde_json::Value;
use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;

const HEADER_USER: &str = "x-auth-user";
const HEADER_EMAIL: &str = "x-auth-email";
const HEADER_GROUPS: &str = "x-auth-groups";
const ROLE_PREFIX: &str = "simple:release:role:";
const TENANT_PREFIX: &str = "simple:release:tenant:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Admin,
    Developer,
    DeployManager,
    Viewer,
}

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub username: String,
    pub email: Option<String>,
    pub groups: Vec<String>,
    pub roles: Vec<Role>,
    pub tenant_slugs: Vec<String>,
    pub tenant_ids: Vec<Uuid>,
}

pub async fn auth_middleware(mut req: Request<Body>, next: Next) -> Response {
    let path = req.uri().path();
    if is_public_path(path) {
        return next.run(req).await;
    }

    let headers = req.headers();
    let username = match get_header(headers, HEADER_USER) {
        Some(value) => value,
        None => {
            warn!(method = %req.method(), path, "Missing X-Auth-User");
            return (StatusCode::UNAUTHORIZED, "Missing X-Auth-User").into_response();
        }
    };

    let groups_raw = match get_header(headers, HEADER_GROUPS) {
        Some(value) => value,
        None => {
            warn!(user = %username, method = %req.method(), path, "Missing X-Auth-Groups");
            return (StatusCode::FORBIDDEN, "Missing X-Auth-Groups").into_response();
        }
    };

    let groups = parse_groups(&groups_raw);
    let roles = roles_from_groups(&groups);
    if roles.is_empty() {
        warn!(user = %username, method = %req.method(), path, "No role assigned");
        return (StatusCode::FORBIDDEN, "No role assigned").into_response();
    }

    let tenant_slugs = tenant_slugs_from_groups(&groups);
    let is_admin = roles.contains(&Role::Admin);
    if !is_admin && tenant_slugs.is_empty() {
        warn!(user = %username, method = %req.method(), path, "No tenant scope assigned");
        return (StatusCode::FORBIDDEN, "No tenant scope assigned").into_response();
    }

    let tenant_ids = if is_admin {
        Vec::new()
    } else {
        match resolve_tenant_ids(req.extensions().get::<PgPool>(), &tenant_slugs).await {
            Ok(ids) if !ids.is_empty() => ids,
            Ok(_) => {
                warn!(user = %username, method = %req.method(), path, "Tenant scope does not match any tenant");
                return (StatusCode::FORBIDDEN, "Tenant scope invalid").into_response();
            }
            Err(err) => {
                warn!(user = %username, method = %req.method(), path, error = %err, "Tenant scope lookup failed");
                return (StatusCode::INTERNAL_SERVER_ERROR, "Tenant scope lookup failed").into_response();
            }
        }
    };

    if !is_admin {
        if let Some(pool) = req.extensions().get::<PgPool>() {
            if let Ok(Some(request_tenant_id)) = resolve_request_tenant(pool, path).await {
                if !tenant_ids.contains(&request_tenant_id) {
                    warn!(user = %username, method = %req.method(), path, "Tenant access denied");
                    return (StatusCode::FORBIDDEN, "Tenant access denied").into_response();
                }
            }
        }
    }

    let ctx = AuthContext {
        username,
        email: get_header(headers, HEADER_EMAIL),
        groups,
        roles: roles.clone(),
        tenant_slugs,
        tenant_ids,
    };

    if !is_authorized(req.method().as_str(), path, &roles) {
        warn!(
            user = %ctx.username,
            method = %req.method(),
            path,
            "Insufficient role"
        );
        return (StatusCode::FORBIDDEN, "Insufficient role").into_response();
    }

    req.extensions_mut().insert(ctx);
    next.run(req).await
}

pub async fn auth_disabled_middleware(mut req: Request<Body>, next: Next) -> Response {
    let ctx = AuthContext {
        username: "no-auth".to_string(),
        email: None,
        groups: vec!["simple:release:role:admin".to_string()],
        roles: vec![Role::Admin],
        tenant_slugs: Vec::new(),
        tenant_ids: Vec::new(),
    };
    req.extensions_mut().insert(ctx);
    next.run(req).await
}

fn get_header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn parse_groups(raw: &str) -> Vec<String> {
    if raw.contains(',') {
        return raw
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .collect();
    }

    if let Ok(decoded) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(raw.as_bytes()) {
        if let Ok(Value::Array(items)) = serde_json::from_slice::<Value>(&decoded) {
            let mut groups = Vec::new();
            for item in items {
                if let Some(value) = item.as_str() {
                    let value = value.trim();
                    if !value.is_empty() {
                        groups.push(value.to_string());
                    }
                }
            }
            if !groups.is_empty() {
                return groups;
            }
        }
    }

    if raw.trim().is_empty() {
        Vec::new()
    } else {
        vec![raw.trim().to_string()]
    }
}

fn roles_from_groups(groups: &[String]) -> Vec<Role> {
    let mut roles = Vec::new();
    for group in groups {
        let group = group.trim();
        if !group.starts_with(ROLE_PREFIX) {
            continue;
        }

        match &group[ROLE_PREFIX.len()..] {
            "admin" => roles.push(Role::Admin),
            "developer" => roles.push(Role::Developer),
            "deploy_manager" => roles.push(Role::DeployManager),
            "viewer" => roles.push(Role::Viewer),
            _ => {}
        }
    }
    roles
}

fn tenant_slugs_from_groups(groups: &[String]) -> Vec<String> {
    let mut slugs = Vec::new();
    for group in groups {
        let group = group.trim();
        if !group.starts_with(TENANT_PREFIX) {
            continue;
        }
        let slug = group[TENANT_PREFIX.len()..].trim();
        if !slug.is_empty() {
            slugs.push(slug.to_string());
        }
    }
    slugs
}

fn is_public_path(path: &str) -> bool {
    path == "/health"
        || path == "/healthz"
        || path.starts_with("/public/contract/")
}

fn is_authorized(method: &str, path: &str, roles: &[Role]) -> bool {
    if roles.contains(&Role::Admin) {
        return true;
    }

    let is_read = matches!(method, "GET" | "HEAD" | "OPTIONS");
    if is_read {
        return roles.contains(&Role::Viewer)
            || roles.contains(&Role::Developer)
            || roles.contains(&Role::DeployManager);
    }

    match required_write_role(path) {
        Some(Role::DeployManager) => roles.contains(&Role::DeployManager),
        Some(Role::Developer) => roles.contains(&Role::Developer),
        _ => false,
    }
}

fn required_write_role(path: &str) -> Option<Role> {
    if is_deploy_action(path) {
        Some(Role::DeployManager)
    } else if is_developer_write_path(path) {
        Some(Role::Developer)
    } else {
        None
    }
}

fn is_deploy_action(path: &str) -> bool {
    if path == "/api/v1/deploy/jobs" || path == "/api/v1/deploy/jobs/from-copy" {
        return true;
    }

    if path.starts_with("/api/v1/deploy/jobs/") && path.ends_with("/start") {
        return true;
    }

    if let Some(tail) = path.split("/api/v1/argocd-apps/").nth(1) {
        return tail.ends_with("/sync")
            || tail.ends_with("/cleanup-sync")
            || tail.ends_with("/refresh")
            || tail.ends_with("/terminate")
            || tail.ends_with("/target-revision")
            || tail.ends_with("/source-path");
    }

    false
}

fn is_developer_write_path(path: &str) -> bool {
    path.starts_with("/api/v1/tenants")
        || path.starts_with("/api/v1/registries")
        || path.starts_with("/api/v1/git-repos")
        || path.starts_with("/api/v1/argocd")
        || path.starts_with("/api/v1/argocd-apps")
        || path.starts_with("/api/v1/kubernetes")
        || path.starts_with("/api/v1/kubernetes-namespaces")
        || path.starts_with("/api/v1/bundles")
        || path.starts_with("/api/v1/releases")
        || path.starts_with("/api/v1/copy")
        || path.starts_with("/api/v1/environments")
}

async fn resolve_tenant_ids(pool: Option<&PgPool>, slugs: &[String]) -> Result<Vec<Uuid>, sqlx::Error> {
    let Some(pool) = pool else {
        return Ok(Vec::new());
    };
    let ids = sqlx::query_scalar::<_, Uuid>("SELECT id FROM tenants WHERE slug = ANY($1)")
        .bind(slugs)
        .fetch_all(pool)
        .await?;
    Ok(ids)
}

async fn resolve_request_tenant(pool: &PgPool, path: &str) -> Result<Option<Uuid>, sqlx::Error> {
    if let Some(tenant_id) = extract_uuid_after(path, "/api/v1/tenants/") {
        return Ok(Some(tenant_id));
    }

    if let Some(id) = extract_uuid_after(path, "/api/v1/registries/") {
        return tenant_id_for_table(pool, "registries", id).await;
    }

    if let Some(id) = extract_uuid_after(path, "/api/v1/git-repos/") {
        return tenant_id_for_table(pool, "git_repositories", id).await;
    }

    if let Some(id) = extract_uuid_after(path, "/api/v1/bundles/") {
        return tenant_id_for_table(pool, "bundles", id).await;
    }

    if let Some(id) = extract_uuid_after(path, "/api/v1/environments/") {
        return tenant_id_for_table(pool, "environments", id).await;
    }

    if let Some(id) = extract_uuid_after(path, "/api/v1/argocd/") {
        return tenant_id_for_table(pool, "argocd_instances", id).await;
    }

    if let Some(id) = extract_uuid_after(path, "/api/v1/kubernetes/") {
        return tenant_id_for_table(pool, "kubernetes_instances", id).await;
    }

    if let Some(id) = extract_uuid_after(path, "/api/v1/argocd-apps/") {
        let env_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT environment_id FROM environment_argocd_apps WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        return match env_id {
            Some(env_id) => tenant_id_for_table(pool, "environments", env_id).await,
            None => Ok(None),
        };
    }

    if let Some(id) = extract_uuid_after(path, "/api/v1/kubernetes-namespaces/") {
        let env_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT environment_id FROM environment_kubernetes_namespaces WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        return match env_id {
            Some(env_id) => tenant_id_for_table(pool, "environments", env_id).await,
            None => Ok(None),
        };
    }

    if let Some(id) = extract_uuid_after(path, "/api/v1/releases/") {
        let tenant_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT b.tenant_id\n             FROM releases r\n             JOIN copy_jobs cj ON r.copy_job_id = cj.id\n             JOIN bundle_versions bv ON cj.bundle_version_id = bv.id\n             JOIN bundles b ON bv.bundle_id = b.id\n             WHERE r.id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        return Ok(tenant_id);
    }

    if let Some(id) = extract_uuid_after(path, "/api/v1/copy/jobs/") {
        let tenant_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT b.tenant_id\n             FROM copy_jobs cj\n             JOIN bundle_versions bv ON cj.bundle_version_id = bv.id\n             JOIN bundles b ON bv.bundle_id = b.id\n             WHERE cj.id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        return Ok(tenant_id);
    }

    if let Some(id) = extract_uuid_after(path, "/api/v1/deploy/jobs/") {
        let tenant_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT e.tenant_id\n             FROM deploy_jobs dj\n             JOIN environments e ON dj.environment_id = e.id\n             WHERE dj.id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        return Ok(tenant_id);
    }

    Ok(None)
}

async fn tenant_id_for_table(
    pool: &PgPool,
    table: &str,
    id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    let query = format!("SELECT tenant_id FROM {} WHERE id = $1", table);
    sqlx::query_scalar::<_, Uuid>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await
}

fn extract_uuid_after(path: &str, prefix: &str) -> Option<Uuid> {
    if !path.starts_with(prefix) {
        return None;
    }
    let rest = &path[prefix.len()..];
    let segment = rest.split('/').next()?;
    Uuid::parse_str(segment).ok()
}

impl AuthContext {
    pub fn is_admin(&self) -> bool {
        self.roles.contains(&Role::Admin)
    }

    pub fn is_tenant_allowed(&self, tenant_id: Uuid) -> bool {
        self.is_admin() || self.tenant_ids.contains(&tenant_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_groups_csv() {
        let groups = parse_groups("a,b, c ,,d");
        assert_eq!(groups, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn parse_groups_jsonb64() {
        let json = serde_json::json!(["simple:release:role:admin", "foo"]);
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(json.to_string().as_bytes());
        let groups = parse_groups(&encoded);
        assert_eq!(
            groups,
            vec!["simple:release:role:admin".to_string(), "foo".to_string()]
        );
    }

    #[test]
    fn roles_from_groups_maps_prefix() {
        let groups = vec![
            "simple:release:role:admin".to_string(),
            "simple:release:role:developer".to_string(),
            "other:role:viewer".to_string(),
        ];
        let roles = roles_from_groups(&groups);
        assert!(roles.contains(&Role::Admin));
        assert!(roles.contains(&Role::Developer));
        assert!(!roles.contains(&Role::Viewer));
    }

    #[test]
    fn tenant_slugs_from_groups_maps_prefix() {
        let groups = vec![
            "simple:release:tenant:o2-cz".to_string(),
            "simple:release:tenant:cetin".to_string(),
            "other:tenant:foo".to_string(),
        ];
        let slugs = tenant_slugs_from_groups(&groups);
        assert_eq!(slugs, vec!["o2-cz", "cetin"]);
    }

    #[test]
    fn tenant_allowed_for_admin() {
        let ctx = AuthContext {
            username: "admin".to_string(),
            email: None,
            groups: vec![],
            roles: vec![Role::Admin],
            tenant_slugs: vec![],
            tenant_ids: vec![],
        };
        assert!(ctx.is_tenant_allowed(uuid::Uuid::nil()));
    }

    #[test]
    fn tenant_allowed_for_scoped_user() {
        let tenant_id = uuid::Uuid::new_v4();
        let ctx = AuthContext {
            username: "dev".to_string(),
            email: None,
            groups: vec![],
            roles: vec![Role::Developer],
            tenant_slugs: vec!["o2-cz".to_string()],
            tenant_ids: vec![tenant_id],
        };
        assert!(ctx.is_tenant_allowed(tenant_id));
        assert!(!ctx.is_tenant_allowed(uuid::Uuid::new_v4()));
    }

    #[test]
    fn deploy_action_detection() {
        assert!(is_deploy_action("/api/v1/deploy/jobs"));
        assert!(is_deploy_action("/api/v1/deploy/jobs/from-copy"));
        assert!(is_deploy_action("/api/v1/deploy/jobs/123/start"));
        assert!(is_deploy_action("/api/v1/argocd-apps/123/sync"));
        assert!(is_deploy_action("/api/v1/argocd-apps/123/cleanup-sync"));
        assert!(is_deploy_action("/api/v1/argocd-apps/123/refresh"));
        assert!(is_deploy_action("/api/v1/argocd-apps/123/terminate"));
        assert!(is_deploy_action("/api/v1/argocd-apps/123/target-revision"));
        assert!(is_deploy_action("/api/v1/argocd-apps/123/source-path"));
        assert!(!is_deploy_action("/api/v1/bundles"));
    }

    #[test]
    fn authorization_rules() {
        let viewer = vec![Role::Viewer];
        let developer = vec![Role::Developer];
        let deploy_manager = vec![Role::DeployManager];

        assert!(is_authorized("GET", "/api/v1/bundles", &viewer));
        assert!(!is_authorized("POST", "/api/v1/bundles", &viewer));

        assert!(is_authorized("POST", "/api/v1/bundles", &developer));
        assert!(!is_authorized("POST", "/api/v1/deploy/jobs", &developer));

        assert!(is_authorized("POST", "/api/v1/deploy/jobs", &deploy_manager));
        assert!(!is_authorized("POST", "/api/v1/registries", &deploy_manager));

        assert!(!is_authorized("POST", "/api/v1/unknown", &developer));
    }
}
