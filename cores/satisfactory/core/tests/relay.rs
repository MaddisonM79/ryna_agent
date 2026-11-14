//! The relay against a local WebSocket server standing in for the API and a local HTTP server
//! standing in for FRM. Every limit on the localhost call is driven from a frame, and FRM's log
//! shows what reached it (nothing, for every refusal).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ryna_agent::auth::{AccessToken, AuthError, TokenStore};
use ryna_agent::config::Config;
use ryna_agent::core::{Core, CoreContext, CoreControl, TokenProvider};
use ryna_satisfactory_core::frm::Frm;
use ryna_satisfactory_core::relay::{
    close_code, Clock, RelaySettings, SatisfactoryEvent, Timing, NEEDS_NEWER, NOT_HOST, REPLACED,
};
use ryna_satisfactory_core::satisfactory_core::SatisfactoryCore;
use serde_json::{json, Value};
use tungstenite::handshake::server::{Callback, ErrorResponse, Request, Response};
use tungstenite::protocol::CloseFrame;
use tungstenite::{Message, WebSocket};

const TOKEN: &str = "tok-SAT-access";
const FRM_SECRET: &str = "secret-frm-write-token";

// ---- FRM, played by a tiny HTTP server ----

#[derive(Debug, Clone)]
struct Seen {
    line: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl Seen {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

type Reply = Box<dyn Fn(&Seen) -> Vec<u8> + Send + Sync>;

struct Frm2 {
    port: u16,
    seen: Arc<Mutex<Vec<Seen>>>,
}

fn http(status: &str, extra: &str, body: &[u8]) -> Vec<u8> {
    let mut out = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n",
        body.len()
    )
    .into_bytes();
    out.extend_from_slice(body);
    out
}

fn frm_server(reply: Reply) -> Frm2 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen: Arc<Mutex<Vec<Seen>>> = Arc::default();
    let (log, reply) = (seen.clone(), Arc::new(reply));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let (log, reply) = (log.clone(), reply.clone());
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() || line.is_empty() {
                    return;
                }
                let mut headers = Vec::new();
                loop {
                    let mut h = String::new();
                    reader.read_line(&mut h).unwrap();
                    let h = h.trim_end().to_string();
                    if h.is_empty() {
                        break;
                    }
                    if let Some((k, v)) = h.split_once(':') {
                        headers.push((k.trim().to_string(), v.trim().to_string()));
                    }
                }
                let len = headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, v)| v.parse::<usize>().ok())
                    .unwrap_or(0);
                let mut body = vec![0u8; len];
                reader.read_exact(&mut body).unwrap();
                let seen = Seen {
                    line: line.trim_end().to_string(),
                    headers,
                    body: String::from_utf8_lossy(&body).into_owned(),
                };
                log.lock().unwrap().push(seen.clone());
                let _ = stream.write_all(&reply(&seen));
            });
        }
    });
    Frm2 { port, seen }
}

fn ok_frm() -> Frm2 {
    frm_server(Box::new(|s| {
        http(
            "200 OK",
            "",
            format!("{{\"echo\":\"{}\"}}", s.line).as_bytes(),
        )
    }))
}

/// A port nothing listens on.
fn dead_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

// ---- The API, played by a WebSocket server ----

#[derive(Clone, Copy)]
enum Upgrade {
    Accept,
    Status(u16),
}

type Ws = WebSocket<TcpStream>;

struct Api {
    port: u16,
    auth: Arc<Mutex<Vec<String>>>,
    conns: std::sync::mpsc::Receiver<Ws>,
    attempts: Arc<Mutex<Vec<Instant>>>,
}

/// Records the upgrade's Authorization header and answers it as planned.
struct Hook {
    auth: Arc<Mutex<Vec<String>>>,
    step: Upgrade,
}

impl Callback for Hook {
    fn on_request(self, req: &Request, res: Response) -> Result<Response, ErrorResponse> {
        let h = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        self.auth.lock().unwrap().push(h);
        match self.step {
            Upgrade::Accept => Ok(res),
            Upgrade::Status(s) => {
                let mut e = ErrorResponse::new(None);
                *e.status_mut() = tungstenite::http::StatusCode::from_u16(s).unwrap();
                Err(e)
            }
        }
    }
}

/// Each upgrade is answered from `plan` in turn (the last repeats); accepted sockets come out of
/// `conns`.
fn api_server(plan: Vec<Upgrade>) -> Api {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let auth: Arc<Mutex<Vec<String>>> = Arc::default();
    let attempts: Arc<Mutex<Vec<Instant>>> = Arc::default();
    let (tx, rx) = std::sync::mpsc::channel();
    let (a, t) = (auth.clone(), attempts.clone());
    std::thread::spawn(move || {
        for (n, stream) in listener.incoming().enumerate() {
            let Ok(stream) = stream else { return };
            t.lock().unwrap().push(Instant::now());
            let step = *plan.get(n).or(plan.last()).unwrap();
            let hook = Hook {
                auth: a.clone(),
                step,
            };
            if let Ok(ws) = tungstenite::accept_hdr(stream, hook) {
                ws.get_ref()
                    .set_read_timeout(Some(Duration::from_millis(50)))
                    .unwrap();
                if tx.send(ws).is_err() {
                    return;
                }
            }
        }
    });
    Api {
        port,
        auth,
        conns: rx,
        attempts,
    }
}

impl Api {
    fn next(&self) -> Ws {
        self.conns
            .recv_timeout(Duration::from_secs(20))
            .expect("the relay connected")
    }
}

/// The next text frame from the relay, as JSON (pings skipped unless asked for).
fn recv(ws: &mut Ws, skip_pings: bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        assert!(Instant::now() < deadline, "no frame from the relay");
        match ws.read() {
            Ok(Message::Text(t)) => {
                let v: Value = serde_json::from_str(&t).unwrap();
                if skip_pings && v["t"] == "ping" {
                    continue;
                }
                return v;
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(e) => panic!("{e}"),
        }
    }
}

fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::text(v.to_string())).unwrap();
}

fn req(id: &str, method: &str, path: &str, query: Option<&str>) -> Value {
    json!({ "t": "req", "id": id, "method": method, "path": path, "query": query,
            "headers": { "X-FRM-Authorization": FRM_SECRET }, "body": "{\"ID\":\"x\"}" })
}

// ---- The core ----

struct NoStore;
impl TokenStore for NoStore {
    fn refresh_token(&self) -> Result<Option<String>, String> {
        Ok(None)
    }
    fn set_refresh_token(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn access_token(&self) -> Option<AccessToken> {
        None
    }
    fn set_access_token(&self, _: &AccessToken) -> Result<(), String> {
        Ok(())
    }
    fn clear(&self) -> Result<(), String> {
        Ok(())
    }
}

struct Tokens {
    config: Config,
    forgotten: AtomicU32,
}

impl TokenProvider for Tokens {
    fn config(&self) -> &Config {
        &self.config
    }
    fn store(&self) -> Box<dyn TokenStore> {
        Box::new(NoStore)
    }
    fn access_token(&self) -> Result<Option<String>, AuthError> {
        Ok(Some(TOKEN.into()))
    }
    fn forget_access_token(&self) {
        self.forgotten.fetch_add(1, Ordering::SeqCst);
    }
}

#[derive(Clone, Default)]
struct FakeClock(Arc<Mutex<Duration>>);

impl FakeClock {
    fn advance(&self, d: Duration) {
        *self.0.lock().unwrap() += d;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Duration {
        *self.0.lock().unwrap()
    }
}

struct Run {
    events: Arc<Mutex<Vec<SatisfactoryEvent>>>,
    control: CoreControl,
    tokens: Arc<Tokens>,
    thread: Option<std::thread::JoinHandle<Result<(), String>>>,
    core: Arc<SatisfactoryCore>,
}

fn timing() -> Timing {
    Timing {
        backoff_start: Duration::from_millis(50),
        backoff_max: Duration::from_millis(400),
        connect_limit: Duration::from_secs(5),
        ..Timing::default()
    }
}

fn start(api: &Api, core: SatisfactoryCore) -> Run {
    let _ = api;
    let tokens = Arc::new(Tokens {
        config: Config {
            issuer: "http://127.0.0.1:9".into(),
            api: core.api.clone(),
            client_id: "c".into(),
            resource: "https://api.ryna.io/wow/ingest".into(),
        },
        forgotten: AtomicU32::new(0),
    });
    let state = std::env::temp_dir();
    let ctx = CoreContext {
        state_dir: state,
        tokens: tokens.clone(),
        device_key: None,
        game_folder: None,
    };
    let control = CoreControl::new();
    let events: Arc<Mutex<Vec<SatisfactoryEvent>>> = Arc::default();
    let core = Arc::new(core);
    let (c, e, k) = (control.clone(), events.clone(), core.clone());
    let thread = std::thread::spawn(move || {
        k.start(&ctx, &c, &mut |ev| e.lock().unwrap().push(ev))
            .map_err(|e| e.to_string())
    });
    Run {
        events,
        control,
        tokens,
        thread: Some(thread),
        core,
    }
}

fn core(api: &Api, frm_port: u16, allow_changes: bool) -> SatisfactoryCore {
    SatisfactoryCore::new(
        &format!("http://127.0.0.1:{}", api.port),
        RelaySettings {
            port: frm_port,
            allow_changes,
        },
    )
    .with_timing(timing())
}

impl Run {
    fn stop(&mut self) -> Result<(), String> {
        self.control.stop();
        self.join()
    }
    fn join(&mut self) -> Result<(), String> {
        self.thread.take().unwrap().join().unwrap()
    }
    fn has(&self, f: impl Fn(&SatisfactoryEvent) -> bool) -> bool {
        self.events.lock().unwrap().iter().any(f)
    }
    fn wait_for(&self, what: &str, f: impl Fn(&SatisfactoryEvent) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !self.has(&f) {
            assert!(
                Instant::now() < deadline,
                "{what}: {:?}",
                self.events.lock().unwrap()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn shown(&self) -> String {
        serde_json::to_string(&*self.events.lock().unwrap()).unwrap()
    }
}

// ---- Tests ----

#[test]
fn hello_then_a_get_round_trip() {
    let frm = ok_frm();
    let api = api_server(vec![Upgrade::Accept]);
    let mut run = start(&api, core(&api, frm.port, false));
    let mut ws = api.next();
    let hello = recv(&mut ws, true);
    assert_eq!(hello["t"], "hello");
    assert_eq!(hello["protocol"], 1);
    assert!(hello["agent"].as_str().unwrap().starts_with("ryna-agent/"));
    assert_eq!(hello["frm"], json!({ "reachable": true }));
    assert_eq!(api.auth.lock().unwrap()[0], format!("Bearer {TOKEN}"));

    send(&mut ws, req("r1", "GET", "/getPower", Some("a=1&b=2")));
    let res = recv(&mut ws, true);
    assert_eq!(res["t"], "res");
    assert_eq!(res["id"], "r1");
    assert_eq!(res["status"], 200);
    assert_eq!(res["contentType"], "application/json");
    assert_eq!(res["body"], "{\"echo\":\"GET /getPower?a=1&b=2 HTTP/1.1\"}");
    // A GET carries no FRM token and no body, whatever the frame held.
    let seen = frm.seen.lock().unwrap().last().cloned().unwrap();
    assert_eq!(seen.line, "GET /getPower?a=1&b=2 HTTP/1.1");
    assert_eq!(seen.header("X-FRM-Authorization"), None);
    assert_eq!(
        seen.header("host"),
        Some(format!("127.0.0.1:{}", frm.port).as_str())
    );
    assert!(seen.body.is_empty());

    assert!(run.has(|e| *e == SatisfactoryEvent::Connected));
    assert!(run.has(|e| matches!(e, SatisfactoryEvent::FrmAnswering { .. })));
    let rows = run.core.rows_now();
    assert_eq!(rows[0].label, "Link to ryna.io");
    assert!(rows[0].value.starts_with("Connected since"), "{rows:?}");
    assert_eq!(rows[2].value.split(',').next(), Some("1 this session"));
    assert_eq!(run.stop(), Ok(()));
    assert_eq!(run.core.rows_now()[0].value, "Off");
}

#[test]
fn a_post_is_refused_while_changes_are_off_and_passed_with_its_header_when_on() {
    let frm = ok_frm();
    let api = api_server(vec![Upgrade::Accept]);
    let mut run = start(&api, core(&api, frm.port, false));
    let mut ws = api.next();
    recv(&mut ws, true);
    let before = frm.seen.lock().unwrap().len();
    send(&mut ws, req("w1", "POST", "/setEnabled", None));
    let err = recv(&mut ws, true);
    assert_eq!(
        err,
        json!({ "t": "err", "id": "w1", "code": "refused",
        "message": "changes to the factory aren't allowed on this computer" })
    );
    assert_eq!(
        frm.seen.lock().unwrap().len(),
        before,
        "nothing reached FRM"
    );
    run.wait_for("a refused write", |e| {
        *e == SatisfactoryEvent::WriteRefused {
            path: "/setEnabled".into(),
        }
    });
    run.stop().unwrap();
    assert!(!run.shown().contains(FRM_SECRET));

    let api = api_server(vec![Upgrade::Accept]);
    let mut run = start(&api, core(&api, frm.port, true));
    let mut ws = api.next();
    recv(&mut ws, true);
    send(&mut ws, req("w2", "POST", "/setEnabled", Some("x=1")));
    let res = recv(&mut ws, true);
    assert_eq!(res["status"], 200);
    let seen = frm.seen.lock().unwrap().last().cloned().unwrap();
    assert_eq!(seen.line, "POST /setEnabled?x=1 HTTP/1.1");
    assert_eq!(seen.header("X-FRM-Authorization"), Some(FRM_SECRET));
    assert_eq!(seen.header("Content-Type"), Some("application/json"));
    assert_eq!(seen.body, "{\"ID\":\"x\"}");
    run.stop().unwrap();
    // The write token is nowhere in what the relay reported.
    assert!(!run.shown().contains(FRM_SECRET));
    assert!(!format!("{:?}", run.core.rows_now()).contains(FRM_SECRET));
}

#[test]
fn every_path_query_and_method_refusal_sends_nothing_to_frm() {
    let frm = ok_frm();
    let api = api_server(vec![Upgrade::Accept]);
    let mut run = start(&api, core(&api, frm.port, true));
    let mut ws = api.next();
    recv(&mut ws, true);
    let before = frm.seen.lock().unwrap().len();
    let refusals: Vec<(&str, &str, Option<&str>)> = vec![
        ("GET", "//evil.example/x", None),
        ("GET", "http://evil.example/getPower", None),
        ("GET", "/getPower/../secret", None),
        ("GET", "/../getPower", None),
        ("GET", "/getPower@evil.example", None),
        ("GET", "getPower", None),
        ("GET", "/", None),
        ("GET", "/2getPower", None),
        ("GET", "/get-power", None),
        ("GET", "/getPower", Some("a=1#frag")),
        ("GET", "/getPower", Some("u=http://evil")),
        ("GET", "/getPower", Some("a b")),
        ("GET", "/getPower", Some("x=\r\nHost: evil")),
        ("PUT", "/getPower", None),
        ("DELETE", "/getPower", None),
        ("get", "/getPower", None),
        ("OPTIONS", "/getPower", None),
    ];
    for (i, (method, path, query)) in refusals.iter().enumerate() {
        let id = format!("bad{i}");
        send(&mut ws, req(&id, method, path, *query));
        let err = recv(&mut ws, true);
        assert_eq!(err["t"], "err", "{method} {path} {query:?}");
        assert_eq!(err["id"], id.as_str());
        assert_eq!(err["code"], "bad_request", "{method} {path} {query:?}");
    }
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        frm.seen.lock().unwrap().len(),
        before,
        "nothing reached FRM"
    );
    run.stop().unwrap();
}

#[test]
fn a_redirect_is_passed_back_not_followed() {
    let elsewhere = ok_frm();
    let target = format!("http://127.0.0.1:{}/getSecret", elsewhere.port);
    let frm = frm_server(Box::new(move |_| {
        http("302 Found", &format!("Location: {target}\r\n"), b"")
    }));
    let api = api_server(vec![Upgrade::Accept]);
    let mut run = start(&api, core(&api, frm.port, false));
    let mut ws = api.next();
    recv(&mut ws, true);
    send(&mut ws, req("r1", "GET", "/getPower", None));
    let res = recv(&mut ws, true);
    assert_eq!(res["status"], 302);
    assert!(
        elsewhere.seen.lock().unwrap().is_empty(),
        "the redirect wasn't followed"
    );
    run.stop().unwrap();
}

#[test]
fn too_large_slow_and_unreachable() {
    let big = frm_server(Box::new(|_| http("200 OK", "", &vec![b'x'; 2048])));
    let slow = frm_server(Box::new(|_| {
        std::thread::sleep(Duration::from_millis(1500));
        http("200 OK", "", b"{}")
    }));
    for (port, code, words) in [
        (big.port, "bad_request", "too large"),
        (slow.port, "timeout", ""),
        (dead_port(), "unreachable", "nothing is answering"),
    ] {
        let api = api_server(vec![Upgrade::Accept]);
        let frm = Frm::with_limits(port, false, Duration::from_millis(400), 1024);
        let mut run = start(&api, core(&api, port, false).with_frm(frm));
        let mut ws = api.next();
        let hello = recv(&mut ws, true);
        send(&mut ws, req("r1", "GET", "/getPower", None));
        let err = recv(&mut ws, true);
        assert_eq!(err["t"], "err", "{code}: {err}");
        assert_eq!(err["code"], code);
        assert!(err["message"].as_str().unwrap().contains(words), "{err}");
        if code == "unreachable" {
            assert_eq!(hello["frm"]["reachable"], false);
            run.wait_for("FRM not answering", |e| {
                matches!(e, SatisfactoryEvent::FrmNotAnswering { .. })
            });
            assert_eq!(
                run.core.rows_now()[1].value,
                "Not answering: is the game running with the mod?"
            );
        }
        run.stop().unwrap();
    }
}

#[test]
fn pings_every_30_s_and_reconnects_after_75_s_without_a_pong() {
    let frm = ok_frm();
    let clock = FakeClock::default();
    let api = api_server(vec![Upgrade::Accept]);
    let mut run = start(
        &api,
        core(&api, frm.port, false).with_clock(Arc::new(clock.clone())),
    );
    let mut ws = api.next();
    recv(&mut ws, true);
    clock.advance(Duration::from_secs(31));
    assert_eq!(recv(&mut ws, false), json!({ "t": "ping" }));
    send(&mut ws, json!({ "t": "pong" }));
    std::thread::sleep(Duration::from_millis(200));
    // A pong at 31 s: 75 s from then is 106 s. At 100 s the link holds.
    clock.advance(Duration::from_secs(69));
    std::thread::sleep(Duration::from_millis(200));
    assert!(!run.has(|e| matches!(e, SatisfactoryEvent::Disconnected { .. })));
    // No pong since: at 107 s it gives up and connects again.
    clock.advance(Duration::from_secs(7));
    run.wait_for(
        "a disconnect",
        |e| matches!(e, SatisfactoryEvent::Disconnected { why } if why.contains("75 s")),
    );
    let mut again = api.next();
    assert_eq!(recv(&mut again, true)["t"], "hello");
    run.stop().unwrap();
}

#[test]
fn close_4000_stops_with_another_computer_and_4001_with_needs_newer() {
    for (code, words) in [(4000u16, REPLACED), (4001, NEEDS_NEWER)] {
        let frm = ok_frm();
        let api = api_server(vec![Upgrade::Accept]);
        let mut run = start(&api, core(&api, frm.port, false));
        let mut ws = api.next();
        recv(&mut ws, true);
        ws.close(Some(CloseFrame {
            code: close_code(code),
            reason: "replaced".into(),
        }))
        .unwrap();
        let _ = ws.flush();
        assert_eq!(run.join(), Err(words.to_string()));
        assert!(run
            .has(|e| matches!(e, SatisfactoryEvent::Stopped { message, .. } if message == words)));
        // It didn't come back to fight over it.
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(api.attempts.lock().unwrap().len(), 1);
    }
}

#[test]
fn a_401_forgets_the_token_and_twice_says_sign_in_and_a_403_stops() {
    let frm = ok_frm();
    let api = api_server(vec![
        Upgrade::Status(401),
        Upgrade::Status(401),
        Upgrade::Accept,
    ]);
    let mut run = start(&api, core(&api, frm.port, false));
    let mut ws = api.next();
    assert_eq!(recv(&mut ws, true)["t"], "hello");
    assert_eq!(run.tokens.forgotten.load(Ordering::SeqCst), 2);
    assert!(run.has(|e| matches!(e, SatisfactoryEvent::Failed { fix, .. } if *fix == ryna_agent::fix::Fix::SignIn)));
    run.stop().unwrap();

    let api = api_server(vec![Upgrade::Status(403)]);
    let mut run = start(&api, core(&api, frm.port, false));
    assert_eq!(run.join(), Err(NOT_HOST.to_string()));
    assert!(
        run.has(|e| matches!(e, SatisfactoryEvent::Stopped { message, .. } if message == NOT_HOST))
    );
    assert_eq!(api.attempts.lock().unwrap().len(), 1);
}

#[test]
fn other_failures_back_off_and_never_give_up() {
    let frm = ok_frm();
    // A 500 at every upgrade: the relay keeps coming back, waiting longer each time.
    let api = api_server(vec![Upgrade::Status(500)]);
    let mut run = start(&api, core(&api, frm.port, false));
    std::thread::sleep(Duration::from_millis(1800));
    let attempts = api.attempts.lock().unwrap().clone();
    assert!(attempts.len() >= 4, "{}", attempts.len());
    let gaps: Vec<Duration> = attempts.windows(2).map(|w| w[1] - w[0]).collect();
    // Waits are at least half the backoff (jitter) and never above the 400 ms ceiling (plus slack).
    assert!(gaps[0] >= Duration::from_millis(20), "{gaps:?}");
    assert!(
        gaps.iter().all(|g| *g < Duration::from_millis(700)),
        "{gaps:?}"
    );
    // One line for the run of failures, not one each.
    let lines = run
        .events
        .lock()
        .unwrap()
        .iter()
        .filter(|e| matches!(e, SatisfactoryEvent::Disconnected { .. }))
        .count();
    assert_eq!(lines, 1);
    assert_eq!(run.core.rows_now()[0].value, "Reconnecting…");
    run.stop().unwrap();
}

#[test]
fn requests_are_answered_four_at_a_time_and_a_running_id_is_ignored() {
    let frm = frm_server(Box::new(|_| {
        std::thread::sleep(Duration::from_millis(400));
        http("200 OK", "", b"{}")
    }));
    let api = api_server(vec![Upgrade::Accept]);
    let mut run = start(&api, core(&api, frm.port, false));
    let mut ws = api.next();
    recv(&mut ws, true);
    let started = Instant::now();
    for id in ["a", "b", "c", "d"] {
        send(&mut ws, req(id, "GET", "/getPower", None));
    }
    // "a" again while it runs: ignored.
    send(&mut ws, req("a", "GET", "/getPower", None));
    let mut ids = Vec::new();
    for _ in 0..4 {
        ids.push(recv(&mut ws, true)["id"].as_str().unwrap().to_string());
    }
    let took = started.elapsed();
    ids.sort();
    assert_eq!(ids, ["a", "b", "c", "d"]);
    assert!(
        took < Duration::from_millis(1200),
        "answered in turn, not together: {took:?}"
    );
    std::thread::sleep(Duration::from_millis(600));
    // No second answer for "a", and FRM was called four times.
    ws.get_ref()
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    assert!(matches!(ws.read(), Err(tungstenite::Error::Io(_))));
    assert_eq!(
        frm.seen.lock().unwrap().len(),
        4 + 1,
        "four requests and the hello's look"
    );
    run.stop().unwrap();
}

#[test]
fn sync_now_reconnects_at_once_while_backing_off() {
    let frm = ok_frm();
    let api = api_server(vec![Upgrade::Status(500), Upgrade::Accept]);
    let mut slow = timing();
    slow.backoff_start = Duration::from_secs(30);
    let mut run = start(
        &api,
        SatisfactoryCore::new(
            &format!("http://127.0.0.1:{}", api.port),
            RelaySettings {
                port: frm.port,
                allow_changes: false,
            },
        )
        .with_timing(slow),
    );
    run.wait_for("the first failure", |e| {
        matches!(e, SatisfactoryEvent::Disconnected { .. })
    });
    let asked = Instant::now();
    run.control.sync();
    let mut ws = api.next();
    assert!(asked.elapsed() < Duration::from_secs(5));
    assert_eq!(recv(&mut ws, true)["t"], "hello");
    run.stop().unwrap();
}
