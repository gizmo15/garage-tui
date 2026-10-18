use futures::future::try_join_all;
use serde::de::DeserializeOwned;
use thiserror::Error;

use crate::model::{
    BucketInfo, BucketSummary, ClusterStatus, KeyInfo, KeySummary, Snapshot, TokenInfo,
};

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("{url}: HTTP {status}{}", format_detail(.detail))]
    Status {
        url: String,
        status: u16,
        detail: String,
    },
}

fn format_detail(detail: &str) -> String {
    if detail.is_empty() {
        String::new()
    } else {
        format!(" - {detail}")
    }
}

/// Extracts the message of a Garage error body (`{"code": .., "message": ..}`),
/// falling back to the trimmed raw body, truncated.
fn error_detail(body: &str) -> String {
    let message = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("message")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| body.trim().to_string());
    message.chars().take(200).collect()
}

#[derive(Clone)]
pub struct GarageClient {
    http: reqwest::Client,
    base: String,
    token: String,
}

impl GarageClient {
    pub fn new(base_url: &str, token: &str) -> Result<Self, ClientError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        Ok(Self {
            http,
            base: base_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
        })
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, ClientError> {
        let url = format!("{}{}", self.base, path);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&self.token)
            .query(query)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ClientError::Status {
                url: path.to_string(),
                status,
                detail: error_detail(&body),
            });
        }
        Ok(resp.json().await?)
    }

    pub async fn list_buckets(&self) -> Result<Vec<BucketSummary>, ClientError> {
        self.get("/v2/ListBuckets", &[]).await
    }

    pub async fn bucket_info(&self, id: &str) -> Result<BucketInfo, ClientError> {
        self.get("/v2/GetBucketInfo", &[("id", id)]).await
    }

    pub async fn list_keys(&self) -> Result<Vec<KeySummary>, ClientError> {
        self.get("/v2/ListKeys", &[]).await
    }

    pub async fn key_info(&self, id: &str, show_secret: bool) -> Result<KeyInfo, ClientError> {
        let secret = if show_secret { "true" } else { "false" };
        self.get("/v2/GetKeyInfo", &[("id", id), ("showSecretKey", secret)])
            .await
    }

    pub async fn cluster_status(&self) -> Result<ClusterStatus, ClientError> {
        self.get("/v2/GetClusterStatus", &[]).await
    }

    pub async fn token_info(&self) -> Result<TokenInfo, ClientError> {
        self.get("/v2/GetCurrentAdminTokenInfo", &[]).await
    }

    pub async fn snapshot(&self, show_secret: bool) -> Result<Snapshot, ClientError> {
        let (buckets, keys, cluster) = tokio::try_join!(
            async {
                let list = self.list_buckets().await?;
                try_join_all(list.iter().map(|b| self.bucket_info(&b.id))).await
            },
            async {
                let list = self.list_keys().await?;
                try_join_all(list.iter().map(|k| self.key_info(&k.id, show_secret))).await
            },
            self.cluster_status()
        )?;
        Ok(Snapshot {
            buckets,
            keys,
            cluster,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_json_message() {
        let body = r#"{"code":"Forbidden","message":"Invalid bearer token","region":"garage"}"#;
        assert_eq!(error_detail(body), "Invalid bearer token");
    }

    #[test]
    fn falls_back_to_raw_body_and_truncates() {
        assert_eq!(error_detail("  oops\n"), "oops");
        assert_eq!(error_detail(&"x".repeat(500)).len(), 200);
        assert_eq!(error_detail(""), "");
    }

    #[test]
    fn formats_status_error() {
        let err = ClientError::Status {
            url: "/v2/ListBuckets".into(),
            status: 403,
            detail: "Invalid bearer token".into(),
        };
        assert_eq!(
            err.to_string(),
            "/v2/ListBuckets: HTTP 403 - Invalid bearer token"
        );
        let err = ClientError::Status {
            url: "/x".into(),
            status: 500,
            detail: String::new(),
        };
        assert_eq!(err.to_string(), "/x: HTTP 500");
    }

    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn json(body: &str) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_raw(body.to_string(), "application/json")
    }

    async fn mount(server: &MockServer, route: &str, body: &str) {
        Mock::given(method("GET"))
            .and(path(route))
            .and(header("authorization", "Bearer tok"))
            .respond_with(json(body))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn token_info_is_parsed() {
        let server = MockServer::start().await;
        mount(
            &server,
            "/v2/GetCurrentAdminTokenInfo",
            r#"{"id":"t1","name":"ro","expired":false,"scope":["ListBuckets"]}"#,
        )
        .await;
        let client = GarageClient::new(&server.uri(), "tok").unwrap();
        let info = client.token_info().await.unwrap();
        assert_eq!(info.name.as_deref(), Some("ro"));
        assert_eq!(info.label(), "read-only");
    }

    #[tokio::test]
    async fn token_info_unsupported_is_an_error() {
        let server = MockServer::start().await;
        let client = GarageClient::new(&server.uri(), "tok").unwrap();
        let err = client.token_info().await.unwrap_err();
        assert!(matches!(err, ClientError::Status { status: 404, .. }));
    }

    #[tokio::test]
    async fn snapshot_fetches_everything() {
        let server = MockServer::start().await;
        mount(&server, "/v2/ListBuckets", r#"[{"id":"b1"},{"id":"b2"}]"#).await;
        for (id, alias) in [("b1", "one"), ("b2", "two")] {
            Mock::given(method("GET"))
                .and(path("/v2/GetBucketInfo"))
                .and(query_param("id", id))
                .respond_with(json(&format!(
                    r#"{{"id":"{id}","globalAliases":["{alias}"],"objects":1,"bytes":10}}"#
                )))
                .mount(&server)
                .await;
        }
        mount(&server, "/v2/ListKeys", r#"[{"id":"GK1"}]"#).await;
        Mock::given(method("GET"))
            .and(path("/v2/GetKeyInfo"))
            .and(query_param("id", "GK1"))
            .and(query_param("showSecretKey", "true"))
            .respond_with(json(
                r#"{"accessKeyId":"GK1","name":"k","secretAccessKey":"sek","buckets":[]}"#,
            ))
            .mount(&server)
            .await;
        mount(
            &server,
            "/v2/GetClusterStatus",
            r#"{"layoutVersion":2,"nodes":[]}"#,
        )
        .await;

        // trailing slash in the base url must be tolerated
        let client = GarageClient::new(&format!("{}/", server.uri()), "tok").unwrap();
        let snap = client.snapshot(true).await.unwrap();
        assert_eq!(snap.buckets.len(), 2);
        assert_eq!(snap.buckets[1].name(), "two");
        assert_eq!(snap.keys[0].secret_access_key.as_deref(), Some("sek"));
        assert_eq!(snap.cluster.layout_version, 2);
    }

    #[tokio::test]
    async fn key_info_hides_secret_by_default() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/GetKeyInfo"))
            .and(query_param("showSecretKey", "false"))
            .respond_with(json(r#"{"accessKeyId":"GK1","name":"k"}"#))
            .mount(&server)
            .await;
        let client = GarageClient::new(&server.uri(), "tok").unwrap();
        let key = client.key_info("GK1", false).await.unwrap();
        assert!(key.secret_access_key.is_none());
    }

    #[tokio::test]
    async fn http_error_carries_garage_message() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/ListBuckets"))
            .respond_with(ResponseTemplate::new(403).set_body_raw(
                r#"{"code":"Forbidden","message":"Invalid bearer token"}"#,
                "application/json",
            ))
            .mount(&server)
            .await;
        let client = GarageClient::new(&server.uri(), "bad").unwrap();
        let err = client.list_buckets().await.unwrap_err();
        assert_eq!(
            err.to_string(),
            "/v2/ListBuckets: HTTP 403 - Invalid bearer token"
        );
        assert!(client.snapshot(false).await.is_err());
    }

    #[tokio::test]
    async fn unreachable_server_is_an_http_error() {
        let client = GarageClient::new("http://127.0.0.1:1", "tok").unwrap();
        let err = client.cluster_status().await.unwrap_err();
        assert!(matches!(err, ClientError::Http(_)));
        assert!(err.to_string().starts_with("request failed"));
    }
}
