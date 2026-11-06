//! Provider adapters against a mock server: auth, error envelopes, retry rules, credential isolation.

use rdbatch_core::error::Error;
use rdbatch_core::models::Provider;
use rdbatch_core::provider::{self, Timing};
use serde_json::json;
use sha2::{Digest, Sha256};
use wiremock::matchers::{body_string_contains, header, method, path, path_regex, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(provider: Provider, server: &MockServer, token: &str) -> std::sync::Arc<dyn provider::Debrid> {
    let base = match provider {
        Provider::TorBox => format!("{}/v1/api/", server.uri()),
        _ => format!("{}/", server.uri()),
    };
    provider::create(provider, reqwest::Client::new(), token, Some(&base), Timing::fast())
}

#[tokio::test]
async fn realdebrid_sends_the_token_only_in_the_header_and_reports_401() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/user")).and(header("authorization", "Bearer secret-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"username":"me","type":"premium","premium":3600})))
        .up_to_n_times(1).mount(&server).await;
    let api = client(Provider::RealDebrid, &server, "secret-token");
    let user = api.user().await.unwrap();
    assert!(user.can_download() && user.username == "me");

    Mock::given(method("GET")).and(path("/user")).respond_with(ResponseTemplate::new(401).set_body_json(json!({"error":"bad_token secret-token","error_code":8}))).mount(&server).await;
    let error = api.user().await.unwrap_err();
    assert!(matches!(error, Error::Api { status: 401, .. }));
    assert!(error.friendly().key.starts_with("Token không hợp lệ"));
    assert!(!error.to_string().contains("secret-token"), "the token must not appear in error text: {error}");
}

#[tokio::test]
async fn realdebrid_delete_uses_exact_escaped_ids_and_never_replays_an_ambiguous_failure() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE")).and(path("/torrents/delete/a%2Fb%20c")).respond_with(ResponseTemplate::new(500)).expect(1).mount(&server).await;
    let api = client(Provider::RealDebrid, &server, "t");
    assert!(api.delete("a/b c").await.is_err());
    assert!(api.delete("  ").await.is_err(), "an empty ID must never reach the network");
    // `expect(1)` is verified when the server drops.
}

#[tokio::test]
async fn realdebrid_replays_a_rate_limited_safe_read() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/torrents/info/T1")).respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0")).up_to_n_times(1).mount(&server).await;
    Mock::given(method("GET")).and(path("/torrents/info/T1")).respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "id":"T1","filename":"x","status":"downloaded","progress":100,"bytes":3,"files":[{"id":1,"path":"/x","bytes":3,"selected":1}],"links":["L"]
    }))).mount(&server).await;
    let info = client(Provider::RealDebrid, &server, "t").info("T1").await.unwrap();
    assert_eq!(info.links, vec!["L".to_string()]);
}

#[tokio::test]
async fn realdebrid_lists_every_page_and_tolerates_an_empty_account() {
    let server = MockServer::start().await;
    let page: Vec<_> = (0..1000).map(|i| json!({"id": format!("T{i}"), "filename":"n", "bytes":1, "progress":100, "status":"downloaded", "added":"2024-01-02T03:04:05.000Z"})).collect();
    Mock::given(method("GET")).and(path("/torrents")).and(query_param("page", "1")).respond_with(ResponseTemplate::new(200).set_body_json(&page)).mount(&server).await;
    Mock::given(method("GET")).and(path("/torrents")).and(query_param("page", "2")).respond_with(ResponseTemplate::new(204)).mount(&server).await;
    let all = client(Provider::RealDebrid, &server, "t").list().await.unwrap();
    assert_eq!(all.len(), 1000);
    assert!(all[0].added.is_some());
}

#[tokio::test]
async fn torbox_pipeline_file_id_zero_credential_isolation_and_deletion() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/v1/api/user/me")).and(header("authorization", "Bearer tb-secret"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success":true,"data":{"auth_id":"acct-1","plan":2}}))).mount(&server).await;
    Mock::given(method("POST")).and(path("/v1/api/torrents/createtorrent")).and(body_string_contains("magnet:?xt=urn:btih:abc")).and(body_string_contains("as_queued"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success":true,"data":{"torrent_id":77}}))).mount(&server).await;
    Mock::given(method("GET")).and(path("/v1/api/torrents/mylist")).and(query_param("id", "77"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success":true,"data":{
            "id":77,"name":"Show","size":9,"progress":1.0,"download_state":"cached","download_finished":true,"download_present":true,"seeds":3,"download_speed":0,
            "files":[{"id":0,"name":"Show/ep0.mkv","size":4},{"id":1,"name":"Show/ep1.mkv","size":5}]}}))).mount(&server).await;
    Mock::given(method("GET")).and(path("/v1/api/torrents/requestdl")).and(query_param("token", "tb-secret")).and(query_param("file_id", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success":true,"data":"https://cdn.example/dl/ep0.mkv"}))).mount(&server).await;
    Mock::given(method("POST")).and(path("/v1/api/torrents/controltorrent")).and(body_string_contains("\"torrent_id\":77")).and(body_string_contains("\"operation\":\"delete\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success":true,"data":{}}))).expect(1).mount(&server).await;

    let api = client(Provider::TorBox, &server, "tb-secret");
    let user = api.user().await.unwrap();
    assert!(user.can_download() && user.username == "acct-1");
    let id = api.add_magnet("magnet:?xt=urn:btih:abc").await.unwrap();
    assert_eq!(id, "77");
    let info = api.info(&id).await.unwrap();
    assert_eq!(info.status, "downloaded");
    assert_eq!(info.files.iter().map(|f| f.id).collect::<Vec<_>>(), vec![0, 1], "file ID zero is a real file");
    assert!(info.links.iter().all(|link| !link.contains("tb-secret")), "references never carry the token");
    let link = api.unrestrict(&info.links[0]).await.unwrap();
    assert_eq!(link.filename, "ep0.mkv");
    assert_eq!(link.filesize, 4);
    assert!(!link.download.contains("tb-secret"));
    api.delete(&id).await.unwrap();
    assert!(api.delete("not-a-number").await.is_err());
}

#[tokio::test]
async fn torbox_error_text_never_contains_the_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path_regex("/v1/api/torrents/requestdl.*")).respond_with(ResponseTemplate::new(400).set_body_json(json!({
        "success":false,"error":"BAD_REQUEST","detail":"invalid token tb-secret rejected"}))).mount(&server).await;
    Mock::given(method("GET")).and(path("/v1/api/torrents/mylist")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"success":true,"data":{
        "id":5,"name":"n","size":1,"progress":0,"download_state":"downloading","files":[{"id":1,"name":"a","size":1}]}}))).mount(&server).await;
    let api = client(Provider::TorBox, &server, "tb-secret");
    let info = api.info("5").await.unwrap();
    let error = api.unrestrict(&info.links[0]).await.unwrap_err();
    assert!(!error.to_string().contains("tb-secret"), "{error}");
    assert!(error.to_string().contains("[redacted]"));
}

fn all_debrid_id(path: &str) -> i32 {
    let hash = Sha256::digest(path.as_bytes());
    let id = (u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]]) & 0x7fff_ffff) as i32;
    if id == 0 { 1 } else { id }
}

#[tokio::test]
async fn alldebrid_nested_files_get_stable_path_based_ids() {
    let server = MockServer::start().await;
    let status = |order: bool| {
        let mut files = vec![
            json!({"n":"Season 1","e":[{"n":"ep1.mkv","s":10,"l":"https://alldebrid.com/f/1"},{"n":"ep2.mkv","s":20,"l":"https://alldebrid.com/f/2"}]}),
            json!({"n":"readme.txt","s":1,"l":"https://alldebrid.com/f/3"}),
        ];
        if order { files.reverse(); }
        json!({"status":"success","data":{"magnets":{"id":42,"filename":"Show","size":31,"statusCode":4,"downloaded":31,"seeders":0,"downloadSpeed":0,"files":files}}})
    };
    Mock::given(method("POST")).and(path("/v4.1/magnet/status")).respond_with(ResponseTemplate::new(200).set_body_json(status(false))).up_to_n_times(1).mount(&server).await;
    Mock::given(method("POST")).and(path("/v4.1/magnet/status")).respond_with(ResponseTemplate::new(200).set_body_json(status(true))).mount(&server).await;
    let api = client(Provider::AllDebrid, &server, "ad-key");
    let first = api.info("42").await.unwrap();
    let second = api.info("42").await.unwrap();
    assert_eq!(first.status, "downloaded");
    let id_of = |info: &rdbatch_core::models::TorrentInfo, p: &str| info.files.iter().find(|f| f.path == p).unwrap().id;
    assert_eq!(id_of(&first, "/Season 1/ep2.mkv"), all_debrid_id("/Season 1/ep2.mkv"));
    assert_eq!(id_of(&first, "/Season 1/ep2.mkv"), id_of(&second, "/Season 1/ep2.mkv"), "IDs follow the path, not the order");
    let link_of = |info: &rdbatch_core::models::TorrentInfo, p: &str| {
        let index = info.files.iter().position(|f| f.path == p).unwrap();
        info.links[index].clone()
    };
    assert_eq!(link_of(&second, "/readme.txt"), "https://alldebrid.com/f/3");
}

#[tokio::test]
async fn alldebrid_delayed_links_poll_without_regenerating_the_unlock() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/v4/link/unlock")).and(body_string_contains("link="))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"success","data":{"filename":"big.mkv","filesize":99,"link":"","delayed":"D1"}}))).expect(1).mount(&server).await;
    Mock::given(method("POST")).and(path("/v4/link/delayed")).and(body_string_contains("id=D1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"success","data":{"status":1}}))).up_to_n_times(2).mount(&server).await;
    Mock::given(method("POST")).and(path("/v4/link/delayed"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"success","data":{"status":2,"link":"https://cdn.example/big.mkv"}}))).mount(&server).await;
    let link = client(Provider::AllDebrid, &server, "k").unrestrict("https://alldebrid.com/f/1").await.unwrap();
    assert_eq!((link.filename.as_str(), link.filesize, link.download.as_str()), ("big.mkv", 99, "https://cdn.example/big.mkv"));
}

#[tokio::test]
async fn alldebrid_error_envelope_maps_bad_keys_to_401() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/v4/user")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"error","error":{"code":"AUTH_BAD_APIKEY","message":"The API key is invalid ad-key"}}))).mount(&server).await;
    let error = client(Provider::AllDebrid, &server, "ad-key").user().await.unwrap_err();
    assert!(matches!(error, Error::Api { status: 401, .. }));
    assert!(!error.to_string().contains("ad-key"));
}

#[tokio::test]
async fn torrent_upload_sends_the_raw_bytes_and_the_token_only_to_the_api() {
    let server = MockServer::start().await;
    let bytes = b"d8:announce3:foo4:infod4:name1:aee".to_vec();
    Mock::given(method("PUT")).and(path("/torrents/addTorrent")).and(header("authorization", "Bearer secret-token")).and(wiremock::matchers::body_bytes(bytes.clone()))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({"id":"UP1"}))).expect(1).mount(&server).await;
    let id = client(Provider::RealDebrid, &server, "secret-token").add_torrent(bytes).await.unwrap();
    assert_eq!(id, "UP1");
}
