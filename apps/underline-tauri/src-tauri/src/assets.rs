//! ADR-023: the app's own protocol `ul-asset`, the only way pictures reach
//! the window: image files of the workspace and the facade's diagrams.
//! `underline_session::Session::asset_plan` decides what is served; the
//! session is held only for that. The plan is resolved (with its file-system
//! checks) and the file read off the main thread, without the session.
//!
//! The window names an asset `ul-asset://localhost/{path}` on Linux and
//! macOS, `http://ul-asset.localhost/{path}` on Windows (WebView2).

use std::io::Read;
use std::path::Path;

use tauri::http::{header, HeaderValue, Method, Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};
use underline_session::assets::{Asset, AssetError, MAX_BYTES};

use crate::commands::SharedSession;

/// The scheme of the protocol.
pub const SCHEME: &str = "ul-asset";

/// An SVG is shown as a picture; should it be opened as a document, it may
/// still run nothing and load nothing.
const SVG_POLICY: &str = "default-src 'none'; style-src 'unsafe-inline'; sandbox";

/// Every address carries the version of what it serves (`?v=`), so an
/// answer stays good.
const CACHE: &str = "max-age=31536000, immutable";

/// Answers a request of the webview for `ul-asset`.
pub fn handle<R: Runtime>(
    context: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    if request.method() != Method::GET {
        responder.respond(status(StatusCode::METHOD_NOT_ALLOWED));
        return;
    }
    let app = context.app_handle().clone();
    let path = request.uri().path().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let plan = app.state::<SharedSession>().lock().asset_plan(&path);
        let asset = plan.and_then(underline_session::assets::AssetPlan::resolve);
        responder.respond(response(asset, read));
    });
}

/// The bytes of a file the protocol serves, at most [`MAX_BYTES`]: read up
/// to one byte more, as the file may have grown since it was checked.
fn read(path: &Path) -> Result<Vec<u8>, AssetError> {
    let file = std::fs::File::open(path).map_err(|_| AssetError::NotFound)?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AssetError::NotFound)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(AssetError::Refused);
    }
    Ok(bytes)
}

/// An answer without a body.
fn status(code: StatusCode) -> Response<Vec<u8>> {
    let mut answer = Response::new(Vec::new());
    *answer.status_mut() = code;
    answer
}

/// The answer for `asset`; files are read with `read`.
pub fn response(
    asset: Result<Asset, AssetError>,
    read: impl FnOnce(&Path) -> Result<Vec<u8>, AssetError>,
) -> Response<Vec<u8>> {
    let served = asset.and_then(|asset| match asset {
        Asset::Svg(svg) => Ok((svg.as_bytes().to_vec(), "image/svg+xml")),
        Asset::File { path, mime } => read(&path).map(|bytes| (bytes, mime)),
    });
    match served {
        Ok((bytes, mime)) => {
            let mut answer = Response::new(bytes);
            let headers = answer.headers_mut();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
            headers.insert(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            );
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(CACHE));
            if mime == "image/svg+xml" {
                headers.insert(
                    header::CONTENT_SECURITY_POLICY,
                    HeaderValue::from_static(SVG_POLICY),
                );
            }
            answer
        }
        Err(AssetError::Refused) => status(StatusCode::FORBIDDEN),
        Err(AssetError::NotFound) => status(StatusCode::NOT_FOUND),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::*;

    #[test]
    fn adr023_a_diagram_is_served_as_svg_that_runs_nothing() {
        let answer = response(Ok(Asset::Svg(Arc::from("<svg/>"))), |_| {
            panic!("no file is read")
        });

        assert_eq!(answer.status(), StatusCode::OK);
        let headers = answer.headers();
        assert_eq!(headers[header::CONTENT_TYPE], "image/svg+xml");
        assert_eq!(headers[header::CONTENT_SECURITY_POLICY], SVG_POLICY);
        assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(headers[header::CACHE_CONTROL], CACHE);
        assert_eq!(answer.body(), b"<svg/>");
    }

    #[test]
    fn adr023_a_file_is_read_and_served_with_its_type() {
        let answer = response(
            Ok(Asset::File {
                path: PathBuf::from("/ws/a.png"),
                mime: "image/png",
            }),
            |path: &Path| {
                assert_eq!(path, Path::new("/ws/a.png"));
                Ok(vec![1, 2, 3])
            },
        );

        assert_eq!(answer.status(), StatusCode::OK);
        assert_eq!(answer.headers()[header::CONTENT_TYPE], "image/png");
        assert!(answer
            .headers()
            .get(header::CONTENT_SECURITY_POLICY)
            .is_none());
        assert_eq!(answer.body(), &[1, 2, 3]);
    }

    #[test]
    fn adr023_refused_and_missing_assets_are_not_served() {
        let refused = response(Err(AssetError::Refused), |_| Ok(Vec::new()));
        let missing = response(Err(AssetError::NotFound), |_| Ok(Vec::new()));
        let unread = response(
            Ok(Asset::File {
                path: PathBuf::from("/ws/gone.png"),
                mime: "image/png",
            }),
            |_| Err(AssetError::NotFound),
        );

        assert_eq!(refused.status(), StatusCode::FORBIDDEN);
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        assert_eq!(unread.status(), StatusCode::NOT_FOUND);
        assert!(refused.body().is_empty());
    }

    #[test]
    fn adr023_a_file_larger_than_allowed_is_not_read_whole() {
        let path = std::env::temp_dir().join(format!("ul-asset-big-{}.png", std::process::id()));
        std::fs::write(&path, b"small").unwrap();
        assert_eq!(read(&path).unwrap(), b"small");
        assert_eq!(
            read(Path::new("/no/such/file.png")),
            Err(AssetError::NotFound)
        );
        std::fs::remove_file(&path).unwrap();
    }
}
