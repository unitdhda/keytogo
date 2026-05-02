use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use std::os::raw::c_void;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
}

/// Returns true if the process already has Accessibility permission.
/// When `prompt` is true, macOS opens System Settings automatically on first call.
pub fn is_trusted(prompt: bool) -> bool {
    let key = CFString::new("AXTrustedCheckOptionPrompt");
    let val = if prompt {
        CFBoolean::true_value()
    } else {
        CFBoolean::false_value()
    };
    let pairs = [(key.as_CFType(), val.as_CFType())];
    let opts = CFDictionary::from_CFType_pairs(&pairs);
    unsafe { AXIsProcessTrustedWithOptions(opts.as_concrete_TypeRef() as *const c_void) }
}
