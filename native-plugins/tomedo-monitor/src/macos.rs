//! macOS-specific window inspection via the Accessibility API.
//!
//! Uses `AXUIElement` to read the focused window title of the Tomedo process.
//! Falls back to checking all running applications for one whose bundle ID or
//! process name matches "tomedo".

use std::ffi::CStr;

use core_foundation::base::{CFRelease, CFType, TCFType, ToVoid};
use core_foundation::string::{CFString, CFStringRef};
use core_foundation_sys::base::{CFIndex, CFTypeRef};
use core_foundation_sys::string::CFStringGetCStringPtr;
use log::{debug, trace, warn};
use objc::runtime::{Class, Object};
use objc::{msg_send, sel, sel_impl};

pub struct WindowInfo {
    pub title: String,
    pub pid: i32,
}

extern "C" {
    fn AXUIElementCreateApplication(pid: libc::pid_t) -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXIsProcessTrusted() -> bool;
}

const K_AX_FOCUSED_WINDOW_ATTRIBUTE: &str = "AXFocusedWindow";
const K_AX_TITLE_ATTRIBUTE: &str = "AXTitle";
const AX_ERROR_SUCCESS: i32 = 0;
const TOMEDO_BUNDLE_IDS: &[&str] = &[
    "de.tomedo.tomedo",
    "com.topaz.tomedo",
    "de.topaz.tomedo",
    "tomedo",
];
const TOMEDO_PROCESS_NAMES: &[&str] = &["tomedo", "Tomedo"];

pub fn get_tomedo_window_info() -> Option<WindowInfo> {
    if !unsafe { AXIsProcessTrusted() } {
        warn!(
            "Accessibility access not granted. Grant access in System Settings → \
             Privacy & Security → Accessibility."
        );
        return None;
    }

    let pid = find_tomedo_pid()?;
    debug!("Found Tomedo process PID: {}", pid);

    let title = get_focused_window_title(pid)?;
    trace!("Tomedo focused window title: {:?}", title);

    Some(WindowInfo { title, pid })
}

fn find_tomedo_pid() -> Option<i32> {
    unsafe {
        let workspace_class = Class::get("NSWorkspace")?;
        let workspace: *mut Object = msg_send![workspace_class, sharedWorkspace];
        let running_apps: *mut Object = msg_send![workspace, runningApplications];
        let count: usize = msg_send![running_apps, count];

        for i in 0..count {
            let app: *mut Object = msg_send![running_apps, objectAtIndex: i];

            let bundle_id_ns: *mut Object = msg_send![app, bundleIdentifier];
            if !bundle_id_ns.is_null() {
                let bundle_id = nsstring_to_string(bundle_id_ns);
                let lower = bundle_id.to_lowercase();
                if TOMEDO_BUNDLE_IDS.iter().any(|b| lower.contains(b)) {
                    let pid: i32 = msg_send![app, processIdentifier];
                    return Some(pid);
                }
            }

            let name_ns: *mut Object = msg_send![app, localizedName];
            if !name_ns.is_null() {
                let name = nsstring_to_string(name_ns);
                if TOMEDO_PROCESS_NAMES
                    .iter()
                    .any(|n| name.eq_ignore_ascii_case(n))
                {
                    let pid: i32 = msg_send![app, processIdentifier];
                    return Some(pid);
                }
            }
        }
    }

    None
}

fn get_focused_window_title(pid: i32) -> Option<String> {
    unsafe {
        let app_element = AXUIElementCreateApplication(pid as libc::pid_t);
        if app_element.is_null() {
            return None;
        }

        let attr_focused = CFString::new(K_AX_FOCUSED_WINDOW_ATTRIBUTE);
        let mut focused_window: CFTypeRef = std::ptr::null();
        let err =
            AXUIElementCopyAttributeValue(app_element, attr_focused.as_concrete_TypeRef(), &mut focused_window);
        CFRelease(app_element);

        if err != AX_ERROR_SUCCESS || focused_window.is_null() {
            return None;
        }

        let attr_title = CFString::new(K_AX_TITLE_ATTRIBUTE);
        let mut title_ref: CFTypeRef = std::ptr::null();
        let err = AXUIElementCopyAttributeValue(focused_window, attr_title.as_concrete_TypeRef(), &mut title_ref);
        CFRelease(focused_window);

        if err != AX_ERROR_SUCCESS || title_ref.is_null() {
            return None;
        }

        let cf_str = title_ref as CFStringRef;
        let title = cfstring_to_string(cf_str);
        CFRelease(title_ref);
        title
    }
}

unsafe fn nsstring_to_string(ns: *mut Object) -> String {
    let cstr: *const libc::c_char = msg_send![ns, UTF8String];
    if cstr.is_null() {
        return String::new();
    }
    CStr::from_ptr(cstr)
        .to_string_lossy()
        .into_owned()
}

unsafe fn cfstring_to_string(cf: CFStringRef) -> Option<String> {
    let ptr = CFStringGetCStringPtr(cf, core_foundation_sys::string::kCFStringEncodingUTF8);
    if !ptr.is_null() {
        return Some(CStr::from_ptr(ptr).to_string_lossy().into_owned());
    }
    let len: CFIndex = core_foundation_sys::string::CFStringGetLength(cf);
    if len == 0 {
        return Some(String::new());
    }
    let mut buf = vec![0u8; (len as usize) * 4 + 1];
    let ok = core_foundation_sys::string::CFStringGetCString(
        cf,
        buf.as_mut_ptr() as *mut libc::c_char,
        buf.len() as CFIndex,
        core_foundation_sys::string::kCFStringEncodingUTF8,
    );
    if ok == 0 {
        return None;
    }
    let cstr = CStr::from_ptr(buf.as_ptr() as *const libc::c_char);
    Some(cstr.to_string_lossy().into_owned())
}
