//! Runs mxm-mono-01 with its real editor, outside a host.
//!
//! # Why this exists
//!
//! The editor is the plugin's, and a plugin's editor needs a host to open it. The player cannot:
//! it loads the `.clap` with `dlopen` and enables no `gui` extension, and closing that gap means
//! embedding a child window — `HWND`, `NSView`, X11/Wayland — which is deliberately deferred.
//!
//! nice-plug's standalone wrapper is the way through. It supplies audio through cpal, MIDI through
//! midir, and opens `Plugin::editor` in a window it owns, **on all three platforms, with no
//! windowing code written here.** That is what makes it the right answer rather than a stopgap: it
//! is not the platform work that was deferred, because there is no platform work to do.
//!
//! # What it establishes, and what it does not
//!
//! It runs the real editor against the real DSP with a live audio thread, so it validates layout,
//! coverage, reflow, contrast, the view switch, telemetry and allocation-free processing.
//!
//! It does **not** validate the CLAP path. The standalone wrapper creates and owns its window;
//! under CLAP the editor is handed a parent and is resized and scaled *by the host*. Parenting,
//! host-driven resize, scale changes and open/close ordering do not run here, and they are where
//! embedded editors typically break. `docs/briefs/mxm-mono-01.md`'s M4b sign-off records that gate as
//! **unmet** until the editor runs in a real host.
//!
//! # Usage
//!
//! ```text
//! cargo run -p mxm-mono-01-standalone -- --help
//! ```
//!
//! Defaults to the system's default audio and MIDI devices. On Linux this links against libjack,
//! so a build needs `libjack-jackd2-dev`.

use nice_plug::prelude::*;

use mxm_mono_01::MxmMono01;

fn main() {
    // Returns false if the plugin fails to initialise or errors while processing. Propagating that
    // as an exit code is what lets CI run this as a check rather than only as a build.
    if !nice_export_standalone::<MxmMono01>() {
        std::process::exit(1);
    }
}
