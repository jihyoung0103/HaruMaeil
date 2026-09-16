# 하루매일 (HaruMaeil)

바탕화면에 붙는 가벼운 캘린더 위젯. 구글 캘린더와 **구글 할 일**을 한 화면에서 봅니다.

- 홈페이지: https://jihyoung0103.github.io/HaruMaeil/
- 개인정보처리방침: https://jihyoung0103.github.io/HaruMaeil/privacy.html
- 할 일 목록: [TODO.md](TODO.md)

## 기능

- 월간 달력 — 여러 날 일정은 칸을 가로지르는 띠로, 일요일/월요일 시작 선택
- 바탕화면 위젯 — 아이콘 뒤에 붙음, 위치·크기 조정, 크기에 맞춰 격자 스케일
- 구글 캘린더 · 구글 할 일 가져오기 — 캘린더/할 일 목록별 색
- 한국 공휴일 — 한국천문연구원 특일 정보 (대체공휴일 포함)
- 로컬 일정·할 일 추가/삭제/완료 (SQLite)

## 스택

Tauri 2 · Svelte 5 + TypeScript · Rust · SQLite

## 개발 환경

필요: Node 22, Rust (MSVC), Visual Studio Build Tools

```bash
npm install
npm run tauri dev
```

`npm run dev`는 화면(vite)만 띄우고 앱 창은 안 뜹니다. 앱은 항상 `npm run tauri dev`.

### 자격증명 (저장소에 올리지 않음)

두 파일 모두 `src-tauri/.gitignore`에 들어 있고 빌드할 때 `build.rs`가 바이너리에 넣습니다.
없어도 빌드는 되고, 해당 기능만 안내 메시지를 띄웁니다.

> **키 파일을 넣거나 바꾼 뒤에는 `npm run tauri dev`를 껐다 켜세요.** 두 파일은 gitignore에 있어서
> `tauri dev`가 변경을 감시하지 않습니다. 옮기거나 복사해 넣은 파일은 수정 시각이 옛날로 남아
> cargo가 바뀐 줄 모를 수 있으니, 그럴 땐 파일을 한 번 다시 저장한 뒤 껐다 켜세요.

| 파일 | 내용 | 받는 곳 |
|---|---|---|
| `src-tauri/google-client.json` | 구글 OAuth 클라이언트 (데스크톱 앱) | 구글 클라우드 콘솔 → 사용자 인증 정보. 콘솔이 주는 JSON 그대로 저장해도 됨 |
| `src-tauri/holiday-api-key.txt` | 공공데이터포털 서비스 키 한 줄 | 공공데이터포털 → 한국천문연구원_특일 정보 활용신청 |

구글 클라우드 프로젝트에서는 **Google Calendar API**와 **Google Tasks API**를 사용 설정하고,
OAuth 동의 화면을 **프로덕션**으로 게시해야 합니다 (테스트 상태면 인증이 7일마다 만료됨).

## 테스트

```bash
node --experimental-strip-types src/lib/calendar.test.ts
cd src-tauri && cargo test --lib
```

## 데이터 위치

`%APPDATA%\com.jihyo.harumaeil\` — `harumaeil.db`(일정), `widget.json`(위젯 위치).
구글 리프레시 토큰은 Windows 자격 증명 관리자에 저장됩니다.
