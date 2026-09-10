//! Bounded HTTPS streaming with an idle timeout, not a one-minute package limit.
//! Partial bytes survive retries; callers must verify the trusted SHA-256 before use.
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
    time::Duration,
};

const ATTEMPTS: usize = 3;
const MAX_BYTES: u64 = 64 * 1024 * 1024 * 1024;

fn agent(idle: Duration, https_only: bool) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .https_only(https_only)
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(6 * 60 * 60)))
        .timeout_resolve(Some(Duration::from_secs(30)))
        .timeout_connect(Some(Duration::from_secs(30)))
        // ureq 3.4 also applies the PREVIOUS phase's configured duration from
        // that phase's completion time (CallTimings::next_timeout). SendRequest
        // therefore bounds the response headers. Setting RecvResponse to 60s
        // would ALSO impose a 60s total body deadline. Leave it unset; RecvBody
        // is recomputed for each read and supplies the idle limit instead.
        .timeout_send_request(Some(Duration::from_secs(60)))
        .timeout_recv_response(None)
        .timeout_recv_body(Some(idle))
        .build();
    config.into()
}

pub fn https(url: &str, destination: &Path) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("Downloads require HTTPS".into());
    }
    download(&agent(Duration::from_secs(60), true), url, destination)
}

fn download(agent: &ureq::Agent, url: &str, destination: &Path) -> Result<(), String> {
    crate::safe_path::reject_link_path(destination)?;
    let partial = destination.with_file_name(format!(
        "{}.partial",
        destination
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    ));
    crate::safe_path::reject_link_path(&partial)?;
    // Staging destinations are unique. Never inherit untrusted partials from a prior request.
    if partial.exists() {
        fs::remove_file(&partial).map_err(|e| e.to_string())?;
    }
    let mut last_error = String::new();
    for _ in 0..ATTEMPTS {
        match attempt(agent, url, &partial) {
            Ok(()) => {
                return fs::rename(&partial, destination)
                    .map_err(|e| format!("Could not activate downloaded package: {e}"));
            }
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "Download failed after {ATTEMPTS} resumable attempts: {last_error}"
    ))
}

fn attempt(agent: &ureq::Agent, url: &str, partial: &Path) -> Result<(), String> {
    let offset = fs::metadata(partial).map(|m| m.len()).unwrap_or(0);
    let mut request = agent.get(url).header("Accept-Encoding", "identity");
    if offset > 0 {
        request = request.header("Range", format!("bytes={offset}-"));
    }
    let mut response = request
        .call()
        .map_err(|e| format!("HTTPS request failed: {e}"))?;
    let headers = response.headers();
    if headers
        .get("content-encoding")
        .is_some_and(|v| v != "identity")
    {
        return Err("Package server returned an unexpected content encoding".into());
    }
    let length = headers
        .get("content-length")
        .map(|v| {
            v.to_str()
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or("Invalid Content-Length")
        })
        .transpose()?;
    let status = response.status().as_u16();
    let (append, total, expected_body) = match status {
        200 => (false, length, length), // Server ignored Range: restart safely, never append.
        206 => {
            let range = headers
                .get("content-range")
                .and_then(|v| v.to_str().ok())
                .ok_or("Missing Content-Range")?;
            let (start, end, total) = parse_range(range)?;
            if start != offset || end + 1 != total || length.is_some_and(|n| n != end - start + 1) {
                return Err("Package server returned an inconsistent resume range".into());
            }
            (true, Some(total), Some(end - start + 1))
        }
        416 => {
            let total = headers
                .get("content-range")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.strip_prefix("bytes */"))
                .and_then(|s| s.parse::<u64>().ok());
            if offset > 0 && total == Some(offset) {
                return Ok(());
            }
            return Err("Package server rejected the resume position".into());
        }
        _ => return Err(format!("Package server returned HTTP {status}")),
    };
    if total.is_some_and(|n| n > MAX_BYTES) {
        return Err("Package exceeds the 64 GiB download limit".into());
    }
    let existing = if append { offset } else { 0 };
    let free = fs2::available_space(partial.parent().unwrap_or(Path::new(".")))
        .map_err(|e| e.to_string())?;
    let remaining = total.unwrap_or(existing).saturating_sub(existing);
    if free < remaining.saturating_add(64 * 1024 * 1024) {
        return Err("Not enough free space for the package download".into());
    }
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(partial)
        .map_err(|e| format!("Could not create partial download: {e}"))?;
    let mut received = 0_u64;
    let result = (|| {
        let mut reader = response.body_mut().as_reader();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = reader
                .read(&mut buffer)
                .map_err(|e| format!("Download interrupted (will resume): {e}"))?;
            if count == 0 {
                break;
            }
            received += count as u64;
            if existing + received > MAX_BYTES || expected_body.is_some_and(|n| received > n) {
                return Err("Download exceeded its declared or maximum size".into());
            }
            file.write_all(&buffer[..count])
                .map_err(|e| format!("Could not save download: {e}"))?;
        }
        if expected_body.is_some_and(|n| received != n) {
            return Err("Package download ended before its declared size".into());
        }
        Ok(())
    })();
    // Flush even on an interrupted connection so Range starts at durable bytes.
    file.sync_all()
        .map_err(|e| format!("Could not flush download: {e}"))?;
    result
}

fn parse_range(value: &str) -> Result<(u64, u64, u64), String> {
    let invalid = || "Invalid Content-Range".to_string();
    let (range, total) = value
        .strip_prefix("bytes ")
        .and_then(|s| s.split_once('/'))
        .ok_or_else(invalid)?;
    let (start, end) = range.split_once('-').ok_or_else(invalid)?;
    let (start, end, total): (u64, u64, u64) = (
        start.parse().map_err(|_| invalid())?,
        end.parse().map_err(|_| invalid())?,
        total.parse().map_err(|_| invalid())?,
    );
    if start > end || end >= total || total > MAX_BYTES {
        return Err(invalid());
    }
    Ok((start, end, total))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        net::{TcpListener, TcpStream},
        thread,
    };

    fn request(stream: &mut TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            let mut b = [0];
            stream.read_exact(&mut b).unwrap();
            bytes.push(b[0]);
        }
        String::from_utf8(bytes).unwrap().to_lowercase()
    }

    #[test]
    fn interrupted_download_resumes_and_ignored_range_restarts() {
        for resume in [true, false] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/pack", listener.local_addr().unwrap());
            let server = thread::spawn(move || {
                let (mut first, _) = listener.accept().unwrap();
                assert!(!request(&mut first).contains("range:"));
                first
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabc",
                    )
                    .unwrap();
                drop(first);
                let (mut second, _) = listener.accept().unwrap();
                assert!(request(&mut second).contains("range: bytes=3-"));
                second.write_all(if resume {
                    b"HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 3-5/6\r\nContent-Length: 3\r\nConnection: close\r\n\r\ndef"
                } else {
                    b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabcdef"
                }).unwrap();
            });
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("pack.zip");
            download(&agent(Duration::from_secs(2), false), &url, &path).unwrap();
            assert_eq!(fs::read(path).unwrap(), b"abcdef");
            server.join().unwrap();
        }
    }

    fn progressing_download(idle: Duration, delay: Duration, count: usize) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/pack", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            request(&mut stream);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {count}\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
            for _ in 0..count {
                thread::sleep(delay);
                stream.write_all(b"x").unwrap();
            }
        });
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("pack.zip");
        download(&agent(idle, false), &url, &path).unwrap();
        assert_eq!(fs::metadata(path).unwrap().len(), count as u64);
        server.join().unwrap();
    }

    #[test]
    fn ongoing_progress_can_exceed_idle_timeout_in_total() {
        progressing_download(Duration::from_secs(2), Duration::from_millis(150), 20);
    }

    #[test]
    #[ignore = "65-second regression proving a transfer survives the old one-minute deadline"]
    fn progressing_download_survives_old_sixty_second_limit() {
        progressing_download(Duration::from_secs(60), Duration::from_secs(1), 65);
    }

    #[test]
    fn malformed_ranges_are_rejected() {
        for value in [
            "bytes 4-3/6",
            "bytes 0-6/6",
            "bytes 0-5/*",
            "bytes 0-18446744073709551615/6",
        ] {
            assert!(parse_range(value).is_err());
        }
        assert_eq!(parse_range("bytes 3-5/6").unwrap(), (3, 5, 6));
    }

    #[test]
    fn stalled_body_times_out_without_activating_package() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/pack", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            for _ in 0..ATTEMPTS {
                let (mut stream, _) = listener.accept().unwrap();
                request(&mut stream);
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n")
                    .unwrap();
                thread::sleep(Duration::from_millis(500));
            }
        });
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("pack.zip");
        assert!(download(&agent(Duration::from_millis(200), false), &url, &path).is_err());
        assert!(!path.exists());
        server.join().unwrap();
    }
}
