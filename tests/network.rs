use std::time::Duration;

use pompiliusd::{Cloud, CloudApi, error::CloudError, rclone_api::Rclone};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
    time::timeout,
};

// One local RC request, with no rclone installation or external service needed.
async fn mock_rc(status: &str, body: &str, endpoint: &str) -> (Cloud, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let endpoint = endpoint.to_owned();
    let server = tokio::spawn(async move {
        timeout(Duration::from_secs(5), async {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                header.push(stream.read_u8().await.unwrap());
            }
            let header = String::from_utf8(header).unwrap();
            assert!(header.starts_with(&format!("POST /{endpoint} HTTP/1.1\r\n")));
            let length = header
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            stream.read_exact(&mut vec![0; length]).await.unwrap();
            stream.write_all(response.as_bytes()).await.unwrap();
        })
        .await
        .expect("daemon must reach the local RC server without an Internet preflight");
    });
    let cloud = Cloud {
        rclone: Rclone {
            client: reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap(),
            url: format!("http://{address}/"),
        },
    };
    (cloud, server)
}

#[tokio::test]
async fn local_configuration_operations_reach_rc() {
    let (cloud, server) = mock_rc(
        "200 OK",
        r#"{"lan":{"type":"ftp"},"disk":{"type":"local"}}"#,
        "config/dump",
    )
    .await;
    let mut profiles = timeout(Duration::from_secs(3), cloud.list_profiles())
        .await
        .unwrap()
        .unwrap();
    profiles.sort();
    assert_eq!(
        profiles,
        [
            ("disk".into(), "local".into()),
            ("lan".into(), "ftp".into())
        ]
    );
    server.await.unwrap();

    let (cloud, server) = mock_rc("200 OK", "{}", "config/delete").await;
    assert!(
        timeout(Duration::from_secs(3), cloud.delete_profile("lan"))
            .await
            .unwrap()
            .is_ok()
    );
    server.await.unwrap();
}

#[tokio::test]
async fn provider_options_preserve_all_fields_and_metadata() {
    let expected = serde_json::json!([
        {"Name": "host", "Help": "Host", "Required": true, "Type": "string"},
        {"Name": "port", "Help": "Port", "Required": false, "Default": 22, "Type": "int"},
        {"Name": "pass", "Help": "Password", "Required": false, "IsPassword": true},
        {"Name": "key_use_agent", "Help": "Force agent", "Required": false, "Advanced": true},
        {"Name": "token", "Help": "Token", "Required": false},
        {"Name": "config_is_local", "Help": "Local config", "Required": false},
        {"Name": "custom_option", "Help": "Future option", "Required": false, "FutureMetadata": 42}
    ]);
    // Selection must work for any provider, without special cases for SFTP.
    for provider in ["sftp", "other"] {
        let body = serde_json::json!({"providers": [
            {"Name": "unselected", "Options": []},
            {"Name": provider, "Options": expected}
        ]});
        let (cloud, server) = mock_rc("200 OK", &body.to_string(), "config/providers").await;
        let options = cloud.get_provider_options(provider).await.unwrap();
        let options: Vec<serde_json::Value> = options
            .iter()
            .map(|option| serde_json::from_str(option).unwrap())
            .collect();
        assert_eq!(serde_json::Value::from(options), expected);
        server.await.unwrap();
    }
}

#[tokio::test]
async fn remote_failure_preserves_rclone_reason() {
    let (cloud, server) = mock_rc(
        "500 Internal Server Error",
        r#"{"error":"dial tcp: network is unreachable","input":{"password":"secret"}}"#,
        "operations/about",
    )
    .await;
    let error = cloud.about("lan").await.unwrap_err();
    assert!(matches!(error, CloudError::Rclone(ref message)
        if message.contains("500") && message.contains("network is unreachable")
            && !message.contains("secret")));
    server.await.unwrap();
}

#[tokio::test]
async fn failed_delete_is_not_reported_as_success() {
    let (cloud, server) = mock_rc(
        "500 Internal Server Error",
        r#"{"error":"cannot write config"}"#,
        "config/delete",
    )
    .await;
    assert!(
        matches!(cloud.delete_profile("lan").await, Err(CloudError::Rclone(message))
        if message.contains("cannot write config"))
    );
    server.await.unwrap();
}

#[tokio::test]
async fn invalid_error_body_still_reports_http_failure() {
    let (cloud, server) = mock_rc("503 Service Unavailable", "not JSON", "core/transfers").await;
    assert!(
        matches!(cloud.is_busy().await, Err(CloudError::Rclone(message))
        if message.contains("503"))
    );
    server.await.unwrap();
}

#[tokio::test]
async fn unavailable_rc_reports_transport_error() {
    // Keep the port reserved but stop accepting connections, avoiding port reuse.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let cloud = Cloud {
        rclone: Rclone {
            client: reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_millis(100))
                .build()
                .unwrap(),
            url: format!("http://{}/", listener.local_addr().unwrap()),
        },
    };
    assert!(matches!(
        cloud.list_profiles().await,
        Err(CloudError::Reqwest(_))
    ));
}
