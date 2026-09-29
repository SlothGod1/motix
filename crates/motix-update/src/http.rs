//! HTTPS access to GitHub, behind a small trait so the updater can be tested offline.

use crate::UpdateError;
use std::io::{Read, Write};
use std::sync::Arc;
use std::time::Duration;

/// A response to a small request.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Response {
    /// The server said nothing changed since the given `ETag`.
    pub not_modified: bool,
    /// Body.
    pub body: Vec<u8>,
    /// The response's `ETag`, for the next request.
    pub etag: Option<String>,
}

/// What the updater needs from the network.
pub trait Http: Send + Sync + 'static {
    /// `GET` a small resource (at most `max_bytes`), optionally conditional on an `ETag`.
    ///
    /// # Errors
    /// Network and HTTP errors as [`UpdateError`].
    fn get(&self, url: &str, etag: Option<&str>, max_bytes: u64) -> Result<Response, UpdateError>;

    /// Streams a download (at most `max_bytes`) into `out`, reporting `(received, total)`.
    ///
    /// # Errors
    /// Network and HTTP errors as [`UpdateError`].
    fn download(
        &self,
        url: &str,
        max_bytes: u64,
        out: &mut dyn Write,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<(), UpdateError>;
}

/// The real network: rustls with the operating system's certificate store, HTTPS only.
pub struct UreqHttp {
    agent: ureq::Agent,
}

impl UreqHttp {
    /// Creates a client that identifies itself as `user_agent`.
    #[must_use]
    pub fn new(user_agent: &str) -> Self {
        let tls = ureq::tls::TlsConfig::builder()
            .provider(ureq::tls::TlsProvider::Rustls)
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .unversioned_rustls_crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .build();
        let agent = ureq::Agent::config_builder()
            .tls_config(tls)
            .https_only(true)
            .max_redirects(5)
            .user_agent(user_agent)
            .timeout_connect(Some(Duration::from_secs(20)))
            .timeout_recv_body(Some(Duration::from_mins(1)))
            .http_status_as_error(false)
            .build()
            .into();
        Self { agent }
    }

    fn status_error(status: u16) -> UpdateError {
        match status {
            403 | 429 => UpdateError::Network("GitHub asked MOTIX to slow down; it will try again later".to_owned()),
            404 => UpdateError::Network("the MOTIX releases page wasn't found on GitHub".to_owned()),
            s => UpdateError::Network(format!("GitHub answered with an error ({s})")),
        }
    }
}

fn net(e: &ureq::Error) -> UpdateError {
    UpdateError::Network(match e {
        ureq::Error::Timeout(_) => "the connection timed out".to_owned(),
        ureq::Error::HostNotFound => "no internet connection (GitHub couldn't be reached)".to_owned(),
        other => format!("couldn't reach GitHub ({other})"),
    })
}

impl Http for UreqHttp {
    fn get(&self, url: &str, etag: Option<&str>, max_bytes: u64) -> Result<Response, UpdateError> {
        let mut req = self
            .agent
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(e) = etag {
            req = req.header("If-None-Match", e);
        }
        let mut resp = req.call().map_err(|e| net(&e))?;
        let status = resp.status().as_u16();
        if status == 304 {
            return Ok(Response {
                not_modified: true,
                ..Response::default()
            });
        }
        if !(200..300).contains(&status) {
            return Err(Self::status_error(status));
        }
        let etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body = resp
            .body_mut()
            .with_config()
            .limit(max_bytes)
            .read_to_vec()
            .map_err(|e| net(&e))?;
        Ok(Response {
            not_modified: false,
            body,
            etag,
        })
    }

    fn download(
        &self,
        url: &str,
        max_bytes: u64,
        out: &mut dyn Write,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<(), UpdateError> {
        let mut resp = self
            .agent
            .get(url)
            .header("Accept", "application/octet-stream")
            .config()
            .timeout_global(Some(Duration::from_hours(1)))
            .build()
            .call()
            .map_err(|e| net(&e))?;
        let status = resp.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(Self::status_error(status));
        }
        let total = resp.body().content_length();
        if total.is_some_and(|t| t > max_bytes) {
            return Err(UpdateError::TooLarge);
        }
        let mut reader = resp.body_mut().with_config().limit(max_bytes).reader();
        let mut buf = vec![0_u8; 256 * 1024];
        let mut done = 0_u64;
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| UpdateError::Network(format!("the download stopped ({e})")))?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).map_err(|e| UpdateError::Disk(e.to_string()))?;
            done += n as u64;
            progress(done, total);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Talks to the real GitHub API. Run with `cargo test -p motix-update -- --ignored`.
    #[test]
    #[ignore = "needs the internet"]
    fn live_github_release_list() {
        let http = UreqHttp::new("MOTIX-test (+https://github.com/SlothGod1/motix)");
        let resp = http
            .get(
                "https://api.github.com/repos/SlothGod1/motix/releases?per_page=5",
                None,
                8 * 1024 * 1024,
            )
            .expect("request");
        assert!(
            crate::github::parse_releases(&resp.body).is_ok(),
            "{}",
            String::from_utf8_lossy(&resp.body)
        );
        // A second, conditional request is answered "not modified".
        let again = http
            .get(
                "https://api.github.com/repos/SlothGod1/motix/releases?per_page=5",
                resp.etag.as_deref(),
                8 * 1024 * 1024,
            )
            .expect("request");
        assert!(again.not_modified || again.etag.is_some());
    }
}
