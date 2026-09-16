mod google;

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
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations(
                    "sqlite:harumaeil.db",
                    vec![Migration {
                        version: 1,
                        description: "items",
                        sql: SCHEMA,
                        kind: MigrationKind::Up,
                    }],
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
            google::google_tasks
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
