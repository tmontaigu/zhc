use std::ffi::{CStr, c_char, c_void};
use std::sync::OnceLock;

const SIGNPOST_EVENT: u8 = 0x00;
const SIGNPOST_INTERVAL_BEGIN: u8 = 0x01;
const SIGNPOST_INTERVAL_END: u8 = 0x02;
// OS_SIGNPOST_ID_EXCLUSIVE — fine for point events; NOT for overlapping intervals.
const ID_EXCLUSIVE: u64 = 0xEEEE_B0B5_B2B2_EEEE;

unsafe extern "C" {
    static __dso_handle: c_void;
    fn os_log_create(subsystem: *const c_char, category: *const c_char) -> *mut c_void;
    fn os_signpost_enabled(log: *mut c_void) -> bool;
    fn _os_signpost_emit_with_name_impl(
        dso: *const c_void,
        log: *mut c_void,
        ty: u8,
        spid: u64,
        name: *const c_char,
        fmt: *const c_char,
        buf: *mut u8,
        len: u32,
    );
}

fn log_handle() -> *mut c_void {
    static HANDLE: OnceLock<usize> = OnceLock::new();
    *HANDLE.get_or_init(|| unsafe {
        os_log_create(c"ai.zama.zhc".as_ptr(), c"PointsOfInterest".as_ptr()) as usize
    }) as *mut c_void
}

// The arg buffer must carry a 2-byte preamble (summary + arg-count, both zero)
// even with no format args. Passing null makes the parser spin forever.
fn emit(ty: u8, spid: u64, name: &CStr) {
    let log = log_handle();
    unsafe {
        if !os_signpost_enabled(log) {
            return;
        }
        let mut buf = [0u8; 2];
        _os_signpost_emit_with_name_impl(
            &raw const __dso_handle,
            log,
            ty,
            spid,
            name.as_ptr(),
            c"".as_ptr(),
            buf.as_mut_ptr(),
            buf.len() as u32,
        );
    }
}

#[inline]
pub fn event(name: &CStr) {
    emit(SIGNPOST_EVENT, ID_EXCLUSIVE, name);
}

/// `thread_id` must be unique among intervals of the same `name` that can be
/// live at the same time — Instruments pairs begin/end by `(name, thread_id)`,
/// so reusing a value across concurrent same-named intervals mis-nests them.
#[inline]
pub fn interval_begin(name: &CStr, thread_id: u64) {
    emit(SIGNPOST_INTERVAL_BEGIN, thread_id, name);
}

/// End the interval opened by [`interval_begin`] with the same `name`/`thread_id`.
#[inline]
pub fn interval_end(name: &CStr, thread_id: u64) {
    emit(SIGNPOST_INTERVAL_END, thread_id, name);
}
