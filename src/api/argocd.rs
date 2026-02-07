use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post, put, delete},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::convert::Infallible;
use std::time::Duration;
use uuid::Uuid;

use crate::crypto;
use crate::db::models::{ArgocdInstance, EnvironmentArgocdApp};

#[derive(Clone)]
pub struct ArgocdApiState {
    pub pool: PgPool,
    pub encryption_secret: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct ArgocdInstanceRequest {
    pub name: String,
    pub base_url: String,
    pub auth_type: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub token: Option<String>,
    pub verify_tls: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ArgocdAppRequest {
    pub argocd_instance_id: Uuid,
    pub application_name: String,
    pub is_active: Option<bool>,
    pub ignore_resources: Option<Vec<String>>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ArgocdAppSummary {
    pub id: Uuid,
    pub environment_id: Uuid,
    pub argocd_instance_id: Uuid,
    pub application_name: String,
    pub is_active: bool,
    pub last_sync_status: Option<String>,
    pub last_health_status: Option<String>,
    pub last_operation_phase: Option<String>,
    pub last_operation_message: Option<String>,
    pub last_revision: Option<String>,
    pub last_checked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub instance_name: String,
    pub instance_base_url: String,
}

#[derive(Debug, Serialize)]
pub struct ArgocdStatus {
    pub sync_status: Option<String>,
    pub health_status: Option<String>,
    pub operation_phase: Option<String>,
    pub operation_message: Option<String>,
    pub revision: Option<String>,
    pub operation_resources: Option<Vec<ArgocdOperationResource>>,
    pub conditions: Option<Vec<ArgocdCondition>>,
    pub resource_issues: Option<Vec<ArgocdResourceIssue>>,
}

#[derive(Debug, Serialize)]
pub struct ArgocdCondition {
    #[serde(rename = "type")]
    pub type_name: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ArgocdResourceIssue {
    pub kind: Option<String>,
    pub name: Option<String>,
    pub namespace: Option<String>,
    pub sync_status: Option<String>,
    pub health_status: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug)]
struct IgnoreMatcher {
    group: Option<String>,
    kind: String,
    namespace: Option<String>,
    name: String,
}

#[derive(Debug, Serialize)]
struct ArgocdSyncResource {
    group: String,
    kind: String,
    name: String,
    namespace: String,
}

#[derive(Debug, Serialize)]
pub struct ArgocdOperationResource {
    pub kind: Option<String>,
    pub name: Option<String>,
    pub namespace: Option<String>,
    pub status: Option<String>,
    pub message: Option<String>,
    pub hook_type: Option<String>,
    pub sync_phase: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ArgocdResource {
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
    pub group: Option<String>,
    pub version: Option<String>,
    pub health: Option<String>,
    pub sync: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ArgocdEvent {
    pub kind: Option<String>,
    pub name: Option<String>,
    pub namespace: Option<String>,
    pub reason: Option<String>,
    pub message: Option<String>,
    pub event_type: Option<String>,
    pub first_timestamp: Option<String>,
    pub last_timestamp: Option<String>,
    pub uid: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StreamQuery {
    pub interval: Option<i64>,
}

pub fn router(state: ArgocdApiState) -> Router {
    Router::new()
        .route("/tenants/{tenant_id}/argocd", get(list_instances).post(create_instance))
        .route("/argocd/{id}", get(get_instance).put(update_instance).delete(delete_instance))
        .route("/environments/{env_id}/argocd-apps", get(list_env_apps).post(create_env_app))
        .route("/argocd-apps/{id}", get(get_env_app).put(update_env_app).delete(delete_env_app))
        .route("/argocd-apps/{id}/status", get(get_app_status))
        .route("/argocd-apps/{id}/refresh", post(refresh_app))
        .route("/argocd-apps/{id}/sync", post(sync_app))
        .route("/argocd-apps/{id}/terminate", post(terminate_app))
        .route("/argocd-apps/{id}/stream", get(stream_app_status))
        .route("/argocd-apps/{id}/resources", get(get_app_resources))
        .route("/argocd-apps/{id}/events", get(get_app_events))
        .route("/argocd-apps/{id}/events/stream", get(stream_app_events))
        .with_state(state)
}

async fn list_instances(
    State(state): State<ArgocdApiState>,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<Vec<ArgocdInstance>>, (StatusCode, Json<ErrorResponse>)> {
    let instances = sqlx::query_as::<_, ArgocdInstance>(
        "SELECT * FROM argocd_instances WHERE tenant_id = $1 ORDER BY name",
    )
    .bind(tenant_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    Ok(Json(instances))
}

async fn get_instance(
    State(state): State<ArgocdApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ArgocdInstance>, (StatusCode, Json<ErrorResponse>)> {
    let instance = sqlx::query_as::<_, ArgocdInstance>(
        "SELECT * FROM argocd_instances WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    match instance {
        Some(instance) => Ok(Json(instance)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "ArgoCD instance not found".to_string(),
            }),
        )),
    }
}

async fn create_instance(
    State(state): State<ArgocdApiState>,
    Path(tenant_id): Path<Uuid>,
    Json(payload): Json<ArgocdInstanceRequest>,
) -> Result<(StatusCode, Json<ArgocdInstance>), (StatusCode, Json<ErrorResponse>)> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Name cannot be empty".to_string(),
            }),
        ));
    }

    let auth_type = payload.auth_type.trim().to_lowercase();
    let auth_type = match auth_type.as_str() {
        "token" => "token".to_string(),
        _ => "basic".to_string(),
    };

    let password_encrypted = payload.password.as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| crypto::encrypt(v, &state.encryption_secret))
        .transpose()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to encrypt password: {}", e),
                }),
            )
        })?;
    let token_encrypted = payload.token.as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| crypto::encrypt(v, &state.encryption_secret))
        .transpose()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to encrypt token: {}", e),
                }),
            )
        })?;

    let instance = sqlx::query_as::<_, ArgocdInstance>(
        r#"
        INSERT INTO argocd_instances
        (id, tenant_id, name, base_url, auth_type, username, password_encrypted, token_encrypted, verify_tls)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING *
        "#
    )
    .bind(Uuid::new_v4())
    .bind(tenant_id)
    .bind(name)
    .bind(payload.base_url.trim())
    .bind(auth_type)
    .bind(payload.username.as_deref().map(str::trim).filter(|v| !v.is_empty()))
    .bind(password_encrypted)
    .bind(token_encrypted)
    .bind(payload.verify_tls.unwrap_or(true))
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    Ok((StatusCode::CREATED, Json(instance)))
}

async fn update_instance(
    State(state): State<ArgocdApiState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ArgocdInstanceRequest>,
) -> Result<Json<ArgocdInstance>, (StatusCode, Json<ErrorResponse>)> {
    let current = sqlx::query_as::<_, ArgocdInstance>(
        "SELECT * FROM argocd_instances WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    let Some(current) = current else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "ArgoCD instance not found".to_string(),
            }),
        ));
    };

    let auth_type = payload.auth_type.trim().to_lowercase();
    let auth_type = match auth_type.as_str() {
        "token" => "token".to_string(),
        _ => "basic".to_string(),
    };

    let password_encrypted = match payload.password.as_deref().map(str::trim) {
        Some(v) if !v.is_empty() => Some(crypto::encrypt(v, &state.encryption_secret).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to encrypt password: {}", e),
                }),
            )
        })?),
        _ => current.password_encrypted.clone(),
    };
    let token_encrypted = match payload.token.as_deref().map(str::trim) {
        Some(v) if !v.is_empty() => Some(crypto::encrypt(v, &state.encryption_secret).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to encrypt token: {}", e),
                }),
            )
        })?),
        _ => current.token_encrypted.clone(),
    };

    let instance = sqlx::query_as::<_, ArgocdInstance>(
        r#"
        UPDATE argocd_instances
        SET name = $1,
            base_url = $2,
            auth_type = $3,
            username = $4,
            password_encrypted = $5,
            token_encrypted = $6,
            verify_tls = $7
        WHERE id = $8
        RETURNING *
        "#
    )
    .bind(payload.name.trim())
    .bind(payload.base_url.trim())
    .bind(auth_type)
    .bind(payload.username.as_deref().map(str::trim).filter(|v| !v.is_empty()))
    .bind(password_encrypted)
    .bind(token_encrypted)
    .bind(payload.verify_tls.unwrap_or(true))
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    Ok(Json(instance))
}

async fn delete_instance(
    State(state): State<ArgocdApiState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let in_use = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM environment_argocd_apps WHERE argocd_instance_id = $1)",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    if in_use {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "ArgoCD instance has apps and cannot be deleted".to_string(),
            }),
        ));
    }

    let result = sqlx::query("DELETE FROM argocd_instances WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Database error: {}", e),
                }),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "ArgoCD instance not found".to_string(),
            }),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn list_env_apps(
    State(state): State<ArgocdApiState>,
    Path(env_id): Path<Uuid>,
) -> Result<Json<Vec<ArgocdAppSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let apps = sqlx::query_as::<_, ArgocdAppSummary>(
        r#"
        SELECT
            a.id,
            a.environment_id,
            a.argocd_instance_id,
            a.application_name,
            a.is_active,
            a.last_sync_status,
            a.last_health_status,
            a.last_operation_phase,
            a.last_operation_message,
            a.last_revision,
            a.last_checked_at,
            i.name AS instance_name,
            i.base_url AS instance_base_url
        FROM environment_argocd_apps a
        JOIN argocd_instances i ON i.id = a.argocd_instance_id
        WHERE a.environment_id = $1
        ORDER BY a.application_name
        "#
    )
    .bind(env_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    Ok(Json(apps))
}

async fn create_env_app(
    State(state): State<ArgocdApiState>,
    Path(env_id): Path<Uuid>,
    Json(payload): Json<ArgocdAppRequest>,
) -> Result<(StatusCode, Json<EnvironmentArgocdApp>), (StatusCode, Json<ErrorResponse>)> {
    let app = sqlx::query_as::<_, EnvironmentArgocdApp>(
        r#"
        INSERT INTO environment_argocd_apps
        (id, environment_id, argocd_instance_id, application_name, is_active, ignore_resources)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *
        "#
    )
    .bind(Uuid::new_v4())
    .bind(env_id)
    .bind(payload.argocd_instance_id)
    .bind(payload.application_name.trim())
    .bind(payload.is_active.unwrap_or(true))
    .bind(serde_json::to_value(payload.ignore_resources.unwrap_or_default()).unwrap_or(serde_json::Value::Array(vec![])))
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    Ok((StatusCode::CREATED, Json(app)))
}

async fn get_env_app(
    State(state): State<ArgocdApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<EnvironmentArgocdApp>, (StatusCode, Json<ErrorResponse>)> {
    let app = sqlx::query_as::<_, EnvironmentArgocdApp>(
        "SELECT * FROM environment_argocd_apps WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    match app {
        Some(app) => Ok(Json(app)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "ArgoCD app not found".to_string(),
            }),
        )),
    }
}

async fn update_env_app(
    State(state): State<ArgocdApiState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ArgocdAppRequest>,
) -> Result<Json<EnvironmentArgocdApp>, (StatusCode, Json<ErrorResponse>)> {
    let app = sqlx::query_as::<_, EnvironmentArgocdApp>(
        r#"
        UPDATE environment_argocd_apps
        SET argocd_instance_id = $1,
            application_name = $2,
            is_active = $3,
            ignore_resources = $4
        WHERE id = $5
        RETURNING *
        "#
    )
    .bind(payload.argocd_instance_id)
    .bind(payload.application_name.trim())
    .bind(payload.is_active.unwrap_or(true))
    .bind(serde_json::to_value(payload.ignore_resources.unwrap_or_default()).unwrap_or(serde_json::Value::Array(vec![])))
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    Ok(Json(app))
}

async fn delete_env_app(
    State(state): State<ArgocdApiState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let result = sqlx::query("DELETE FROM environment_argocd_apps WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Database error: {}", e),
                }),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "ArgoCD app not found".to_string(),
            }),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn get_app_status(
    State(state): State<ArgocdApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ArgocdStatus>, (StatusCode, Json<ErrorResponse>)> {
    let app = sqlx::query_as::<_, EnvironmentArgocdApp>(
        "SELECT * FROM environment_argocd_apps WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;
    let Some(app) = app else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "ArgoCD app not found".to_string(),
            }),
        ));
    };

    let instance = sqlx::query_as::<_, ArgocdInstance>(
        "SELECT * FROM argocd_instances WHERE id = $1",
    )
    .bind(app.argocd_instance_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    let status = fetch_argocd_status(&state, &instance, &app).await?;
    cache_status(&state.pool, id, &status).await?;
    Ok(Json(status))
}

async fn refresh_app(
    State(state): State<ArgocdApiState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let (instance, app) = load_instance_and_app(&state.pool, id).await?;
    call_argocd_action(&state, &instance, &app, "refresh").await?;
    Ok(StatusCode::ACCEPTED)
}

async fn sync_app(
    State(state): State<ArgocdApiState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let (instance, app) = load_instance_and_app(&state.pool, id).await?;
    let ignore_list = extract_ignore_list(&app);
    if ignore_list.is_empty() {
        call_argocd_action(&state, &instance, &app, "sync").await?;
        return Ok(StatusCode::ACCEPTED);
    }
    let matchers = parse_ignore_matchers(&ignore_list);
    let resources = fetch_argocd_resources(&state, &instance, &app).await?;
    let filtered: Vec<ArgocdSyncResource> = resources
        .into_iter()
        .filter(|r| !matches_ignore(&matchers, r))
        .map(|r| ArgocdSyncResource {
            group: r.group.unwrap_or_default(),
            kind: r.kind,
            name: r.name,
            namespace: r.namespace.unwrap_or_default(),
        })
        .collect();
    call_argocd_sync(&state, &instance, &app, filtered).await?;
    Ok(StatusCode::ACCEPTED)
}

async fn terminate_app(
    State(state): State<ArgocdApiState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let (instance, app) = load_instance_and_app(&state.pool, id).await?;
    call_argocd_action(&state, &instance, &app, "terminate").await?;
    Ok(StatusCode::ACCEPTED)
}

async fn stream_app_status(
    State(state): State<ArgocdApiState>,
    Path(id): Path<Uuid>,
    Query(query): Query<StreamQuery>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, (StatusCode, Json<ErrorResponse>)> {
    let env_poll = sqlx::query_scalar::<_, i32>(
        "SELECT e.argocd_poll_interval_seconds
         FROM environment_argocd_apps a
         JOIN environments e ON e.id = a.environment_id
         WHERE a.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?
    .unwrap_or(0);

    let interval = query.interval.unwrap_or(env_poll as i64).max(1);

    let stream = async_stream::stream! {
        loop {
            match get_app_status(State(state.clone()), Path(id)).await {
                Ok(Json(status)) => {
                    let payload = serde_json::to_string(&status).unwrap_or_else(|_| "{}".to_string());
                    yield Ok(Event::default().data(payload));
                }
                Err(_) => {
                    yield Ok(Event::default().data("{}"));
                }
            }
            tokio::time::sleep(Duration::from_secs(interval as u64)).await;
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn get_app_resources(
    State(state): State<ArgocdApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ArgocdResource>>, (StatusCode, Json<ErrorResponse>)> {
    let (instance, app) = load_instance_and_app(&state.pool, id).await?;
    let resources = fetch_argocd_resources(&state, &instance, &app).await?;
    Ok(Json(resources))
}

async fn get_app_events(
    State(state): State<ArgocdApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ArgocdEvent>>, (StatusCode, Json<ErrorResponse>)> {
    let (instance, app) = load_instance_and_app(&state.pool, id).await?;
    let events = fetch_argocd_events(&state, &instance, &app).await?;
    Ok(Json(events))
}

async fn stream_app_events(
    State(state): State<ArgocdApiState>,
    Path(id): Path<Uuid>,
    Query(query): Query<StreamQuery>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, (StatusCode, Json<ErrorResponse>)> {
    let env_poll = sqlx::query_scalar::<_, i32>(
        "SELECT e.argocd_poll_interval_seconds
         FROM environment_argocd_apps a
         JOIN environments e ON e.id = a.environment_id
         WHERE a.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?
    .unwrap_or(0);
    let interval = query.interval.unwrap_or(env_poll as i64).max(5);

    let stream = async_stream::stream! {
        let mut seen: std::collections::VecDeque<String> = std::collections::VecDeque::new();
        loop {
            match get_app_events(State(state.clone()), Path(id)).await {
                Ok(Json(events)) => {
                    for ev in events {
                        let key = ev.uid.clone().unwrap_or_else(|| format!("{}:{}:{}", ev.namespace.clone().unwrap_or_default(), ev.kind.clone().unwrap_or_default(), ev.name.clone().unwrap_or_default()));
                        if seen.iter().any(|v| v == &key) {
                            continue;
                        }
                        seen.push_back(key);
                        if seen.len() > 200 {
                            seen.pop_front();
                        }
                        let payload = serde_json::to_string(&ev).unwrap_or_else(|_| "{}".to_string());
                        yield Ok(Event::default().data(payload));
                    }
                }
                Err(_) => {}
            }
            tokio::time::sleep(Duration::from_secs(interval as u64)).await;
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn load_instance_and_app(
    pool: &PgPool,
    app_id: Uuid,
) -> Result<(ArgocdInstance, EnvironmentArgocdApp), (StatusCode, Json<ErrorResponse>)> {
    let app = sqlx::query_as::<_, EnvironmentArgocdApp>(
        "SELECT * FROM environment_argocd_apps WHERE id = $1",
    )
    .bind(app_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;
    let Some(app) = app else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "ArgoCD app not found".to_string(),
            }),
        ));
    };
    let instance = sqlx::query_as::<_, ArgocdInstance>(
        "SELECT * FROM argocd_instances WHERE id = $1",
    )
    .bind(app.argocd_instance_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    Ok((instance, app))
}

async fn call_argocd_action(
    state: &ArgocdApiState,
    instance: &ArgocdInstance,
    app: &EnvironmentArgocdApp,
    action: &str,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let client = build_client(instance.verify_tls).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create HTTP client: {}", e),
            }),
        )
    })?;
    let url = match action {
        "refresh" => format!("{}/api/v1/applications/{}/refresh", instance.base_url, app.application_name),
        "sync" => format!("{}/api/v1/applications/{}/sync", instance.base_url, app.application_name),
        "terminate" => format!("{}/api/v1/applications/{}/operation", instance.base_url, app.application_name),
        _ => return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "Invalid action".to_string() }))),
    };

    let mut req = match action {
        "terminate" => client.delete(url).json(&serde_json::json!({})),
        _ => client.post(url).json(&serde_json::json!({})),
    };
    req = apply_auth(state, &client, req, instance).await?;
    let resp = req.send().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("ArgoCD request failed: {}", e),
            }),
        )
    })?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("ArgoCD {} failed: {} {}", action, status, body),
            }),
        ));
    }
    Ok(())
}

async fn call_argocd_sync(
    state: &ArgocdApiState,
    instance: &ArgocdInstance,
    app: &EnvironmentArgocdApp,
    resources: Vec<ArgocdSyncResource>,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let client = build_client(instance.verify_tls).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create HTTP client: {}", e),
            }),
        )
    })?;
    let url = format!("{}/api/v1/applications/{}/sync", instance.base_url, app.application_name);
    let mut req = client.post(url).json(&serde_json::json!({
        "resources": resources
    }));
    req = apply_auth(state, &client, req, instance).await?;
    let resp = req.send().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("ArgoCD request failed: {}", e),
            }),
        )
    })?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("ArgoCD sync failed: {} {}", status, body),
            }),
        ));
    }
    Ok(())
}

async fn fetch_argocd_status(
    state: &ArgocdApiState,
    instance: &ArgocdInstance,
    app: &EnvironmentArgocdApp,
) -> Result<ArgocdStatus, (StatusCode, Json<ErrorResponse>)> {
    let client = build_client(instance.verify_tls).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create HTTP client: {}", e),
            }),
        )
    })?;

    let url = format!("{}/api/v1/applications/{}", instance.base_url, app.application_name);
    let mut req = client.get(url);
    req = apply_auth(state, &client, req, instance).await?;
    let resp = req.send().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("ArgoCD request failed: {}", e),
            }),
        )
    })?;
    let value: serde_json::Value = resp.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("ArgoCD response decode failed: {}", e),
            }),
        )
    })?;

    let sync_status = value.pointer("/status/sync/status").and_then(|v| v.as_str()).map(str::to_string);
    let health_status = value.pointer("/status/health/status").and_then(|v| v.as_str()).map(str::to_string);
    let operation_phase = value.pointer("/status/operationState/phase").and_then(|v| v.as_str()).map(str::to_string);
    let operation_message = value.pointer("/status/operationState/message").and_then(|v| v.as_str()).map(str::to_string);
    let revision = value.pointer("/status/operationState/syncResult/revision")
        .or_else(|| value.pointer("/status/sync/revision"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let operation_resources = value.pointer("/status/operationState/syncResult/resources")
        .and_then(|v| v.as_array())
        .map(|items| {
            items.iter().map(|item| ArgocdOperationResource {
                kind: item.get("kind").and_then(|v| v.as_str()).map(str::to_string),
                name: item.get("name").and_then(|v| v.as_str()).map(str::to_string),
                namespace: item.get("namespace").and_then(|v| v.as_str()).map(str::to_string),
                status: item.get("status").and_then(|v| v.as_str()).map(str::to_string),
                message: item.get("message").and_then(|v| v.as_str()).map(str::to_string),
                hook_type: item.get("hookType").and_then(|v| v.as_str()).map(str::to_string),
                sync_phase: item.get("syncPhase").and_then(|v| v.as_str()).map(str::to_string),
            }).collect::<Vec<_>>()
        });

    let conditions = value.pointer("/status/conditions")
        .and_then(|v| v.as_array())
        .map(|items| {
            items.iter().map(|item| ArgocdCondition {
                type_name: item.get("type").and_then(|v| v.as_str()).map(str::to_string),
                message: item.get("message").and_then(|v| v.as_str()).map(str::to_string),
            }).collect::<Vec<_>>()
        });

    let resource_issues = value.pointer("/status/resources")
        .and_then(|v| v.as_array())
        .map(|items| {
            items.iter().filter_map(|item| {
                let kind = item.get("kind").and_then(|v| v.as_str()).map(str::to_string);
                let name = item.get("name").and_then(|v| v.as_str()).map(str::to_string);
                let namespace = item.get("namespace").and_then(|v| v.as_str()).map(str::to_string);
                let sync_status = item.pointer("/sync/status").and_then(|v| v.as_str()).map(str::to_string);
                let health_status = item.pointer("/health/status").and_then(|v| v.as_str()).map(str::to_string);
                let message = item.pointer("/health/message").and_then(|v| v.as_str()).map(str::to_string);
                let is_issue = match (sync_status.as_deref(), health_status.as_deref()) {
                    (Some("Synced"), Some("Healthy")) => false,
                    (None, Some("Healthy")) => false,
                    (Some("Synced"), None) => false,
                    _ => true,
                };
                if !is_issue {
                    return None;
                }
                Some(ArgocdResourceIssue {
                    kind,
                    name,
                    namespace,
                    sync_status,
                    health_status,
                    message,
                })
            }).collect::<Vec<_>>()
        });

    Ok(ArgocdStatus {
        sync_status,
        health_status,
        operation_phase,
        operation_message,
        revision,
        operation_resources,
        conditions,
        resource_issues,
    })
}

async fn fetch_argocd_resources(
    state: &ArgocdApiState,
    instance: &ArgocdInstance,
    app: &EnvironmentArgocdApp,
) -> Result<Vec<ArgocdResource>, (StatusCode, Json<ErrorResponse>)> {
    let client = build_client(instance.verify_tls).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create HTTP client: {}", e),
            }),
        )
    })?;
    let url = format!("{}/api/v1/applications/{}/resource-tree", instance.base_url, app.application_name);
    let mut req = client.get(url);
    req = apply_auth(state, &client, req, instance).await?;
    let resp = req.send().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("ArgoCD request failed: {}", e),
            }),
        )
    })?;
    let value: serde_json::Value = resp.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("ArgoCD response decode failed: {}", e),
            }),
        )
    })?;
    let nodes = value.get("nodes").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut resources = Vec::new();
    for node in nodes {
        let kind = node.get("kind").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if kind.is_empty() || name.is_empty() {
            continue;
        }
        let namespace = node.get("namespace").and_then(|v| v.as_str()).map(str::to_string);
        let group = node.get("group").and_then(|v| v.as_str()).map(str::to_string);
        let version = node.get("version").and_then(|v| v.as_str()).map(str::to_string);
        let health = node.pointer("/health/status").and_then(|v| v.as_str()).map(str::to_string);
        let sync = node.pointer("/sync/status").and_then(|v| v.as_str()).map(str::to_string);
        resources.push(ArgocdResource {
            kind,
            name,
            namespace,
            group,
            version,
            health,
            sync,
        });
    }
    Ok(resources)
}

async fn fetch_argocd_events(
    state: &ArgocdApiState,
    instance: &ArgocdInstance,
    app: &EnvironmentArgocdApp,
) -> Result<Vec<ArgocdEvent>, (StatusCode, Json<ErrorResponse>)> {
    let client = build_client(instance.verify_tls).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create HTTP client: {}", e),
            }),
        )
    })?;
    let url = format!("{}/api/v1/applications/{}/events", instance.base_url, app.application_name);
    let mut req = client.get(url);
    req = apply_auth(state, &client, req, instance).await?;
    let resp = req.send().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("ArgoCD request failed: {}", e),
            }),
        )
    })?;
    let value: serde_json::Value = resp.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("ArgoCD response decode failed: {}", e),
            }),
        )
    })?;
    let items = value.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut events = Vec::new();
    for ev in items {
        let uid = ev.pointer("/metadata/uid").and_then(|v| v.as_str()).map(str::to_string);
        let kind = ev.pointer("/involvedObject/kind").and_then(|v| v.as_str()).map(str::to_string);
        let name = ev.pointer("/involvedObject/name").and_then(|v| v.as_str()).map(str::to_string);
        let namespace = ev.pointer("/involvedObject/namespace").and_then(|v| v.as_str()).map(str::to_string);
        let reason = ev.get("reason").and_then(|v| v.as_str()).map(str::to_string);
        let message = ev.get("message").and_then(|v| v.as_str()).map(str::to_string);
        let event_type = ev.get("type").and_then(|v| v.as_str()).map(str::to_string);
        let first_timestamp = ev.get("firstTimestamp").and_then(|v| v.as_str()).map(str::to_string);
        let last_timestamp = ev.get("lastTimestamp").and_then(|v| v.as_str()).map(str::to_string);
        events.push(ArgocdEvent {
            kind,
            name,
            namespace,
            reason,
            message,
            event_type,
            first_timestamp,
            last_timestamp,
            uid,
        });
    }
    Ok(events)
}

fn extract_ignore_list(app: &EnvironmentArgocdApp) -> Vec<String> {
    app.ignore_resources
        .as_ref()
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn parse_ignore_matchers(items: &[String]) -> Vec<IgnoreMatcher> {
    items
        .iter()
        .filter_map(|raw| {
            let line = raw.trim();
            if line.is_empty() {
                return None;
            }
            let mut parts = line.split_whitespace().collect::<Vec<_>>();
            if parts.is_empty() {
                return None;
            }
            if parts.len() == 1 {
                // kind/ns/name or kind/name or group/kind/ns/name
                let segs: Vec<&str> = parts[0].split('/').collect();
                if segs.len() == 2 {
                    return Some(IgnoreMatcher {
                        group: None,
                        kind: segs[0].to_string(),
                        namespace: None,
                        name: segs[1].to_string(),
                    });
                }
                if segs.len() == 3 {
                    return Some(IgnoreMatcher {
                        group: None,
                        kind: segs[0].to_string(),
                        namespace: Some(segs[1].to_string()),
                        name: segs[2].to_string(),
                    });
                }
                if segs.len() == 4 {
                    return Some(IgnoreMatcher {
                        group: Some(segs[0].to_string()),
                        kind: segs[1].to_string(),
                        namespace: Some(segs[2].to_string()),
                        name: segs[3].to_string(),
                    });
                }
            }
            // kind or group/kind + namespace/name
            let kind_part = parts.remove(0);
            let name_part = parts.get(0).copied().unwrap_or("");
            let (group, kind) = if kind_part.contains('/') {
                let segs: Vec<&str> = kind_part.split('/').collect();
                if segs.len() == 2 {
                    (Some(segs[0].to_string()), segs[1].to_string())
                } else {
                    (None, kind_part.to_string())
                }
            } else {
                (None, kind_part.to_string())
            };
            let (namespace, name) = if name_part.contains('/') {
                let segs: Vec<&str> = name_part.split('/').collect();
                if segs.len() == 2 {
                    (Some(segs[0].to_string()), segs[1].to_string())
                } else {
                    (None, name_part.to_string())
                }
            } else {
                (None, name_part.to_string())
            };
            if kind.is_empty() || name.is_empty() {
                return None;
            }
            Some(IgnoreMatcher {
                group,
                kind,
                namespace,
                name,
            })
        })
        .collect()
}

fn matches_ignore(matchers: &[IgnoreMatcher], res: &ArgocdResource) -> bool {
    matchers.iter().any(|m| {
        if m.kind != res.kind {
            return false;
        }
        if let Some(group) = &m.group {
            if res.group.as_deref().unwrap_or("") != group {
                return false;
            }
        }
        if let Some(ns) = &m.namespace {
            if res.namespace.as_deref().unwrap_or("") != ns {
                return false;
            }
        }
        m.name == res.name
    })
}

async fn cache_status(
    pool: &PgPool,
    app_id: Uuid,
    status: &ArgocdStatus,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    sqlx::query(
        r#"
        UPDATE environment_argocd_apps
        SET last_sync_status = $1,
            last_health_status = $2,
            last_operation_phase = $3,
            last_operation_message = $4,
            last_revision = $5,
            last_checked_at = NOW()
        WHERE id = $6
        "#
    )
    .bind(status.sync_status.clone())
    .bind(status.health_status.clone())
    .bind(status.operation_phase.clone())
    .bind(status.operation_message.clone())
    .bind(status.revision.clone())
    .bind(app_id)
    .execute(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;
    Ok(())
}

fn build_client(verify_tls: bool) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(!verify_tls)
        .build()
}

async fn apply_auth(
    state: &ArgocdApiState,
    client: &reqwest::Client,
    req: reqwest::RequestBuilder,
    instance: &ArgocdInstance,
) -> Result<reqwest::RequestBuilder, (StatusCode, Json<ErrorResponse>)> {
    match instance.auth_type.as_str() {
        "token" => {
            let token = instance
                .token_encrypted
                .as_ref()
                .and_then(|v| crypto::decrypt(v, &state.encryption_secret).ok());
            if let Some(token) = token {
                Ok(req.bearer_auth(token))
            } else {
                Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Token missing for ArgoCD instance".to_string(),
                    }),
                ))
            }
        }
        _ => {
            let username = instance.username.clone().unwrap_or_default();
            let password = instance
                .password_encrypted
                .as_ref()
                .and_then(|v| crypto::decrypt(v, &state.encryption_secret).ok())
                .unwrap_or_default();

            let session_url = format!("{}/api/v1/session", instance.base_url);
            let resp = client
                .post(session_url)
                .json(&serde_json::json!({
                    "username": username,
                    "password": password,
                }))
                .send()
                .await
                .map_err(|e| {
                    (
                        StatusCode::BAD_GATEWAY,
                        Json(ErrorResponse {
                            error: format!("ArgoCD session request failed: {}", e),
                        }),
                    )
                })?;

            let value: serde_json::Value = resp.json().await.map_err(|e| {
                (
                    StatusCode::BAD_GATEWAY,
                    Json(ErrorResponse {
                        error: format!("ArgoCD session decode failed: {}", e),
                    }),
                )
            })?;
            let token = value.get("token").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if token.is_empty() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "ArgoCD session token missing".to_string(),
                    }),
                ));
            }
            Ok(req.bearer_auth(token))
        }
    }
}
