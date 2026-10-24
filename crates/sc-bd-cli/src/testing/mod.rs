//! Test doubles for [`CliInvoker`](crate::CliInvoker) (feature `test-support`).
//!
//! - [`RecordingInvoker`]: a scripted invoker that spawns nothing and records every
//!   argv (btit's double, ported).
//! - [`FakeBd`]: an invoker that behaves like `bd` for the cache's command forms. It
//!   parses the argv the way `bd` does and applies it to `sc_beads::testing::FakeBackend`,
//!   so a [`BdCli`](crate::BdCli) over it exercises the real argv builders and the real
//!   output parsing without a `bd` binary (NFR-BDC-0005).

mod fake_bd;
mod recording;

#[doc(inline)]
pub use fake_bd::FakeBd;
#[doc(inline)]
pub use recording::RecordingInvoker;
