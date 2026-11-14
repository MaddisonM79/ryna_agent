//! The relay: one outbound WebSocket to the API (`wss://<api host>/v1/frm/agent/relay`), held
//! while the core is on, answering each request the API sends down it by calling FRM on
//! `127.0.0.1` ([`crate::frm`]). Nothing listens for the internet.
//!
//! The wire (text frames, one JSON object each):
//! - up, first: `{ "t": "hello", "protocol": 1, "agent": "ryna-agent/<v>", "frm": { "reachable": bool } }`
//! - down: `{ "t": "req", "id", "method", "path", "query", "headers", "body" }`
//! - up: `{ "t": "res", "id", "status", "contentType", "body" }` or `{ "t": "err", "id", "code", "message" }`
//! - up every 30 s: `{ "t": "ping" }`; down: `{ "t": "pong" }`; no pong for 75 s: reconnect.
//!
//! Endings: close 4000 (another Ryna is the factory's origin now) and 4001 (protocol) stop the
//! core; a 401 at the upgrade forgets the access token and tries again (twice in a row: sign in
//! again); a 403 stops it ("This account doesn't host the factory."); anything else reconnects,
//! 1 s doubling to 60 s with jitter, for as long as the core is on. Requests are answered by a
//! small pool (four at a time); a request whose id is already running is ignored.

use std::collections::HashSet;
use std::io::ErrorKind;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ryna_agent::cancel::Waited;
use ryna_agent::core::{CoreControl, TokenProvider};
use ryna_agent::fix::Fix;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tungstenite::client::IntoClientRequest;
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

use crate::frm::{Answer, Frm, FrmRequest};

/// The relay's protocol.
pub const PROTOCOL: u64 = 1;
pub const RELAY_PATH: &str = "/v1/frm/agent/relay";
/// The words for each way the core stops for good.
pub const REPLACED: &str = "Another computer is now the factory's link to ryna.io.";
pub const NEEDS_NEWER: &str = "this core needs a newer Ryna";
pub const NOT_HOST: &str = "This account doesn't host the factory.";

/// The relay's clocks: the spec's, or shorter ones for tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    pub ping_every: Duration,
    pub pong_limit: Duration,
    pub backoff_start: Duration,
    pub backoff_max: Duration,
    /// How often the loop looks at the socket, the answers and the control.
    pub tick: Duration,
    /// The TCP connect and the upgrade.
    pub connect_limit: Duration,
    pub workers: usize,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            ping_every: Duration::from_secs(30),
            pong_limit: Duration::from_secs(75),
            backoff_start: Duration::from_secs(1),
            backoff_max: Duration::from_secs(60),
            tick: Duration::from_millis(50),
            connect_limit: Duration::from_secs(20),
            workers: 4,
        }
    }
}

/// A monotonic clock, injected so a test can run the 75 s rule in an instant.
pub trait Clock: Send + Sync {
    /// Time since some fixed start.
    fn now(&self) -> Duration;
}

pub struct RealClock(Instant);

impl Default for RealClock {
    fn default() -> Self {
        RealClock(Instant::now())
    }
}

impl Clock for RealClock {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }
}

/// What the relay reports (`{"event": "connected"}` and so on): the link and FRM changing, and a
/// refused write. Not one per request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum SatisfactoryEvent {
    Connected,
    Disconnected {
        why: String,
    },
    #[serde(rename_all = "camelCase")]
    FrmAnswering {
        port: u16,
    },
    #[serde(rename_all = "camelCase")]
    FrmNotAnswering {
        port: u16,
    },
    WriteRefused {
        path: String,
    },
    /// The link can't be made for a reason the person can fix (the sign-in).
    #[serde(rename_all = "camelCase")]
    Failed {
        message: String,
        fix: Fix,
        advice: String,
    },
    /// The core has stopped for good (until started again).
    Stopped {
        message: String,
        fix: Option<String>,
    },
}

/// Where the link is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Link {
    #[default]
    Off,
    Reconnecting,
    /// Since (unix seconds).
    Connected(i64),
}

/// What the status card reads.
#[derive(Debug, Clone, Default)]
pub struct RelayStatus {
    pub link: Link,
    /// None until known.
    pub frm_answering: Option<bool>,
    pub requests: u64,
    pub last_request_at: Option<i64>,
}

/// The two settings the person chooses on the core's page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaySettings {
    pub port: u16,
    pub allow_changes: bool,
}

impl Default for RelaySettings {
    fn default() -> Self {
        RelaySettings {
            port: crate::frm::DEFAULT_PORT,
            allow_changes: false,
        }
    }
}

/// `wss://<api host>/v1/frm/agent/relay` from the API's URL (`ws://` for a loopback `http://`).
pub fn relay_url(api: &str) -> Option<String> {
    let api = api.trim_end_matches('/');
    let rest = match api.strip_prefix("https://") {
        Some(r) => format!("wss://{r}"),
        None => format!("ws://{}", api.strip_prefix("http://")?),
    };
    Some(format!("{rest}{RELAY_PATH}"))
}

/// The jittered wait before the next connect: between half and all of `d`.
fn jitter(d: Duration) -> Duration {
    let mut b = [0u8; 2];
    let r = if getrandom::fill(&mut b).is_ok() {
        u16::from_le_bytes(b)
    } else {
        u16::MAX
    };
    d / 2 + d.mul_f64(f64::from(r) / f64::from(u16::MAX) / 2.0)
}

/// The next backoff after `d`.
pub fn next_backoff(d: Duration, timing: &Timing) -> Duration {
    (d * 2).min(timing.backoff_max)
}

#[derive(Debug)]
enum ConnectError {
    /// The upgrade was answered with this HTTP status.
    Status(u16),
    Other(String),
}

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

fn tcp(stream: &MaybeTlsStream<TcpStream>) -> Option<&TcpStream> {
    match stream {
        MaybeTlsStream::Plain(s) => Some(s),
        MaybeTlsStream::Rustls(s) => Some(s.get_ref()),
        _ => None,
    }
}

fn connect(url: &str, token: &str, timing: &Timing) -> Result<Socket, ConnectError> {
    let other = |e: &dyn std::fmt::Display| ConnectError::Other(e.to_string());
    let mut request = url.into_client_request().map_err(|e| other(&e))?;
    let bearer = format!("Bearer {token}")
        .parse()
        .map_err(|_| ConnectError::Other("the access token isn't a header value".into()))?;
    request.headers_mut().insert("Authorization", bearer);
    let uri = request.uri().clone();
    let host = uri.host().unwrap_or("").trim_matches(['[', ']']);
    let port = uri
        .port_u16()
        .unwrap_or(if uri.scheme_str() == Some("wss") {
            443
        } else {
            80
        });
    let addr = (host, port)
        .to_socket_addrs()
        .map_err(|e| other(&e))?
        .next()
        .ok_or_else(|| ConnectError::Other(format!("{host} has no address")))?;
    let stream = TcpStream::connect_timeout(&addr, timing.connect_limit).map_err(|e| other(&e))?;
    stream
        .set_read_timeout(Some(timing.connect_limit))
        .map_err(|e| other(&e))?;
    stream
        .set_write_timeout(Some(timing.connect_limit))
        .map_err(|e| other(&e))?;
    let (ws, _) =
        tungstenite::client_tls_with_config(request, stream, None, None).map_err(|e| match e {
            tungstenite::HandshakeError::Failure(tungstenite::Error::Http(res)) => {
                ConnectError::Status(res.status().as_u16())
            }
            e => other(&e),
        })?;
    if let Some(s) = tcp(ws.get_ref()) {
        s.set_read_timeout(Some(timing.tick))
            .map_err(|e| other(&e))?;
    }
    Ok(ws)
}

/// How a connected session ended.
enum End {
    Asked,
    Replaced,
    Protocol,
    Lost(String),
}

/// The relay, as the core runs it.
pub struct Relay<'a> {
    pub url: String,
    pub frm: Frm,
    pub timing: Timing,
    pub clock: Arc<dyn Clock>,
    pub status: Arc<Mutex<RelayStatus>>,
    pub tokens: &'a dyn TokenProvider,
}

fn lock(s: &Mutex<RelayStatus>) -> MutexGuard<'_, RelayStatus> {
    s.lock().unwrap_or_else(|e| e.into_inner())
}

fn up_frame(id: &str, answer: &Answer) -> Value {
    match answer {
        Answer::Res {
            status,
            content_type,
            body,
        } => {
            json!({ "t": "res", "id": id, "status": status, "contentType": content_type, "body": body })
        }
        Answer::Err { code, message } => {
            json!({ "t": "err", "id": id, "code": code, "message": message })
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
enum Down {
    Req {
        id: String,
        method: String,
        path: String,
        #[serde(default)]
        query: Option<String>,
        #[serde(default)]
        headers: Option<std::collections::BTreeMap<String, String>>,
        #[serde(default)]
        body: Option<String>,
    },
    Pong,
    #[serde(other)]
    Other,
}

impl Relay<'_> {
    /// Hold the link until `control` stops it, or a close says to stop. `Err` is the words the
    /// core stopped with (4000, 4001, 403).
    pub fn run(
        &self,
        control: &CoreControl,
        events: &mut dyn FnMut(SatisfactoryEvent),
    ) -> Result<(), String> {
        let mut delay = self.timing.backoff_start;
        let mut refused_in_a_row = 0u32;
        let mut quiet = false;
        let result = loop {
            if control.is_stopped() {
                break Ok(());
            }
            lock(&self.status).link = Link::Reconnecting;
            // One line for a run of failed connects, not one each time.
            let lost =
                |why: String, quiet: &mut bool, events: &mut dyn FnMut(SatisfactoryEvent)| {
                    if !*quiet {
                        events(SatisfactoryEvent::Disconnected { why });
                        *quiet = true;
                    }
                };
            match self.tokens.access_token() {
                Ok(Some(token)) => match connect(&self.url, &token, &self.timing) {
                    Ok(ws) => {
                        refused_in_a_row = 0;
                        let since = self.clock.now();
                        lock(&self.status).link = Link::Connected(ryna_agent::util::now_secs());
                        events(SatisfactoryEvent::Connected);
                        match self.session(ws, control, events) {
                            End::Asked => break Ok(()),
                            End::Replaced => break Err(REPLACED.to_string()),
                            End::Protocol => break Err(NEEDS_NEWER.to_string()),
                            End::Lost(why) => {
                                lock(&self.status).link = Link::Reconnecting;
                                events(SatisfactoryEvent::Disconnected { why });
                                quiet = true;
                                if self.clock.now() - since >= self.timing.backoff_max {
                                    delay = self.timing.backoff_start;
                                }
                            }
                        }
                    }
                    Err(ConnectError::Status(401)) => {
                        refused_in_a_row += 1;
                        self.tokens.forget_access_token();
                        if refused_in_a_row >= 2 {
                            let message = "ryna.io refused the sign-in".to_string();
                            events(SatisfactoryEvent::Failed {
                                advice: Fix::SignIn.message(&message),
                                message,
                                fix: Fix::SignIn,
                            });
                        }
                    }
                    Err(ConnectError::Status(403)) => break Err(NOT_HOST.to_string()),
                    Err(ConnectError::Status(s)) => {
                        lost(format!("ryna.io answered HTTP {s}"), &mut quiet, events)
                    }
                    Err(ConnectError::Other(e)) => {
                        lost(format!("couldn't reach ryna.io: {e}"), &mut quiet, events)
                    }
                },
                Ok(None) => {
                    let message = "not signed in".to_string();
                    events(SatisfactoryEvent::Failed {
                        advice: Fix::SignIn.message(&message),
                        message,
                        fix: Fix::SignIn,
                    });
                }
                Err(e) => {
                    let message = e.to_string();
                    events(SatisfactoryEvent::Failed {
                        advice: e.fix().message(&message),
                        fix: e.fix(),
                        message,
                    });
                }
            }
            match control.token().wait_for(jitter(delay)) {
                Waited::Cancelled => break Ok(()),
                // Sync now: try at once.
                Waited::Woken => {}
                Waited::TimedOut => delay = next_backoff(delay, &self.timing),
            }
        };
        lock(&self.status).link = Link::Off;
        result
    }

    fn set_frm(&self, answering: bool, events: &mut dyn FnMut(SatisfactoryEvent)) {
        let was = lock(&self.status).frm_answering.replace(answering);
        if was != Some(answering) {
            let port = self.frm.port;
            events(if answering {
                SatisfactoryEvent::FrmAnswering { port }
            } else {
                SatisfactoryEvent::FrmNotAnswering { port }
            });
        }
    }

    fn session(
        &self,
        mut ws: Socket,
        control: &CoreControl,
        events: &mut dyn FnMut(SatisfactoryEvent),
    ) -> End {
        // The keepalive's clocks start with the link, not after the hello and the pool.
        let opened = self.clock.now();
        let reachable = self.frm.reachable();
        self.set_frm(reachable, events);
        let hello = json!({ "t": "hello", "protocol": PROTOCOL, "agent": ryna_agent::user_agent(),
                            "frm": { "reachable": reachable } });
        if let Err(e) = ws.send(Message::text(hello.to_string())) {
            return End::Lost(format!("the link broke: {e}"));
        }
        // The pool: jobs down, answers back, the ids running.
        let (job_tx, job_rx) = mpsc::channel::<FrmRequest>();
        let job_rx = Arc::new(Mutex::new(job_rx));
        let (ans_tx, ans_rx) = mpsc::channel::<(String, Answer)>();
        let running: Arc<Mutex<HashSet<String>>> = Arc::default();
        for _ in 0..self.timing.workers.max(1) {
            spawn_worker(self.frm.clone(), job_rx.clone(), ans_tx.clone());
        }
        drop(ans_tx);
        let pool = Pool {
            jobs: job_tx,
            answers: ans_rx,
            running,
        };
        let end = self.pump(&mut ws, opened, control, events, &pool);
        drop(pool);
        match end {
            End::Asked => {
                let _ = ws.close(None);
                let _ = ws.flush();
            }
            End::Lost(_) => {
                let _ = ws.close(None);
            }
            _ => {}
        }
        end
    }

    fn pump(
        &self,
        ws: &mut Socket,
        opened: Duration,
        control: &CoreControl,
        events: &mut dyn FnMut(SatisfactoryEvent),
        pool: &Pool,
    ) -> End {
        let Pool {
            jobs,
            answers,
            running,
        } = pool;
        let mut last_ping = opened;
        let mut last_pong = opened;
        loop {
            match ws.read() {
                Ok(Message::Text(text)) => match serde_json::from_str::<Down>(&text) {
                    Ok(Down::Pong) => last_pong = self.clock.now(),
                    Ok(Down::Req {
                        id,
                        method,
                        path,
                        query,
                        headers,
                        body,
                    }) => {
                        let fresh = running
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(id.clone());
                        if !fresh {
                            continue;
                        }
                        {
                            let mut st = lock(&self.status);
                            st.requests += 1;
                            st.last_request_at = Some(ryna_agent::util::now_secs());
                        }
                        let req = FrmRequest {
                            id,
                            method,
                            path,
                            query,
                            headers: headers.unwrap_or_default(),
                            body,
                        };
                        if let Err(refusal) = self.frm.check(&req) {
                            // Refused here: nothing is sent to FRM.
                            if matches!(
                                refusal,
                                Answer::Err {
                                    code: crate::frm::ErrCode::Refused,
                                    ..
                                }
                            ) {
                                events(SatisfactoryEvent::WriteRefused {
                                    path: req.path.clone(),
                                });
                            }
                            running
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .remove(&req.id);
                            if ws
                                .send(Message::text(up_frame(&req.id, &refusal).to_string()))
                                .is_err()
                            {
                                return End::Lost("the link broke".into());
                            }
                            continue;
                        }
                        if jobs.send(req).is_err() {
                            return End::Lost("the request pool stopped".into());
                        }
                    }
                    Ok(Down::Other) | Err(_) => {}
                },
                Ok(Message::Close(frame)) => {
                    return match frame.map(|f| u16::from(f.code)) {
                        Some(4000) => End::Replaced,
                        Some(4001) => End::Protocol,
                        Some(code) => End::Lost(format!("ryna.io closed the link ({code})")),
                        None => End::Lost("ryna.io closed the link".into()),
                    };
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                    return End::Lost("ryna.io closed the link".into())
                }
                Err(e) => return End::Lost(format!("the link broke: {e}")),
            }
            while let Ok((id, answer)) = answers.try_recv() {
                running
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&id);
                let answered = answer.frm_answered()
                    || !matches!(
                        answer,
                        Answer::Err {
                            code: crate::frm::ErrCode::Unreachable,
                            ..
                        }
                    );
                self.set_frm(answered, events);
                if ws
                    .send(Message::text(up_frame(&id, &answer).to_string()))
                    .is_err()
                {
                    return End::Lost("the link broke".into());
                }
            }
            let now = self.clock.now();
            if now.saturating_sub(last_pong) > self.timing.pong_limit {
                return End::Lost(format!(
                    "no answer from ryna.io for {} s",
                    self.timing.pong_limit.as_secs()
                ));
            }
            let mut ping = now.saturating_sub(last_ping) >= self.timing.ping_every;
            match control.token().wait_for(Duration::ZERO) {
                Waited::Cancelled => return End::Asked,
                // Sync now: check the link at once.
                Waited::Woken => ping = true,
                Waited::TimedOut => {}
            }
            if ping {
                last_ping = now;
                if ws
                    .send(Message::text(json!({ "t": "ping" }).to_string()))
                    .is_err()
                {
                    return End::Lost("the link broke".into());
                }
            }
        }
    }
}

/// A session's request pool: jobs to the workers, their answers back, the ids running.
struct Pool {
    jobs: Sender<FrmRequest>,
    answers: Receiver<(String, Answer)>,
    running: Arc<Mutex<HashSet<String>>>,
}

fn spawn_worker(
    frm: Frm,
    jobs: Arc<Mutex<Receiver<FrmRequest>>>,
    answers: Sender<(String, Answer)>,
) {
    std::thread::spawn(move || loop {
        let job = {
            let rx = jobs.lock().unwrap_or_else(|e| e.into_inner());
            rx.recv()
        };
        let Ok(req) = job else { return };
        let answer = frm.call(&req);
        if answers.send((req.id, answer)).is_err() {
            return;
        }
    });
}

/// `CloseCode` for 4000/4001, for a test server.
pub fn close_code(code: u16) -> CloseCode {
    CloseCode::from(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_relays_address_comes_from_the_apis() {
        assert_eq!(
            relay_url("https://api.ryna.io").as_deref(),
            Some("wss://api.ryna.io/v1/frm/agent/relay")
        );
        assert_eq!(
            relay_url("http://localhost:8797/").as_deref(),
            Some("ws://localhost:8797/v1/frm/agent/relay")
        );
        assert_eq!(relay_url("ftp://x"), None);
    }

    #[test]
    fn a_wss_link_builds_its_tls_without_panicking() {
        // A local listener that speaks no TLS: the handshake fails cleanly, which proves rustls
        // found its crypto provider (two in the tree would panic here).
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (s, _) = listener.accept().unwrap();
            drop(s);
        });
        let timing = Timing {
            connect_limit: Duration::from_secs(5),
            ..Timing::default()
        };
        let e = connect(&format!("wss://127.0.0.1:{port}{RELAY_PATH}"), "t", &timing).unwrap_err();
        assert!(matches!(e, ConnectError::Other(_)), "{e:?}");
        server.join().unwrap();
    }

    #[test]
    fn backoff_doubles_to_a_minute_with_jitter() {
        let t = Timing::default();
        let mut d = t.backoff_start;
        let mut seen = vec![d];
        for _ in 0..8 {
            d = next_backoff(d, &t);
            seen.push(d);
        }
        let secs: Vec<u64> = seen.iter().map(|d| d.as_secs()).collect();
        assert_eq!(secs, [1, 2, 4, 8, 16, 32, 60, 60, 60]);
        for _ in 0..50 {
            let j = jitter(Duration::from_secs(60));
            assert!(
                j >= Duration::from_secs(30) && j <= Duration::from_secs(60),
                "{j:?}"
            );
        }
    }

    #[test]
    fn answers_go_up_in_the_wire_shape() {
        assert_eq!(
            up_frame(
                "r1",
                &Answer::Res {
                    status: 200,
                    content_type: "application/json".into(),
                    body: "[]".into()
                }
            ),
            json!({ "t": "res", "id": "r1", "status": 200, "contentType": "application/json", "body": "[]" })
        );
        assert_eq!(
            up_frame(
                "r2",
                &Answer::Err {
                    code: crate::frm::ErrCode::BadRequest,
                    message: "no".into()
                }
            ),
            json!({ "t": "err", "id": "r2", "code": "bad_request", "message": "no" })
        );
    }
}
