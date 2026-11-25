//! The small differences between macOS, Windows and Linux.

use std::path::Path;
use std::process::{Command, Stdio};

/// How the app talks about the computer it runs on.
#[cfg(target_os = "macos")]
pub const THIS: &str = "this Mac";
#[cfg(target_os = "windows")]
pub const THIS: &str = "this PC";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const THIS: &str = "this computer";

/// The operating system's name, for messages.
#[cfg(target_os = "macos")]
pub const OS: &str = "macOS";
#[cfg(target_os = "windows")]
pub const OS: &str = "Windows";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const OS: &str = "Linux";

/// Where the microphone switch lives.
#[cfg(target_os = "macos")]
pub const MIC_SETTINGS: &str = "System Settings › Privacy & Security › Microphone";
#[cfg(target_os = "windows")]
pub const MIC_SETTINGS: &str =
    "Settings › Privacy & security › Microphone (turn on “Let desktop apps access your microphone”)";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const MIC_SETTINGS: &str = "your Sound settings (check the input isn't muted and its volume is up)";

/// Whether there's a button that can open the microphone settings.
pub const CAN_OPEN_MIC_SETTINGS: bool = cfg!(any(target_os = "macos", target_os = "windows"));

/// Open a folder in Finder / File Explorer / the file manager.
pub fn open_path(path: &Path) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let _ = Command::new("explorer").arg(path).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = Command::new("xdg-open").arg(path).spawn();
}

/// Open the system's microphone privacy settings.
pub fn open_mic_settings() {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
        .spawn();
    #[cfg(target_os = "windows")]
    let _ = Command::new("explorer").arg("ms-settings:privacy-microphone").spawn();
}

/// Find a program on the PATH.
fn which(name: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(name)).find(|p| p.is_file())
}

fn serve(exe: &Path) -> bool {
    let mut c = Command::new(exe);
    c.arg("serve").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c.spawn().is_ok()
}

/// Start Ollama in the background. Returns true if something was started.
pub fn launch_ollama() -> bool {
    #[cfg(target_os = "macos")]
    {
        if Path::new("/Applications/Ollama.app").exists() {
            return Command::new("open").args(["-g", "-a", "Ollama"]).spawn().is_ok();
        }
        for p in ["/usr/local/bin/ollama", "/opt/homebrew/bin/ollama"] {
            if Path::new(p).exists() {
                return serve(Path::new(p));
            }
        }
        which("ollama").map(|p| serve(&p)).unwrap_or(false)
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(local) = dirs::data_local_dir() {
            let app = local.join("Programs").join("Ollama").join("ollama app.exe");
            if app.exists() {
                return Command::new(app).spawn().is_ok();
            }
            let exe = local.join("Programs").join("Ollama").join("ollama.exe");
            if exe.exists() {
                return serve(&exe);
            }
        }
        which("ollama.exe").map(|p| serve(&p)).unwrap_or(false)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        for p in ["/usr/local/bin/ollama", "/usr/bin/ollama"] {
            if Path::new(p).exists() {
                return serve(Path::new(p));
            }
        }
        which("ollama").map(|p| serve(&p)).unwrap_or(false)
    }
}
