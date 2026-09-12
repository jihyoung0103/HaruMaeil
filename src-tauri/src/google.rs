//! 구글 캘린더 연동.
//!
//! client_id는 비밀이 아니라 앱 식별자다 (OAuth URL에 그대로 실려 나간다). 다른 캘린더
//! 앱들처럼 앱에 박아둔다 — 설치할 때마다 사용자가 넣을 값이 아니다.
//! client secret은 안 넣는다: 구글 문서상 loopback + PKCE 조합에서 선택 사항이다.
//! 리프레시 토큰은 OS 자격 증명 저장소(Windows는 자격 증명 관리자)에 둔다.

use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    time::{Duration, Instant},
};

use oauth2::{
    basic::BasicClient, reqwest, AuthUrl, AuthorizationCode, ClientId, CsrfToken,
    PkceCodeChallenge, RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const EVENTS_URL: &str = "https://www.googleapis.com/calendar/v3/calendars/primary/events";

/// 동의는 한 번만 받고 캘린더·할일 둘 다 받아둔다. 나중에 쓰기를 붙일 때 재동의가
/// 없도록 읽기 전용(.readonly)이 아닌 전체 스코프를 요청한다.
const SCOPES: [&str; 2] = [
    "https://www.googleapis.com/auth/calendar",
    "https://www.googleapis.com/auth/tasks",
];

const KEYRING_SERVICE: &str = "harumaeil";
const KEYRING_USER: &str = "google-refresh-token";

// ---------- client_id ----------

/// 구글 클라우드 콘솔 > 사용자 인증 정보 > OAuth 클라이언트 ID(데스크톱 앱)에서 받은 값.
/// 여기를 채우면 앱은 그냥 "연결하기" 버튼 하나가 된다.
const CLIENT_ID: &str = "344606506253-bkifgpp4vbcv7bfkrv9iekrr3thg6n9f.apps.googleusercontent.com";

fn client_id() -> Option<String> {
    (!CLIENT_ID.is_empty()).then(|| CLIENT_ID.to_string())
}

#[derive(Serialize)]
pub struct GoogleStatus {
    #[serde(rename = "hasClientId")]
    pub has_client_id: bool,
    pub connected: bool,
}

#[tauri::command]
pub fn google_status() -> GoogleStatus {
    GoogleStatus {
        has_client_id: client_id().is_some(),
        connected: load_refresh().is_some(),
    }
}

// ---------- 리프레시 토큰 ----------

fn keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| e.to_string())
}

fn load_refresh() -> Option<String> {
    keyring_entry().ok()?.get_password().ok()
}

#[tauri::command]
pub fn google_disconnect() -> Result<(), String> {
    match keyring_entry()?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

// ---------- OAuth ----------

/// 리다이렉트를 딱 한 번 받는다. 127.0.0.1만 듣고, 2분 안에 안 오면 포기한다.
fn wait_for_code(listener: TcpListener, expect_state: &str) -> Result<String, String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut stream = loop {
        match listener.accept() {
            Ok((s, _)) => break s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    return Err("브라우저 로그인이 2분 안에 끝나지 않았습니다".into());
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;

    let mut request_line = String::new();
    BufReader::new(&stream)
        .read_line(&mut request_line)
        .map_err(|e| e.to_string())?;

    // "GET /?code=...&state=... HTTP/1.1"
    let query = request_line
        .split_whitespace()
        .nth(1)
        .and_then(|p| p.split_once('?'))
        .map(|(_, q)| q.to_string())
        .unwrap_or_default();

    let mut code = None;
    let mut state = None;
    let mut error = None;
    for pair in query.split('&') {
        let Some((k, v)) = pair.split_once('=') else {
            continue;
        };
        let v = percent_encoding::percent_decode_str(v)
            .decode_utf8_lossy()
            .to_string();
        match k {
            "code" => code = Some(v),
            "state" => state = Some(v),
            "error" => error = Some(v),
            _ => {}
        }
    }

    let body = "<!doctype html><meta charset=\"utf-8\"><body style=\"font-family:sans-serif;padding:3rem\"><p>하루매일에 연결했습니다. 이 창은 닫으셔도 됩니다.</p>";
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );

    if let Some(e) = error {
        return Err(format!("구글이 거절했습니다: {e}"));
    }
    // CSRF 검사 — 이게 없으면 남이 보낸 code를 받아버린다
    if state.as_deref() != Some(expect_state) {
        return Err("state가 일치하지 않습니다 (요청이 변조됐을 수 있음)".into());
    }
    code.ok_or_else(|| "code가 오지 않았습니다".to_string())
}

fn http() -> Result<reqwest::Client, String> {
    reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn google_connect(app: AppHandle) -> Result<(), String> {
    let client_id = client_id().ok_or("빌드에 구글 클라이언트 ID가 없습니다 (google.rs의 CLIENT_ID)")?;

    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();

    let client = BasicClient::new(ClientId::new(client_id))
        .set_auth_uri(AuthUrl::new(AUTH_URL.to_string()).map_err(|e| e.to_string())?)
        .set_token_uri(TokenUrl::new(TOKEN_URL.to_string()).map_err(|e| e.to_string())?)
        .set_redirect_uri(
            RedirectUrl::new(format!("http://127.0.0.1:{port}")).map_err(|e| e.to_string())?,
        );

    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, csrf) = client
        .authorize_url(CsrfToken::new_random)
        .add_scopes(SCOPES.iter().map(|s| Scope::new(s.to_string())))
        .set_pkce_challenge(challenge)
        // 이 둘이 없으면 리프레시 토큰을 안 준다
        .add_extra_param("access_type", "offline")
        .add_extra_param("prompt", "consent")
        .url();

    app.opener()
        .open_url(url.to_string(), None::<&str>)
        .map_err(|e| e.to_string())?;

    let expect = csrf.secret().clone();
    let code = tauri::async_runtime::spawn_blocking(move || wait_for_code(listener, &expect))
        .await
        .map_err(|e| e.to_string())??;

    let token = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(verifier)
        .request_async(&http()?)
        .await
        .map_err(|e| e.to_string())?;

    let refresh = token
        .refresh_token()
        .ok_or("리프레시 토큰을 받지 못했습니다")?;
    keyring_entry()?
        .set_password(refresh.secret())
        .map_err(|e| e.to_string())
}

/// 매번 리프레시로 액세스 토큰을 새로 받는다.
/// ponytail: 호출마다 왕복 한 번. 동기화가 잦아지면 만료시각까지 메모리에 캐시할 것
async fn access_token() -> Result<String, String> {
    let client_id = client_id().ok_or("빌드에 구글 클라이언트 ID가 없습니다 (google.rs의 CLIENT_ID)")?;
    let refresh = load_refresh().ok_or("구글 계정이 연결되지 않았습니다")?;

    let client = BasicClient::new(ClientId::new(client_id))
        .set_auth_uri(AuthUrl::new(AUTH_URL.to_string()).map_err(|e| e.to_string())?)
        .set_token_uri(TokenUrl::new(TOKEN_URL.to_string()).map_err(|e| e.to_string())?);

    let token = client
        .exchange_refresh_token(&RefreshToken::new(refresh))
        .request_async(&http()?)
        .await
        .map_err(|e| e.to_string())?;
    Ok(token.access_token().secret().clone())
}

// ---------- 캘린더 ----------

#[derive(Deserialize)]
struct EventsResponse {
    #[serde(default)]
    items: Vec<GEvent>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct GEvent {
    id: String,
    summary: Option<String>,
    status: Option<String>,
    start: Option<GTime>,
    end: Option<GTime>,
}

#[derive(Deserialize)]
struct GTime {
    /// 종일 일정은 date("2026-09-17"), 시간 있는 일정은 dateTime(RFC3339)
    date: Option<String>,
    #[serde(rename = "dateTime")]
    date_time: Option<String>,
}

#[derive(Serialize)]
pub struct RemoteEvent {
    pub id: String,
    pub title: String,
    /// RFC3339 또는 YYYY-MM-DD. 로컬 시각 변환은 Date를 가진 프런트에서 한다
    pub start: String,
    pub end: Option<String>,
    #[serde(rename = "allDay")]
    pub all_day: bool,
}

/// time_min~time_max 구간의 일정. 반복 일정은 펼쳐서(singleEvents) 받는다.
#[tauri::command]
pub async fn google_events(time_min: String, time_max: String) -> Result<Vec<RemoteEvent>, String> {
    let token = access_token().await?;
    let http = http()?;
    let mut out = Vec::new();
    let mut page: Option<String> = None;

    loop {
        let mut query = vec![
            ("timeMin", time_min.clone()),
            ("timeMax", time_max.clone()),
            ("singleEvents", "true".to_string()),
            ("orderBy", "startTime".to_string()),
            ("maxResults", "2500".to_string()),
        ];
        if let Some(p) = &page {
            query.push(("pageToken", p.clone()));
        }

        let res = http
            .get(EVENTS_URL)
            .bearer_auth(&token)
            .query(&query)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = res.status();
        let body = res.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("구글 캘린더 {status}: {body}"));
        }

        let parsed: EventsResponse = serde_json::from_str(&body).map_err(|e| e.to_string())?;
        for e in parsed.items {
            if e.status.as_deref() == Some("cancelled") {
                continue;
            }
            let Some(start) = e.start.as_ref() else {
                continue;
            };
            let all_day = start.date.is_some();
            let Some(at) = start.date_time.clone().or_else(|| start.date.clone()) else {
                continue;
            };
            out.push(RemoteEvent {
                id: format!("gcal:{}", e.id),
                title: e.summary.unwrap_or_else(|| "(제목 없음)".to_string()),
                start: at,
                end: e.end.and_then(|t| t.date_time.or(t.date)),
                all_day,
            });
        }

        page = parsed.next_page_token;
        if page.is_none() {
            return Ok(out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::wait_for_code;
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};

    /// 진짜 소켓으로 리다이렉트를 한 번 쏴본다
    fn redirect(path: &str, expect_state: &str) -> Result<String, String> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let state = expect_state.to_string();
        let handle = std::thread::spawn(move || wait_for_code(listener, &state));
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(s, "GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
        s.flush().unwrap();
        handle.join().unwrap()
    }

    #[test]
    fn code_is_percent_decoded() {
        // 구글 code에는 %2F(/)가 들어온다
        let got = redirect("/?state=abc&code=4%2F0Ab_XyZ-1", "abc").unwrap();
        assert_eq!(got, "4/0Ab_XyZ-1");
    }

    #[test]
    fn wrong_state_is_rejected() {
        let err = redirect("/?state=attacker&code=zzz", "abc").unwrap_err();
        assert!(err.contains("state"), "{err}");
    }

    #[test]
    fn google_error_is_surfaced() {
        let err = redirect("/?error=access_denied&state=abc", "abc").unwrap_err();
        assert!(err.contains("access_denied"), "{err}");
    }
}
