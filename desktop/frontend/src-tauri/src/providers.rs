// Spend-ledger provider registry. Non-secret settings live in
// `<lpm_dir>/providers.json`; credentials go through `secrets` (the login
// Keychain on macOS) under the account `provider:<id>` and never reach that
// file or any frontend response.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::fsatomic::{self, Mode};
use crate::secrets;

pub const KINDS: &[&str] = &["hermes", "vertex", "devpass", "reported"];
const FILE_NAME: &str = "providers.json";
const MAX_PROVIDERS: usize = 32;
const MAX_ID_LEN: usize = 40;
const MAX_LABEL_LEN: usize = 80;
const MAX_MODELS: usize = 50;
const MAX_MODEL_LEN: usize = 120;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ProviderRecord {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub enabled: bool,
    pub model_allowlist: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    #[serde(flatten)]
    pub record: ProviderRecord,
    pub has_credentials: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ProviderFile {
    providers: Vec<ProviderRecord>,
}

pub fn account(id: &str) -> String {
    format!("provider:{id}")
}

pub struct ProviderStore {
    path: PathBuf,
}

impl ProviderStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn list(&self) -> Result<Vec<ProviderRecord>, String> {
        match std::fs::read(&self.path) {
            Ok(bytes) => {
                let file: ProviderFile = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("providers.json is unreadable: {e}"))?;
                Ok(file.providers)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn upsert(&self, record: ProviderRecord) -> Result<ProviderRecord, String> {
        let record = normalize(record)?;
        let mut providers = self.list()?;
        match providers.iter_mut().find(|p| p.id == record.id) {
            Some(existing) => *existing = record.clone(),
            None => {
                if providers.len() >= MAX_PROVIDERS {
                    return Err(format!("at most {MAX_PROVIDERS} providers can be configured"));
                }
                providers.push(record.clone());
            }
        }
        self.write(&providers)?;
        Ok(record)
    }

    pub fn remove(&self, id: &str) -> Result<bool, String> {
        let mut providers = self.list()?;
        let before = providers.len();
        providers.retain(|p| p.id != id);
        if providers.len() == before {
            return Ok(false);
        }
        self.write(&providers)?;
        Ok(true)
    }

    fn write(&self, providers: &[ProviderRecord]) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let bytes = serde_json::to_vec_pretty(&ProviderFile {
            providers: providers.to_vec(),
        })
        .map_err(|e| e.to_string())?;
        fsatomic::write(&self.path, &bytes, Mode::Exact(0o600)).map_err(|e| e.to_string())
    }

    #[cfg(test)]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

fn normalize(mut r: ProviderRecord) -> Result<ProviderRecord, String> {
    r.id = r.id.trim().to_string();
    if r.id.is_empty() || r.id.len() > MAX_ID_LEN {
        return Err(format!("provider id must be 1-{MAX_ID_LEN} characters"));
    }
    if !r
        .id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        || r.id.starts_with('-')
    {
        return Err("provider id may use only a-z, 0-9 and '-'".into());
    }
    r.label = r.label.trim().to_string();
    if r.label.is_empty() || r.label.len() > MAX_LABEL_LEN {
        return Err(format!("provider label must be 1-{MAX_LABEL_LEN} characters"));
    }
    if !KINDS.contains(&r.kind.as_str()) {
        return Err(format!("unknown provider kind '{}'", r.kind));
    }
    let mut models: Vec<String> = Vec::new();
    for m in r.model_allowlist.iter().map(|m| m.trim()) {
        if m.is_empty() {
            continue;
        }
        if m.len() > MAX_MODEL_LEN {
            return Err(format!("model names are limited to {MAX_MODEL_LEN} characters"));
        }
        if !models.iter().any(|x| x == m) {
            models.push(m.to_string());
        }
    }
    if models.len() > MAX_MODELS {
        return Err(format!("at most {MAX_MODELS} models per provider"));
    }
    r.model_allowlist = models;
    Ok(r)
}

fn store() -> ProviderStore {
    ProviderStore::new(crate::config::lpm_dir().join(FILE_NAME))
}

pub fn accepted_provider_names(provider: &str, model: &str) -> Result<Vec<String>, String> {
    accepted_names(&store().list()?, provider, model)
}

fn accepted_names(records: &[ProviderRecord], provider: &str, model: &str) -> Result<Vec<String>, String> {
    let matching: Vec<_> = records.iter().filter(|p| {
        if p.kind == "reported" { p.id == provider } else { p.kind == provider }
    }).collect();
    if matching.is_empty() && provider == "hermes" {
        return Ok(vec![provider.to_string()]);
    }
    if matching.iter().any(|p| p.enabled &&
        (p.model_allowlist.is_empty() || p.model_allowlist.iter().any(|m| m == model))) {
        Ok(vec![provider.to_string()])
    } else {
        Err("Provider is disabled, unconfigured, or does not allow this model".into())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub id: String,
    pub readiness: crate::provider_connectors::Readiness,
    pub credential_hint: &'static str,
    pub env: &'static [&'static str],
}

#[tauri::command]
pub fn provider_readiness() -> Result<Vec<ProviderStatus>, String> {
    Ok(store()
        .list()?
        .into_iter()
        .map(|p| {
            let has = secrets::has(&account(&p.id));
            let meta = crate::provider_connectors::connector(&p.kind);
            ProviderStatus {
                readiness: crate::provider_connectors::readiness(&p, has),
                credential_hint: meta.map_or("", |c| c.credential_hint),
                env: meta.map_or(&[], |c| c.env),
                id: p.id,
            }
        })
        .collect())
}

fn view(record: ProviderRecord) -> ProviderView {
    let has_credentials = secrets::has(&account(&record.id));
    ProviderView {
        record,
        has_credentials,
    }
}

#[tauri::command]
pub fn list_providers() -> Result<Vec<ProviderView>, String> {
    Ok(store().list()?.into_iter().map(view).collect())
}

#[tauri::command]
pub fn save_provider(
    record: ProviderRecord,
    credential: Option<String>,
) -> Result<ProviderView, String> {
    let saved = store().upsert(record)?;
    if let Some(value) = credential {
        secrets::set(&account(&saved.id), &value)?;
    }
    Ok(view(saved))
}

#[tauri::command]
pub fn remove_provider(id: String) -> Result<(), String> {
    let store = store();
    let id = id.trim();
    let existed = store.remove(id)?;
    if existed {
        secrets::delete(&account(id))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str) -> ProviderRecord {
        ProviderRecord {
            id: id.into(),
            label: "Vertex (Konton)".into(),
            kind: "vertex".into(),
            enabled: true,
            model_allowlist: vec!["google/gemini-3.8-flash".into()],
        }
    }

    fn store_in(dir: &tempfile::TempDir) -> ProviderStore {
        ProviderStore::new(dir.path().join("providers.json"))
    }

    #[test]
    fn ingestion_respects_enabled_models_and_custom_provider_ids() {
        let mut p = record("vertex-main");
        let model = p.model_allowlist[0].clone();
        assert!(accepted_names(&[p.clone()], "vertex", &model).is_ok());
        assert!(accepted_names(&[p.clone()], "vertex", "other").is_err());
        p.enabled = false;
        assert!(accepted_names(&[p.clone()], "vertex", &model).is_err());
        p.kind = "hermes".into();
        assert!(accepted_names(&[p], "hermes", &model).is_err());
        assert!(accepted_names(&[], "hermes", "any").is_ok());
        let p = ProviderRecord { kind: "reported".into(), model_allowlist: vec![], ..record("my-provider") };
        assert!(accepted_names(&[p.clone()], "my-provider", "any").is_ok());
        assert!(accepted_names(&[p], "unconfigured", "any").is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "writes a temporary test credential to the macOS Keychain"]
    fn native_provider_keychain_round_trip() {
        let account = account(&format!("astra-qa-{}", uuid::Uuid::new_v4()));
        let value = uuid::Uuid::new_v4().to_string();
        secrets::set(&account, &value).unwrap();
        let actual = secrets::get(&account);
        let present = secrets::has(&account);
        let deleted = secrets::delete(&account);
        assert!(deleted.is_ok());
        assert!(present);
        assert!(actual.unwrap().as_deref() == Some(value.as_str()));
        assert!(!secrets::has(&account));
    }

    #[test]
    fn empty_when_file_absent() {
        let dir = tempfile::tempdir().unwrap();
        assert!(store_in(&dir).list().unwrap().is_empty());
    }

    #[test]
    fn upsert_round_trips_and_replaces_by_id() {
        let dir = tempfile::tempdir().unwrap();
        let s = store_in(&dir);
        s.upsert(record("vertex-main")).unwrap();
        let mut changed = record("vertex-main");
        changed.enabled = false;
        s.upsert(changed.clone()).unwrap();
        let list = s.list().unwrap();
        assert_eq!(list, vec![changed]);
    }

    #[test]
    fn rejects_bad_id_kind_and_empty_label() {
        let dir = tempfile::tempdir().unwrap();
        let s = store_in(&dir);
        assert!(s.upsert(ProviderRecord { id: "Bad Id".into(), ..record("x") }).is_err());
        assert!(s.upsert(ProviderRecord { id: "-lead".into(), ..record("x") }).is_err());
        assert!(s.upsert(ProviderRecord { kind: "openai".into(), ..record("x") }).is_err());
        assert!(s.upsert(ProviderRecord { label: "   ".into(), ..record("x") }).is_err());
        assert!(s.list().unwrap().is_empty());
    }

    #[test]
    fn dedupes_and_trims_models() {
        let dir = tempfile::tempdir().unwrap();
        let s = store_in(&dir);
        let saved = s
            .upsert(ProviderRecord {
                model_allowlist: vec![" a ".into(), "a".into(), "".into(), "b".into()],
                ..record("p")
            })
            .unwrap();
        assert_eq!(saved.model_allowlist, vec!["a", "b"]);
    }

    #[test]
    fn enforces_provider_cap() {
        let dir = tempfile::tempdir().unwrap();
        let s = store_in(&dir);
        for i in 0..MAX_PROVIDERS {
            s.upsert(record(&format!("p{i}"))).unwrap();
        }
        assert!(s.upsert(record("overflow")).is_err());
    }

    #[test]
    fn remove_reports_presence() {
        let dir = tempfile::tempdir().unwrap();
        let s = store_in(&dir);
        s.upsert(record("p")).unwrap();
        assert!(s.remove("p").unwrap());
        assert!(!s.remove("p").unwrap());
    }

    #[test]
    fn file_never_contains_credentials_field() {
        let dir = tempfile::tempdir().unwrap();
        let s = store_in(&dir);
        s.upsert(record("p")).unwrap();
        let text = std::fs::read_to_string(s.path()).unwrap();
        assert!(!text.contains("credential"));
        assert!(!text.contains("token"));
    }

    #[test]
    fn view_serialises_camel_case_with_flat_fields() {
        let view = ProviderView {
            record: record("vertex-main"),
            has_credentials: true,
        };
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["id"], "vertex-main");
        assert_eq!(json["kind"], "vertex");
        assert_eq!(json["modelAllowlist"][0], "google/gemini-3.8-flash");
        assert_eq!(json["hasCredentials"], true);
        assert!(json.get("record").is_none());
        assert!(json.get("credential").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let s = store_in(&dir);
        s.upsert(record("p")).unwrap();
        let mode = std::fs::metadata(s.path()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
