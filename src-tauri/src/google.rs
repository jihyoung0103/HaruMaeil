//! 구글 캘린더·할 일 연동.
//!
//! 자격증명은 앱에 박는다 — 설치할 때마다 사용자가 넣을 값이 아니다.
//! 데스크톱 앱 유형 클라이언트는 토큰 교환에 client_secret을 요구한다. 구글 문서상 이
//! 값은 "비밀로 취급되지 않는" 값이고 실제 보호는 PKCE가 한다. 다만 공개 저장소에
//! 커밋되면 시크릿 스캐닝에 걸리므로 build.rs가 저장소 밖 파일에서 주입한다.
//! 리프레시 토큰은 OS 자격 증명 저장소(Windows는 자격 증명 관리자)에 둔다.

use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    time::{Duration, Instant},
};

use oauth2::{
    basic::BasicClient, reqwest, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
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

/// build.rs가 src-tauri/google-client.json에서 읽어 넣는다 (저장소에는 없는 파일)
const CLIENT_ID: &str = env!("GOOGLE_CLIENT_ID");
const CLIENT_SECRET: &str = env!("GOOGLE_CLIENT_SECRET");

fn client_id() -> Option<String> {
    (!CLIENT_ID.is_empty()).then(|| CLIENT_ID.to_string())
}

/// 데스크톱 클라이언트는 토큰 교환에 시크릿을 요구한다. 없으면 구글이
/// "client_secret is missing"으로 거절한다.
fn client_secret() -> Option<ClientSecret> {
    (!CLIENT_SECRET.is_empty()).then(|| ClientSecret::new(CLIENT_SECRET.to_string()))
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
    let client_id = client_id().ok_or("빌드에 구글 자격증명이 없습니다 (src-tauri/google-client.json)")?;

    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();

    let client = BasicClient::new(ClientId::new(client_id))
        .set_client_secret(client_secret().ok_or("빌드에 구글 클라이언트 시크릿이 없습니다")?)
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
    let client_id = client_id().ok_or("빌드에 구글 자격증명이 없습니다 (src-tauri/google-client.json)")?;
    let refresh = load_refresh().ok_or("구글 계정이 연결되지 않았습니다")?;

    let client = BasicClient::new(ClientId::new(client_id))
        .set_client_secret(client_secret().ok_or("빌드에 구글 클라이언트 시크릿이 없습니다")?)
        .set_auth_uri(AuthUrl::new(AUTH_URL.to_string()).map_err(|e| e.to_string())?)
        .set_token_uri(TokenUrl::new(TOKEN_URL.to_string()).map_err(|e| e.to_string())?);

    let token = client
        .exchange_refresh_token(&RefreshToken::new(refresh))
        .request_async(&http()?)
        .await
        .map_err(|e| e.to_string())?;
    Ok(token.access_token().secret().clone())
}

// ---------- API 공통 ----------

/// 토큰 달고 GET 해서 JSON으로 푼다. 실패하면 구글이 준 본문을 그대로 에러에 싣는다
/// (API 사용 설정이 꺼져 있다 같은 원인이 거기 적혀 온다).
async fn get_json<T: serde::de::DeserializeOwned>(
    http: &reqwest::Client,
    token: &str,
    url: &str,
    query: &[(&str, String)],
) -> Result<T, String> {
    let res = http
        .get(url)
        .bearer_auth(token)
        .query(query)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = res.status();
    let body = res.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("구글 API {status}: {body}"));
    }
    serde_json::from_str(&body).map_err(|e| e.to_string())
}

// ---------- 캘린더 ----------

#[derive(Deserialize)]
struct GEvent {
    id: String,
    summary: Option<String>,
    status: Option<String>,
    start: Option<GTime>,
    end: Option<GTime>,
    /// 일정 우클릭으로 색을 고른 경우에만 온다 ("1"~"11")
    #[serde(rename = "colorId")]
    color_id: Option<String>,
}

/// colors.get — colorId → 색. 구글 캘린더 화면 이름으로는 1 라벤더, 2 세이지, 3 포도, 4 플라밍고,
/// 5 바나나, 6 귤, 7 공작, 8 흑연, 9 블루베리, 10 바질, 11 토마토
#[derive(Deserialize)]
struct GColors {
    #[serde(default)]
    event: std::collections::HashMap<String, GColor>,
}

#[derive(Deserialize)]
struct GColor {
    background: String,
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
    /// 일정에 따로 지정한 색. 없으면 캘린더 색을 쓴다
    pub color: Option<String>,
}

/// 구글 목록 응답 공통 모양 (일정, 할 일 목록, 할 일 전부 이 형태로 온다)
#[derive(Deserialize)]
struct Paged<T> {
    // 그냥 default면 serde가 T: Default를 요구한다. 빈 Vec만 있으면 됨
    #[serde(default = "Vec::new")]
    items: Vec<T>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct GCalendarListEntry {
    id: String,
    summary: Option<String>,
    #[serde(rename = "summaryOverride")]
    summary_override: Option<String>,
    #[serde(rename = "backgroundColor")]
    background_color: Option<String>,
}

/// 달력에 색·이름을 붙일 단위. 구글 캘린더 하나, 할 일 목록 하나가 각각 하나.
#[derive(Serialize)]
pub struct RemoteCalendar {
    pub id: String,
    pub name: String,
    /// 할 일 목록은 구글이 색을 주지 않아서 None — 프런트가 정한다
    pub color: Option<String>,
}

#[derive(Serialize)]
pub struct GoogleEvents {
    pub calendar: RemoteCalendar,
    pub events: Vec<RemoteEvent>,
}

/// time_min~time_max 구간의 기본 캘린더 일정. 반복 일정은 펼쳐서(singleEvents) 받는다.
#[tauri::command]
pub async fn google_events(time_min: String, time_max: String) -> Result<GoogleEvents, String> {
    let token = access_token().await?;
    let http = http()?;

    let entry: GCalendarListEntry = get_json(
        &http,
        &token,
        "https://www.googleapis.com/calendar/v3/users/me/calendarList/primary",
        &[],
    )
    .await?;
    let calendar = RemoteCalendar {
        id: format!("gcal:{}", entry.id),
        name: entry
            .summary_override
            .or(entry.summary)
            .unwrap_or_else(|| "구글 캘린더".to_string()),
        color: entry.background_color,
    };
    let palette: GColors = get_json(
        &http,
        &token,
        "https://www.googleapis.com/calendar/v3/colors",
        &[],
    )
    .await?;

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
        let parsed: Paged<GEvent> = get_json(&http, &token, EVENTS_URL, &query).await?;

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
                id: format!("{}:{}", calendar.id, e.id),
                title: e.summary.unwrap_or_else(|| "(제목 없음)".to_string()),
                start: at,
                end: e.end.and_then(|t| t.date_time.or(t.date)),
                all_day,
                color: e
                    .color_id
                    .as_ref()
                    .and_then(|id| palette.event.get(id))
                    .map(|c| c.background.clone()),
            });
        }

        page = parsed.next_page_token;
        if page.is_none() {
            return Ok(GoogleEvents {
                calendar,
                events: out,
            });
        }
    }
}

// ---------- 할 일 ----------

const TASKLISTS_URL: &str = "https://tasks.googleapis.com/tasks/v1/users/@me/lists";

#[derive(Deserialize)]
struct GTaskList {
    id: String,
    title: Option<String>,
}

#[derive(Deserialize)]
struct GTask {
    id: String,
    title: Option<String>,
    /// "needsAction" | "completed"
    status: Option<String>,
    /// RFC3339 형식이지만 구글 할 일은 날짜만 의미가 있다 (시각 부분은 버려짐)
    due: Option<String>,
    #[serde(default)]
    deleted: bool,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct RemoteTask {
    pub id: String,
    /// 소속 할 일 목록의 캘린더 id ("gtask:<목록 id>")
    pub list: String,
    pub title: String,
    /// YYYY-MM-DD. 날짜만 넘겨야 시간대 때문에 하루 밀리지 않는다
    pub due: String,
    pub done: bool,
}

/// 달력에 놓을 수 없는 것(마감 없음, 삭제됨)은 None
fn to_remote_task(list_id: &str, t: GTask) -> Option<RemoteTask> {
    if t.deleted {
        return None;
    }
    let due = t.due?.get(..10)?.to_string();
    let title = t
        .title
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "(제목 없음)".to_string());
    Some(RemoteTask {
        // 할 일 id는 목록 안에서만 유일하다고 보장되므로 목록 id를 붙인다
        id: format!("gtask:{list_id}:{}", t.id),
        list: format!("gtask:{list_id}"),
        title,
        due,
        done: t.status.as_deref() == Some("completed"),
    })
}

#[derive(Serialize)]
pub struct GoogleTasks {
    pub lists: Vec<RemoteCalendar>,
    pub tasks: Vec<RemoteTask>,
}

/// 모든 할 일 목록에서 마감일이 due_min 이상 due_max 미만인 할 일.
/// 완료된 것도 가져온다(달력에 지운 줄로 보이게). 마감 없는 할 일은 달력에 둘 곳이 없어 뺀다.
#[tauri::command]
pub async fn google_tasks(due_min: String, due_max: String) -> Result<GoogleTasks, String> {
    let token = access_token().await?;
    let http = http()?;

    let mut lists = Vec::new();
    let mut page: Option<String> = None;
    loop {
        let mut query = vec![("maxResults", "100".to_string())];
        if let Some(p) = &page {
            query.push(("pageToken", p.clone()));
        }
        let parsed: Paged<GTaskList> = get_json(&http, &token, TASKLISTS_URL, &query).await?;
        lists.extend(parsed.items);
        page = parsed.next_page_token;
        if page.is_none() {
            break;
        }
    }

    let mut out = Vec::new();
    for list in &lists {
        let url = format!(
            "https://tasks.googleapis.com/tasks/v1/lists/{}/tasks",
            percent_encoding::utf8_percent_encode(&list.id, percent_encoding::NON_ALPHANUMERIC)
        );
        let mut page: Option<String> = None;
        loop {
            let mut query = vec![
                ("dueMin", due_min.clone()),
                ("dueMax", due_max.clone()),
                ("showCompleted", "true".to_string()),
                // 완료 후 "완료된 항목 지우기"를 하면 hidden이 된다. 이게 없으면 완료 항목이 대부분 빠진다
                ("showHidden", "true".to_string()),
                ("maxResults", "100".to_string()),
            ];
            if let Some(p) = &page {
                query.push(("pageToken", p.clone()));
            }
            let parsed: Paged<GTask> = get_json(&http, &token, &url, &query).await?;
            out.extend(parsed.items.into_iter().filter_map(|t| to_remote_task(&list.id, t)));
            page = parsed.next_page_token;
            if page.is_none() {
                break;
            }
        }
    }
    Ok(GoogleTasks {
        lists: lists
            .into_iter()
            .map(|l| RemoteCalendar {
                id: format!("gtask:{}", l.id),
                name: l.title.unwrap_or_else(|| "할 일".to_string()),
                color: None,
            })
            .collect(),
        tasks: out,
    })
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

    fn task(json: &str) -> super::GTask {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn task_due_keeps_date_only() {
        let t = task(r#"{"id":"t1","title":"월세","status":"needsAction","due":"2026-09-25T00:00:00.000Z"}"#);
        let r = super::to_remote_task("L1", t).unwrap();
        assert_eq!(r.due, "2026-09-25");
        assert_eq!(r.id, "gtask:L1:t1");
        assert_eq!(r.list, "gtask:L1");
        assert!(!r.done);
    }

    #[test]
    fn task_completed_and_untitled() {
        let t = task(r#"{"id":"t2","title":"  ","status":"completed","due":"2026-09-01T00:00:00.000Z"}"#);
        let r = super::to_remote_task("L1", t).unwrap();
        assert!(r.done);
        assert_eq!(r.title, "(제목 없음)");
    }

    #[test]
    fn task_without_due_or_deleted_is_skipped() {
        assert!(super::to_remote_task("L", task(r#"{"id":"a","title":"x"}"#)).is_none());
        let deleted = r#"{"id":"b","title":"x","due":"2026-09-01T00:00:00.000Z","deleted":true}"#;
        assert!(super::to_remote_task("L", task(deleted)).is_none());
    }

    #[test]
    fn google_error_is_surfaced() {
        let err = redirect("/?error=access_denied&state=abc", "abc").unwrap_err();
        assert!(err.contains("access_denied"), "{err}");
    }
}
