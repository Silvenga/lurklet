use assert_fs::TempDir;
use axum::Router;
use axum::http::{HeaderMap, StatusCode, header};
use axum::routing::get;
use lurklet_api::ResponseCache;
use std::env;
use std::io::Read;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::net::TcpListener as AsyncTcpListener;
use tokio::task::JoinSet;
use tokio::time::{sleep, timeout};

struct ServerProcess {
    child: Child,
    url: String,
}

impl ServerProcess {
    async fn start(root: &Path, label: &str, client: &wreq::Client) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let child = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "when_processes_overlap_then_they_should_share_complete_cache_entries",
            ])
            .env("LURKLET_TEST_CACHE_SERVER_ROOT", root)
            .env("LURKLET_TEST_CACHE_SERVER_BIND", address.to_string())
            .env("LURKLET_TEST_CACHE_SERVER_LABEL", label)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut server = Self {
            child,
            url: format!("http://{address}"),
        };
        timeout(Duration::from_secs(10), async {
            loop {
                if let Some(status) = server.child.try_wait().unwrap() {
                    let mut stderr = String::new();
                    server
                        .child
                        .stderr
                        .as_mut()
                        .unwrap()
                        .read_to_string(&mut stderr)
                        .unwrap();
                    panic!("cache fixture server exited: {status}: {stderr}");
                }
                if let Ok(response) = client.get(format!("{}/health", server.url)).send().await {
                    assert_eq!(response.status(), StatusCode::OK);
                    break;
                }
                sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        server
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test]
async fn when_processes_overlap_then_they_should_share_complete_cache_entries() {
    if let Some(root) = env::var_os("LURKLET_TEST_CACHE_SERVER_ROOT") {
        let cache = ResponseCache::new(root).await.unwrap();
        let label = env::var("LURKLET_TEST_CACHE_SERVER_LABEL").unwrap();
        let app = cache.wrap(Router::new()
            .route("/health", get(|| async { StatusCode::OK }))
            .route("/resource", get({
                let label = label.clone();
                move || { let label = label.clone(); async move { ([(header::CACHE_CONTROL, "public, max-age=3600")], label) } }
            }))
            .route("/volatile", get(move |headers: HeaderMap| {
                let label = label.clone();
                async move {
                    let status = if headers.contains_key("x-test-failure") { StatusCode::NOT_FOUND } else { StatusCode::OK };
                    (status, [(header::CACHE_CONTROL, "public, max-age=0")], label)
                }
            })));
        let listener = AsyncTcpListener::bind(env::var("LURKLET_TEST_CACHE_SERVER_BIND").unwrap())
            .await
            .unwrap();
        axum::serve(listener, app).await.unwrap();
        return;
    }

    let directory = TempDir::new().unwrap();
    let client = wreq::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let first = ServerProcess::start(directory.path(), "first", &client).await;
    let initial = client
        .get(format!("{}/resource?b=2&a=1", first.url))
        .send()
        .await
        .unwrap();
    assert_eq!(initial.headers()["x-lurklet-cache"], "miss");
    assert_eq!(initial.text().await.unwrap(), "first");
    let second = ServerProcess::start(directory.path(), "second", &client).await;

    let shared = client
        .get(format!("{}/resource?a=1&b=2", second.url))
        .send()
        .await
        .unwrap();
    let head = client
        .head(format!("{}/resource?a=1&b=2", second.url))
        .send()
        .await
        .unwrap();
    let mut requests = JoinSet::new();
    for _ in 0..16 {
        for server in [&first, &second] {
            let request = client.get(format!("{}/volatile", server.url));
            requests.spawn(async move {
                let response = request.send().await.unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                assert_eq!(response.headers()["x-lurklet-cache"], "miss");
                response.text().await.unwrap()
            });
        }
    }
    while let Some(result) = requests.join_next().await {
        assert!(matches!(result.unwrap().as_str(), "first" | "second"));
    }
    drop(first);
    let after_rollout = client
        .get(format!("{}/resource?a=1&b=2", second.url))
        .send()
        .await
        .unwrap();
    let stale = client
        .get(format!("{}/volatile", second.url))
        .header("x-test-failure", "yes")
        .send()
        .await
        .unwrap();

    assert_eq!(shared.headers()["x-lurklet-cache"], "hit");
    assert_eq!(shared.text().await.unwrap(), "first");
    assert_eq!(head.headers()["x-lurklet-cache"], "hit");
    assert_eq!(head.headers()[header::CONTENT_LENGTH], "5");
    assert!(head.bytes().await.unwrap().is_empty());
    assert_eq!(after_rollout.headers()["x-lurklet-cache"], "hit");
    assert_eq!(after_rollout.text().await.unwrap(), "first");
    assert_eq!(stale.headers()["x-lurklet-cache"], "stale");
    assert_eq!(stale.headers()[header::CACHE_CONTROL], "no-store");
    assert!(matches!(
        stale.text().await.unwrap().as_str(),
        "first" | "second"
    ));
}
