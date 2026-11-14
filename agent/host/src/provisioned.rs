//! The cores a person is provisioned for (docs/ryna-agent-cores-mods.md 13.1): the API's cores
//! answer, `GET {api}/v1/portal/agent/cores` with the agent's access token, and the last one kept
//! in the state folder.
//!
//! The answer names each core the person has (`{ id, name }`); the roster is the gate, so the
//! token this agent already holds is admitted for every one of them, and nothing about the
//! sign-in changes when a core is added. Fields an older answer carried (`scope`, `allowed`,
//! `signIn`) are ignored. A core the answer leaves out doesn't exist in the window. The rules for
//! what's shown:
//! - An answer (`ok: true`) replaces what was kept, so a core is removed only by an answer that
//!   leaves it out.
//! - No answer (offline, the API failing, an `ok: false`, a route that isn't there yet) keeps the
//!   last one, so a blip never hides a running core.
//! - With nothing kept and no answer: the WoW core alone, as before stage 4.
//! - A 401 or 403 on this call is the sign-in's problem, said as [`Asked::SignInAgain`] (the
//!   kept answer still stands).

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::http::{Method, Request, Transport};

/// Where the API answers.
pub const CORES_PATH: &str = "/v1/portal/agent/cores";
/// The kept answer's file in the state folder.
pub const CACHE_FILE: &str = "cores.json";
/// The WoW core: everyone's (its role is flagged `everyone`), shown alone when nothing is known.
pub const WOW_CORE_ID: &str = "wow";

/// One core the person is provisioned for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionedCore {
    pub id: String,
    pub name: String,
}

/// The cores answer, as kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cores {
    pub cores: Vec<ProvisionedCore>,
    /// When it was answered (unix seconds); 0 for the built-in WoW-alone.
    #[serde(default)]
    pub at: i64,
}

impl Cores {
    /// The WoW core alone: what's shown before any answer has been had.
    pub fn wow_alone() -> Cores {
        Cores {
            cores: vec![ProvisionedCore {
                id: WOW_CORE_ID.into(),
                name: "World of Warcraft: Forever".into(),
            }],
            at: 0,
        }
    }

    pub fn get(&self, id: &str) -> Option<&ProvisionedCore> {
        self.cores.iter().find(|c| c.id == id)
    }
}

/// What asking the API came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    Answer(Cores),
    /// 401 or 403: the sign-in needs doing again. The words are the API's, when it gave some.
    SignInAgain(String),
    /// Anything else: offline, a 5xx, an `ok: false`, a body that isn't the answer.
    NoAnswer(String),
}

#[derive(Deserialize)]
struct Envelope {
    ok: bool,
    data: Option<Cores>,
    error: Option<EnvelopeError>,
}

#[derive(Deserialize)]
struct EnvelopeError {
    code: Option<String>,
    message: Option<String>,
}

/// Ask the API which cores the person has, with the agent's access token.
pub fn ask(config: &Config, http: &dyn Transport, token: &str, now: i64) -> Asked {
    let res = match http.post(&Request {
        method: Method::Get,
        url: format!("{}{CORES_PATH}", config.api),
        content_type: "application/json",
        content_encoding: None,
        body: Vec::new(),
        bearer: Some(token.to_string()),
        headers: Vec::new(),
        timeout: Duration::from_secs(20),
    }) {
        Ok(res) => res,
        Err(e) => return Asked::NoAnswer(format!("couldn't reach the API: {e}")),
    };
    let envelope: Option<Envelope> = serde_json::from_slice(&res.body).ok();
    let words = |e: &Option<Envelope>| {
        e.as_ref()
            .and_then(|e| e.error.as_ref())
            .map(|e| {
                e.message
                    .clone()
                    .or_else(|| e.code.clone())
                    .unwrap_or_default()
            })
            .filter(|w| !w.is_empty())
            .unwrap_or_else(|| format!("HTTP {}", res.status))
    };
    if res.status == 401 || res.status == 403 {
        return Asked::SignInAgain(words(&envelope));
    }
    match envelope {
        Some(Envelope {
            ok: true,
            data: Some(mut cores),
            ..
        }) if (200..300).contains(&res.status) => {
            cores.at = now;
            Asked::Answer(cores)
        }
        other => Asked::NoAnswer(format!("the cores answer: {}", words(&other))),
    }
}

/// The kept answer, if there is one that reads.
pub fn load(state_dir: &Path) -> Option<Cores> {
    let text = std::fs::read(state_dir.join(CACHE_FILE)).ok()?;
    serde_json::from_slice(&text).ok()
}

/// Keep an answer (through a temporary file and a rename, so a crash leaves the old one whole).
pub fn save(state_dir: &Path, cores: &Cores) -> std::io::Result<()> {
    std::fs::create_dir_all(state_dir)?;
    let tmp = state_dir.join(format!("{CACHE_FILE}.tmp"));
    std::fs::write(&tmp, serde_json::to_vec_pretty(cores)?)?;
    std::fs::rename(tmp, state_dir.join(CACHE_FILE))
}

/// What to show now: the kept answer, else the WoW core alone.
pub fn current(state_dir: Option<&Path>) -> Cores {
    state_dir.and_then(load).unwrap_or_else(Cores::wow_alone)
}

/// Take what `ask` came to: an answer is kept and shown; anything else leaves what was kept.
/// Returns what to show, and the words for a failure (None after an answer).
pub fn update(state_dir: Option<&Path>, asked: Asked) -> (Cores, Option<Asked>) {
    match asked {
        Asked::Answer(cores) => {
            if let Some(dir) = state_dir {
                if let Err(e) = save(dir, &cores) {
                    eprintln!("couldn't keep the cores answer: {e}");
                }
            }
            (cores, None)
        }
        other => (current(state_dir), Some(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::test_config;
    use crate::http::testing::ScriptedTransport;
    use serde_json::json;

    fn answer(cores: serde_json::Value) -> serde_json::Value {
        json!({ "ok": true, "data": { "cores": cores } })
    }

    fn both() -> serde_json::Value {
        json!([
            { "id": "wow", "name": "World of Warcraft: Forever" },
            { "id": "palworld", "name": "Palworld" }
        ])
    }

    #[test]
    fn an_answer_is_read_with_the_bearer_and_kept() {
        let http = ScriptedTransport::new();
        http.json(200, answer(both()));
        let asked = ask(&test_config(), &http, "at-1", 42);
        let req = http.requests.borrow()[0].clone();
        assert_eq!(req.url, "https://api.test/v1/portal/agent/cores");
        assert_eq!(req.method, Method::Get);
        assert_eq!(req.bearer.as_deref(), Some("at-1"));
        let Asked::Answer(cores) = asked else {
            panic!("{asked:?}")
        };
        assert_eq!(cores.at, 42);
        assert_eq!(
            cores.cores,
            [
                ProvisionedCore {
                    id: "wow".into(),
                    name: "World of Warcraft: Forever".into()
                },
                ProvisionedCore {
                    id: "palworld".into(),
                    name: "Palworld".into()
                }
            ]
        );
    }

    #[test]
    fn an_older_answers_extra_fields_are_ignored() {
        // Around a deploy the API may still send what stage 4's first answer carried.
        let http = ScriptedTransport::new();
        http.json(
            200,
            json!({ "ok": true, "data": {
                "cores": [{ "id": "palworld", "name": "Palworld", "scope": "palworld:ingest", "allowed": false }],
                "signIn": { "resource": "https://api.ryna.io/agent", "scopes": ["palworld:ingest"] } } }),
        );
        let Asked::Answer(cores) = ask(&test_config(), &http, "t", 1) else {
            panic!()
        };
        assert_eq!(cores.get("palworld").unwrap().name, "Palworld");
        // And a kept file in the older shape still reads.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(CACHE_FILE),
            r#"{ "cores": [{ "id": "wow", "name": "WoW", "scope": "wow:ingest", "allowed": true }], "signIn": null, "at": 5 }"#,
        )
        .unwrap();
        assert_eq!(load(dir.path()).unwrap().cores[0].id, "wow");
    }

    #[test]
    fn a_core_is_removed_only_by_an_answer_that_leaves_it_out() {
        let dir = tempfile::tempdir().unwrap();
        let state = Some(dir.path());
        // Nothing kept, no answer: WoW alone.
        let (shown, _) = update(state, Asked::NoAnswer("offline".into()));
        assert_eq!(shown, Cores::wow_alone());
        // An answer with Palworld is kept.
        let http = ScriptedTransport::new();
        http.json(200, answer(both()));
        let (shown, failed) = update(state, ask(&test_config(), &http, "t", 1));
        assert!(failed.is_none());
        assert_eq!(shown.cores.len(), 2);
        // Offline, a 500, an ok:false, a 404 and a sign-in refusal all keep it.
        let http = ScriptedTransport::new();
        http.fail("dns")
            .json(
                500,
                json!({ "ok": false, "error": { "code": "INTERNAL", "message": "boom" } }),
            )
            .json(
                200,
                json!({ "ok": false, "error": { "code": "X", "message": "no" } }),
            )
            .text(404, "not found")
            .json(
                401,
                json!({ "ok": false, "error": { "code": "UNAUTHORIZED", "message": "token expired" } }),
            );
        for _ in 0..5 {
            let (shown, failed) = update(state, ask(&test_config(), &http, "t", 2));
            assert_eq!(shown.cores.len(), 2);
            assert!(failed.is_some());
        }
        // An answer without Palworld removes it.
        let http = ScriptedTransport::new();
        http.json(200, answer(json!([{ "id": "wow", "name": "WoW" }])));
        let (shown, _) = update(state, ask(&test_config(), &http, "t", 3));
        assert_eq!(shown.cores.len(), 1);
        assert_eq!(current(state).cores.len(), 1);
        // Without a state folder nothing is kept, and WoW alone is shown.
        assert_eq!(current(None), Cores::wow_alone());
    }

    #[test]
    fn a_refusal_of_the_token_says_sign_in_again() {
        for status in [401, 403] {
            let http = ScriptedTransport::new();
            http.json(
                status,
                json!({ "ok": false, "error": { "code": "FORBIDDEN", "message": "not your token" } }),
            );
            assert_eq!(
                ask(&test_config(), &http, "t", 1),
                Asked::SignInAgain("not your token".into())
            );
        }
    }
}
