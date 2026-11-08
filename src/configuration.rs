//! The single reader of installed application values. Installation delivers the source files;
//! subsequent starts read them from disk, so valid operator edits need no binary rebuild.
use crate::IoError;
pub use nddev_browseruse_contract::{
    ActivitySettings, AdbSettings, ExecutorSettings, NativeSettings, PhoneLimits, RecorderSettings,
    Settings,
};
use std::path::Path;

/// Installed explicit native-messaging admission, separate from executor paths. Missing policy
/// retains the two existing identities; no wildcard or implicit catalog identity is accepted.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Admission {
    schema: u8,
    ids: Vec<String>,
}

fn extension_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| (b'a'..=b'p').contains(&b))
}

pub(crate) fn extension_ids() -> Result<Vec<String>, IoError> {
    let path = crate::install::config_dir()?.join("admission.json");
    let mut ids: Vec<String> = crate::EXTENSION_IDS.iter().map(|id| (*id).into()).collect();
    if present(&path)? {
        let metadata =
            std::fs::symlink_metadata(&path).map_err(|_| IoError::InvalidConfiguration)?;
        if metadata.len() > 4096 {
            return Err(IoError::InvalidConfiguration);
        }
        crate::device::private(&path, &metadata)?;
        let policy: Admission =
            serde_json::from_str(&file(&path)?).map_err(|_| IoError::InvalidConfiguration)?;
        if policy.schema != 1
            || policy.ids.len() > 8
            || !policy.ids.iter().all(|id| extension_id(id))
        {
            return Err(IoError::InvalidConfiguration);
        }
        ids.extend(policy.ids);
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

pub(crate) fn admission(additional: &[String]) -> Result<(Vec<String>, Vec<u8>), IoError> {
    if additional.len() > 8 || !additional.iter().all(|id| extension_id(id)) {
        return Err(IoError::InvalidConfiguration);
    }
    let mut ids = extension_ids()?;
    ids.extend_from_slice(additional);
    ids.sort();
    ids.dedup();
    if ids.len() > 8 {
        return Err(IoError::InvalidConfiguration);
    }
    let bytes = serde_json::to_vec_pretty(&Admission {
        schema: 1,
        ids: ids.clone(),
    })
    .map_err(|_| IoError::InvalidConfiguration)?;
    Ok((ids, bytes))
}

pub(crate) fn admitted(origin: &str) -> bool {
    let Some(id) = origin
        .strip_prefix("chrome-extension://")
        .and_then(|s| s.strip_suffix('/'))
    else {
        return false;
    };
    extension_id(id) && extension_ids().is_ok_and(|ids| ids.iter().any(|each| each == id))
}

fn file(path: &Path) -> Result<String, IoError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| IoError::InvalidConfiguration)?;
    if !metadata.is_file() || metadata.len() > 65_536 {
        return Err(IoError::InvalidConfiguration);
    }
    std::fs::read_to_string(path).map_err(|_| IoError::InvalidConfiguration)
}

fn present(path: &Path) -> Result<bool, IoError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(IoError::InvalidConfiguration),
    }
}

fn read(directory: &Path) -> Result<Settings, IoError> {
    Settings::parse(
        &file(&directory.join("common.toml"))?,
        &file(&directory.join("profile.toml"))?,
    )
    .map_err(|_| IoError::InvalidConfiguration)
}

pub fn load() -> Result<Settings, IoError> {
    read(&crate::install::config_dir()?)
}

/// Bootstrap commands can run before installation. Their defaults still come from the same
/// declared source files; a partially delivered or invalid installed configuration never falls back.
pub fn effective() -> Result<Settings, IoError> {
    let directory = crate::install::config_dir()?;
    let common = present(&directory.join("common.toml"))?;
    let profile = present(&directory.join("profile.toml"))?;
    if !common && !profile {
        Settings::parse(
            include_str!("../config/common.toml"),
            include_str!("../config/production.toml"),
        )
        .map_err(|_| IoError::InvalidConfiguration)
    } else {
        read(&directory)
    }
}

#[derive(Default)]
pub struct NativeOverrides {
    pub runtime_directory: Option<String>,
    pub default_device: Option<String>,
    pub default_phone: Option<String>,
    pub adb_server: Option<String>,
}

impl NativeOverrides {
    pub fn is_empty(&self) -> bool {
        self.runtime_directory.is_none()
            && self.default_device.is_none()
            && self.default_phone.is_none()
            && self.adb_server.is_none()
    }
}

/// Explicit local CLI configuration, applied only after the complete composition validates.
pub fn configure(values: &NativeOverrides) -> Result<(), IoError> {
    configure_in(&crate::install::config_dir()?, values)
}

fn configure_in(directory: &Path, values: &NativeOverrides) -> Result<(), IoError> {
    let common_path = directory.join("common.toml");
    let profile_path = directory.join("profile.toml");
    let has_common = present(&common_path)?;
    let has_profile = present(&profile_path)?;
    let original = if has_common {
        file(&common_path)?
    } else {
        include_str!("../config/common.toml").into()
    };
    let profile = if has_profile {
        file(&profile_path)?
    } else {
        include_str!("../config/production.toml").into()
    };
    Settings::parse(&original, &profile).map_err(|_| IoError::InvalidConfiguration)?;
    let mut common: toml::Value =
        toml::from_str(&original).map_err(|_| IoError::InvalidConfiguration)?;
    for (name, value) in [
        ("runtimeDirectory", &values.runtime_directory),
        ("defaultDevice", &values.default_device),
        ("defaultPhone", &values.default_phone),
    ] {
        if let Some(value) = value {
            common
                .get_mut("native")
                .and_then(toml::Value::as_table_mut)
                .ok_or(IoError::InvalidConfiguration)?
                .insert(name.into(), value.clone().into());
        }
    }
    if let Some(value) = &values.adb_server {
        common
            .get_mut("native")
            .and_then(|native| native.get_mut("adb"))
            .and_then(toml::Value::as_table_mut)
            .ok_or(IoError::InvalidConfiguration)?
            .insert("server".into(), value.clone().into());
    }
    let encoded = if values.is_empty() {
        original
    } else {
        toml::to_string_pretty(&common).map_err(|_| IoError::InvalidConfiguration)?
    };
    Settings::parse(&encoded, &profile).map_err(|_| IoError::InvalidConfiguration)?;
    crate::runtime::private_tree(directory)?;
    if !has_common || !values.is_empty() {
        crate::install::write_private(&common_path, encoded.as_bytes())?;
    }
    if !has_profile {
        crate::install::write_private(&profile_path, profile.as_bytes())?;
    }
    Ok(())
}

/// Legacy values are rejected rather than silently directing an action to another device.
pub fn reject_legacy_environment() -> Result<(), IoError> {
    for name in [
        "NDS_BROWSER_DEVICE",
        "NDS_BROWSER_PHONE",
        "NDS_BROWSER_USE_RUNTIME_DIR",
    ] {
        if std::env::var_os(name).is_some_and(|value| !value.is_empty()) {
            return Err(IoError::InvalidConfiguration);
        }
    }
    Ok(())
}

pub fn adb() -> Result<AdbSettings, IoError> {
    for name in ["ADB_SERVER_SOCKET", "ANDROID_ADB_SERVER_PORT"] {
        if std::env::var_os(name).is_some_and(|value| !value.is_empty()) {
            return Err(IoError::InvalidConfiguration);
        }
    }
    Ok(effective()?.native.adb)
}

pub(crate) fn devices() -> Result<crate::device::Devices, IoError> {
    let path = crate::install::config_dir()?.join("devices.json");
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) => {
            if metadata.len() > 1_048_576 {
                return Err(IoError::InvalidConfiguration);
            }
            crate::device::private(&path, &metadata)?;
            crate::device::Devices::parse(&std::fs::read(&path).map_err(|_| IoError::Unavailable)?)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(crate::device::Devices::empty())
        }
        Err(_) => Err(IoError::Unavailable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn private_fixture() -> tempfile::TempDir {
        #[cfg(windows)]
        let root = tempfile::tempdir_in(crate::runtime::local_app_data().unwrap()).unwrap();
        #[cfg(unix)]
        let root = {
            use std::os::unix::fs::PermissionsExt;
            let root = tempfile::tempdir().unwrap();
            std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
            root
        };
        root
    }

    #[test]
    fn reinstall_keeps_operator_values_and_completes_an_interrupted_delivery() {
        let directory = private_fixture();
        configure_in(directory.path(), &Default::default()).unwrap();
        let common = directory.path().join("common.toml");
        let edited = file(&common)
            .unwrap()
            .replace("queueMessages = 128", "queueMessages = 96");
        std::fs::write(&common, &edited).unwrap();
        std::fs::remove_file(directory.path().join("profile.toml")).unwrap();
        configure_in(directory.path(), &Default::default()).unwrap();
        assert_eq!(file(&common).unwrap(), edited);
        assert_eq!(
            read(directory.path()).unwrap().phone_link.queue_messages,
            96
        );
    }

    #[test]
    fn invalid_installed_values_stop_install_without_overwriting_them() {
        let directory = private_fixture();
        let common = directory.path().join("common.toml");
        std::fs::write(&common, "[phone_link]\nqueueMessages=0").unwrap();
        assert_eq!(
            configure_in(directory.path(), &Default::default()),
            Err(IoError::InvalidConfiguration)
        );
        assert_eq!(file(&common).unwrap(), "[phone_link]\nqueueMessages=0");
        assert!(!directory.path().join("profile.toml").exists());
    }

    #[test]
    fn invalid_destination_never_changes_or_delivers_configuration() {
        let directory = private_fixture();
        let values = NativeOverrides {
            default_device: Some("-wrong".into()),
            ..Default::default()
        };
        assert_eq!(
            configure_in(directory.path(), &values),
            Err(IoError::InvalidConfiguration)
        );
        assert!(!directory.path().join("common.toml").exists());
        configure_in(directory.path(), &Default::default()).unwrap();
        let before = file(&directory.path().join("common.toml")).unwrap();
        assert_eq!(
            configure_in(directory.path(), &values),
            Err(IoError::InvalidConfiguration)
        );
        assert_eq!(file(&directory.path().join("common.toml")).unwrap(), before);
    }

    #[test]
    fn explicit_configuration_preserves_other_operator_budgets() {
        let directory = private_fixture();
        let values = NativeOverrides {
            default_device: Some("lab".into()),
            adb_server: Some("127.0.0.1:15037".into()),
            ..Default::default()
        };
        configure_in(directory.path(), &values).unwrap();
        let settings = read(directory.path()).unwrap();
        assert_eq!(settings.native.default_device, "lab");
        assert_eq!(settings.native.adb.server, "127.0.0.1:15037");
        assert_eq!(settings.executor.recorder.frame_interval_ms, 500);
        let conflicting = NativeOverrides {
            default_phone: Some("phone-1".into()),
            ..Default::default()
        };
        assert_eq!(
            configure_in(directory.path(), &conflicting),
            Err(IoError::InvalidConfiguration)
        );
        assert_eq!(read(directory.path()).unwrap().native.default_device, "lab");
    }

    #[test]
    fn profile_owned_native_settings_are_refused_without_panicking_or_rewriting() {
        let directory = private_fixture();
        let mut common: toml::Value =
            toml::from_str(include_str!("../config/common.toml")).unwrap();
        let native = common.as_table_mut().unwrap().remove("native").unwrap();
        let mut profile = toml::map::Map::new();
        profile.insert("native".into(), native);
        let common = toml::to_string(&common).unwrap();
        let profile = toml::to_string(&profile).unwrap();
        std::fs::write(directory.path().join("common.toml"), &common).unwrap();
        std::fs::write(directory.path().join("profile.toml"), &profile).unwrap();
        assert!(read(directory.path()).is_ok());
        let values = NativeOverrides {
            default_device: Some("lab".into()),
            ..Default::default()
        };
        assert_eq!(
            configure_in(directory.path(), &values),
            Err(IoError::InvalidConfiguration)
        );
        assert_eq!(file(&directory.path().join("common.toml")).unwrap(), common);
        assert_eq!(
            file(&directory.path().join("profile.toml")).unwrap(),
            profile
        );
    }
}
