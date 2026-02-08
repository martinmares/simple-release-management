use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::crypto;
use crate::db::models::{EnvironmentKubernetesNamespace, KubernetesInstance};

#[derive(Clone)]
pub struct KubernetesApiState {
    pub pool: PgPool,
    pub encryption_secret: String,
    pub client_tls: reqwest::Client,
    pub client_insecure: reqwest::Client,
    pub oauth_client_tls: reqwest::Client,
    pub oauth_client_insecure: reqwest::Client,
    pub token_cache: Arc<RwLock<HashMap<Uuid, String>>>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct KubernetesInstanceRequest {
    pub name: String,
    pub base_url: String,
    pub oauth_base_url: Option<String>,
    pub auth_type: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub token: Option<String>,
    pub verify_tls: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct KubernetesNamespaceRequest {
    pub kubernetes_instance_id: Uuid,
    pub namespace: String,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct KubernetesNamespaceSummary {
    pub id: Uuid,
    pub environment_id: Uuid,
    pub kubernetes_instance_id: Uuid,
    pub namespace: String,
    pub is_active: bool,
    pub instance_name: String,
    pub instance_base_url: String,
}

#[derive(Debug, Serialize)]
pub struct KubernetesEvent {
    pub uid: Option<String>,
    pub timestamp: Option<String>,
    #[serde(rename = "type")]
    pub event_type: Option<String>,
    pub reason: Option<String>,
    pub kind: Option<String>,
    pub name: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StreamQuery {
    pub interval: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ResourceQuery {
    pub kind: Option<String>,
}

pub fn router(state: KubernetesApiState) -> Router {
    Router::new()
        .route("/tenants/{tenant_id}/kubernetes", get(list_instances).post(create_instance))
        .route("/kubernetes/{id}", get(get_instance).put(update_instance).delete(delete_instance))
        .route(
            "/environments/{env_id}/kubernetes-namespaces",
            get(list_env_namespaces).post(create_env_namespace),
        )
        .route(
            "/kubernetes-namespaces/{id}",
            get(get_env_namespace).put(update_env_namespace).delete(delete_env_namespace),
        )
        .route("/kubernetes-namespaces/{id}/status", get(get_namespace_status))
        .route("/kubernetes-namespaces/{id}/events", get(get_namespace_events))
        .route("/kubernetes-namespaces/{id}/events/stream", get(stream_namespace_events))
        .route("/kubernetes-namespaces/{id}/resources", get(get_namespace_resources))
        .with_state(state)
}

async fn list_instances(
    State(state): State<KubernetesApiState>,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<Vec<KubernetesInstance>>, (StatusCode, Json<ErrorResponse>)> {
    let instances = sqlx::query_as::<_, KubernetesInstance>(
        "SELECT * FROM kubernetes_instances WHERE tenant_id = $1 ORDER BY name",
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
    State(state): State<KubernetesApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<KubernetesInstance>, (StatusCode, Json<ErrorResponse>)> {
    let instance = sqlx::query_as::<_, KubernetesInstance>(
        "SELECT * FROM kubernetes_instances WHERE id = $1",
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
                error: "Kubernetes instance not found".to_string(),
            }),
        )),
    }
}

async fn create_instance(
    State(state): State<KubernetesApiState>,
    Path(tenant_id): Path<Uuid>,
    Json(payload): Json<KubernetesInstanceRequest>,
) -> Result<(StatusCode, Json<KubernetesInstance>), (StatusCode, Json<ErrorResponse>)> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Name cannot be empty".to_string(),
            }),
        ));
    }

    let auth_type = normalize_auth_type(&payload.auth_type);

    let password_encrypted = payload
        .password
        .as_deref()
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
    let token_encrypted = payload
        .token
        .as_deref()
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

    let instance = sqlx::query_as::<_, KubernetesInstance>(
        r#"
        INSERT INTO kubernetes_instances
        (id, tenant_id, name, base_url, oauth_base_url, auth_type, username, password_encrypted, token_encrypted, verify_tls)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING *
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(tenant_id)
    .bind(name)
    .bind(payload.base_url.trim())
    .bind(payload.oauth_base_url.as_deref().map(str::trim).filter(|v| !v.is_empty()))
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
    State(state): State<KubernetesApiState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<KubernetesInstanceRequest>,
) -> Result<Json<KubernetesInstance>, (StatusCode, Json<ErrorResponse>)> {
    let current = sqlx::query_as::<_, KubernetesInstance>(
        "SELECT * FROM kubernetes_instances WHERE id = $1",
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
                error: "Kubernetes instance not found".to_string(),
            }),
        ));
    };

    let auth_type = normalize_auth_type(&payload.auth_type);

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

    let instance = sqlx::query_as::<_, KubernetesInstance>(
        r#"
        UPDATE kubernetes_instances
        SET name = $1,
            base_url = $2,
            oauth_base_url = $3,
            auth_type = $4,
            username = $5,
            password_encrypted = $6,
            token_encrypted = $7,
            verify_tls = $8
        WHERE id = $9
        RETURNING *
        "#,
    )
    .bind(payload.name.trim())
    .bind(payload.base_url.trim())
    .bind(payload.oauth_base_url.as_deref().map(str::trim).filter(|v| !v.is_empty()))
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
    State(state): State<KubernetesApiState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let in_use = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM environment_kubernetes_namespaces WHERE kubernetes_instance_id = $1)",
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
                error: "Kubernetes instance has namespaces and cannot be deleted".to_string(),
            }),
        ));
    }

    let result = sqlx::query("DELETE FROM kubernetes_instances WHERE id = $1")
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
                error: "Kubernetes instance not found".to_string(),
            }),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn list_env_namespaces(
    State(state): State<KubernetesApiState>,
    Path(env_id): Path<Uuid>,
) -> Result<Json<Vec<KubernetesNamespaceSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let namespaces = sqlx::query_as::<_, KubernetesNamespaceSummary>(
        r#"
        SELECT
            n.id,
            n.environment_id,
            n.kubernetes_instance_id,
            n.namespace,
            n.is_active,
            i.name AS instance_name,
            i.base_url AS instance_base_url
        FROM environment_kubernetes_namespaces n
        JOIN kubernetes_instances i ON i.id = n.kubernetes_instance_id
        WHERE n.environment_id = $1
        ORDER BY n.namespace
        "#,
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

    Ok(Json(namespaces))
}

async fn create_env_namespace(
    State(state): State<KubernetesApiState>,
    Path(env_id): Path<Uuid>,
    Json(payload): Json<KubernetesNamespaceRequest>,
) -> Result<(StatusCode, Json<EnvironmentKubernetesNamespace>), (StatusCode, Json<ErrorResponse>)> {
    let namespace = payload.namespace.trim();
    if namespace.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Namespace cannot be empty".to_string(),
            }),
        ));
    }

    let entry = sqlx::query_as::<_, EnvironmentKubernetesNamespace>(
        r#"
        INSERT INTO environment_kubernetes_namespaces
        (id, environment_id, kubernetes_instance_id, namespace, is_active)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(env_id)
    .bind(payload.kubernetes_instance_id)
    .bind(namespace)
    .bind(payload.is_active.unwrap_or(true))
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

    Ok((StatusCode::CREATED, Json(entry)))
}

async fn get_env_namespace(
    State(state): State<KubernetesApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<EnvironmentKubernetesNamespace>, (StatusCode, Json<ErrorResponse>)> {
    let entry = sqlx::query_as::<_, EnvironmentKubernetesNamespace>(
        "SELECT * FROM environment_kubernetes_namespaces WHERE id = $1",
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

    match entry {
        Some(entry) => Ok(Json(entry)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Kubernetes namespace not found".to_string(),
            }),
        )),
    }
}

async fn update_env_namespace(
    State(state): State<KubernetesApiState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<KubernetesNamespaceRequest>,
) -> Result<Json<EnvironmentKubernetesNamespace>, (StatusCode, Json<ErrorResponse>)> {
    let namespace = payload.namespace.trim();
    if namespace.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Namespace cannot be empty".to_string(),
            }),
        ));
    }

    let entry = sqlx::query_as::<_, EnvironmentKubernetesNamespace>(
        r#"
        UPDATE environment_kubernetes_namespaces
        SET kubernetes_instance_id = $1,
            namespace = $2,
            is_active = $3
        WHERE id = $4
        RETURNING *
        "#,
    )
    .bind(payload.kubernetes_instance_id)
    .bind(namespace)
    .bind(payload.is_active.unwrap_or(true))
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

    Ok(Json(entry))
}

async fn delete_env_namespace(
    State(state): State<KubernetesApiState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let result = sqlx::query("DELETE FROM environment_kubernetes_namespaces WHERE id = $1")
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
                error: "Kubernetes namespace not found".to_string(),
            }),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn get_namespace_status(
    State(state): State<KubernetesApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let (instance, _namespace) = load_instance_and_namespace(&state.pool, id).await?;
    let status = fetch_k8s_version(&state, &instance).await?;
    Ok(Json(status))
}

async fn get_namespace_events(
    State(state): State<KubernetesApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<KubernetesEvent>>, (StatusCode, Json<ErrorResponse>)> {
    let (instance, namespace) = load_instance_and_namespace(&state.pool, id).await?;
    let events = fetch_k8s_events(&state, &instance, &namespace.namespace).await?;
    Ok(Json(events))
}

async fn stream_namespace_events(
    State(state): State<KubernetesApiState>,
    Path(id): Path<Uuid>,
    Query(query): Query<StreamQuery>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, (StatusCode, Json<ErrorResponse>)> {
    let interval = query.interval.unwrap_or(15).max(5);

    let stream = async_stream::stream! {
        let mut seen: VecDeque<String> = VecDeque::new();
        loop {
            match get_namespace_events(State(state.clone()), Path(id)).await {
                Ok(Json(events)) => {
                    for ev in events {
                        let key = ev.uid.clone().unwrap_or_else(|| {
                            format!("{}:{}:{}:{}", ev.timestamp.clone().unwrap_or_default(), ev.kind.clone().unwrap_or_default(), ev.name.clone().unwrap_or_default(), ev.reason.clone().unwrap_or_default())
                        });
                        if seen.iter().any(|v| v == &key) {
                            continue;
                        }
                        seen.push_back(key);
                        if seen.len() > 100 {
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

async fn get_namespace_resources(
    State(state): State<KubernetesApiState>,
    Path(id): Path<Uuid>,
    Query(query): Query<ResourceQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let (instance, namespace) = load_instance_and_namespace(&state.pool, id).await?;
    let kind = query.kind.clone().unwrap_or_else(|| "deployments".to_string());
    let path = match kind.as_str() {
        "namespaces" => format!("/api/v1/namespaces/{}", namespace.namespace),
        "deployments" => format!("/apis/apps/v1/namespaces/{}/deployments", namespace.namespace),
        "pods" => format!("/api/v1/namespaces/{}/pods", namespace.namespace),
        "services" => format!("/api/v1/namespaces/{}/services", namespace.namespace),
        "routes" => format!("/apis/route.openshift.io/v1/namespaces/{}/routes", namespace.namespace),
        "configmaps" => format!("/api/v1/namespaces/{}/configmaps", namespace.namespace),
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Unsupported resource kind".to_string(),
                }),
            ));
        }
    };

    let data = fetch_k8s_json(&state, &instance, &path).await?;
    Ok(Json(data))
}

async fn load_instance_and_namespace(
    pool: &PgPool,
    namespace_id: Uuid,
) -> Result<(KubernetesInstance, EnvironmentKubernetesNamespace), (StatusCode, Json<ErrorResponse>)> {
    let namespace = sqlx::query_as::<_, EnvironmentKubernetesNamespace>(
        "SELECT * FROM environment_kubernetes_namespaces WHERE id = $1",
    )
    .bind(namespace_id)
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

    let Some(namespace) = namespace else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Kubernetes namespace not found".to_string(),
            }),
        ));
    };

    let instance = sqlx::query_as::<_, KubernetesInstance>(
        "SELECT * FROM kubernetes_instances WHERE id = $1",
    )
    .bind(namespace.kubernetes_instance_id)
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

    Ok((instance, namespace))
}

fn normalize_auth_type(raw: &str) -> String {
    let auth = raw.trim().to_lowercase();
    match auth.as_str() {
        "token" => "token".to_string(),
        _ => "basic".to_string(),
    }
}

fn get_client<'a>(state: &'a KubernetesApiState, verify_tls: bool) -> &'a reqwest::Client {
    if verify_tls {
        &state.client_tls
    } else {
        &state.client_insecure
    }
}

fn get_oauth_client<'a>(state: &'a KubernetesApiState, verify_tls: bool) -> &'a reqwest::Client {
    if verify_tls {
        &state.oauth_client_tls
    } else {
        &state.oauth_client_insecure
    }
}

async fn send_with_auth<F>(
    state: &KubernetesApiState,
    instance: &KubernetesInstance,
    build_req: F,
) -> Result<reqwest::Response, (StatusCode, Json<ErrorResponse>)>
where
    F: Fn(&reqwest::Client) -> reqwest::RequestBuilder,
{
    let client = get_client(state, instance.verify_tls);
    let req = apply_auth(state, build_req(client), instance, false).await?;
    let resp = req.send().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("Kubernetes request failed: {}", e),
            }),
        )
    })?;

    if resp.status() == StatusCode::UNAUTHORIZED && instance.auth_type != "token" {
        let req = apply_auth(state, build_req(client), instance, true).await?;
        let retry = req.send().await.map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: format!("Kubernetes request failed: {}", e),
                }),
            )
        })?;
        return Ok(retry);
    }

    Ok(resp)
}

async fn apply_auth(
    state: &KubernetesApiState,
    req: reqwest::RequestBuilder,
    instance: &KubernetesInstance,
    force_refresh: bool,
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
                        error: "Token missing for Kubernetes instance".to_string(),
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

            if username.trim().is_empty() || password.trim().is_empty() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Username/password missing for Kubernetes instance".to_string(),
                    }),
                ));
            }

            let token = fetch_openshift_token(state, instance, &username, &password, force_refresh).await?;
            Ok(req.bearer_auth(token))
        }
    }
}

async fn fetch_k8s_version(
    state: &KubernetesApiState,
    instance: &KubernetesInstance,
) -> Result<serde_json::Value, (StatusCode, Json<ErrorResponse>)> {
    fetch_k8s_json(state, instance, "/version").await
}

async fn fetch_k8s_events(
    state: &KubernetesApiState,
    instance: &KubernetesInstance,
    namespace: &str,
) -> Result<Vec<KubernetesEvent>, (StatusCode, Json<ErrorResponse>)> {
    let path = format!("/api/v1/namespaces/{}/events", namespace);
    let value = fetch_k8s_json(state, instance, &path).await?;

    let items = value.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut events: Vec<KubernetesEvent> = items
        .into_iter()
        .map(|ev| {
            let timestamp = ev
                .get("lastTimestamp")
                .and_then(|v| v.as_str())
                .or_else(|| ev.get("eventTime").and_then(|v| v.as_str()))
                .or_else(|| ev.pointer("/metadata/creationTimestamp").and_then(|v| v.as_str()))
                .map(str::to_string);

            KubernetesEvent {
                uid: ev.pointer("/metadata/uid").and_then(|v| v.as_str()).map(str::to_string),
                timestamp,
                event_type: ev.get("type").and_then(|v| v.as_str()).map(str::to_string),
                reason: ev.get("reason").and_then(|v| v.as_str()).map(str::to_string),
                kind: ev.pointer("/involvedObject/kind").and_then(|v| v.as_str()).map(str::to_string),
                name: ev.pointer("/involvedObject/name").and_then(|v| v.as_str()).map(str::to_string),
                message: ev.get("message").and_then(|v| v.as_str()).map(str::to_string),
            }
        })
        .collect();

    events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    events.truncate(20);

    Ok(events)
}

async fn fetch_k8s_json(
    state: &KubernetesApiState,
    instance: &KubernetesInstance,
    path: &str,
) -> Result<serde_json::Value, (StatusCode, Json<ErrorResponse>)> {
    let url = format!("{}{}", instance.base_url.trim_end_matches('/'), path);
    let resp = send_with_auth(state, instance, |client| {
        client.get(url.clone()).header("Accept", "application/json")
    })
    .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("Kubernetes request failed: {} {}", status, body),
            }),
        ));
    }

    let value: serde_json::Value = resp.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("Kubernetes response decode failed: {}", e),
            }),
        )
    })?;

    Ok(value)
}

async fn fetch_openshift_token(
    state: &KubernetesApiState,
    instance: &KubernetesInstance,
    username: &str,
    password: &str,
    force_refresh: bool,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    if !force_refresh {
        if let Some(token) = state.token_cache.read().await.get(&instance.id).cloned() {
            return Ok(token);
        }
    }

    let oauth_url = instance
        .oauth_base_url
        .as_deref()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .and_then(|v| build_oauth_url_from_base(&v))
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "OAuth base URL missing or invalid for Kubernetes instance".to_string(),
                }),
            )
        })?;

    let client = get_oauth_client(state, instance.verify_tls);
    let csrf = Uuid::new_v4().to_string();
    let resp = client
        .get(oauth_url)
        .basic_auth(username, Some(password))
        .header("X-CSRF-Token", csrf)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: format!("OpenShift OAuth request failed: {}", e),
                }),
            )
        })?;

    let location = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = extract_access_token(location);
    if token.is_empty() {
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: "OpenShift access token missing in redirect".to_string(),
            }),
        ));
    }

    state.token_cache.write().await.insert(instance.id, token.clone());
    Ok(token)
}

fn build_oauth_url_from_base(base_url: &str) -> Option<String> {
    let url = reqwest::Url::parse(base_url).ok()?;
    let scheme = url.scheme();
    let host = url.host_str()?;
    let mut oauth_url = reqwest::Url::parse(&format!("{}://{}/oauth/authorize", scheme, host)).ok()?;
    oauth_url
        .query_pairs_mut()
        .append_pair("client_id", "openshift-challenging-client")
        .append_pair("response_type", "token");
    Some(oauth_url.to_string())
}

fn extract_access_token(location: &str) -> String {
    if location.is_empty() {
        return String::new();
    }
    let parts: Vec<&str> = location.split('#').collect();
    let fragment = if parts.len() > 1 { parts[1] } else { "" };
    for pair in fragment.split('&') {
        let mut kv = pair.splitn(2, '=');
        let key = kv.next().unwrap_or("");
        let val = kv.next().unwrap_or("");
        if key == "access_token" {
            return val.to_string();
        }
    }
    String::new()
}
