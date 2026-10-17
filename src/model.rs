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
    fn formats_sizes() {
        assert_eq!(human_bytes(1024), "1 KiB");
    }
}
