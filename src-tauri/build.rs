use std::fs;

/// 구글 OAuth 자격증명을 빌드 시점에 바이너리로 넣는다.
///
/// 설치형 앱의 client_secret은 구글 문서상 "비밀로 취급되지 않는" 값이고 실제 보호는
/// PKCE가 한다. 그래도 공개 저장소에 커밋하면 GitHub 시크릿 스캐닝이 잡아서 구글이
/// 자동 무효화할 수 있으므로, 저장소 밖(gitignore된 파일)에 두고 여기서 주입한다.
///
/// src-tauri/google-client.json 형식:
///   { "client_id": "...", "client_secret": "..." }
fn emit_google_credentials() {
    println!("cargo:rerun-if-changed=google-client.json");

    let raw = fs::read_to_string("google-client.json").unwrap_or_default();
    let get = |key: &str| -> String {
        // serde를 빌드 의존성으로 끌어오지 않으려고 직접 긁는다. 값은 따옴표 안의 평범한 문자열뿐.
        raw.split_once(&format!("\"{key}\""))
            .and_then(|(_, rest)| rest.split_once(':'))
            .and_then(|(_, rest)| {
                let rest = rest.trim_start();
                rest.strip_prefix('"')?.split_once('"').map(|(v, _)| v.to_string())
            })
            .unwrap_or_default()
    };

    // 없으면 빈 문자열 — 앱이 "클라이언트 ID가 없습니다"라고 안내한다
    println!("cargo:rustc-env=GOOGLE_CLIENT_ID={}", get("client_id"));
    println!("cargo:rustc-env=GOOGLE_CLIENT_SECRET={}", get("client_secret"));
}

/// 공공데이터포털 서비스 키. src-tauri/holiday-api-key.txt 에 키 문자열만 넣는다.
fn emit_holiday_key() {
    println!("cargo:rerun-if-changed=holiday-api-key.txt");
    let key = fs::read_to_string("holiday-api-key.txt").unwrap_or_default();
    // 메모장이 BOM을 붙여 저장해도 키가 깨지지 않게. trim()은 BOM을 안 지운다
    let key = key.trim_start_matches('\u{feff}').trim();
    println!("cargo:rustc-env=HOLIDAY_API_KEY={key}");
}

fn main() {
    emit_google_credentials();
    emit_holiday_key();
    tauri_build::build()
}
