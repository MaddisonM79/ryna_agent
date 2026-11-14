//! The one call the relay makes: Ficsit Remote Monitoring on this computer's `127.0.0.1`. This is
//! the security boundary between ryna.io and the PC, so every rule is checked here, before
//! anything is sent, whatever a frame says:
//!
//! - only `http://127.0.0.1:<port>`, the port from the core's setting; never a redirect, never a
//!   proxy, never another host;
//! - `path` is one FRM endpoint word (`^/[A-Za-z][A-Za-z0-9_]*$`), `query` only
//!   `[A-Za-z0-9_=&%.,-]*`; anything else is `bad_request` and nothing is sent;
//! - `GET` always; `POST` only while "Let ryna.io change the factory" is on (else `refused`); no
//!   other method;
//! - only `X-FRM-Authorization` is forwarded, only on a POST, and its value never reaches a log,
//!   an event or the state folder (the request's `Debug` hides it);
//! - 20 s a call (`timeout`), 8 MB a response (`bad_request`, too large); FRM's own status is
//!   passed through;
//! - the connect has its own, shorter limit (3 s, and always under the call's), and any failure
//!   before the connection is made, refused or timed out, is `unreachable`: on Windows a connect
//!   to a closed port on 127.0.0.1 isn't refused at once but retried for about 2 s, and must still
//!   read as "nothing is listening", not as a slow FRM. Only a connection that was made and then
//!   didn't answer in time is `timeout`.

use std::collections::BTreeMap;
use std::io::Read;
use std::time::Duration;

use serde::Serialize;

/// The header FRM takes its write token in: the only one forwarded.
pub const AUTH_HEADER: &str = "X-FRM-Authorization";
pub const DEFAULT_PORT: u16 = 8080;
pub const CALL_LIMIT: Duration = Duration::from_secs(20);
/// The connect's own limit (and never more than half the call's, so it always ends first).
pub const CONNECT_LIMIT: Duration = Duration::from_secs(3);
pub const MAX_BODY: usize = 8 * 1024 * 1024;

/// Why a request wasn't answered by FRM (the wire's `err.code`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrCode {
    Unreachable,
    Refused,
    Timeout,
    BadRequest,
}

/// One request from the API, as the relay received it.
#[derive(Clone, PartialEq, Eq)]
pub struct FrmRequest {
    pub id: String,
    pub method: String,
    pub path: String,
    pub query: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub body: Option<String>,
}

impl std::fmt::Debug for FrmRequest {
    // Never a header's value: the FRM write token travels in one.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrmRequest")
            .field("id", &self.id)
            .field("method", &self.method)
            .field("path", &self.path)
            .field("query", &self.query)
            .field("headers", &self.headers.keys().collect::<Vec<_>>())
            .field("body", &self.body.as_ref().map(|b| b.len()))
            .finish()
    }
}

/// What the relay sends back for a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    Res {
        status: u16,
        content_type: String,
        body: String,
    },
    Err {
        code: ErrCode,
        message: String,
    },
}

impl Answer {
    fn err(code: ErrCode, message: impl Into<String>) -> Answer {
        Answer::Err {
            code,
            message: message.into(),
        }
    }

    /// FRM answered (whatever its status): it is running.
    pub fn frm_answered(&self) -> bool {
        matches!(self, Answer::Res { .. })
    }
}

fn path_ok(path: &str) -> bool {
    let mut chars = path.chars();
    chars.next() == Some('/')
        && chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn query_ok(query: &str) -> bool {
    query
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "_=&%.,-".contains(c))
}

/// How the core calls FRM: the port and whether changes are allowed (the core's settings).
#[derive(Debug, Clone)]
pub struct Frm {
    pub port: u16,
    pub allow_changes: bool,
    pub limit: Duration,
    /// The connect's limit: `CONNECT_LIMIT`, or half the call's when that is shorter.
    pub connect_limit: Duration,
    pub max_body: usize,
    client: reqwest::blocking::Client,
}

impl Frm {
    pub fn new(port: u16, allow_changes: bool) -> Frm {
        Frm::with_limits(port, allow_changes, CALL_LIMIT, MAX_BODY)
    }

    /// The same with other limits (tests use shorter ones).
    pub fn with_limits(port: u16, allow_changes: bool, limit: Duration, max_body: usize) -> Frm {
        let connect_limit = CONNECT_LIMIT.min(limit / 2);
        let client = reqwest::blocking::Client::builder()
            .user_agent(ryna_agent::user_agent())
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            // Ends a connect that hangs before the call's own limit can: then it is a connect
            // failure (`is_connect`), never a call that ran out of time.
            .connect_timeout(connect_limit)
            .timeout(limit)
            .build()
            // Only fails when TLS can't initialise, and this client speaks plain http alone.
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Frm {
            port,
            allow_changes,
            limit,
            connect_limit,
            max_body,
            client,
        }
    }

    pub fn base(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Whether a request may be sent at all, before anything is.
    pub fn check(&self, req: &FrmRequest) -> Result<(), Answer> {
        if !path_ok(&req.path) {
            return Err(Answer::err(
                ErrCode::BadRequest,
                "that isn't an FRM endpoint",
            ));
        }
        if !req.query.as_deref().is_none_or(query_ok) {
            return Err(Answer::err(
                ErrCode::BadRequest,
                "the query holds characters FRM's don't",
            ));
        }
        match req.method.as_str() {
            "GET" => Ok(()),
            "POST" if self.allow_changes => Ok(()),
            "POST" => Err(Answer::err(
                ErrCode::Refused,
                "changes to the factory aren't allowed on this computer",
            )),
            _ => Err(Answer::err(ErrCode::BadRequest, "only GET and POST")),
        }
    }

    /// Call FRM for `req`, within every rule above.
    pub fn call(&self, req: &FrmRequest) -> Answer {
        if let Err(refusal) = self.check(req) {
            return refusal;
        }
        let mut url = format!("{}{}", self.base(), req.path);
        if let Some(q) = req.query.as_deref().filter(|q| !q.is_empty()) {
            url.push('?');
            url.push_str(q);
        }
        let mut r = if req.method == "POST" {
            let mut r = self.client.post(&url);
            if let Some(token) = req
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(AUTH_HEADER))
                .map(|(_, v)| v)
            {
                r = r.header(AUTH_HEADER, token.as_str());
            }
            if let Some(body) = &req.body {
                r = r
                    .header("Content-Type", "application/json")
                    .body(body.clone());
            }
            r
        } else {
            self.client.get(&url)
        };
        r = r.header("Accept", "application/json");
        let res = match r.send() {
            Ok(res) => res,
            Err(e) => return self.failure(&e),
        };
        let status = res.status().as_u16();
        let content_type = res
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        if res
            .content_length()
            .is_some_and(|n| n > self.max_body as u64)
        {
            return Answer::err(ErrCode::BadRequest, "FRM's answer is too large");
        }
        let mut body = Vec::new();
        let read = res.take(self.max_body as u64 + 1).read_to_end(&mut body);
        if let Err(e) = read {
            return if e.kind() == std::io::ErrorKind::TimedOut
                || e.to_string().contains("timed out")
            {
                Answer::err(ErrCode::Timeout, "FRM didn't finish answering in time")
            } else {
                Answer::err(ErrCode::Unreachable, format!("FRM's answer broke off: {e}"))
            };
        }
        if body.len() > self.max_body {
            return Answer::err(ErrCode::BadRequest, "FRM's answer is too large");
        }
        Answer::Res {
            status,
            content_type,
            body: String::from_utf8_lossy(&body).into_owned(),
        }
    }

    fn failure(&self, e: &reqwest::Error) -> Answer {
        // The connect phase first: a refused connect and one that timed out (`is_connect` and
        // `is_timeout` both) are the same thing here, nothing listening.
        if e.is_connect() {
            Answer::err(
                ErrCode::Unreachable,
                format!("nothing is answering on {}", self.base()),
            )
        } else if e.is_timeout() {
            Answer::err(
                ErrCode::Timeout,
                format!("FRM didn't answer within {} s", self.limit.as_secs()),
            )
        } else {
            Answer::err(ErrCode::Unreachable, "FRM couldn't be reached")
        }
    }

    /// A quick look: does anything answer on the port? (For `hello` and the Factory row.)
    pub fn reachable(&self) -> bool {
        let quick = Frm::with_limits(self.port, false, Duration::from_secs(3), 64 * 1024);
        quick
            .call(&FrmRequest {
                id: String::new(),
                method: "GET".into(),
                path: "/getSessionInfo".into(),
                query: None,
                headers: BTreeMap::new(),
                body: None,
            })
            .frm_answered()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_queries_are_frms_shapes_only() {
        for good in ["/getPower", "/getFactory", "/a", "/get_Drone2"] {
            assert!(path_ok(good), "{good}");
        }
        for bad in [
            "",
            "/",
            "getPower",
            "//evil",
            "/2fast",
            "/get-power",
            "/getPower/x",
            "/../x",
            "/getPower?x=1",
            "/get Power",
            "/getPower#",
            "/é",
            "/a\n",
        ] {
            assert!(!path_ok(bad), "{bad:?}");
        }
        for good in ["", "a=1&b=2", "id=Build_X_C,Build_Y_C", "x=%20.5-1"] {
            assert!(query_ok(good), "{good}");
        }
        for bad in ["a=1 b", "a=/etc", "@host", "a=1#f", "a=?", "x=\r\n"] {
            assert!(!query_ok(bad), "{bad:?}");
        }
    }

    fn get() -> FrmRequest {
        FrmRequest {
            id: "1".into(),
            method: "GET".into(),
            path: "/getPower".into(),
            query: None,
            headers: BTreeMap::new(),
            body: None,
        }
    }

    #[test]
    fn the_connect_always_ends_before_the_call() {
        assert_eq!(Frm::new(8080, false).connect_limit, CONNECT_LIMIT);
        let quick = Frm::with_limits(8080, false, Duration::from_millis(400), 1024);
        assert_eq!(quick.connect_limit, Duration::from_millis(200));
    }

    #[test]
    fn nothing_listening_is_unreachable_and_a_stalled_frm_is_a_timeout() {
        // A port nothing listens on, at the real limits: refused (or, on Windows, retried until
        // the connect's own limit), but never the call's timeout.
        let dead = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = dead.local_addr().unwrap().port();
        drop(dead);
        let started = std::time::Instant::now();
        let Answer::Err { code, .. } = Frm::new(port, false).call(&get()) else {
            panic!("answered")
        };
        assert_eq!(code, ErrCode::Unreachable);
        assert!(started.elapsed() < CONNECT_LIMIT + Duration::from_secs(2));
        assert!(!Frm::new(port, false).reachable());
        // Short limits too: the connect's half ends first, so it's still unreachable.
        let Answer::Err { code, .. } =
            Frm::with_limits(port, false, Duration::from_millis(400), 1024).call(&get())
        else {
            panic!("answered")
        };
        assert_eq!(code, ErrCode::Unreachable);

        // Accepts the connection, then never answers: that one is a timeout.
        let stall = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = stall.local_addr().unwrap().port();
        let held = std::thread::spawn(move || {
            let (s, _) = stall.accept().unwrap();
            std::thread::sleep(Duration::from_millis(1500));
            drop(s);
        });
        let Answer::Err { code, .. } =
            Frm::with_limits(port, false, Duration::from_millis(400), 1024).call(&get())
        else {
            panic!("answered")
        };
        assert_eq!(code, ErrCode::Timeout);
        held.join().unwrap();
    }

    #[test]
    fn a_connect_that_runs_out_of_time_is_unreachable_not_a_timeout() {
        // What Windows does to a closed loopback port (retry the connect for about 2 s), played by
        // an address that never answers a connect: the connect's own limit ends it, reqwest says
        // so with `is_connect` (and `is_timeout`), and that is `unreachable`.
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_millis(200))
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let e = client.get("http://10.255.255.1:9/").send().unwrap_err();
        assert!(e.is_connect(), "{e:?}");
        let frm = Frm::new(8080, false);
        assert!(matches!(
            frm.failure(&e),
            Answer::Err {
                code: ErrCode::Unreachable,
                ..
            }
        ));
    }

    #[test]
    fn a_requests_debug_never_shows_a_header_value() {
        let req = FrmRequest {
            id: "1".into(),
            method: "POST".into(),
            path: "/setEnabled".into(),
            query: None,
            headers: BTreeMap::from([(AUTH_HEADER.to_string(), "secret-frm-token".to_string())]),
            body: Some("{}".into()),
        };
        let shown = format!("{req:?}");
        assert!(!shown.contains("secret"), "{shown}");
        assert!(shown.contains(AUTH_HEADER));
    }
}
