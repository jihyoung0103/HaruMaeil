mod google;
mod holiday;

use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};
use tauri_plugin_sql::{Migration, MigrationKind};
use tauri_plugin_wallpaper::{AttachRequest, DetachRequest, WallpaperExt};

/// event와 task를 한 테이블에. 달 조회할 때 두 번 긁어서 합치지 않으려고.
/// at = event의 시작 / task의 마감 (epoch ms). end_at은 event에만.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS items (
  id     TEXT PRIMARY KEY,
  title  TEXT NOT NULL,
  kind   TEXT NOT NULL,
  at     INTEGER NOT NULL,
  end_at INTEGER,
  done   INTEGER NOT NULL DEFAULT 0,
  source TEXT NOT NULL DEFAULT 'local'
);
CREATE INDEX IF NOT EXISTS items_at ON items(at);
";

/// v2: 항목마다 어느 캘린더(구글 캘린더 하나, 할 일 목록 하나, 공휴일, 내 일정)에 속하는지.
/// 색·이름은 캘린더에 둔다. 날짜 없는 항목(마감 없는 할 일)을 담을 수 있게 at을 NULL 허용.
/// source 컬럼은 calendar_id가 대신하므로 테이블을 다시 만든다 — SQLite는 컬럼 삭제·제약 변경이 안 된다.
/// 구글 항목은 원격의 캐시라 버리고 다음 가져오기 때 다시 받는다. 직접 만든 것만 옮긴다.
/// 이 SQL도 적용된 뒤에는 체크섬 때문에 고치지 말고 v3를 추가할 것.
const SCHEMA_V2: &str = "
CREATE TABLE calendars (
  id    TEXT PRIMARY KEY,
  name  TEXT NOT NULL,
  color TEXT NOT NULL
);
INSERT INTO calendars (id, name, color) VALUES ('local', '내 일정', '#4a86e8');

CREATE TABLE items_v2 (
  id          TEXT PRIMARY KEY,
  calendar_id TEXT NOT NULL REFERENCES calendars(id) ON DELETE CASCADE,
  title       TEXT NOT NULL,
  kind        TEXT NOT NULL,
  at          INTEGER,
  end_at      INTEGER,
  done        INTEGER NOT NULL DEFAULT 0
);

INSERT INTO items_v2 (id, calendar_id, title, kind, at, end_at, done)
  SELECT id, 'local', title, kind, at, end_at, done FROM items WHERE source = 'local';

DROP TABLE items;
ALTER TABLE items_v2 RENAME TO items;
CREATE INDEX items_at ON items(at);
CREATE INDEX items_calendar ON items(calendar_id);
";

/// v3: 구글에서 일정마다 따로 고른 색. NULL이면 캘린더 색을 쓴다.
const SCHEMA_V3: &str = "ALTER TABLE items ADD COLUMN color TEXT;";

/// v4: 종일 여부. 구글은 종일 일정에 start.dateTime 없이 start.date만 준다.
/// 자정 시작으로 어림하면 00:00에 시작하는 일정을 종일로 잘못 본다.
/// 할 일(구글 할 일은 날짜만 유효)과 공휴일은 전부 종일. 구글 일정은 다음 가져오기 때 정확해진다.
const SCHEMA_V4: &str = "
ALTER TABLE items ADD COLUMN all_day INTEGER NOT NULL DEFAULT 0;
UPDATE items SET all_day = 1 WHERE kind = 'task' OR calendar_id = 'holiday:kr';
";

/// 위젯 위치·크기. 위치는 화면 좌표(모니터 배치 기준)로 저장한다.
#[derive(Serialize, Deserialize, Clone, Copy)]
struct WidgetRect {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

const DEFAULT_RECT: WidgetRect = WidgetRect {
    x: 40,
    y: 40,
    w: 360,
    h: 320,
};

fn pos_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("widget.json"))
}

fn load_rect(app: &AppHandle) -> WidgetRect {
    pos_path(app)
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(DEFAULT_RECT)
}

fn save_rect(app: &AppHandle, pos: WidgetRect) {
    let Some(path) = pos_path(app) else { return };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(&pos) {
        let _ = fs::write(path, json);
    }
}

/// 가상 화면 원점. WorkerW 자식 좌표는 여기가 (0,0)이라 붙은 상태에선 이만큼 빼야 한다.
/// tauri의 primary_monitor()는 주 모니터를 안 돌려줄 때가 있어 직접 최솟값을 구한다.
fn virtual_origin<R: Runtime>(win: &WebviewWindow<R>) -> (i32, i32) {
    win.available_monitors()
        .map(|ms| {
            (
                ms.iter().map(|m| m.position().x).min().unwrap_or(0),
                ms.iter().map(|m| m.position().y).min().unwrap_or(0),
            )
        })
        .unwrap_or((0, 0))
}

/// 벽지 레이어에 붙은 상태에서 화면 좌표 rect 자리에 놓기
fn place_attached<R: Runtime>(win: &WebviewWindow<R>, rect: WidgetRect) {
    let (vx, vy) = virtual_origin(win);
    let _ = win.set_size(PhysicalSize::new(rect.w, rect.h));
    let _ = win.set_position(PhysicalPosition::new(rect.x - vx, rect.y - vy));
}

/// 위치 조정 모드 토글.
/// on  → 벽지 레이어에서 떼내 일반 창으로. 그래야 입력을 받아 드래그가 된다.
/// off → 현재 위치를 저장하고 다시 아이콘 뒤로.
#[tauri::command]
fn widget_edit(app: AppHandle, on: bool) -> Result<(), String> {
    let win = app
        .get_webview_window("widget")
        .ok_or("위젯 창을 찾을 수 없음")?;

    if on {
        app.wallpaper()
            .detach(DetachRequest::new("widget"))
            .map_err(|e| e.to_string())?;
        // 떼어낸 뒤에는 화면 좌표 그대로
        let rect = load_rect(&app);
        let _ = win.set_size(PhysicalSize::new(rect.w, rect.h));
        let _ = win.set_position(PhysicalPosition::new(rect.x, rect.y));
        let _ = win.set_always_on_top(true);
        let _ = win.set_focus();
    } else {
        if let (Ok(p), Ok(s)) = (win.outer_position(), win.outer_size()) {
            save_rect(
                &app,
                WidgetRect {
                    x: p.x,
                    y: p.y,
                    w: s.width,
                    h: s.height,
                },
            );
        }
        let _ = win.set_always_on_top(false);
        app.wallpaper()
            .attach(AttachRequest::new("widget"))
            .map_err(|e| e.to_string())?;
        place_attached(&win, load_rect(&app));
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();

    // 두 번 켜지 못하게. 두 번째 실행은 이미 떠 있는 창을 앞으로 보내고 스스로 끝난다.
    // 개발 빌드에는 걸지 않는다 — 잠금이 앱 식별자 기준이라, 시작프로그램으로 떠 있는
    // 릴리스 앱 때문에 `npm run tauri dev`가 바로 죽는다.
    // 이 플러그인은 가장 먼저 등록해야 한다.
    #[cfg(not(debug_assertions))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }));
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations(
                    "sqlite:harumaeil.db",
                    vec![
                        Migration {
                            version: 1,
                            description: "items",
                            sql: SCHEMA,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 2,
                            description: "calendars",
                            sql: SCHEMA_V2,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 3,
                            description: "item color",
                            sql: SCHEMA_V3,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 4,
                            description: "all day",
                            sql: SCHEMA_V4,
                            kind: MigrationKind::Up,
                        },
                    ],
                )
                .build(),
        )
        .plugin(tauri_plugin_wallpaper::init())
        .invoke_handler(tauri::generate_handler![
            widget_edit,
            google::google_status,
            google::google_connect,
            google::google_disconnect,
            google::google_events,
            google::google_tasks,
            holiday::holidays
        ])
        .setup(|app| {
            if let Some(widget) = app.get_webview_window("widget") {
                // 바탕화면 아이콘 뒤(WorkerW)로. 실패하면 그냥 일반 창으로 뜸
                match app.wallpaper().attach(AttachRequest::new("widget")) {
                    Ok(()) => place_attached(&widget, load_rect(app.handle())),
                    Err(e) => eprintln!("[widget] attach 실패: {e}"),
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
