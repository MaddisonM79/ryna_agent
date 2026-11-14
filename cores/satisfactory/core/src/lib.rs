//! The Satisfactory core: a relay between ryna.io and the Ficsit Remote Monitoring mod on the PC
//! that hosts the factory (docs/ryna-agent-cores-mods.md 14). Builtin, like the WoW core: no
//! sidecar, no package. It holds one outbound WebSocket to the API and answers each request the
//! API sends down it by calling FRM on `127.0.0.1`; nothing listens for the internet.
//!
//! - [`satisfactory_core`]: the core behind the agent's interface, its manifest and status rows.
//! - [`relay`]: the WebSocket, its frames, keepalive, reconnecting and the request pool.
//! - [`frm`]: the localhost call and every limit on it (the security boundary).

pub mod frm;
pub mod relay;
pub mod satisfactory_core;
