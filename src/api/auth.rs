use axum::{Extension, Json};
use serde::Serialize;

use crate::auth::{AuthContext, Role};

#[derive(Serialize)]
pub struct AuthMeResponse {
    pub username: String,
    pub email: Option<String>,
    pub roles: Vec<String>,
    pub tenant_slugs: Vec<String>,
    pub tenant_ids: Vec<String>,
    pub is_admin: bool,
}

pub async fn me(Extension(auth): Extension<AuthContext>) -> Json<AuthMeResponse> {
    let AuthContext {
        username,
        email,
        roles,
        tenant_slugs,
        tenant_ids,
        ..
    } = auth;
    let is_admin = roles.iter().any(|role| matches!(role, Role::Admin));
    Json(AuthMeResponse {
        username,
        email,
        roles: roles.iter().map(role_to_string).collect(),
        tenant_slugs,
        tenant_ids: tenant_ids.iter().map(|id| id.to_string()).collect(),
        is_admin,
    })
}

fn role_to_string(role: &Role) -> String {
    match role {
        Role::Admin => "admin",
        Role::Developer => "developer",
        Role::DeployManager => "deploy_manager",
        Role::Viewer => "viewer",
    }
    .to_string()
}
