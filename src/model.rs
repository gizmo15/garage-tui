use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Quotas {
    pub max_size: Option<u64>,
    pub max_objects: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default)]
pub struct Perms {
    pub read: bool,
    pub write: bool,
    pub owner: bool,
}

impl Perms {
    pub fn label(&self) -> String {
        format!(
            "{}{}{}",
            if self.read { 'R' } else { '-' },
            if self.write { 'W' } else { '-' },
            if self.owner { 'O' } else { '-' }
        )
    }
}

/// Size-quota usage above which a bucket is highlighted.
pub const QUOTA_WARN_PERCENT: f64 = 85.0;

/// Endpoints used by the app, all read-only.
const READ_ONLY_SCOPE: &[&str] = &[
    "ListBuckets",
    "GetBucketInfo",
    "ListKeys",
    "GetKeyInfo",
    "GetClusterStatus",
    "GetCurrentAdminTokenInfo",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Admin,
    ReadOnly,
    Custom,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TokenInfo {
    pub name: Option<String>,
    pub expiration: Option<String>,
    pub expired: bool,
    pub scope: Vec<String>,
}

impl TokenInfo {
    pub fn kind(&self) -> TokenKind {
        if self.scope.iter().any(|s| s == "*") {
            TokenKind::Admin
        } else if !self.scope.is_empty()
            && self
                .scope
                .iter()
                .all(|s| s.starts_with("List") || s.starts_with("Get"))
        {
            TokenKind::ReadOnly
        } else {
            TokenKind::Custom
        }
    }

    pub fn label(&self) -> String {
        if self.expired {
            return "expired".to_string();
        }
        match self.kind() {
            TokenKind::Admin => "admin".to_string(),
            TokenKind::ReadOnly => "read-only".to_string(),
            TokenKind::Custom => format!("custom: {} endpoints", self.scope.len()),
        }
    }

    /// True when the token grants more than the app needs.
    pub fn is_excessive(&self) -> bool {
        self.scope
            .iter()
            .any(|s| s == "*" || !READ_ONLY_SCOPE.contains(&s.as_str()))
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BucketSummary {
    pub id: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BucketKey {
    pub access_key_id: String,
    pub name: String,
    pub permissions: Perms,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BucketInfo {
    pub id: String,
    pub created: Option<String>,
    pub global_aliases: Vec<String>,
    pub keys: Vec<BucketKey>,
    pub objects: u64,
    pub bytes: u64,
    pub unfinished_uploads: u64,
    pub quotas: Quotas,
}

impl BucketInfo {
    pub fn name(&self) -> String {
        self.global_aliases
            .first()
            .cloned()
            .unwrap_or_else(|| self.id.chars().take(12).collect())
    }

    /// True when a size quota is set and more than `QUOTA_WARN_PERCENT` of it is used.
    pub fn quota_nearly_full(&self) -> bool {
        self.quota_percent().is_some_and(|p| p > QUOTA_WARN_PERCENT)
    }

    /// Percentage of the size quota used, if a size quota is set.
    pub fn quota_percent(&self) -> Option<f64> {
        self.quotas
            .max_size
            .filter(|max| *max > 0)
            .map(|max| self.bytes as f64 * 100.0 / max as f64)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeySummary {
    pub id: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct KeyBucket {
    pub id: String,
    pub global_aliases: Vec<String>,
    pub permissions: Perms,
}

impl KeyBucket {
    pub fn name(&self) -> String {
        self.global_aliases
            .first()
            .cloned()
            .unwrap_or_else(|| self.id.chars().take(12).collect())
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct KeyPermissions {
    pub create_bucket: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct KeyInfo {
    pub access_key_id: String,
    pub name: String,
    pub created: Option<String>,
    pub expiration: Option<String>,
    pub expired: bool,
    pub secret_access_key: Option<String>,
    pub permissions: KeyPermissions,
    pub buckets: Vec<KeyBucket>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Partition {
    pub available: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct NodeRole {
    pub zone: String,
    pub capacity: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Node {
    pub id: String,
    pub hostname: Option<String>,
    pub addr: Option<String>,
    pub is_up: bool,
    pub draining: bool,
    pub role: Option<NodeRole>,
    pub data_partition: Option<Partition>,
    pub metadata_partition: Option<Partition>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClusterStatus {
    pub layout_version: u64,
    pub nodes: Vec<Node>,
}

/// Everything displayed by the UI, fetched in one refresh.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub buckets: Vec<BucketInfo>,
    pub keys: Vec<KeyInfo>,
    pub cluster: ClusterStatus,
}

pub fn human_bytes(bytes: u64) -> String {
    humansize::format_size(bytes, humansize::BINARY)
}

/// Snapshot shared by the unit tests of app and ui.
#[cfg(test)]
pub fn sample_snapshot() -> Snapshot {
    let buckets = serde_json::json!([
        {"id": "b1b1b1b1b1b1b1b1", "created": "2026-01-01T00:00:00Z", "globalAliases": ["photos"],
         "keys": [{"accessKeyId": "GK1", "name": "backup", "permissions": {"read": true, "write": true, "owner": true}}],
         "objects": 42, "bytes": 1073741824u64, "unfinishedUploads": 1,
         "quotas": {"maxSize": 2147483648u64, "maxObjects": 100}},
        {"id": "b2b2b2b2b2b2b2b2", "globalAliases": ["archives"], "keys": [],
         "objects": 1000, "bytes": 1024, "quotas": {}},
        {"id": "b3b3b3b3b3b3b3b3", "globalAliases": [], "keys": [],
         "objects": 7, "bytes": 5000000, "quotas": {}}
    ]);
    let keys = serde_json::json!([
        {"accessKeyId": "GK1", "name": "backup", "created": "2026-01-02T00:00:00Z",
         "secretAccessKey": "s3cr3t", "permissions": {"createBucket": true},
         "buckets": [{"id": "b1b1b1b1b1b1b1b1", "globalAliases": ["photos"], "permissions": {"read": true, "write": false, "owner": false}},
                     {"id": "b3b3b3b3b3b3b3b3", "globalAliases": [], "permissions": {"read": true, "write": true, "owner": false}}]},
        {"accessKeyId": "GK2", "name": "readonly", "expired": true, "expiration": "2026-02-01T00:00:00Z", "buckets": []}
    ]);
    let cluster = serde_json::json!({
        "layoutVersion": 3,
        "nodes": [
            {"id": "aaaaaaaaaaaaaaaa", "isUp": true, "hostname": "node1", "role": {"zone": "dc1", "capacity": 1000},
             "dataPartition": {"available": 400, "total": 1000}},
            {"id": "bbbbbbbbbbbbbbbb", "isUp": true, "draining": true, "hostname": "node2", "role": {"zone": "dc2"},
             "dataPartition": {"available": 50, "total": 1000}},
            {"id": "cccccccccccccccc", "isUp": true, "role": {"zone": "dc3"},
             "dataPartition": {"available": 200, "total": 1000}},
            {"id": "dddddddddddddddd", "isUp": false}
        ]
    });
    Snapshot {
        buckets: serde_json::from_value(buckets).unwrap(),
        keys: serde_json::from_value(keys).unwrap(),
        cluster: serde_json::from_value(cluster).unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bucket_info_fixture() {
        let info: BucketInfo =
            serde_json::from_str(include_str!("../tests/fixtures/bucket_info.json")).unwrap();
        assert_eq!(info.name(), "photos");
        assert_eq!(info.objects, 42);
        assert_eq!(info.bytes, 1_073_741_824);
        assert_eq!(info.keys[0].permissions.label(), "RWO");
        assert_eq!(info.quota_percent(), Some(50.0));
    }

    #[test]
    fn parses_cluster_fixture() {
        let status: ClusterStatus =
            serde_json::from_str(include_str!("../tests/fixtures/cluster_status.json")).unwrap();
        assert_eq!(status.nodes.len(), 2);
        assert!(status.nodes[1].role.is_none());
        assert_eq!(status.nodes[0].data_partition.as_ref().unwrap().total, 1000);
    }

    #[test]
    fn names_fall_back_to_short_id() {
        let snap = sample_snapshot();
        assert_eq!(snap.buckets[2].name(), "b3b3b3b3b3b3");
        assert_eq!(snap.keys[0].buckets[1].name(), "b3b3b3b3b3b3");
        assert_eq!(snap.keys[0].buckets[0].name(), "photos");
        assert_eq!(snap.buckets[1].quota_percent(), None);
        assert_eq!(Perms::default().label(), "---");
    }

    #[test]
    fn formats_sizes() {
        assert_eq!(human_bytes(1024), "1 KiB");
    }

    fn token(scope: &[&str]) -> TokenInfo {
        TokenInfo {
            scope: scope.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn token_kind_from_scope() {
        assert_eq!(token(&["*"]).kind(), TokenKind::Admin);
        assert_eq!(
            token(&["ListBuckets", "GetKeyInfo"]).kind(),
            TokenKind::ReadOnly
        );
        assert_eq!(
            token(&["ListBuckets", "CreateBucket"]).kind(),
            TokenKind::Custom
        );
        assert_eq!(token(&[]).kind(), TokenKind::Custom);
        assert_eq!(token(&["ListBuckets"]).label(), "read-only");
        assert_eq!(token(&["*"]).label(), "admin");
        assert!(token(&["*"]).is_excessive());
        assert!(!token(READ_ONLY_SCOPE).is_excessive());
        let expired: TokenInfo = serde_json::from_str(r#"{"expired":true,"scope":["*"]}"#).unwrap();
        assert_eq!(expired.label(), "expired");
    }

    #[test]
    fn quota_warning_above_threshold_only() {
        let bucket = |bytes: u64, max_size: Option<u64>| BucketInfo {
            bytes,
            quotas: Quotas {
                max_size,
                max_objects: None,
            },
            ..Default::default()
        };
        assert!(!bucket(85, Some(100)).quota_nearly_full());
        assert!(bucket(86, Some(100)).quota_nearly_full());
        assert!(!bucket(1000, None).quota_nearly_full());
        assert!(!bucket(1000, Some(0)).quota_nearly_full());
    }
}
