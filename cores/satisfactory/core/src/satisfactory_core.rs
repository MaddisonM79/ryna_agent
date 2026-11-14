//! The Satisfactory core behind the agent's interface (`ryna_agent::core::Core`), with the
//! manifest `cores/satisfactory/core.json` built in.

use std::sync::{Arc, Mutex, OnceLock};

use ryna_agent::core::{
    Activity, Chip, Core, CoreContext, CoreControl, CoreHeartbeat, CoreManifest, StatusRow,
};
use ryna_agent::fix::Fix;
use serde_json::json;

use crate::frm::Frm;
use crate::relay::{
    relay_url, Clock, Link, RealClock, Relay, RelaySettings, RelayStatus, SatisfactoryEvent, Timing,
};

/// `cores/satisfactory/core.json`, as built into this crate.
pub const MANIFEST_JSON: &str = include_str!("../../core.json");

/// The manifest, parsed once. A test holds `core.json` to what the code does.
pub fn manifest() -> &'static CoreManifest {
    static MANIFEST: OnceLock<CoreManifest> = OnceLock::new();
    MANIFEST
        .get_or_init(|| CoreManifest::parse(MANIFEST_JSON).expect("cores/satisfactory/core.json"))
}

#[derive(Debug, thiserror::Error)]
pub enum SatisfactoryError {
    #[error("{0}")]
    Stopped(String),
}

/// The core, with the person's two settings and where the API is.
pub struct SatisfactoryCore {
    pub api: String,
    pub settings: RelaySettings,
    pub timing: Timing,
    clock: Arc<dyn Clock>,
    status: Arc<Mutex<RelayStatus>>,
    frm: Option<Frm>,
}

impl SatisfactoryCore {
    pub fn new(api: &str, settings: RelaySettings) -> Self {
        SatisfactoryCore {
            api: api.to_string(),
            settings,
            timing: Timing::default(),
            clock: Arc::new(RealClock::default()),
            status: Arc::default(),
            frm: None,
        }
    }

    #[must_use]
    pub fn with_timing(mut self, timing: Timing) -> Self {
        self.timing = timing;
        self
    }

    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// Another FRM client (tests: shorter limits).
    #[must_use]
    pub fn with_frm(mut self, frm: Frm) -> Self {
        self.frm = Some(frm);
        self
    }

    fn frm(&self) -> Frm {
        self.frm
            .clone()
            .unwrap_or_else(|| Frm::new(self.settings.port, self.settings.allow_changes))
    }

    /// The status card's rows now.
    pub fn rows_now(&self) -> Vec<StatusRow> {
        let st = self
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        rows(&st, &self.settings, ryna_agent::util::now_secs())
    }
}

/// "just now", "5 min", "2 h": how long ago `at` was.
fn since(at: i64, now: i64) -> String {
    let d = (now - at).max(0);
    match d {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", d / 60),
        _ => format!("{} h ago", d / 3600),
    }
}

/// The status card: the link, FRM, the requests, whether changes are allowed.
pub fn rows(st: &RelayStatus, settings: &RelaySettings, now: i64) -> Vec<StatusRow> {
    let link = match st.link {
        Link::Off => StatusRow::new("Link to ryna.io", "Off", Some(Chip::Waiting)),
        Link::Reconnecting => {
            StatusRow::new("Link to ryna.io", "Reconnecting…", Some(Chip::Update))
        }
        Link::Connected(at) => StatusRow::new(
            "Link to ryna.io",
            format!("Connected since {}", since(at, now)),
            Some(Chip::Pushed),
        ),
    };
    let factory = match st.frm_answering {
        Some(true) => StatusRow::new(
            "Factory",
            format!("Answering on 127.0.0.1:{}", settings.port),
            Some(Chip::Pushed),
        ),
        Some(false) => StatusRow::new(
            "Factory",
            "Not answering: is the game running with the mod?",
            Some(Chip::Error),
        ),
        None => StatusRow::new("Factory", format!("127.0.0.1:{}", settings.port), None),
    };
    let requests = match st.last_request_at {
        Some(at) => format!("{} this session, last {}", st.requests, since(at, now)),
        None => format!("{} this session", st.requests),
    };
    vec![
        link,
        factory,
        StatusRow::new("Requests", requests, None),
        StatusRow::new(
            "Changes",
            if settings.allow_changes {
                "Allowed"
            } else {
                "Not allowed"
            },
            None,
        ),
    ]
}

impl Core for SatisfactoryCore {
    type Event = SatisfactoryEvent;
    type Error = SatisfactoryError;

    fn manifest(&self) -> &CoreManifest {
        manifest()
    }

    fn start(
        &self,
        ctx: &CoreContext,
        control: &CoreControl,
        events: &mut dyn FnMut(SatisfactoryEvent),
    ) -> Result<(), SatisfactoryError> {
        let url = relay_url(&self.api)
            .ok_or_else(|| SatisfactoryError::Stopped(format!("{} isn't a URL", self.api)))?;
        let relay = Relay {
            url,
            frm: self.frm(),
            timing: self.timing,
            clock: self.clock.clone(),
            status: self.status.clone(),
            tokens: ctx.tokens.as_ref(),
        };
        relay.run(control, events).map_err(|message| {
            events(SatisfactoryEvent::Stopped {
                message: message.clone(),
                fix: None,
            });
            SatisfactoryError::Stopped(message)
        })
    }

    fn activity(&self, event: &SatisfactoryEvent) -> Activity {
        activity(event)
    }

    fn failed(&self, message: String, fix: Fix) -> SatisfactoryEvent {
        SatisfactoryEvent::Failed {
            advice: fix.message(&message),
            message,
            fix,
        }
    }

    fn status_rows(&self, _ctx: &CoreContext) -> Vec<StatusRow> {
        self.rows_now()
    }

    fn heartbeat(&self, _ctx: &CoreContext) -> CoreHeartbeat {
        CoreHeartbeat {
            core: manifest().id.clone(),
            version: manifest().version.clone(),
            fields: json!({}),
        }
    }
}

/// What the agent's state takes from an event: a failure or a stop to show; a connect clears it.
pub fn activity(event: &SatisfactoryEvent) -> Activity {
    match event {
        SatisfactoryEvent::Connected => Activity {
            clears_error: true,
            ..Activity::default()
        },
        SatisfactoryEvent::Failed { advice, .. } => Activity {
            error: Some(advice.clone()),
            ..Activity::default()
        },
        SatisfactoryEvent::Stopped { message, .. } => Activity {
            error: Some(message.clone()),
            ..Activity::default()
        },
        _ => Activity::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ryna_agent::core::Runtime;

    #[test]
    fn the_manifest_says_what_the_code_does() {
        let m = manifest();
        assert_eq!(m.id, "satisfactory");
        assert_eq!(m.name, "Satisfactory");
        assert_eq!(m.runtime, Runtime::Builtin);
        assert_eq!(m.version, ryna_agent::AGENT_VERSION);
        assert_eq!(m.platforms, None, "every platform");
    }

    #[test]
    fn rows_say_where_the_link_and_the_factory_are() {
        let off = rows(&RelayStatus::default(), &RelaySettings::default(), 1000);
        let words: Vec<(&str, &str)> = off
            .iter()
            .map(|r| (r.label.as_str(), r.value.as_str()))
            .collect();
        assert_eq!(
            words,
            [
                ("Link to ryna.io", "Off"),
                ("Factory", "127.0.0.1:8080"),
                ("Requests", "0 this session"),
                ("Changes", "Not allowed")
            ]
        );
        let st = RelayStatus {
            link: Link::Connected(1000 - 300),
            frm_answering: Some(true),
            requests: 12,
            last_request_at: Some(1000 - 20),
        };
        let on = rows(
            &st,
            &RelaySettings {
                port: 9090,
                allow_changes: true,
            },
            1000,
        );
        assert_eq!(on[0].value, "Connected since 5 min ago");
        assert_eq!(on[1].value, "Answering on 127.0.0.1:9090");
        assert_eq!(on[2].value, "12 this session, last just now");
        assert_eq!(on[3].value, "Allowed");
        let down = RelayStatus {
            link: Link::Reconnecting,
            frm_answering: Some(false),
            ..RelayStatus::default()
        };
        let down = rows(&down, &RelaySettings::default(), 1000);
        assert_eq!(down[0].value, "Reconnecting…");
        assert_eq!(
            down[1].value,
            "Not answering: is the game running with the mod?"
        );
        assert_eq!(down[1].chip, Some(Chip::Error));
    }

    #[test]
    fn a_connect_clears_the_error_and_a_stop_shows_one() {
        assert!(activity(&SatisfactoryEvent::Connected).clears_error);
        assert_eq!(
            activity(&SatisfactoryEvent::Stopped {
                message: crate::relay::REPLACED.into(),
                fix: None
            })
            .error
            .as_deref(),
            Some(crate::relay::REPLACED)
        );
        assert_eq!(
            serde_json::to_value(SatisfactoryEvent::FrmAnswering { port: 8080 }).unwrap(),
            json!({ "event": "frmAnswering", "port": 8080 })
        );
    }
}
