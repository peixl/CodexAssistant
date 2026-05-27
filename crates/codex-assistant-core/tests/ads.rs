use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use codex_assistant_core::ads::{
    DEFAULT_AD_LIST_URLS, cache_busted_ad_url, fetch_ad_list_from_urls_with_client,
    normalize_ad_payload,
};
use serde_json::json;

#[test]
fn default_ad_urls_match_legacy_helper_sources() {
    assert_eq!(
        DEFAULT_AD_LIST_URLS,
        [
            "https://raw.githubusercontent.com/peixl/Ad-List/main/ads.json",
            "https://cdn.jsdelivr.net/gh/peixl/Ad-List@main/ads.json",
        ]
    );
}

#[test]
fn cache_busted_ad_url_appends_version_query_to_plain_url() {
    assert_eq!(
        cache_busted_ad_url("https://example.test/ads.json", 1779035222758),
        "https://example.test/ads.json?v=1779035222758"
    );
}

#[test]
fn cache_busted_ad_url_preserves_existing_query() {
    assert_eq!(
        cache_busted_ad_url("https://example.test/ads.json?source=cdn", 1779035222758),
        "https://example.test/ads.json?source=cdn&v=1779035222758"
    );
}

#[test]
fn normalizes_remote_ads_for_plugin_and_manager_rendering() {
    let payload = normalize_ad_payload(json!({
        "version": 1,
        "ads": [
            {
                "id": "sponsor",
                "type": "sponsor",
                "title": "赞助商",
                "description": "推荐内容",
                "url": "https://example.test",
                "highlights": ["稳定"]
            },
            {
                "id": "normal",
                "type": "normal",
                "title": "普通推荐",
                "description": "推荐内容",
                "url": "https://example.org"
            },
            {
                "id": "broken",
                "type": "normal",
                "title": "",
                "description": "missing title",
                "url": "https://example.invalid"
            }
        ]
    }));

    assert_eq!(payload["version"], json!(1));
    assert_eq!(payload["ads"].as_array().unwrap().len(), 2);
    assert_eq!(payload["ads"][0]["type"], json!("sponsor"));
    assert_eq!(payload["ads"][1]["type"], json!("normal"));
}

#[tokio::test]
async fn fetch_ad_list_tries_backup_url_when_primary_fails() {
    if !loopback_tcp_available() {
        eprintln!("skipping loopback HTTP test because 127.0.0.1 TCP is unavailable");
        return;
    }

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener
        .set_nonblocking(true)
        .expect("listener should switch to nonblocking mode");
    let thread = thread::spawn(move || {
        let started = std::time::Instant::now();
        let mut handled = 0;
        while handled < 2 && started.elapsed() < Duration::from_secs(5) {
            let Ok((mut stream, _)) = listener.accept() else {
                thread::sleep(Duration::from_millis(10));
                continue;
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("stream should accept read timeout");
            let mut buffer = [0; 1024];
            let read = stream.read(&mut buffer).unwrap();
            let request = String::from_utf8_lossy(&buffer[..read]);
            if request.starts_with("GET /primary.json?") {
                stream
                    .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .unwrap();
            } else {
                assert!(request.starts_with("GET /backup.json?"), "{request}");
                let body = json!({
                    "version": 1,
                    "ads": [{
                        "id": "backup-ad",
                        "type": "normal",
                        "title": "Backup",
                        "description": "Loaded from backup",
                        "url": "https://example.test",
                        "highlights": []
                    }]
                })
                .to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
            handled += 1;
        }
    });

    let client = reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(1))
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    let payload = fetch_ad_list_from_urls_with_client(
        &client,
        &[
            format!("http://127.0.0.1:{port}/primary.json"),
            format!("http://127.0.0.1:{port}/backup.json"),
        ],
    )
    .await
    .unwrap();
    thread.join().unwrap();

    assert_eq!(payload["ads"][0]["id"], json!("backup-ad"));
}

fn loopback_tcp_available() -> bool {
    let Ok(listener) = TcpListener::bind("127.0.0.1:0") else {
        return false;
    };
    if listener.set_nonblocking(true).is_err() {
        return false;
    }
    let Ok(address) = listener.local_addr() else {
        return false;
    };
    let thread = thread::spawn(move || {
        let started = std::time::Instant::now();
        while started.elapsed() < Duration::from_millis(500) {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = stream.write_all(b"ok");
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
    });
    let available = can_read_loopback_probe(address);
    let _ = thread.join();
    available
}

fn can_read_loopback_probe(address: SocketAddr) -> bool {
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(250)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    let mut buffer = [0u8; 2];
    stream.read_exact(&mut buffer).is_ok() && buffer == *b"ok"
}
