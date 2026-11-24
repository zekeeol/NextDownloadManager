//! Runtime Dock icon for native-host and `cargo run` launches as well as app bundles.
#[cfg(target_os = "macos")]
pub fn install() {
    use objc2::{AllocAnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    let main = MainThreadMarker::new().expect("Install the application icon on the main thread");
    let data = NSData::with_bytes(include_bytes!("../assets/app-icon.png"));
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        // SAFETY: AppKit is used on the main thread and is always given a non-null image.
        #[allow(unsafe_code)]
        unsafe {
            NSApplication::sharedApplication(main).setApplicationIconImage(Some(&image))
        };
    }
}
#[cfg(not(target_os = "macos"))]
pub fn install() {}
