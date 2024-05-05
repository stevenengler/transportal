macro_rules! css {
    ($path:literal) => {
        static_content!($path, "text/css")
    };
}

macro_rules! js {
    ($path:literal) => {
        static_content!($path, "text/javascript")
    };
}

macro_rules! json {
    ($path:literal) => {
        static_content!($path, "application/json")
    };
}

macro_rules! static_content {
    ($path:literal, $mime:literal) => {{
        const DATA: &[u8] = ::std::include_bytes!(::std::concat!(
            ::std::env!("CARGO_MANIFEST_DIR"),
            "/",
            $path,
        ));

        let hash = {
            let mut hasher = ::std::hash::DefaultHasher::new();
            <::std::hash::DefaultHasher as ::std::hash::Hasher>::write(&mut hasher, DATA);
            <::std::hash::DefaultHasher as ::std::hash::Hasher>::finish(&mut hasher)
        };

        // we only leak the memory where the macro is called, not every request
        let etag = &*::std::format!("\"{hash}\"").leak();

        let resp_headers: [(::axum::http::header::HeaderName, &str); 2] = [
            (::axum::http::header::CONTENT_TYPE, $mime),
            (::axum::http::header::ETAG, etag),
        ];

        ::axum::routing::get(
            move |req_headers: ::axum::http::header::HeaderMap| async move {
                if let Some(x) = req_headers.get(::axum::http::header::IF_NONE_MATCH) {
                    if x.as_bytes() == etag.as_bytes() {
                        return Err(::axum::http::StatusCode::NOT_MODIFIED);
                    }
                }

                Ok((resp_headers, DATA))
            },
        )
    }};
}
