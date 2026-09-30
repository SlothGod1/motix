//! What the UI needs to know about software updates. The checking, downloading and
//! verifying happen in `motix-update`, driven by the host application; this module only
//! describes the state so the UI can show it.

/// Where the updater is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdatePhase {
    /// Updates are switched off for this build (for example a developer build).
    Disabled {
        /// Why, in plain language.
        reason: String,
    },
    /// Nothing has happened yet.
    Idle,
    /// Asking GitHub whether there is a newer version.
    Checking,
    /// The newest version is installed.
    UpToDate,
    /// Downloading a newer version in the background.
    Downloading {
        /// Version being downloaded.
        version: String,
        /// Bytes received.
        done: u64,
        /// Total size, when known.
        total: Option<u64>,
    },
    /// Downloaded and verified; installs on restart.
    Ready {
        /// Version ready to install.
        version: String,
        /// Release notes (plain text, possibly empty).
        notes: String,
    },
    /// Already installed in the shared folder (by this PC or another); restarting
    /// MOTIX switches to it.
    Installed {
        /// Version installed.
        version: String,
        /// Release notes (plain text, possibly empty).
        notes: String,
    },
    /// Replacing the program files.
    Installing,
    /// Something went wrong; the current version keeps working.
    Failed {
        /// What happened, in plain language.
        message: String,
    },
}

/// Sharing MOTIX with other PCs on the network (Help > Share MOTIX on your network…).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SharingInfo {
    /// The shared folder this copy runs from, if it runs from one.
    pub running_from: Option<String>,
    /// The result of the last "share" attempt: the folder, or what went wrong.
    pub last_result: Option<Result<String, String>>,
    /// This PC's network name (for the instructions), when known.
    pub pc_name: Option<String>,
}

/// Update information shown in the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateInfo {
    /// The running version, e.g. `"0.1.0-preview.14"`.
    pub current_version: String,
    /// Current phase.
    pub phase: UpdatePhase,
    /// Whether MOTIX checks by itself (at start-up and every 10 minutes).
    pub auto_check: bool,
    /// When the last check finished, as text for display (e.g. `"14:05"`).
    pub last_checked: Option<String>,
    /// The ready update will be installed when MOTIX closes.
    pub install_on_exit: bool,
    /// Where updates come from, in plain language.
    pub source_text: String,
    /// Network sharing.
    pub sharing: SharingInfo,
}

impl Default for UpdateInfo {
    fn default() -> Self {
        Self {
            current_version: env!("CARGO_PKG_VERSION").to_owned(),
            phase: UpdatePhase::Idle,
            auto_check: true,
            last_checked: None,
            install_on_exit: false,
            source_text: String::new(),
            sharing: SharingInfo::default(),
        }
    }
}

impl UpdateInfo {
    /// One-line summary for the status bar, when there's something worth showing.
    #[must_use]
    pub fn status_line(&self) -> Option<String> {
        match &self.phase {
            UpdatePhase::Downloading { version, done, total } => Some(match total {
                Some(t) if *t > 0 => format!("Downloading update {version}: {}%", done.saturating_mul(100) / t),
                _ => format!("Downloading update {version}…"),
            }),
            UpdatePhase::Ready { version, .. } => Some(format!("Update {version} ready")),
            UpdatePhase::Installed { version, .. } => Some(format!("Restart to use MOTIX {version}")),
            UpdatePhase::Installing => Some("Installing update…".to_owned()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_lines() {
        let mut u = UpdateInfo::default();
        assert_eq!(u.status_line(), None);
        u.phase = UpdatePhase::Downloading {
            version: "0.2.0".into(),
            done: 50,
            total: Some(200),
        };
        assert_eq!(u.status_line().unwrap(), "Downloading update 0.2.0: 25%");
        u.phase = UpdatePhase::Ready {
            version: "0.2.0".into(),
            notes: String::new(),
        };
        assert_eq!(u.status_line().unwrap(), "Update 0.2.0 ready");
    }
}
