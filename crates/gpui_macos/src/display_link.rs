//! Per-window display synchronisation via CADisplayLink (macOS 14+).
//!
//! Replaced the legacy CVDisplayLink-based implementation in mid-2026:
//! starting on macOS 26, CVDisplayLink fires once or twice and then ceases,
//! leaving windows frozen on their last rendered frame. CADisplayLink
//! attached to the owning NSView is the supported replacement.
use anyhow::Result;
use cocoa::base::{id, nil};
use ctor::ctor;
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Class, Object, Sel, NO, YES},
    sel, sel_impl,
};
use std::ffi::c_void;
use std::ptr;

const TARGET_DATA_IVAR: &str = "displayLinkData";
const TARGET_CALLBACK_IVAR: &str = "displayLinkCallback";

static mut DISPLAY_LINK_TARGET_CLASS: *const Class = ptr::null();

#[ctor]
unsafe fn build_display_link_target_class() {
    unsafe {
        let mut decl =
            ClassDecl::new("GPUIDisplayLinkTarget", class!(NSObject)).unwrap();
        decl.add_ivar::<*mut c_void>(TARGET_DATA_IVAR);
        decl.add_ivar::<*mut c_void>(TARGET_CALLBACK_IVAR);
        decl.add_method(
            sel!(displayLinkFired:),
            display_link_fired as extern "C" fn(&Object, Sel, id),
        );
        DISPLAY_LINK_TARGET_CLASS = decl.register();
    }
}

extern "C" fn display_link_fired(this: &Object, _: Sel, _link: id) {
    unsafe {
        let data: *mut c_void = *this.get_ivar(TARGET_DATA_IVAR);
        let callback_raw: *mut c_void = *this.get_ivar(TARGET_CALLBACK_IVAR);
        if callback_raw.is_null() {
            return;
        }
        let callback: extern "C" fn(*mut c_void) = std::mem::transmute(callback_raw);
        callback(data);
    }
}

pub struct DisplayLink {
    link: id,   // CADisplayLink, retained
    target: id, // GPUIDisplayLinkTarget, retained
}

impl DisplayLink {
    /// `view` must be a non-null NSView pointer (macOS 14+ provides
    /// `-[NSView displayLinkWithTarget:selector:]`, which schedules the
    /// link on `NSRunLoopCommonModes` of the main run loop automatically).
    pub fn new(
        view: *mut c_void,
        data: *mut c_void,
        callback: extern "C" fn(*mut c_void),
    ) -> Result<DisplayLink> {
        anyhow::ensure!(!view.is_null(), "view pointer is null");
        unsafe {
            let target: id = msg_send![DISPLAY_LINK_TARGET_CLASS, alloc];
            let target: id = msg_send![target, init];
            anyhow::ensure!(!target.is_null(), "could not allocate display-link target");
            (*target).set_ivar(TARGET_DATA_IVAR, data);
            (*target).set_ivar(TARGET_CALLBACK_IVAR, callback as *mut c_void);

            let view = view as id;
            let link: id = msg_send![
                view,
                displayLinkWithTarget: target
                selector: sel!(displayLinkFired:)
            ];
            if link.is_null() {
                let _: () = msg_send![target, release];
                anyhow::bail!("NSView returned null display link (requires macOS 14+)");
            }
            let link: id = msg_send![link, retain];

            // Paused until start() — matches CVDisplayLink semantics.
            let _: () = msg_send![link, setPaused: YES];

            Ok(DisplayLink { link, target })
        }
    }

    pub fn start(&mut self) -> Result<()> {
        unsafe {
            let _: () = msg_send![self.link, setPaused: NO];
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn stop(&mut self) -> Result<()> {
        unsafe {
            let _: () = msg_send![self.link, setPaused: YES];
        }
        Ok(())
    }
}

impl Drop for DisplayLink {
    fn drop(&mut self) {
        unsafe {
            if self.link != nil {
                let _: () = msg_send![self.link, invalidate];
                let _: () = msg_send![self.link, release];
            }
            if self.target != nil {
                let _: () = msg_send![self.target, release];
            }
        }
    }
}
