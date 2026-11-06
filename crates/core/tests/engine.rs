//! The queue engine end to end against a mock provider and mock file host.

use rdbatch_core::engine::{wait_done, Engine, Queue};
use rdbatch_core::models::{Job, JobState, Provider};
use rdbatch_core::provider::{self, Timing};
use serde_json::json;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Fixture {
    server: MockServer,
    engine: Arc<Engine>,
    queue: Arc<Queue>,
    persisted: Arc<AtomicUsize>,
}

async fn fixture(account: Option<&str>) -> Fixture {
    let server = MockServer::start().await;
    let persisted = Arc::new(AtomicUsize::new(0));
    let counter = persisted.clone();
    let queue = Queue::new(vec![], |_, _| {}, move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    let api = provider::create(Provider::RealDebrid, reqwest::Client::new(), "token", Some(&format!("{}/", server.uri())), Timing::fast());
    let engine = Engine::new(api, reqwest::Client::new(), account.map(str::to_string), queue.clone(), Timing::fast());
    Fixture { server, engine, queue, persisted }
}

/// A torrent that first waits for file selection and is "downloaded" afterwards.
async fn mount_torrent(f: &Fixture, torrent: &str, files: &[(i32, &str, &[u8])], selected: &[i32]) {
    let describe = |ready: bool| {
        let rows: Vec<_> = files
            .iter()
            .map(|(id, p, body)| json!({"id": id, "path": p, "bytes": body.len(), "selected": i32::from(ready && selected.contains(id))}))
            .collect();
        let links: Vec<String> = if ready { files.iter().filter(|(id, _, _)| selected.contains(id)).map(|(id, _, _)| format!("L-{torrent}-{id}")).collect() } else { vec![] };
        json!({"id": torrent, "filename": format!("Torrent {torrent}"), "status": if ready {"downloaded"} else {"waiting_files_selection"}, "progress": if ready {100} else {0},
               "bytes": files.iter().map(|(_, _, b)| b.len()).sum::<usize>(), "files": rows, "links": links})
    };
    Mock::given(method("POST")).and(path("/torrents/addMagnet")).and(body_string_contains(format!("btih%3A{torrent}"))).respond_with(ResponseTemplate::new(201).set_body_json(json!({"id": torrent}))).mount(&f.server).await;
    Mock::given(method("GET")).and(path(format!("/torrents/info/{torrent}"))).respond_with(ResponseTemplate::new(200).set_body_json(describe(false))).up_to_n_times(1).with_priority(1).mount(&f.server).await;
    Mock::given(method("GET")).and(path(format!("/torrents/info/{torrent}"))).respond_with(ResponseTemplate::new(200).set_body_json(describe(true))).with_priority(5).mount(&f.server).await;
    Mock::given(method("POST")).and(path(format!("/torrents/selectFiles/{torrent}"))).respond_with(ResponseTemplate::new(204)).mount(&f.server).await;
    for (id, p, body) in files {
        let name = p.rsplit('/').next().unwrap();
        Mock::given(method("POST")).and(path("/unrestrict/link")).and(body_string_contains(format!("link=L-{torrent}-{id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"filename": name, "filesize": body.len(), "download": format!("{}/dl/{torrent}/{id}", f.server.uri())})))
            .mount(&f.server).await;
        Mock::given(method("GET")).and(path(format!("/dl/{torrent}/{id}"))).respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec())).mount(&f.server).await;
    }
}

fn job(torrent_key: &str, selected: Option<Vec<i32>>) -> Job {
    let mut job = Job::new(Provider::RealDebrid, &format!("magnet:?xt=urn:btih:{torrent_key}"), torrent_key, "queued");
    job.selected_file_ids = selected;
    job
}

async fn run(f: &Fixture, root: &Path, jobs: Vec<Job>) -> Vec<Job> {
    let ids: Vec<String> = jobs.iter().map(|j| j.id.clone()).collect();
    for job in jobs {
        f.queue.push(job);
    }
    let session = f.engine.session(root.to_path_buf(), 3, true);
    let waits: Vec<_> = ids.iter().filter_map(|id| session.enqueue(id)).collect();
    for done in waits {
        wait_done(done).await;
    }
    ids.iter().map(|id| f.queue.get(id).unwrap()).collect()
}

#[tokio::test]
async fn a_complete_batch_adds_selects_all_polls_unrestricts_and_downloads_every_file() {
    let f = fixture(Some("me")).await;
    mount_torrent(&f, "T1", &[(1, "/Show/a.bin", b"alpha"), (2, "/Show/b.bin", b"bravo!")], &[1, 2]).await;
    let dir = tempfile::tempdir().unwrap();
    let done = run(&f, dir.path(), vec![job("T1", None)]).await;
    assert_eq!(done[0].state, JobState::Completed, "{:?}", done[0].detail);
    assert_eq!(std::fs::read(dir.path().join("a.bin")).unwrap(), b"alpha");
    assert_eq!(std::fs::read(dir.path().join("b.bin")).unwrap(), b"bravo!");
    assert_eq!(done[0].remote_account.as_deref(), Some("me"));
    assert!(f.persisted.load(Ordering::SeqCst) >= 1, "the remote ID must be flushed to disk immediately");
    assert!(!done[0].is_active && done[0].local_progress == 100.0);
    assert!(done[0].files.iter().all(|file| file.completed));
}

#[tokio::test]
async fn selected_file_ids_download_only_the_requested_files() {
    let f = fixture(None).await;
    mount_torrent(&f, "T2", &[(1, "/a.bin", b"one"), (2, "/b.bin", b"two")], &[2]).await;
    let dir = tempfile::tempdir().unwrap();
    let done = run(&f, dir.path(), vec![job("T2", Some(vec![2]))]).await;
    assert_eq!(done[0].state, JobState::Completed, "{:?}", done[0].detail);
    assert!(!dir.path().join("a.bin").exists());
    assert_eq!(std::fs::read(dir.path().join("b.bin")).unwrap(), b"two");
    assert_eq!(done[0].total_bytes, 3);
}

#[tokio::test]
async fn a_folder_chosen_for_a_job_replaces_the_session_folder() {
    let f = fixture(Some("me")).await;
    mount_torrent(&f, "T1", &[(1, "/Show/a.bin", b"alpha")], &[1]).await;
    let session_dir = tempfile::tempdir().unwrap();
    let chosen = tempfile::tempdir().unwrap();
    let mut picked = job("T1", Some(vec![1]));
    picked.target_directory = Some(chosen.path().to_string_lossy().to_string());
    let done = run(&f, session_dir.path(), vec![picked]).await;
    assert_eq!(done[0].state, JobState::Completed, "{:?}", done[0].detail);
    assert_eq!(std::fs::read(chosen.path().join("a.bin")).unwrap(), b"alpha");
    assert!(!session_dir.path().join("a.bin").exists(), "the Settings folder must stay untouched");
}

#[tokio::test]
async fn combined_links_are_rejected_for_a_selection() {
    let f = fixture(None).await;
    // Two files are selected on the account but the provider returns a single combined link.
    Mock::given(method("POST")).and(path("/torrents/addMagnet")).respond_with(ResponseTemplate::new(201).set_body_json(json!({"id":"T3"}))).mount(&f.server).await;
    Mock::given(method("GET")).and(path("/torrents/info/T3")).respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "id":"T3","filename":"x","status":"downloaded","progress":100,"bytes":6,
        "files":[{"id":1,"path":"/a","bytes":3,"selected":1},{"id":2,"path":"/b","bytes":3,"selected":1}],"links":["ONE-LINK"]}))).mount(&f.server).await;
    Mock::given(method("POST")).and(path("/unrestrict/link")).respond_with(ResponseTemplate::new(200)).expect(0).mount(&f.server).await;
    let dir = tempfile::tempdir().unwrap();
    let done = run(&f, dir.path(), vec![job("T3", Some(vec![1]))]).await;
    assert_eq!(done[0].state, JobState::Failed);
    assert!(done[0].detail.key.starts_with("Dịch vụ trả về link gộp"));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn zero_seeds_report_remote_waiting_without_any_local_transfer() {
    let f = fixture(None).await;
    Mock::given(method("POST")).and(path("/torrents/addMagnet")).respond_with(ResponseTemplate::new(201).set_body_json(json!({"id":"T4"}))).mount(&f.server).await;
    Mock::given(method("GET")).and(path("/torrents/info/T4")).respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "id":"T4","filename":"x","status":"downloading","progress":7,"bytes":3,"seeders":0,"speed":0,
        "files":[{"id":1,"path":"/a","bytes":3,"selected":1}],"links":[]}))).mount(&f.server).await;
    Mock::given(method("POST")).and(path("/unrestrict/link")).respond_with(ResponseTemplate::new(200)).expect(0).mount(&f.server).await;
    let dir = tempfile::tempdir().unwrap();
    let j = job("T4", Some(vec![1]));
    let id = j.id.clone();
    f.queue.push(j);
    let session = f.engine.session(dir.path().to_path_buf(), 2, true);
    let done = session.enqueue(&id).unwrap();
    for _ in 0..200 {
        if f.queue.get(&id).unwrap().state == JobState::Waiting && f.queue.get(&id).unwrap().detail.key.contains("0 seed") {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let waiting = f.queue.get(&id).unwrap();
    assert_eq!(waiting.state, JobState::Waiting);
    assert!(waiting.detail.key.contains("đang chờ nguồn tải") && waiting.detail.args[1] == "7");
    session.stop(&id).await;
    wait_done(done).await;
    let stopped = f.queue.get(&id).unwrap();
    assert_eq!(stopped.state, JobState::Stopped);
    assert!(!stopped.is_active && !stopped.is_stopping);
    assert!(stopped.torrent_id.is_some(), "the remote torrent stays so Resume can continue it");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn names_never_collide_across_torrents_or_existing_files() {
    let f = fixture(None).await;
    mount_torrent(&f, "TA", &[(1, "/x/same.bin", b"first")], &[1]).await;
    mount_torrent(&f, "TB", &[(1, "/y/same.bin", b"second")], &[1]).await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("same.bin"), b"already here").unwrap();
    let done = run(&f, dir.path(), vec![job("TA", Some(vec![1])), job("TB", Some(vec![1]))]).await;
    assert!(done.iter().all(|j| j.state == JobState::Completed), "{:?}", done.iter().map(|j| j.detail.render()).collect::<Vec<_>>());
    assert_eq!(std::fs::read(dir.path().join("same.bin")).unwrap(), b"already here", "an existing file is never replaced");
    let mut contents = vec![std::fs::read(dir.path().join("same (2).bin")).unwrap(), std::fs::read(dir.path().join("same (3).bin")).unwrap()];
    contents.sort();
    assert_eq!(contents, vec![b"first".to_vec(), b"second".to_vec()]);
}

#[tokio::test]
async fn one_failed_torrent_does_not_stop_the_others() {
    let f = fixture(None).await;
    mount_torrent(&f, "GOOD", &[(1, "/ok.bin", b"fine")], &[1]).await;
    Mock::given(method("POST")).and(path("/torrents/addMagnet")).and(body_string_contains("BAD")).respond_with(ResponseTemplate::new(403).set_body_json(json!({"error":"permission_denied","error_code":2}))).with_priority(1).mount(&f.server).await;
    let dir = tempfile::tempdir().unwrap();
    let done = run(&f, dir.path(), vec![job("BAD", None), job("GOOD", Some(vec![1]))]).await;
    assert_eq!(done[0].state, JobState::Failed);
    assert_eq!(done[1].state, JobState::Completed);
    assert!(dir.path().join("ok.bin").exists());
}

#[tokio::test]
async fn a_torrent_from_another_account_or_provider_is_refused_before_any_request() {
    let f = fixture(Some("me")).await;
    Mock::given(wiremock::matchers::any()).respond_with(ResponseTemplate::new(500)).expect(0).mount(&f.server).await;
    let mut foreign = job("X", None);
    foreign.torrent_id = Some("T9".into());
    foreign.remote_account = Some("someone-else".into());
    let mut other_provider = job("Y", None);
    other_provider.provider = Provider::TorBox;
    let dir = tempfile::tempdir().unwrap();
    let done = run(&f, dir.path(), vec![foreign, other_provider]).await;
    assert_eq!(done[0].state, JobState::Failed);
    assert_eq!(done[0].detail.args, vec!["someone-else".to_string(), "me".to_string()]);
    assert_eq!(done[1].state, JobState::Failed);
    assert!(done[1].detail.key.starts_with("Chọn dịch vụ"));
}

#[tokio::test]
async fn removing_a_queued_job_prevents_network_work_and_a_retry_runs_in_the_same_session() {
    let f = fixture(None).await;
    mount_torrent(&f, "TR", &[(1, "/r.bin", b"retry")], &[1]).await;
    let dir = tempfile::tempdir().unwrap();
    // Concurrency 1 keeps the second job queued while the first one runs.
    let first = job("TR", Some(vec![1]));
    let mut removed = job("NEVER", None);
    removed.is_removing = true;
    let ids = (first.id.clone(), removed.id.clone());
    f.queue.push(first);
    f.queue.push(removed);
    let session = f.engine.session(dir.path().to_path_buf(), 1, true);
    assert!(session.enqueue(&ids.1).is_none(), "a row being removed is never started");
    wait_done(session.enqueue(&ids.0).unwrap()).await;
    assert_eq!(f.queue.get(&ids.0).unwrap().state, JobState::Completed);
    // Retry: mark failed, then enqueue again on the same session.
    f.queue.mutate(&ids.0, |j| {
        j.state = JobState::Failed;
        j.files.clear();
    });
    std::fs::remove_file(dir.path().join("r.bin")).unwrap();
    wait_done(session.enqueue(&ids.0).unwrap()).await;
    assert_eq!(f.queue.get(&ids.0).unwrap().state, JobState::Completed);
    session.when_idle().await;
    assert_eq!(session.active_count(), 0);
}

#[tokio::test]
async fn inspection_waits_for_metadata_without_selecting_or_downloading() {
    let f = fixture(None).await;
    Mock::given(method("GET")).and(path("/torrents/info/TI")).respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "id":"TI","filename":"x","status":"magnet_conversion","progress":0,"bytes":0,"files":[],"links":[]}))).up_to_n_times(2).with_priority(1).mount(&f.server).await;
    Mock::given(method("GET")).and(path("/torrents/info/TI")).respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "id":"TI","filename":"Ready","status":"waiting_files_selection","progress":0,"bytes":3,
        "files":[{"id":1,"path":"/a.bin","bytes":3,"selected":0}],"links":[]}))).with_priority(5).mount(&f.server).await;
    Mock::given(method("POST")).and(path("/torrents/selectFiles/TI")).respond_with(ResponseTemplate::new(204)).expect(0).mount(&f.server).await;
    Mock::given(method("POST")).and(path("/unrestrict/link")).respond_with(ResponseTemplate::new(200)).expect(0).mount(&f.server).await;
    let info = f.engine.inspect("TI", &tokio_util::sync::CancellationToken::new()).await.unwrap();
    assert_eq!((info.filename.as_str(), info.files.len()), ("Ready", 1));
}

#[tokio::test]
async fn stop_all_cancels_running_and_queued_jobs_and_keeps_the_remote_torrents() {
    let f = fixture(None).await;
    for key in ["S1", "S2"] {
        Mock::given(method("POST")).and(path("/torrents/addMagnet")).and(body_string_contains(format!("btih%3A{key}"))).respond_with(ResponseTemplate::new(201).set_body_json(json!({"id": key}))).mount(&f.server).await;
        Mock::given(method("GET")).and(path(format!("/torrents/info/{key}"))).respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id":key,"filename":"x","status":"downloading","progress":1,"bytes":3,"seeders":0,"speed":0,"files":[{"id":1,"path":"/a","bytes":3,"selected":1}],"links":[]}))).mount(&f.server).await;
    }
    let dir = tempfile::tempdir().unwrap();
    let (first, second) = (job("S1", Some(vec![1])), job("S2", Some(vec![1])));
    let ids = [first.id.clone(), second.id.clone()];
    f.queue.push(first);
    f.queue.push(second);
    let session = f.engine.session(dir.path().to_path_buf(), 1, true); // one slot: the second job stays queued
    let waits: Vec<_> = ids.iter().filter_map(|id| session.enqueue(id)).collect();
    for _ in 0..200 {
        if f.queue.get(&ids[0]).unwrap().state == JobState::Waiting {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    session.stop_all().await;
    for done in waits {
        wait_done(done).await;
    }
    session.when_idle().await;
    for id in &ids {
        let job = f.queue.get(id).unwrap();
        assert_eq!(job.state, JobState::Stopped, "{:?}", job.detail);
        assert!(!job.is_active && !job.is_stopping);
    }
    assert_eq!(session.active_count(), 0);
}

/// Resuming a big, mostly finished torrent must not redo work per file for every other file (it used to clone the
/// whole job several times per link, which is quadratic and froze Resume on torrents with thousands of files).
#[tokio::test]
async fn resuming_a_large_mostly_finished_torrent_is_fast() {
    const COUNT: i32 = 3000;
    let server = MockServer::start().await;
    // Mirror the app's sink, which keeps a copy of every changed job for the next UI tick.
    let pending = Arc::new(std::sync::Mutex::new(std::collections::HashMap::<String, Job>::new()));
    let sink = pending.clone();
    let queue = Queue::new(vec![], move |job, _| {
        sink.lock().unwrap().insert(job.id.clone(), job);
    }, || {});
    let api = provider::create(Provider::RealDebrid, reqwest::Client::new(), "token", Some(&format!("{}/", server.uri())), Timing::fast());
    let engine = Engine::new(api, reqwest::Client::new(), None, queue.clone(), Timing::fast());
    let rows: Vec<_> = (1..=COUNT).map(|id| json!({"id": id, "path": format!("/Big/f{id}.bin"), "bytes": 1, "selected": 1})).collect();
    let links: Vec<String> = (1..=COUNT).map(|id| format!("https://example.invalid/d/L{id}")).collect();
    Mock::given(method("GET")).and(path("/torrents/info/BIG")).respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "id":"BIG","filename":"Big","status":"downloaded","progress":100,"bytes":COUNT,"files":rows,"links":links}))).mount(&server).await;
    Mock::given(method("POST")).and(path("/unrestrict/link")).respond_with(ResponseTemplate::new(200)).expect(0).mount(&server).await;

    let dir = tempfile::tempdir().unwrap();
    let mut big = job("BIG", Some((1..=COUNT).collect()));
    big.torrent_id = Some("BIG".into());
    big.output_directory = Some(dir.path().to_string_lossy().to_string());
    big.flatten_files = Some(true);
    big.files = (1..=COUNT)
        .map(|id| {
            let name = format!("f{id}.bin");
            std::fs::write(dir.path().join(&name), b"x").unwrap();
            rdbatch_core::models::JobFile { torrent_file_id: Some(id), link_index: id - 1, link: links[(id - 1) as usize].clone(), relative_path: name, expected_bytes: 1, downloaded_bytes: 1, completed: true }
        })
        .collect();
    let id = big.id.clone();
    queue.push(big);
    let started = std::time::Instant::now();
    let session = engine.session(dir.path().to_path_buf(), 3, true);
    wait_done(session.enqueue(&id).unwrap()).await;
    let elapsed = started.elapsed();
    let done = queue.get(&id).unwrap();
    assert_eq!(done.state, JobState::Completed, "{:?}", done.detail);
    eprintln!("resume of {COUNT} finished files took {elapsed:?}");
    assert!(elapsed < std::time::Duration::from_secs(2), "resume took {elapsed:?}");
}

/// A download host that keeps failing for a while must not leave the job failed: it retries on its own and finishes.
#[tokio::test]
async fn a_job_that_fails_on_a_connection_problem_retries_by_itself() {
    let f = fixture(None).await;
    mount_torrent(&f, "TAR", &[(1, "/flaky.bin", b"finally")], &[1]).await;
    // Five failures outlast the per-file retries (three tries), so only the job-level retry can finish the download.
    Mock::given(method("GET")).and(path("/dl/TAR/1")).respond_with(ResponseTemplate::new(503)).up_to_n_times(5).with_priority(1).mount(&f.server).await;
    let dir = tempfile::tempdir().unwrap();
    let mut seen_retry = false;
    let j = job("TAR", Some(vec![1]));
    let id = j.id.clone();
    f.queue.push(j);
    let session = f.engine.session(dir.path().to_path_buf(), 1, true);
    let done = session.enqueue(&id).unwrap();
    for _ in 0..400 {
        let current = f.queue.get(&id).unwrap();
        seen_retry |= current.detail.key.starts_with("Lỗi kết nối · tự thử lại");
        if !current.is_active {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }
    wait_done(done).await;
    let finished = f.queue.get(&id).unwrap();
    assert_eq!(finished.state, JobState::Completed, "{:?}", finished.detail);
    assert_eq!(std::fs::read(dir.path().join("flaky.bin")).unwrap(), b"finally");
    assert_eq!(finished.files.len(), 1, "retries reuse the file's row instead of adding copies: {:?}", finished.files);
    assert!(seen_retry, "the row should say it is retrying by itself");
}

#[tokio::test]
async fn an_interrupted_file_resumes_from_its_exact_partial_bytes() {
    let f = fixture(None).await;
    let body: &[u8] = b"0123456789";
    mount_torrent(&f, "TS", &[(1, "/r.bin", body)], &[1]).await;
    // Replace the plain file mock: only a ranged request for the missing tail may succeed.
    f.server.reset().await;
    Mock::given(method("POST")).and(path("/torrents/addMagnet")).respond_with(ResponseTemplate::new(201).set_body_json(json!({"id":"TS"}))).mount(&f.server).await;
    Mock::given(method("GET")).and(path("/torrents/info/TS")).respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "id":"TS","filename":"x","status":"downloaded","progress":100,"bytes":10,"files":[{"id":1,"path":"/r.bin","bytes":10,"selected":1}],"links":["L-TS-1"]}))).mount(&f.server).await;
    Mock::given(method("POST")).and(path("/unrestrict/link")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"filename":"r.bin","filesize":10,"download": format!("{}/dl/ts", f.server.uri())}))).mount(&f.server).await;
    Mock::given(method("GET")).and(path("/dl/ts")).and(wiremock::matchers::header("range", "bytes=4-")).respond_with(ResponseTemplate::new(206).insert_header("content-range", "bytes 4-9/10").set_body_bytes(&body[4..])).expect(1).mount(&f.server).await;

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("r.bin.part"), &body[..4]).unwrap();
    std::fs::write(dir.path().join("r.bin.part.json"), br#"{"expectedBytes":10,"validator":null}"#).unwrap();
    let mut resumed = job("TS", Some(vec![1]));
    resumed.torrent_id = Some("TS".into());
    resumed.output_directory = Some(dir.path().to_string_lossy().to_string());
    resumed.flatten_files = Some(true);
    resumed.files = vec![rdbatch_core::models::JobFile { torrent_file_id: Some(1), link_index: 0, link: "L-TS-1".into(), relative_path: "r.bin".into(), expected_bytes: 10, downloaded_bytes: 4, completed: false }];
    let done = run(&f, dir.path(), vec![resumed]).await;
    assert_eq!(done[0].state, JobState::Completed, "{:?}", done[0].detail);
    assert_eq!(std::fs::read(dir.path().join("r.bin")).unwrap(), body);
    assert!(!dir.path().join("r.bin.part").exists());
}
