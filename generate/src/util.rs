#![allow(dead_code)]

use std::cell::Cell;
use std::any::Any;
use std::pin::Pin;


/// append-only structure for doing easy Maybe-borrowed memory
pub(crate) struct MemCtx {
    mem: Cell<Vec<Pin<Box<dyn Managed>>>>
}

trait Managed {}
impl<T> Managed for T {}

impl MemCtx {
    pub fn new() -> Self {
        MemCtx { mem: Cell::new(Vec::new()) }
    }

    pub fn add<T: Any>(&self, item: impl Into<Box<T>>) -> &T {
        let bx = Box::into_pin(item.into());
        let ptr: *const T = &*bx;
        let mut v = self.mem.take();
        v.push(bx);
        self.mem.set(v);
        unsafe { ptr.as_ref().unwrap() }
    }
}

pub(crate) struct OptSetting(Option<Box<str>>);
impl OptSetting {
    pub fn get(&self) -> Option<&str> {
        self.0.as_deref()
    }

    pub const fn new() -> Self {
        OptSetting(None)
    }

    pub const fn is_set(&self) -> bool {
        self.0.is_some()
    }

    pub fn set(&mut self, val: impl Into<Self>) {
        *self = val.into()
    }
}

impl<T: Into<Box<str>>> From<T> for OptSetting {
    fn from(value: T) -> Self {
        OptSetting(Some(value.into()))
    }
}

pub(crate) enum Setting {
    Default(&'static str),
    Override(Box<str>),
}

impl Setting {
    pub fn set(&mut self, val: impl Into<Box<str>>) {
        *self = Setting::Override(val.into());
    }

    pub const fn dft(dft: &'static str) -> Self {
        Setting::Default(dft)
    }
}

impl std::ops::Deref for Setting {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match self {
            Setting::Default(x) => &*x,
            Setting::Override(x) => &*x,
        }
    }
}

impl From<&str> for Setting {
    fn from(value: &str) -> Self {
        Setting::Override(value.into())
    }
}

impl From<Box<str>> for Setting {
    fn from(value: Box<str>) -> Self {
        Setting::Override(value)
    }
}

impl From<String> for Setting {
    fn from(value: String) -> Self {
        Setting::Override(value.into_boxed_str())
    }
}
