use std::cell::RefCell;
use std::ffi::CStr;
use std::panic::Location;
use std::sync::OnceLock;
use tracy_client::{Client, Span};

fn client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(Client::start)
}

thread_local! {
    static OPEN: RefCell<Vec<Span>> = const { RefCell::new(Vec::new()) };
}

#[inline]
pub fn event(name: &CStr) {
    client().message(name.to_str().unwrap(), 0);
}

#[inline]
#[track_caller]
pub fn interval_begin(name: &CStr, _thread_id: u64) {
    let loc = Location::caller();
    let span =
        client()
            .clone()
            .span_alloc(Some(name.to_str().unwrap()), "", loc.file(), loc.line(), 0);
    OPEN.with(|s| s.borrow_mut().push(span));
}

#[inline]
pub fn interval_end(_name: &CStr, _thread_id: u64) {
    OPEN.with(|s| {
        s.borrow_mut().pop();
    });
}
