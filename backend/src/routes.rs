use std::{collections::HashMap, sync::Arc};

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

use serde::Deserialize;

use crate::{
    api::{TTL_CATALOG, TTL_FILTERS, TTL_LISTING, TTL_PRODUCT, Upstream},
    db::{Db, now_ms},
    error::AppError,
};

#[derive(Clone)]
pub struct AppState {
    pub up: Arc<Upstream>,
    pub db: Arc<Db>,
}

type Q = Query<HashMap<String, String>>;

fn pick(query: &HashMap<String, String>, keys: &[&str]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for key in keys {
        if let Some(value) = query.get(*key) {
            if !value.trim().is_empty() {
                out.push(((*key).to_string(), value.clone()));
            }
        }
    }
    out
}

fn with_locale(mut params: Vec<(String, String)>) -> Vec<(String, String)> {
    params.push(("locale".to_string(), "vi".to_string()));
    params
}

pub fn router(state: AppState) -> axum::Router {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .route("/search", get(search))
        .route("/suggest", get(suggest))
        .route("/categories", get(categories))
        .route("/categories/{id}/products", get(category_products))
        .route("/categories/{id}/filters", get(category_filters))
        .route("/categories/{id}/suppliers", get(category_suppliers))
        .route("/product/{slug}", get(product))
        .route("/suppliers", get(suppliers))
        .route("/image/{id}", get(image))
        .route("/alternates", get(alternates))
        .route("/related", get(related))
        .route("/bom/docs", get(bom_list).post(bom_save))
        .route("/bom/docs/{id}", get(bom_get).put(bom_update).delete(bom_delete))
        .route("/bom/draft", get(draft_get).put(draft_save))
        .route("/settings", get(settings_get).put(settings_put))
        .route("/pick", post(pick_path))
        .route("/jlcpcb/status", get(jlc_status))
        .route("/jlcpcb/lookup", get(jlc_lookup))
        .route("/localfiles", get(local_files))
        .route("/footprint3d", get(footprint3d))
        .with_state(state)
}

async fn index() -> Json<Value> {
    Json(json!({
        "name": "re-search-accessory",
        "version": env!("CARGO_PKG_VERSION"),
        "endpoints": [
            "GET /api/health",
            "GET /api/search?q=&page=&limit=&in_stock=&order_sort=",
            "GET /api/suggest?q=&limit=",
            "GET /api/categories",
            "GET /api/categories/{id}/products?page=&limit=&supplier=&prid=&in_stock=&order_sort=",
            "GET /api/categories/{id}/filters?supplier=&prid=",
            "GET /api/product/{slug}",
            "GET /api/suppliers?page=&limit=",
            "GET /api/image/{id}?size=small|medium|large"
        ]
    }))
}

async fn health(State(st): State<AppState>) -> Json<Value> {
    // A cheap catalogue read doubles as an upstream reachability probe.
    let upstream_ok = st
        .up
        .json("/categories/main/menu", &with_locale(vec![]), TTL_CATALOG)
        .await
        .is_ok();
    Json(json!({
        "ok": true,
        "upstream": if upstream_ok { "reachable" } else { "unreachable" },
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn search(State(st): State<AppState>, q: Q) -> Result<Json<Value>, AppError> {
    let text = q.get("q").cloned().unwrap_or_default();
    if text.trim().is_empty() {
        return Ok(Json(json!({
            "data": { "products": { "data": [], "total": 0, "total_page": 0, "current_page": 1, "per_page": 0 }, "categories": [] }
        })));
    }
    let params = with_locale(pick(
        &q,
        &["q", "page", "limit", "in_stock", "order_sort"],
    ));
    let body = st.up.json("/products/search", &params, TTL_LISTING).await?;
    Ok(Json((*body).clone()))
}

async fn suggest(State(st): State<AppState>, q: Q) -> Result<Json<Value>, AppError> {
    let text = q.get("q").cloned().unwrap_or_default();
    if text.trim().is_empty() {
        return Ok(Json(json!({ "data": { "products": [] } })));
    }
    let mut params = vec![
        ("q".to_string(), text),
        ("full_search".to_string(), "0".to_string()),
        (
            "limit".to_string(),
            q.get("limit").cloned().unwrap_or_else(|| "8".into()),
        ),
    ];
    params = with_locale(params);
    let body = st.up.json("/products/search/auto", &params, TTL_LISTING).await?;
    Ok(Json((*body).clone()))
}

async fn categories(State(st): State<AppState>) -> Result<Json<Value>, AppError> {
    let body = st
        .up
        .json("/categories", &with_locale(vec![]), TTL_CATALOG)
        .await?;
    Ok(Json((*body).clone()))
}

async fn category_products(
    State(st): State<AppState>,
    Path(id): Path<String>,
    q: Q,
) -> Result<Json<Value>, AppError> {
    let id = id.parse::<u32>().map_err(|_| AppError::Upstream {
        status: 400,
        message: "id danh mục phải là số (lấy từ /api/categories)".to_string(),
    })?;
    let params = with_locale(pick(
        &q,
        &[
            "page",
            "limit",
            "supplier",
            "prid",
            "in_stock",
            "order_sort",
        ],
    ));
    let body = st
        .up
        .json(
            &format!("/categories/{id}/products"),
            &params,
            TTL_LISTING,
        )
        .await?;
    Ok(Json((*body).clone()))
}

async fn category_filters(
    State(st): State<AppState>,
    Path(id): Path<String>,
    q: Q,
) -> Result<Json<Value>, AppError> {
    let id = id.parse::<u32>().map_err(|_| AppError::Upstream {
        status: 400,
        message: "id danh mục phải là số (lấy từ /api/categories)".to_string(),
    })?;
    let params = with_locale(pick(&q, &["supplier", "prid", "in_stock"]));

    let base_path = format!("/categories/filter/property/{id}");
    let filtered_path = format!("/categories/filtered/property/{id}");
    let (base, filtered) = tokio::join!(
        st.up.json(&base_path, &params, TTL_FILTERS),
        st.up.json(&filtered_path, &params, TTL_FILTERS)
    );

    Ok(Json(json!({
        "filters": base.map(|v| (*v).clone()).unwrap_or(Value::Null),
        "filtered": filtered.map(|v| (*v).clone()).unwrap_or(Value::Null),
    })))
}

async fn category_suppliers(
    State(st): State<AppState>,
    Path(id): Path<String>,
    q: Q,
) -> Result<Json<Value>, AppError> {
    let id = id.parse::<u32>().map_err(|_| AppError::Upstream {
        status: 400,
        message: "id danh mục phải là số (lấy từ /api/categories)".to_string(),
    })?;
    let params = with_locale(pick(&q, &["prid", "in_stock"]));
    let body = st
        .up
        .json(
            &format!("/categories/filter/supplier/{id}"),
            &params,
            TTL_FILTERS,
        )
        .await?;
    Ok(Json((*body).clone()))
}

async fn product(State(st): State<AppState>, Path(slug): Path<String>) -> Result<Json<Value>, AppError> {
    let api_path = format!("/products/{slug}");
    let params = with_locale(Vec::new());
    let (body, specs) = tokio::join!(
        st.up.json(&api_path, &params, TTL_PRODUCT),
        st.up.specs(&slug)
    );
    let body = body?;
    let mut out = (*body).clone();
    let product = out
        .pointer_mut("/data/product")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| AppError::Upstream {
            status: 404,
            message: "không tìm thấy sản phẩm".to_string(),
        })?;
    product.insert("specs".to_string(), Value::Array(specs));
    product.insert(
        "url".to_string(),
        Value::String(st.up.product_url(&slug)),
    );
    Ok(Json(out))
}

async fn suppliers(State(st): State<AppState>, q: Q) -> Result<Json<Value>, AppError> {
    let params = with_locale(pick(&q, &["page", "limit"]));
    let body = st.up.json("/suppliers", &params, TTL_CATALOG).await?;
    Ok(Json((*body).clone()))
}

async fn image(
    State(st): State<AppState>,
    Path(id): Path<String>,
    q: Q,
) -> Result<Response, AppError> {
    let size = q.get("size").map(String::as_str).unwrap_or("medium");
    let (content_type, bytes) = st.up.image(&id, size).await?;
    let mut response = bytes.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content_type).unwrap_or(HeaderValue::from_static("image/jpeg")),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=604800"),
    );
    Ok(response)
}

/* ---------------- alternates / related ---------------- */

async fn alternates(State(st): State<AppState>, q: Q) -> Result<Json<Value>, AppError> {
    related_products(st, q.0, "/products/alternates").await
}

async fn related(State(st): State<AppState>, q: Q) -> Result<Json<Value>, AppError> {
    related_products(st, q.0, "/products/related").await
}

async fn related_products(
    st: AppState,
    q: HashMap<String, String>,
    path: &str,
) -> Result<Json<Value>, AppError> {
    let id = q.get("product_id").cloned().unwrap_or_default();

    if id.trim().is_empty() {
        return Err(AppError::Upstream {
            status: 400,
            message: "thiếu product_id".to_string(),
        });
    }

    let params = with_locale(vec![("product_id".to_string(), id)]);
    let body = st.up.json(path, &params, TTL_PRODUCT).await?;

    Ok(Json((*body).clone()))
}

/* ---------------- BOM storage (SQLite) ---------------- */

#[derive(Deserialize)]
struct BomSaveBody {
    id: Option<String>,
    name: String,
    payload: String,
}

#[derive(Deserialize)]
struct DraftBody {
    payload: String,
}

async fn bom_list(State(st): State<AppState>) -> Result<Json<Value>, AppError> {
    let docs = st.db.list_boms().map_err(AppError::Store)?;

    Ok(Json(json!({ "docs": docs, "memory_only": st.db.is_memory_only() })))
}

async fn bom_get(State(st): State<AppState>, Path(id): Path<String>) -> Result<Json<Value>, AppError> {
    match st.db.get_bom(&id).map_err(AppError::Store)? {
        Some(doc) => Ok(Json(json!({ "doc": doc }))),
        None => Err(AppError::Upstream {
            status: 404,
            message: "BOM không tồn tại".to_string(),
        }),
    }
}

async fn bom_save(
    State(st): State<AppState>,
    Json(body): Json<BomSaveBody>,
) -> Result<Json<Value>, AppError> {
    let id = body
        .id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("bom-{}", now_ms()));

    let saved_at = st
        .db
        .save_bom(&id, &body.name, &body.payload)
        .map_err(AppError::Store)?;

    Ok(Json(json!({ "id": id, "saved_at": saved_at })))
}

async fn bom_update(
    State(st): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<BomSaveBody>,
) -> Result<Json<Value>, AppError> {
    // Autosave of an already-saved BOM: keep the stored name when the client
    // only sends the payload.
    let name = if body.name.trim().is_empty() {
        st.db
            .get_bom(&id)
            .map_err(AppError::Store)?
            .map(|doc| doc.name)
            .unwrap_or_else(|| "BOM".to_string())
    }
    else {
        body.name
    };

    let saved_at = st
        .db
        .save_bom(&id, &name, &body.payload)
        .map_err(AppError::Store)?;

    Ok(Json(json!({ "id": id, "name": name, "saved_at": saved_at })))
}

async fn bom_delete(State(st): State<AppState>, Path(id): Path<String>) -> Result<Json<Value>, AppError> {
    st.db.delete_bom(&id).map_err(AppError::Store)?;

    Ok(Json(json!({ "ok": true })))
}

async fn draft_get(State(st): State<AppState>) -> Result<Json<Value>, AppError> {
    let draft = st.db.get_draft().map_err(AppError::Store)?;

    Ok(Json(json!({ "draft": draft })))
}

async fn draft_save(
    State(st): State<AppState>,
    Json(body): Json<DraftBody>,
) -> Result<Json<Value>, AppError> {
    let updated_at = st.db.save_draft(&body.payload).map_err(AppError::Store)?;

    Ok(Json(json!({ "updated_at": updated_at })))
}

/* ---------------- UI settings (theme, last folders) ---------------- */

async fn settings_get(State(st): State<AppState>) -> Result<Json<Value>, AppError> {
    let map = st.db.settings().map_err(AppError::Store)?;

    Ok(Json(Value::Object(map.into_iter().collect())))
}

async fn settings_put(
    State(st): State<AppState>,
    Json(body): Json<HashMap<String, Value>>,
) -> Result<Json<Value>, AppError> {
    for (key, value) in &body {
        st.db.set_setting(key, value).map_err(AppError::Store)?;
    }

    settings_get(State(st)).await
}

/// Native "open file / choose folder" dialog, so the user never has to type a
/// path. The server is loopback-only, so the dialog opens on this machine.
async fn pick_path(Json(body): Json<Value>) -> Json<Value> {
    let kind = body.get("kind").and_then(Value::as_str).unwrap_or("folder").to_string();
    let title = body
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or(if kind == "file" { "Chọn file" } else { "Chọn thư mục" })
        .to_string();
    let filter_name = body.get("filter_name").and_then(Value::as_str).map(str::to_string);
    let filter_ext = body.get("filter").and_then(Value::as_str).map(str::to_string);

    let picked = tokio::task::spawn_blocking(move || {
        let mut dialog = rfd::FileDialog::new().set_title(&title);

        if let (Some(name), Some(ext)) = (filter_name, filter_ext) {
            let exts: Vec<&str> = ext.split(',').map(str::trim).filter(|item| !item.is_empty()).collect();

            if !exts.is_empty() {
                dialog = dialog.add_filter(name, &exts);
            }
        }

        if kind == "file" {
            dialog.pick_file()
        }
        else {
            dialog.pick_folder()
        }
    })
    .await
    .ok()
    .flatten();

    let cancelled = picked.is_none();

    Json(json!({
        "path": picked.map(|path| path.display().to_string()),
        "cancelled": cancelled,
    }))
}

/* ---------------- JLCPCB read-only cache ---------------- */

async fn jlc_status(State(st): State<AppState>) -> Json<Value> {
    Json(serde_json::to_value(st.db.jlc_status()).unwrap_or_else(|_| json!({ "available": false })))
}

const GERBER_EXT: [&str; 14] = [
    "gbr", "gtl", "gbl", "gto", "gbo", "gts", "gbs", "gko", "gm1", "gtp", "gbp", "drl", "xln", "zip",
];
const STEP_EXT: [&str; 2] = ["step", "stp"];

/// Read a local folder (or single file) so the viewer can preview gerbers/STEP
/// without uploading them anywhere. Loopback-only tool, size-capped.
async fn local_files(q: Q) -> Result<Json<Value>, AppError> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use std::path::{Path, PathBuf};

    let raw = q.get("path").cloned().unwrap_or_default();
    let path = PathBuf::from(raw.trim());
    let kind = q.get("kind").cloned().unwrap_or_else(|| "gerber".to_string());
    let allowed = if kind == "step" { &STEP_EXT[..] } else { &GERBER_EXT[..] };

    if raw.trim().is_empty() || !path.exists() {
        return Err(AppError::Upstream {
            status: 400,
            message: "đường dẫn không tồn tại".to_string(),
        });
    }

    let mut entries: Vec<PathBuf> = Vec::new();

    if path.is_dir() {
        let read = std::fs::read_dir(&path).map_err(|err| AppError::Store(err.to_string()))?;

        for entry in read.flatten() {
            let candidate = entry.path();

            if !candidate.is_file() {
                continue;
            }

            let ext = candidate
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_lowercase();

            if allowed.contains(&ext.as_str()) && !candidate.to_string_lossy().ends_with(".gbrjob") {
                entries.push(candidate);
            }
        }

        entries.sort();
    }
    else {
        entries.push(path.clone());
    }

    if entries.is_empty() {
        return Err(AppError::Upstream {
            status: 404,
            message: format!("không có file {} trong thư mục", if kind == "step" { "STEP" } else { "gerber" }),
        });
    }

    const MAX_FILES: usize = 80;
    const MAX_BYTES: u64 = 80 * 1024 * 1024;
    let mut total = 0u64;
    let mut files = Vec::new();

    for entry in entries.into_iter().take(MAX_FILES) {
        let meta = std::fs::metadata(&entry).map_err(|err| AppError::Store(err.to_string()))?;

        total += meta.len();

        if total > MAX_BYTES {
            break;
        }

        let bytes = std::fs::read(&entry).map_err(|err| AppError::Store(err.to_string()))?;
        let name = entry
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("file")
            .to_string();

        files.push(json!({
            "name": name,
            "size": bytes.len(),
            "data": STANDARD.encode(&bytes),
        }));
    }

    let _ = Path::new(&raw);

    Ok(Json(json!({
        "dir": path.display().to_string(),
        "files": files,
    })))
}

async fn footprint3d(q: Q) -> Json<Value> {
    let footprint = q.get("fp").cloned().unwrap_or_default();
    let project = q
        .get("project")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty());

    Json(serde_json::to_value(crate::fp3d::resolve(&footprint, project)).unwrap_or_else(|_| json!({ "found": false })))
}

async fn jlc_lookup(State(st): State<AppState>, q: Q) -> Json<Value> {
    let mpn = q.get("mpn").cloned().unwrap_or_default();
    let limit = q
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(3);

    match st.db.jlc_lookup(&mpn, limit) {
        Ok(parts) => Json(json!({ "parts": parts })),
        Err(err) => Json(json!({ "parts": [], "error": err })),
    }
}
