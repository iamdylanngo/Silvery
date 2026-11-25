//! Microphone list and macOS microphone permission.

use cpal::traits::{DeviceTrait, HostTrait};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Permission {
    Allowed,
    NotAsked,
    Denied,
    Unknown,
}

#[cfg(target_os = "macos")]
pub fn permission() -> Permission {
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
    unsafe {
        let Some(media) = AVMediaTypeAudio else { return Permission::Unknown };
        let s = AVCaptureDevice::authorizationStatusForMediaType(media);
        if s == AVAuthorizationStatus::Authorized {
            Permission::Allowed
        } else if s == AVAuthorizationStatus::NotDetermined {
            Permission::NotAsked
        } else if s == AVAuthorizationStatus::Denied || s == AVAuthorizationStatus::Restricted {
            Permission::Denied
        } else {
            Permission::Unknown
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn permission() -> Permission {
    Permission::Unknown
}

/// Ask macOS for microphone access (shows the system prompt the first time).
#[cfg(target_os = "macos")]
pub fn request(done: impl Fn(bool) + Send + Sync + 'static) {
    use objc2::runtime::Bool;
    use objc2_av_foundation::{AVCaptureDevice, AVMediaTypeAudio};
    unsafe {
        let Some(media) = AVMediaTypeAudio else { return };
        let block = block2::RcBlock::new(move |granted: Bool| done(granted.as_bool()));
        AVCaptureDevice::requestAccessForMediaType_completionHandler(media, &block);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn request(done: impl Fn(bool) + Send + Sync + 'static) {
    done(true);
}

/// True when running as Silvery.app (macOS then asks the person for microphone access).
/// A bare program started by another app can't get the microphone: macOS hands it silence.
pub fn in_app_bundle() -> bool {
    if !cfg!(target_os = "macos") {
        return true;
    }
    std::env::current_exe()
        .map(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
        .unwrap_or(false)
}

/// Where the stand-alone app is installed.
pub fn bundle_path() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("SILVERY_APP_DIR") {
        return std::path::PathBuf::from(dir).join(format!("{}.app", crate::APP_NAME));
    }
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("Applications")
        .join(format!("{}.app", crate::APP_NAME))
}

const INFO_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Silvery</string>
    <key>CFBundleDisplayName</key><string>Silvery</string>
    <key>CFBundleIdentifier</key><string>app.silvery.meetingnotes</string>
    <key>CFBundleExecutable</key><string>silvery</string>
    <key>CFBundleIconFile</key><string>Silvery</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>0.1.0</string>
    <key>CFBundleVersion</key><string>1</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSMicrophoneUsageDescription</key><string>Silvery records your meetings so it can turn speech into notes on this Mac.</string>
</dict>
</plist>
"#;

/// Copy this program into ~/Applications/Silvery.app with the microphone description and sign it.
pub fn install_bundle() -> Result<std::path::PathBuf, String> {
    let app = bundle_path();
    let macos = app.join("Contents").join("MacOS");
    std::fs::create_dir_all(&macos).map_err(|e| format!("Couldn't create the app: {e}"))?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dest = macos.join("silvery");
    let tmp = macos.join("silvery.new");
    std::fs::copy(&exe, &tmp).map_err(|e| format!("Couldn't copy the app: {e}"))?;
    std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    std::fs::write(app.join("Contents").join("Info.plist"), INFO_PLIST).map_err(|e| e.to_string())?;
    // The app icon (the Silvery logo)
    let res = app.join("Contents").join("Resources");
    std::fs::create_dir_all(&res).map_err(|e| e.to_string())?;
    std::fs::write(res.join("Silvery.icns"), include_bytes!("../assets/Silvery.icns")).map_err(|e| e.to_string())?;
    let _ = std::process::Command::new("codesign")
        .args(["--force", "--deep", "-s", "-"])
        .arg(&app)
        .output();
    Ok(app)
}

/// Install (or update) Silvery.app and open it.
pub fn open_as_app() -> Result<(), String> {
    let app = install_bundle()?;
    std::process::Command::new("open")
        .arg(&app)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Couldn't open the app: {e}"))
}

fn device_name(d: &cpal::Device) -> Option<String> {
    d.description().ok().map(|x| x.name().to_string())
}

/// Names of all microphones / audio inputs.
pub fn input_names() -> Vec<String> {
    let host = cpal::default_host();
    let mut v: Vec<String> = host
        .input_devices()
        .map(|it| it.filter_map(|d| device_name(&d)).collect())
        .unwrap_or_default();
    v.dedup();
    v
}

pub fn default_name() -> Option<String> {
    cpal::default_host().default_input_device().and_then(|d| device_name(&d))
}

/// The chosen input, falling back to the system default.
pub fn find(name: &str) -> Option<(cpal::Device, String)> {
    let host = cpal::default_host();
    if !name.is_empty() {
        if let Ok(it) = host.input_devices() {
            for d in it {
                if device_name(&d).as_deref() == Some(name) {
                    return Some((d, name.to_string()));
                }
            }
        }
    }
    let d = host.default_input_device()?;
    let n = device_name(&d).unwrap_or_else(|| "Default microphone".into());
    Some((d, n))
}
