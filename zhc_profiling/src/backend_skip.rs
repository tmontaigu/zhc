use std::ffi::CStr;

#[inline]
pub fn event(_name: &CStr) {}
#[inline]
pub fn interval_begin(_name: &CStr, _thread_id: u64) {}
#[inline]
pub fn interval_end(_name: &CStr, _thread_id: u64) {}
