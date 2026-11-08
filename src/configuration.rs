//! The single reader of installed application values. Installation delivers the source files;
//! subsequent starts read them from disk, so valid operator edits need no binary rebuild.
use crate::IoError;
pub use nddev_browseruse_contract::{
    ActivitySettings, ExecutorSettings, PhoneLimits, RecorderSettings, Settings,
};
use std::path::Path;

fn file(path: &Path) -> Result<String, IoError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| IoError::InvalidConfiguration)?;
    if !metadata.is_file() || metadata.len() > 65_536 {
        return Err(IoError::InvalidConfiguration);
    }
    std::fs::read_to_string(path).map_err(|_| IoError::InvalidConfiguration)
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

pub(crate) fn install(directory: &Path) -> Result<(), IoError> {
    let common_path = directory.join("common.toml");
    let profile_path = directory.join("profile.toml");
    let has_common = common_path.try_exists().map_err(|_| IoError::Unavailable)?;
    let has_profile = profile_path
        .try_exists()
        .map_err(|_| IoError::Unavailable)?;
    let common = if has_common {
        file(&common_path)?
    } else {
        include_str!("../config/common.toml").into()
    };
    let profile = if has_profile {
        file(&profile_path)?
    } else {
        include_str!("../config/production.toml").into()
    };
    Settings::parse(&common, &profile).map_err(|_| IoError::InvalidConfiguration)?;
    if !has_common {
        crate::install::write_private(&common_path, common.as_bytes())?;
    }
    if !has_profile {
        crate::install::write_private(&profile_path, profile.as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reinstall_keeps_operator_values_and_completes_an_interrupted_delivery() {
        let directory = tempfile::tempdir().unwrap();
        install(directory.path()).unwrap();
        let common = directory.path().join("common.toml");
        let edited = file(&common)
            .unwrap()
            .replace("queueMessages = 128", "queueMessages = 96");
        std::fs::write(&common, &edited).unwrap();
        std::fs::remove_file(directory.path().join("profile.toml")).unwrap();
        install(directory.path()).unwrap();
        assert_eq!(file(&common).unwrap(), edited);
        assert_eq!(
            read(directory.path()).unwrap().phone_link.queue_messages,
            96
        );
    }

    #[test]
    fn invalid_installed_values_stop_install_without_overwriting_them() {
        let directory = tempfile::tempdir().unwrap();
        let common = directory.path().join("common.toml");
        std::fs::write(&common, "[phone_link]\nqueueMessages=0").unwrap();
        assert_eq!(
            install(directory.path()),
            Err(IoError::InvalidConfiguration)
        );
        assert_eq!(file(&common).unwrap(), "[phone_link]\nqueueMessages=0");
        assert!(!directory.path().join("profile.toml").exists());
    }
}
