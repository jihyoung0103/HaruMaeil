//! 한국 공휴일 — 한국천문연구원 특일 정보 API (공공데이터포털).
//!
//! 음력 명절과 대체공휴일을 계산하지 않고 공식 데이터로 받는다. 서비스 키는 구글 시크릿처럼
//! 저장소 밖 파일(src-tauri/holiday-api-key.txt)에서 build.rs가 주입한다.

use oauth2::reqwest;
use serde::Serialize;
use serde_json::Value;

const URL: &str = "https://apis.data.go.kr/B090041/openapi/service/SpcdeInfoService/getRestDeInfo";
const API_KEY: &str = env!("HOLIDAY_API_KEY");

#[derive(Serialize, Debug, PartialEq)]
pub struct Holiday {
    /// YYYY-MM-DD
    pub date: String,
    pub name: String,
}

/// 응답 본문에서 공휴일만 뽑는다. 이 API는 결과 모양이 개수에 따라 바뀐다:
/// 0건이면 items가 "" (빈 문자열), 1건이면 item이 배열이 아닌 객체, 2건 이상이면 배열.
/// 키가 틀리면 _type=json을 줘도 XML로 에러가 온다.
fn parse(body: &str) -> Result<Vec<Holiday>, String> {
    let snippet = || body.chars().take(300).collect::<String>();
    let v: Value = serde_json::from_str(body)
        .map_err(|_| format!("공휴일 API가 JSON이 아닌 응답을 줬습니다 (키 문제일 수 있음): {}", snippet()))?;

    let code = v["response"]["header"]["resultCode"].as_str().unwrap_or("");
    if code != "00" {
        return Err(format!("공휴일 API 오류: {}", snippet()));
    }

    let item = &v["response"]["body"]["items"]["item"];
    let list: Vec<&Value> = match item {
        Value::Array(a) => a.iter().collect(),
        Value::Object(_) => vec![item],
        _ => vec![],
    };

    Ok(list
        .into_iter()
        .filter(|it| it["isHoliday"].as_str() == Some("Y"))
        .filter_map(|it| {
            // locdate는 숫자(20260925)로 오지만 문자열로 올 때도 대비
            let d = match &it["locdate"] {
                Value::Number(n) => n.to_string(),
                Value::String(s) => s.clone(),
                _ => return None,
            };
            if d.len() != 8 {
                return None;
            }
            Some(Holiday {
                date: format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..]),
                name: it["dateName"].as_str()?.to_string(),
            })
        })
        .collect())
}

/// 한 해의 공휴일. 연 단위 조회가 되는지 문서가 불분명해서 월별로 12번 묻는다.
#[tauri::command]
pub async fn holidays(year: i32) -> Result<Vec<Holiday>, String> {
    if API_KEY.is_empty() {
        return Err("빌드에 공휴일 API 키가 없습니다 (src-tauri/holiday-api-key.txt)".into());
    }
    // 포털은 '인코딩' 키와 '디코딩' 키를 둘 다 준다. query()가 다시 인코딩하므로
    // 인코딩 키가 들어와도 한 번 풀어서 이중 인코딩을 막는다.
    let key = percent_encoding::percent_decode_str(API_KEY)
        .decode_utf8_lossy()
        .to_string();
    let http = reqwest::Client::new();

    let mut out = Vec::new();
    for month in 1..=12 {
        let body = http
            .get(URL)
            .query(&[
                ("serviceKey", key.as_str()),
                ("solYear", &year.to_string()),
                ("solMonth", &format!("{month:02}")),
                ("numOfRows", "100"),
                ("_type", "json"),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?
            .text()
            .await
            .map_err(|e| e.to_string())?;
        out.extend(parse(&body)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{parse, Holiday};

    fn ok(items: &str) -> String {
        format!(
            r#"{{"response":{{"header":{{"resultCode":"00","resultMsg":"NORMAL SERVICE."}},"body":{{"items":{items},"numOfRows":100,"pageNo":1,"totalCount":0}}}}}}"#
        )
    }

    #[test]
    fn many_items_array() {
        let body = ok(r#"{"item":[
            {"dateKind":"01","dateName":"추석","isHoliday":"Y","locdate":20260924,"seq":1},
            {"dateKind":"01","dateName":"추석","isHoliday":"Y","locdate":20260925,"seq":1},
            {"dateKind":"01","dateName":"평일인 날","isHoliday":"N","locdate":20260926,"seq":1}
        ]}"#);
        let got = parse(&body).unwrap();
        assert_eq!(got.len(), 2, "isHoliday=N은 빠져야 함");
        assert_eq!(got[1], Holiday { date: "2026-09-25".into(), name: "추석".into() });
    }

    #[test]
    fn single_item_is_object_not_array() {
        let body = ok(r#"{"item":{"dateKind":"01","dateName":"개천절","isHoliday":"Y","locdate":20261003,"seq":1}}"#);
        assert_eq!(parse(&body).unwrap(), vec![Holiday { date: "2026-10-03".into(), name: "개천절".into() }]);
    }

    #[test]
    fn empty_month_items_is_empty_string() {
        assert!(parse(&ok(r#""""#)).unwrap().is_empty());
    }

    #[test]
    fn bad_key_xml_is_reported() {
        let xml = "<OpenAPI_ServiceResponse><cmmMsgHeader><returnAuthMsg>SERVICE_KEY_IS_NOT_REGISTERED_ERROR</returnAuthMsg></cmmMsgHeader></OpenAPI_ServiceResponse>";
        let err = parse(xml).unwrap_err();
        assert!(err.contains("SERVICE_KEY_IS_NOT_REGISTERED_ERROR"), "{err}");
    }

    #[test]
    fn api_error_code_is_reported() {
        let body = r#"{"response":{"header":{"resultCode":"30","resultMsg":"SERVICE KEY IS NOT REGISTERED ERROR."}}}"#;
        assert!(parse(body).unwrap_err().contains("NOT REGISTERED"));
    }
}
