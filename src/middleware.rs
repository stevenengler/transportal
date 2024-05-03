use axum::body::{Body, BodyDataStream, Bytes, HttpBody};
use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use flate2::write::GzEncoder;
use flate2::Compression;
use futures_util::stream::Stream;

use std::io::Write;
use std::pin::{pin, Pin};
use std::task::Context;
use std::task::Poll;

pub async fn unauthorized_redirect(request: Request, next: Next) -> Response {
    let accept = request.headers().get(header::ACCEPT).cloned();

    let mut response = next.run(request).await;

    if response.status() == StatusCode::UNAUTHORIZED {
        let content_type = response.headers().get(header::CONTENT_TYPE);
        let is_empty = response.body().is_end_stream();

        let response_can_be_html = if let Some(content_type) = content_type {
            content_type.to_str().ok() == Some("text/html")
        } else {
            true
        };

        let request_allows_html = if let Some(accept) = accept {
            accept
                .to_str()
                .ok()
                .map(|x| x.split(',').any(|x| x == "text/html"))
                .unwrap_or(false)
        } else {
            true
        };

        if is_empty && request_allows_html && response_can_be_html {
            let html =
                r#"<meta http-equiv="refresh" content="0; url=/login"> Unauthorized. Redirecting."#;
            *response.body_mut() = Body::from(html);

            let headers = response.headers_mut();

            headers.insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_str("text/html").unwrap(),
            );

            if let Some(len) = headers.remove(header::CONTENT_LENGTH) {
                assert_eq!(len, "0");
            }
        }
    }

    response
}

pub async fn compress_sse(request: Request, next: Next) -> Response {
    let accept_encoding = request.headers().get(header::ACCEPT_ENCODING).cloned();

    let response = next.run(request).await;

    if let Some(content_type) = response.headers().get(header::CONTENT_TYPE) {
        // if the response content type is not for an sse stream
        if trim_whitespace(content_type.as_bytes()) != b"text/event-stream" {
            return response;
        }
    } else {
        // if the response has no Content-Type header
        return response;
    }

    if response.headers().contains_key(header::CONTENT_ENCODING) {
        // response already has an encoding
        return response;
    }

    if let Some(accept_encoding) = accept_encoding {
        // if no accepted encoding options are gzip
        if accept_encoding
            .as_bytes()
            .split(|x| *x == b","[0])
            .all(|x| trim_whitespace(x) != b"gzip")
        {
            return response;
        }
    } else {
        // if no Accept-Encoding header
        return response;
    }

    let (mut parts, body) = response.into_parts();

    let body = body.into_data_stream();
    let body = Body::from_stream(CompressedStream::new(body));

    parts.headers.insert(
        header::CONTENT_ENCODING,
        header::HeaderValue::from_static("gzip"),
    );

    Response::from_parts(parts, body)
}

struct CompressedStream {
    inner: BodyDataStream,
    compression: GzEncoder<Vec<u8>>,
}

impl CompressedStream {
    pub fn new(body: BodyDataStream) -> Self {
        Self {
            inner: body,
            compression: GzEncoder::new(Vec::new(), Compression::default()),
        }
    }
}

impl Stream for CompressedStream {
    type Item = Result<Bytes, axum::Error>;

    #[inline]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match pin!(&mut self.inner).as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(x))) => {
                self.compression.write_all(&x).unwrap();
                self.compression.flush().unwrap();

                let mut buf = Vec::new();
                std::mem::swap(&mut buf, self.compression.get_mut());

                Poll::Ready(Some(Ok(buf.into())))
            }
            x => x,
        }
    }
}

fn trim_whitespace(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|x| !x.is_ascii_whitespace())
        .unwrap_or(bytes.len());

    let bytes = &bytes[start..];

    let end = bytes
        .iter()
        .rposition(|x| !x.is_ascii_whitespace())
        .map(|x| x + 1)
        .unwrap_or(0);

    &bytes[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_whitespace() {
        assert_eq!(trim_whitespace(b""), b"");
        assert_eq!(trim_whitespace(b" "), b"");
        assert_eq!(trim_whitespace(b"abc"), b"abc");
        assert_eq!(trim_whitespace(b" abc"), b"abc");
        assert_eq!(trim_whitespace(b"abc "), b"abc");
        assert_eq!(trim_whitespace(b" abc "), b"abc");
        assert_eq!(trim_whitespace(b"   abc  "), b"abc");
        assert_eq!(trim_whitespace(b"hello world"), b"hello world");
        assert_eq!(trim_whitespace(b" hello world "), b"hello world");
        assert_eq!(trim_whitespace(b"\thello world\t"), b"hello world");
        assert_eq!(trim_whitespace(b" \t hello world \t "), b"hello world");
    }
}
