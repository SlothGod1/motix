//! Reading GitHub's release list and choosing the release to install.

use semver::Version;
use serde::Deserialize;

/// One downloadable file of a release.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Asset {
    /// File name.
    pub name: String,
    /// Where to download it.
    pub browser_download_url: String,
    /// Size in bytes.
    #[serde(default)]
    pub size: u64,
}

/// One release, as GitHub describes it.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Release {
    /// Tag, e.g. `v0.2.0-preview.14`.
    pub tag_name: String,
    /// Unpublished drafts are ignored.
    #[serde(default)]
    pub draft: bool,
    /// Pre-releases (previews).
    #[serde(default)]
    pub prerelease: bool,
    /// Release notes.
    #[serde(default)]
    pub body: Option<String>,
    /// Files.
    #[serde(default)]
    pub assets: Vec<Asset>,
}

/// A release newer than the running version, with everything needed to install it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// Its version.
    pub version: Version,
    /// Release notes.
    pub notes: String,
    /// The program archive for this platform.
    pub archive: Asset,
    /// `SHA256SUMS`.
    pub sums: Asset,
    /// `SHA256SUMS.sig`.
    pub signature: Asset,
}

/// Parses GitHub's `GET /repos/{owner}/{repo}/releases` response.
///
/// # Errors
/// If the JSON isn't a release list.
pub fn parse_releases(json: &[u8]) -> Result<Vec<Release>, serde_json::Error> {
    serde_json::from_slice(json)
}

/// The newest release that is newer than `current`, has all three files, and (unless
/// `include_prereleases`) isn't a preview.
#[must_use]
pub fn choose(
    releases: &[Release],
    current: &Version,
    include_prereleases: bool,
    archive_name: &str,
) -> Option<Candidate> {
    releases
        .iter()
        .filter(|r| !r.draft && (include_prereleases || !r.prerelease))
        .filter_map(|r| {
            let version = Version::parse(r.tag_name.trim_start_matches('v')).ok()?;
            let find = |name: &str| r.assets.iter().find(|a| a.name == name).cloned();
            Some(Candidate {
                version,
                notes: r.body.clone().unwrap_or_default(),
                archive: find(archive_name)?,
                sums: find("SHA256SUMS")?,
                signature: find("SHA256SUMS.sig")?,
            })
        })
        .filter(|c| c.version > *current)
        .max_by(|a, b| a.version.cmp(&b.version))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, pre: bool, files: &[&str]) -> Release {
        Release {
            tag_name: tag.to_owned(),
            draft: false,
            prerelease: pre,
            body: Some(format!("notes for {tag}")),
            assets: files
                .iter()
                .map(|f| Asset {
                    name: (*f).to_owned(),
                    browser_download_url: format!("https://github.com/x/y/releases/download/{tag}/{f}"),
                    size: 10,
                })
                .collect(),
        }
    }

    const ALL: [&str; 3] = ["motix-windows-x64.zip", "SHA256SUMS", "SHA256SUMS.sig"];

    #[test]
    fn picks_the_newest_complete_newer_release() {
        let current = Version::parse("0.1.0-preview.5").unwrap();
        let list = vec![
            release("v0.1.0-preview.4", true, &ALL),
            release("v0.1.0-preview.12", true, &ALL),
            release("v0.1.0-preview.13", true, &ALL[..2]), // unsigned: skipped
            release("v0.1.0-preview.9", true, &ALL),
            release("not-a-version", true, &ALL),
        ];
        let c = choose(&list, &current, true, "motix-windows-x64.zip").unwrap();
        assert_eq!(c.version.to_string(), "0.1.0-preview.12", "numeric, not text, ordering");
        assert_eq!(c.notes, "notes for v0.1.0-preview.12");
        assert!(
            choose(&list, &current, false, "motix-windows-x64.zip").is_none(),
            "stable channel ignores previews"
        );
        assert!(
            choose(&list, &current, true, "motix-linux-x64.zip").is_none(),
            "no file for this platform"
        );
        let newest = Version::parse("0.1.0-preview.12").unwrap();
        assert!(
            choose(&list, &newest, true, "motix-windows-x64.zip").is_none(),
            "already newest"
        );
        let stable = vec![release("v0.1.0", false, &ALL)];
        assert_eq!(
            choose(&stable, &current, true, "motix-windows-x64.zip")
                .unwrap()
                .version
                .to_string(),
            "0.1.0"
        );
    }

    #[test]
    fn parses_github_json() {
        let json = br#"[{"tag_name":"v1.2.3","draft":false,"prerelease":true,"body":null,
            "assets":[{"name":"SHA256SUMS","browser_download_url":"https://x/y","size":90,"extra":1}],
            "html_url":"ignored"}]"#;
        let r = parse_releases(json).unwrap();
        assert_eq!(r[0].tag_name, "v1.2.3");
        assert_eq!(r[0].assets[0].size, 90);
        assert!(parse_releases(b"{\"message\":\"rate limited\"}").is_err());
    }
}
