use anyhow::Context;
use axum::http::Request;
use axum::Router;
use hyper::body::Incoming;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use tokio::net::UnixListener;
use tower::Service;

use std::ffi::CString;
use std::io::{Error, ErrorKind};
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileTypeExt;
use std::path::Path;

/// Serve `app` at a unix socket bound to `bind_addr` with `perms` permissions. Any existing unix
/// socket at the given path will be removed.
pub async fn serve<P: AsRef<Path>>(bind_addr: P, perms: u32, app: Router) -> anyhow::Result<()> {
    let bind_addr = bind_addr.as_ref();

    // delete any existing unix socket
    if let Ok(metadata) = std::fs::metadata(bind_addr) {
        // there's a race condition here between when we check the file type and when we
        // delete the file, but not much we can do about that
        if metadata.file_type().is_socket() {
            std::fs::remove_file(bind_addr).context(format!(
                r#"Failed to remove socket file "{}""#,
                bind_addr.display()
            ))?;
        }
    }

    let listener =
        unix_stream_socket(/* non_blocking= */ true).context("Failed to create unix socket")?;

    // when bind() is called, it applies the umask to these permissions
    fchmod(listener.as_fd(), perms).context("Failed to fchmod socket")?;

    bind(listener.as_fd(), bind_addr).context(format!(
        r#"Failed to bind socket to "{}""#,
        bind_addr.display()
    ))?;

    listen(listener.as_fd(), 1024).context("Failed to mark socket as listening")?;

    // since the umask applied during the fchmod + bind will result in more-restrictive permissions
    // than the user asked for, we need to chmod the path to apply the requested permissions
    chmod(bind_addr, perms).context("Failed to chmod socket file path")?;

    let listener =
        UnixListener::from_std(listener.into()).context("Failed to convert to tokio socket")?;

    let mut make_service = app.into_make_service();

    // adapted from the example at
    // https://github.com/tokio-rs/axum/blob/e3bb7083c886247f4e6931e149ef6067e6b82e1b/examples/unix-domain-socket/src/main.rs
    loop {
        let (socket, _remote_addr) = listener.accept().await.context("Failed to accept socket")?;

        let tower_service = unwrap_infallible(make_service.call(&socket).await);

        tokio::spawn(async move {
            let socket = TokioIo::new(socket);

            let hyper_service = hyper::service::service_fn(move |request: Request<Incoming>| {
                tower_service.clone().call(request)
            });

            let builder = Builder::new(TokioExecutor::new());

            if let Err(_err) = builder
                .serve_connection_with_upgrades(socket, hyper_service)
                .await
            {
                // this can error for long-lived sse connections
            }
        });
    }
}

fn unix_stream_socket(non_blocking: bool) -> std::io::Result<OwnedFd> {
    let mut flags = libc::SOCK_CLOEXEC;

    if non_blocking {
        flags |= libc::SOCK_NONBLOCK;
    }

    let sock = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM | flags, 0) };
    if sock < 0 {
        return Err(Error::last_os_error());
    }

    let sock = unsafe { OwnedFd::from_raw_fd(sock) };

    Ok(sock)
}

fn bind<S: AsRawFd, P: AsRef<Path>>(sock: S, path: P) -> std::io::Result<()> {
    let sock = sock.as_raw_fd();
    let path = path.as_ref().as_os_str().as_bytes();

    let mut addr = libc::sockaddr_un {
        sun_family: libc::AF_UNIX as u16,
        sun_path: [0; 108],
    };

    let path = u8_slice_to_c_char(path);

    // make sure we leave a nul byte at the end of the array
    let sun_path_len = addr.sun_path.len();
    let sun_path = &mut addr.sun_path[..sun_path_len - 1];

    if path.len() > sun_path.len() {
        return Err(Error::new(ErrorKind::Other, "Path too long"));
    }

    sun_path[..path.len()].copy_from_slice(path);

    let (addr_ptr, addr_len) = ptr_and_len(&addr);

    let addr_ptr = addr_ptr as *const libc::sockaddr;
    let addr_len: libc::socklen_t = addr_len.try_into().unwrap();

    let rv = unsafe { libc::bind(sock, addr_ptr, addr_len) };
    if rv != 0 {
        return Err(Error::last_os_error());
    }

    Ok(())
}

fn listen<S: AsRawFd>(sock: S, backlog: libc::c_int) -> std::io::Result<()> {
    let sock = sock.as_raw_fd();

    let rv = unsafe { libc::listen(sock, backlog) };
    if rv != 0 {
        return Err(Error::last_os_error());
    }

    Ok(())
}

fn fchmod<S: AsRawFd>(sock: S, perms: u32) -> std::io::Result<()> {
    let sock = sock.as_raw_fd();

    let rv = unsafe { libc::fchmod(sock, perms) };
    if rv != 0 {
        return Err(Error::last_os_error());
    }

    Ok(())
}

fn chmod<P: AsRef<Path>>(path: P, perms: u32) -> std::io::Result<()> {
    let path = path.as_ref().as_os_str().as_bytes();
    let path = CString::new(path).unwrap();

    let rv = unsafe { libc::chmod(path.as_ptr(), perms) };
    if rv != 0 {
        return Err(Error::last_os_error());
    }

    Ok(())
}

fn ptr_and_len<T>(x: &T) -> (*const T, usize) {
    (std::ptr::from_ref(x), std::mem::size_of_val(x))
}

fn u8_slice_to_c_char(x: &[u8]) -> &[libc::c_char] {
    // platforms should typecast c_char as u8 or i8
    const _: () = assert!(std::mem::size_of::<libc::c_char>() == 1);
    const _: () = assert!(std::mem::align_of::<libc::c_char>() == 1);
    unsafe { std::slice::from_raw_parts(x.as_ptr() as *const libc::c_char, x.len()) }
}

fn unwrap_infallible<T>(result: Result<T, std::convert::Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(err) => match err {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ptr_and_len() {
        let x = [0u8; 10];
        let (_ptr, len) = ptr_and_len(&x);
        assert_eq!(len, 10);
    }

    #[test]
    fn test_u8_slice_to_c_char() {
        let x = [1u8, 2u8, 3u8];
        assert_eq!(u8_slice_to_c_char(&x), [1, 2, 3]);

        let x = [1u8, 255u8];
        assert_eq!(u8_slice_to_c_char(&x), [1, -1i8 as libc::c_char]);

        assert_eq!(u8_slice_to_c_char(&[]), [0; 0]);
    }
}
