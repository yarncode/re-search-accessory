use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use serde_json::Value;

use crate::{cache::Cache, db::Db, error::AppError, scrape};

pub const TTL_CATALOG: Duration = Duration::from_secs(6 * 60 * 60);
pub const TTL_FILTERS: Duration = Duration::from_secs(15 * 60);
pub const TTL_PRODUCT: Duration = Duration::from_secs(15 * 60);
pub const TTL_SPECS: Duration = Duration::from_secs(6 * 60 * 60);
pub const TTL_LISTING: Duration = Duration::from_secs(2 * 60);

const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 ",
    "(KHTML, like Gecko) Chrome/131.0 Safari/537.36 re-search-accessory/",
    env!("CARGO_PKG_VERSION")
);

pub struct Upstream {
    http: reqwest::Client,
    cache: Cache,
    db: Arc<Db>,
    api_base: String,
    file_base: String,
    web_base: String,
}

impl Upstream {
    pub fn new(db: Arc<Db>) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(20))
            .connect_timeout(Duration::from_secs(8))
            .build()
            .expect("build reqwest client");
        Self {
            http,
            cache: Cache::new(512),
            db,
            api_base: "https://api.thegioiic.com/v1".to_string(),
            file_base: "https://file.thegioiic.com/upload".to_string(),
            web_base: "https://www.thegioiic.com".to_string(),
        }
    }

    /// GET a JSON document from the upstream API, memoised for `ttl`.
    pub async fn json(
        &self,
        path: &str,
        params: &[(String, String)],
        ttl: Duration,
    ) -> Result<Arc<Value>, AppError> {
        let key = cache_key(path, params);
        if let Some(hit) = self.cache.get(&key) {
            return Ok(hit);
        }

        // Durable cache: survives restarts, and doubles as the offline cache.
        if let Some(body) = self.db.cache_get(&key) {
            if let Ok(parsed) = serde_json::from_str::<Value>(&body) {
                let value = Arc::new(parsed);

                self.cache.set(key.clone(), value.clone(), ttl);

                return Ok(value);
            }
        }

        let started = Instant::now();
        let response = self
            .http
            .get(format!("{}{}", self.api_base, path))
            .query(params)
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;

        let parsed: Value = serde_json::from_str(&body).map_err(|err| AppError::Upstream {
            status: 502,
            message: format!("thegioiic trả về dữ liệu lạ: {err}"),
        })?;

        if !status.is_success() {
            let message = parsed
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("thegioiic từ chối yêu cầu")
                .to_string();
            return Err(AppError::Upstream {
                status: status.as_u16(),
                message,
            });
        }

        tracing::debug!(
            path,
            status = status.as_u16(),
            ms = started.elapsed().as_millis(),
            "upstream ok"
        );

        let value = Arc::new(parsed);
        self.cache.set(key.clone(), value.clone(), ttl);

        if let Ok(serialized) = serde_json::to_string(&*value) {
            self.db.cache_put(&key, &serialized, ttl);
        }

        Ok(value)
    }

    /// Fetch a product image from the CDN. The CDN only publishes `medium` and
    /// `large` variants, so anything else falls back to `medium`.
    pub async fn image(&self, id: &str, size: &str) -> Result<(String, Vec<u8>), AppError> {
        let requested = match size {
            "large" => "large",
            _ => "medium",
        };
        let mut candidates = vec![requested];
        if requested != "medium" {
            candidates.push("medium");
        }

        for candidate in candidates {
            let url = format!("{}/{candidate}/{id}.jpg", self.file_base);
            let response = self.http.get(url).send().await?;
            if !response.status().is_success() {
                continue;
            }
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("image/jpeg")
                .to_string();
            let bytes = response.bytes().await?;
            self.db.image_put(id, size, &content_type, &bytes);

            return Ok((content_type, bytes.to_vec()));
        }
        Err(AppError::NotFound)
    }

    /// Specification table scraped from the product page (empty when unknown).
    pub async fn specs(&self, slug: &str) -> Vec<Value> {
        let key = format!("specs|{slug}");
        if let Some(hit) = self.cache.get(&key) {
            return hit.as_array().cloned().unwrap_or_default();
        }

        let url = self.product_url(slug);
        let html = match self.http.get(&url).send().await {
            Ok(response) if response.status().is_success() => match response.text().await {
                Ok(text) => text,
                Err(err) => {
                    tracing::warn!(slug, %err, "product page body failed");
                    String::new()
                }
            },
            Ok(response) => {
                tracing::debug!(slug, status = %response.status(), "product page not readable");
                String::new()
            }
            Err(err) => {
                tracing::warn!(slug, %err, "product page fetch failed");
                String::new()
            }
        };

        let specs = if html.is_empty() {
            Vec::new()
        } else {
            scrape::extract_specs(&html)
        };
        let value = Arc::new(Value::Array(specs.clone()));
        self.cache.set(key, value, TTL_SPECS);
        specs
    }

    pub fn product_url(&self, slug: &str) -> String {
        format!("{}/{slug}", self.web_base)
    }
}

fn cache_key(path: &str, params: &[(String, String)]) -> String {
    let mut key = String::with_capacity(path.len() + params.len() * 16);
    key.push_str(path);
    for (k, v) in params {
        key.push('|');
        key.push_str(k);
        key.push('=');
        key.push_str(v);
    }
    key
}
