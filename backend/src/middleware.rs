use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::error::ErrorResponse;

/// JSON 値の文字列フィールドに null バイトが含まれているかを再帰的にチェックする
fn json_contains_null_byte(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(s) => s.contains('\0'),
        serde_json::Value::Array(arr) => arr.iter().any(json_contains_null_byte),
        serde_json::Value::Object(obj) => obj.values().any(json_contains_null_byte),
        _ => false,
    }
}

/// リクエストに null バイト (\0) が含まれている場合に 400 を返すミドルウェア。
/// PostgreSQL の TEXT/VARCHAR カラムは null バイトを受け付けないため、
/// ハンドラに到達する前に一括で拒否する。
///
/// 検査対象:
/// - URL パス (パーセントデコード後の %00 を検出)
/// - リクエストボディ (バイトレベルの 0x00 + JSON エスケープ `\u0000`)
pub async fn reject_null_bytes(request: Request, next: Next) -> Response {
    // URL パスにパーセントエンコードされた null バイト (%00) が含まれていないか検査する。
    // Axum がパスパラメータをデコードする前にここで拒否することで、
    // DB クエリに null バイトが到達するのを防ぐ。
    if let Ok(decoded) = percent_encoding::percent_decode_str(request.uri().path()).decode_utf8()
        && decoded.contains('\0')
    {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(ErrorResponse {
                error: "request path must not contain null bytes".to_string(),
            }),
        )
            .into_response();
    }

    let (parts, body) = request.into_parts();

    let bytes = match axum::body::to_bytes(body, 1024 * 1024).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                axum::Json(ErrorResponse {
                    error: "request body too large".to_string(),
                }),
            )
                .into_response();
        }
    };

    // バイトレベルで生の 0x00 を検出
    if bytes.contains(&0) {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(ErrorResponse {
                error: "request body must not contain null bytes".to_string(),
            }),
        )
            .into_response();
    }

    // JSON ボディの場合、デシリアライズ後の文字列値に null バイトが含まれていないか検査する。
    // \u0000 のような JSON エスケープはバイトレベルでは検出できないため。
    let is_json = parts
        .headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("application/json"));

    if is_json
        && !bytes.is_empty()
        && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes)
        && json_contains_null_byte(&value)
    {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(ErrorResponse {
                error: "request body must not contain null bytes".to_string(),
            }),
        )
            .into_response();
    }

    let request = Request::from_parts(parts, Body::from(bytes));
    next.run(request).await
}
