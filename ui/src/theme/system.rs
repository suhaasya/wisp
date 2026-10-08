//! OS appearance detection for `ThemeMode::System`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemAppearance {
    Light,
    Dark,
}

pub fn detect_system_appearance() -> SystemAppearance {
    #[cfg(target_os = "macos")]
    {
        macos_appearance().unwrap_or(SystemAppearance::Light)
    }
    #[cfg(windows)]
    {
        windows_appearance().unwrap_or(SystemAppearance::Light)
    }
    #[cfg(target_os = "linux")]
    {
        linux_appearance().unwrap_or(SystemAppearance::Light)
    }
    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    {
        SystemAppearance::Light
    }
}

#[cfg(target_os = "macos")]
fn macos_appearance() -> Option<SystemAppearance> {
    let output = std::process::Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
        .ok()?;
    if !output.status.success() {
        return Some(SystemAppearance::Light);
    }
    let value = String::from_utf8(output.stdout).ok()?;
    if value.trim().eq_ignore_ascii_case("dark") {
        Some(SystemAppearance::Dark)
    } else {
        Some(SystemAppearance::Light)
    }
}

#[cfg(windows)]
fn windows_appearance() -> Option<SystemAppearance> {
    use std::mem::size_of;

    type ShouldAppsUseDarkMode = unsafe extern "system" fn() -> i32;

    unsafe {
        let module = load_library("uxtheme.dll")?;
        let should: ShouldAppsUseDarkMode =
            std::mem::transmute(get_proc(module, "ShouldAppsUseDarkMode")?);
        Ok(if should() != 0 {
            SystemAppearance::Dark
        } else {
            SystemAppearance::Light
        })
    }
}

#[cfg(windows)]
unsafe fn load_library(name: &str) -> Option<*mut std::ffi::c_void> {
    extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut std::ffi::c_void;
    }
    let lib = LoadLibraryA(format!("{name}\0").as_ptr());
    if lib.is_null() {
        None
    } else {
        Some(lib)
    }
}

#[cfg(windows)]
unsafe fn get_proc(module: *mut std::ffi::c_void, name: &str) -> Option<*mut std::ffi::c_void> {
    extern "system" {
        fn GetProcAddress(module: *mut std::ffi::c_void, name: *const u8) -> *mut std::ffi::c_void;
    }
    let proc = GetProcAddress(module, format!("{name}\0").as_ptr());
    if proc.is_null() {
        None
    } else {
        Some(proc)
    }
}

#[cfg(target_os = "linux")]
fn linux_appearance() -> Option<SystemAppearance> {
    let output = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
        .ok()?;
    if output.status.success() {
        let value = String::from_utf8(output.stdout).ok()?;
        if value.contains("dark") {
            return Some(SystemAppearance::Dark);
        }
        if value.contains("light") {
            return Some(SystemAppearance::Light);
        }
    }
    let output = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let theme = String::from_utf8(output.stdout).ok()?;
    if theme.to_ascii_lowercase().contains("dark") {
        Some(SystemAppearance::Dark)
    } else {
        Some(SystemAppearance::Light)
    }
}
