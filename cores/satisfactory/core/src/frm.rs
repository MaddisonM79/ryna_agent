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
//! - 20 s a call (`timeout`), 8 MB a response (`bad_request`, too large), a refused connection is
//!   `unreachable`; FRM's own status is passed through.

use std::collections::BTreeMap;
use std::io::Read;
use std::time::Duration;

use serde::Serialize;

/// The header FRM takes its write token in: the only one forwarded.
pub const AUTH_HEADER: &str = "X-FRM-Authorization";
pub const DEFAULT_PORT: u16 = 8080;
pub const CALL_LIMIT: Duration = Duration::from_secs(20);
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
    pub max_body: usize,
    client: reqwest::blocking::Client,
}

impl Frm {
    pub fn new(port: u16, allow_changes: bool) -> Frm {
        Frm::with_limits(port, allow_changes, CALL_LIMIT, MAX_BODY)
    }

    /// The same with other limits (tests use shorter ones).
    pub fn with_limits(port: u16, allow_changes: bool, limit: Duration, max_body: usize) -> Frm {
        let client = reqwest::blocking::Client::builder()
            .user_agent(ryna_agent::user_agent())
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .timeout(limit)
            .build()
            // Only fails when TLS can't initialise, and this client speaks plain http alone.
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Frm {
            port,
            allow_changes,
            limit,
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
        if e.is_timeout() {
            Answer::err(
                ErrCode::Timeout,
                format!("FRM didn't answer within {} s", self.limit.as_secs()),
            )
        } else if e.is_connect() {
            Answer::err(
                ErrCode::Unreachable,
                format!("nothing is answering on {}", self.base()),
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
