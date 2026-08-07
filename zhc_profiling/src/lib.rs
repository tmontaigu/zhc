#[cfg(all(feature = "backend_skip", feature = "backend_tracy"))]
compile_error!("features `backend_skip` and `backend_tracy` are mutually exclusive");

#[cfg(all(feature = "backend_skip", feature = "backend_xctrace"))]
compile_error!("features `backend_skip` and `backend_xctrace` are mutually exclusive");

#[cfg(all(feature = "backend_tracy", feature = "backend_xctrace"))]
compile_error!("features `backend_tracy` and `backend_xctrace` are mutually exclusive");

#[cfg(not(any(
    feature = "backend_skip",
    feature = "backend_tracy",
    feature = "backend_xctrace"
)))]
compile_error!("one of `backend_skip`, `backend_tracy`, or `backend_xctrace` must be active");

#[cfg(all(feature = "backend_xctrace", not(target_os = "macos")))]
compile_error!("feature `backend_xctrace` is only available on macOS");

#[cfg(feature = "backend_tracy")]
use backend_tracy as imp;
#[cfg(feature = "backend_tracy")]
mod backend_tracy;

#[cfg(feature = "backend_xctrace")]
use backend_xctrace as imp;
#[cfg(feature = "backend_xctrace")]
mod backend_xctrace;

#[cfg(feature = "backend_skip")]
use backend_skip as imp;
#[cfg(feature = "backend_skip")]
mod backend_skip;

pub use imp::{event, interval_begin, interval_end};
