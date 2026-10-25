//! Google sign-in and the owner's Drive `appDataFolder` as the sync storage. On Android the
//! grant is asked of Play Services (system consent screen, kept by Play Services); on desktop
//! through the system browser with a loopback redirect, the refresh token kept in a file only
//! the owner can read. See docs/guides/google.md.

use std::path::PathBuf;
use std::time::Duration;

use cinelens_core::SyncStorage;
use serde::Deserialize;
use tauri::{AppHandle, Manager, Runtime};

const FILES: &str = "https://www.googleapis.com/drive/v3/files";
const UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3/files";
const FILE_NAME: &str = "owner.json";

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(60))).build().into()
}

/// Present once the owner has signed in on this device.
fn grant_file<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    app.path().app_data_dir().expect("app data dir").join("google-grant")
}

pub fn signed_in<R: Runtime>(app: &AppHandle<R>) -> bool {
    grant_file(app).exists()
}

/// Asks the owner for access to the Drive `appDataFolder`; once per device.
pub fn sign_in<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let grant = platform::sign_in(app)?;
    crate::write_private(&grant_file(app), grant.as_bytes()).map_err(|e| e.to_string())
}

/// What the platform answers when the grant is gone (revoked, expired): sign in again.
const REVOKED: &str = "sign-in needed";

/// A fresh access token, never asking the owner. A grant Google no longer honours is
/// forgotten, so Perfil offers to sign in again.
pub fn access_token<R: Runtime>(app: &AppHandle<R>) -> Result<String, String> {
    let grant = std::fs::read_to_string(grant_file(app)).map_err(|e| e.to_string())?;
    platform::access_token(app, &grant).inspect_err(|e| {
        if e.contains(REVOKED) {
            let _ = std::fs::remove_file(grant_file(app));
        }
    })
}

#[cfg(target_os = "android")]
mod platform {
    use serde::{Deserialize, Serialize};
    use tauri::plugin::PluginHandle;
    use tauri::{AppHandle, Manager, Runtime};

    pub struct Plugin<R: Runtime>(pub PluginHandle<R>);

    #[derive(Serialize)]
    struct Authorize {
        interactive: bool,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Token {
        access_token: String,
    }

    pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
        tauri::plugin::Builder::new("google")
            .setup(|app, api| {
                let handle = api.register_android_plugin("io.github.lucasjlbraz.cinelens", "GooglePlugin")?;
                app.manage(Plugin(handle));
                Ok(())
            })
            .build()
    }

    fn authorize<R: Runtime>(app: &AppHandle<R>, interactive: bool) -> Result<String, String> {
        let plugin = app.state::<Plugin<R>>();
        let token: Token = plugin.0.run_mobile_plugin("authorize", Authorize { interactive }).map_err(|e| e.to_string())?;
        Ok(token.access_token)
    }

    /// Play Services keeps the grant itself: the file only marks that there is one.
    pub fn sign_in<R: Runtime>(app: &AppHandle<R>) -> Result<String, String> {
        authorize(app, true).map(|_| "play-services".to_string())
    }

    pub fn access_token<R: Runtime>(app: &AppHandle<R>, _grant: &str) -> Result<String, String> {
        authorize(app, false)
    }
}

#[cfg(not(target_os = "android"))]
mod platform {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    use serde::Deserialize;
    use sha2::{Digest, Sha256};
    use tauri::{AppHandle, Runtime};

    const SCOPE: &str = "https://www.googleapis.com/auth/drive.appdata";
    const AUTH: &str = "https://accounts.google.com/o/oauth2/v2/auth";
    const TOKEN: &str = "https://oauth2.googleapis.com/token";
    /// The Google Cloud project's Desktop client, set at build time (docs/guides/google.md).
    /// Google does not treat an installed app's client secret as secret.
    const CLIENT_ID: Option<&str> = option_env!("CINELENS_GOOGLE_CLIENT_ID");
    const CLIENT_SECRET: Option<&str> = option_env!("CINELENS_GOOGLE_CLIENT_SECRET");

    #[derive(Deserialize)]
    struct Tokens {
        access_token: String,
        refresh_token: Option<String>,
    }

    fn client() -> Result<(&'static str, &'static str), String> {
        CLIENT_ID.zip(CLIENT_SECRET).ok_or_else(|| "built without CINELENS_GOOGLE_CLIENT_ID/SECRET".to_string())
    }

    fn random() -> Result<String, String> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
        Ok(base64url(&bytes))
    }

    fn base64url(bytes: &[u8]) -> String {
        const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
            (0..chunk.len() + 1).for_each(|i| out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char));
        }
        out
    }

    /// The value of `name` in the query of the request line `GET /?a=b&c=d HTTP/1.1`.
    fn query_param(request_line: &str, name: &str) -> Option<String> {
        let query = request_line.split_whitespace().nth(1)?.split_once('?')?.1;
        let value = query.split('&').find_map(|kv| kv.strip_prefix(name)?.strip_prefix('='))?;
        Some(percent_decode(value))
    }

    fn percent_decode(s: &str) -> String {
        let bytes = s.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            let hex = bytes.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok());
            match (bytes[i], hex) {
                (b'%', Some(b)) => {
                    out.push(b);
                    i += 3;
                }
                (b'+', _) => {
                    out.push(b' ');
                    i += 1;
                }
                (b, _) => {
                    out.push(b);
                    i += 1;
                }
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    /// Authorization code flow with PKCE, in the system browser, redirected to a loopback port.
    /// Returns the refresh token.
    pub fn sign_in<R: Runtime>(_app: &AppHandle<R>) -> Result<String, String> {
        let (client_id, client_secret) = client()?;
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let redirect = format!("http://127.0.0.1:{}", listener.local_addr().map_err(|e| e.to_string())?.port());
        let (verifier, state) = (random()?, random()?);
        let challenge = base64url(&Sha256::digest(verifier.as_bytes()));
        let url = format!(
            "{AUTH}?client_id={client_id}&redirect_uri={redirect}&response_type=code&scope={}&code_challenge={challenge}&code_challenge_method=S256&state={state}&access_type=offline&prompt=consent",
            SCOPE.replace(':', "%3A").replace('/', "%2F"),
        );
        tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())?;
        // ponytail: waits for the browser with no timeout; the owner closing the tab leaves this
        // thread waiting until the app quits. Give `accept` a deadline if that ever matters.
        let code = loop {
            let (mut stream, _) = listener.accept().map_err(|e| e.to_string())?;
            // A browser preconnect sends nothing: drop it rather than wait on it.
            stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).map_err(|e| e.to_string())?;
            let mut line = String::new();
            if BufReader::new(&stream).read_line(&mut line).is_err() {
                continue;
            }
            let page = "<meta charset=utf-8><p>Pode fechar esta aba e voltar ao Cinelens.";
            let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n{page}");
            if query_param(&line, "state").as_deref() != Some(state.as_str()) {
                continue; // Not Google's redirect (a favicon request, another page).
            }
            if let Some(error) = query_param(&line, "error") {
                return Err(error);
            }
            break query_param(&line, "code").ok_or("no code in the redirect")?;
        };
        let tokens: Tokens = super::agent()
            .post(TOKEN)
            .send_form([
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("code", &code),
                ("code_verifier", &verifier),
                ("redirect_uri", &redirect),
                ("grant_type", "authorization_code"),
            ])
            .and_then(|mut r| r.body_mut().read_json())
            .map_err(|e| e.to_string())?;
        tokens.refresh_token.ok_or_else(|| "Google sent no refresh token".to_string())
    }

    // ponytail: a new access token per sync; cache it until it expires if syncs get frequent.
    pub fn access_token<R: Runtime>(_app: &AppHandle<R>, refresh_token: &str) -> Result<String, String> {
        let (client_id, client_secret) = client()?;
        let tokens: Tokens = super::agent()
            .post(TOKEN)
            .send_form([
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("refresh_token", refresh_token),
                ("grant_type", "refresh_token"),
            ])
            .and_then(|mut r| r.body_mut().read_json())
            .map_err(|e| match e {
                // invalid_grant: revoked by the owner, or unused for six months.
                ureq::Error::StatusCode(400 | 401) => super::REVOKED.to_string(),
                e => e.to_string(),
            })?;
        Ok(tokens.access_token)
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn base64url_matches_rfc_4648() {
            assert_eq!(super::base64url(b"f"), "Zg");
            assert_eq!(super::base64url(b"fo"), "Zm8");
            assert_eq!(super::base64url(b"foo"), "Zm9v");
            assert_eq!(super::base64url(&[0xfb, 0xff]), "-_8");
        }

        #[test]
        fn the_redirect_query_is_read() {
            let line = "GET /?state=abc&code=4%2F0A-x+y HTTP/1.1\r\n";
            assert_eq!(super::query_param(line, "code").as_deref(), Some("4/0A-x y"));
            assert_eq!(super::query_param(line, "state").as_deref(), Some("abc"));
            assert_eq!(super::query_param(line, "error"), None);
        }
    }
}

#[cfg(target_os = "android")]
pub use platform::init;

#[derive(Deserialize)]
struct FileList {
    files: Vec<FileId>,
}

#[derive(Deserialize)]
struct FileId {
    id: String,
}

/// The owner data blob in the Drive `appDataFolder`, which only this app can see.
pub struct Drive {
    token: String,
    agent: ureq::Agent,
    /// Found by `get`; `None` before the first device has synced.
    id: Option<String>,
}

impl Drive {
    pub fn new(token: String) -> Drive {
        Drive { token, agent: agent(), id: None }
    }

    fn auth(&self) -> String {
        format!("Bearer {}", self.token)
    }
}

impl SyncStorage for Drive {
    fn get(&mut self) -> Result<Option<Vec<u8>>, String> {
        let list: FileList = self
            .agent
            .get(FILES)
            .query("spaces", "appDataFolder")
            .query("q", format!("name = '{FILE_NAME}'"))
            .query("fields", "files(id)")
            // Two devices syncing for the first time at once may each create one: all use the first.
            .query("orderBy", "createdTime")
            .header("Authorization", self.auth())
            .call()
            .and_then(|mut r| r.body_mut().read_json())
            .map_err(|e| e.to_string())?;
        self.id = list.files.into_iter().next().map(|f| f.id);
        let Some(id) = &self.id else { return Ok(None) };
        let bytes = self
            .agent
            .get(format!("{FILES}/{id}"))
            .query("alt", "media")
            .header("Authorization", self.auth())
            .call()
            .and_then(|mut r| r.body_mut().with_config().limit(u64::MAX).read_to_vec())
            .map_err(|e| e.to_string())?;
        Ok(Some(bytes))
    }

    fn put(&mut self, bytes: &[u8]) -> Result<(), String> {
        let sent = match &self.id {
            Some(id) => self
                .agent
                .patch(format!("{UPLOAD}/{id}"))
                .query("uploadType", "media")
                .header("Authorization", self.auth())
                .content_type("application/json")
                .send(bytes),
            None => {
                const BOUNDARY: &str = "cinelens-sync-boundary";
                let metadata = format!(r#"{{"name": "{FILE_NAME}", "parents": ["appDataFolder"]}}"#);
                let mut body = format!(
                    "--{BOUNDARY}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata}\r\n--{BOUNDARY}\r\nContent-Type: application/json\r\n\r\n"
                )
                .into_bytes();
                body.extend_from_slice(bytes);
                body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
                self.agent
                    .post(UPLOAD)
                    .query("uploadType", "multipart")
                    .header("Authorization", self.auth())
                    .content_type(format!("multipart/related; boundary={BOUNDARY}"))
                    .send(&body[..])
            }
        };
        sent.map(|_| ()).map_err(|e| e.to_string())
    }
}
