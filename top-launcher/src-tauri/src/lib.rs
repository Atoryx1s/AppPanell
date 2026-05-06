use tauri::{
    Manager, tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
    menu::{Menu, MenuItem},
};
use std::process::Command;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_updater::UpdaterExt;

use std::sync::Mutex;
use tauri::State;
use base64::{Engine as _, engine::general_purpose};
use std::path::Path;

#[cfg(target_os = "windows")]
use std::ffi::OsStr;
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
use windows::{
    core::PCWSTR,
    Win32::{
        Graphics::Gdi::*,
        UI::{
            Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON},
            WindowsAndMessaging::*,
        },
        Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES,
    },
};

use tauri_plugin_single_instance::init as single_instance;

#[tauri::command]
fn resize_and_center(app: tauri::AppHandle, width: f64, height: f64) {
    let window = app.get_webview_window("main").unwrap();
    
    window.set_size(tauri::LogicalSize::new(width, height)).unwrap();
    
    if let Ok(Some(monitor)) = window.current_monitor() {
        let screen = monitor.size();
        let win_size = window.outer_size().unwrap();
        let x = ((screen.width - win_size.width) / 2) as i32;
        window.set_position(tauri::PhysicalPosition::new(x, 0)).unwrap();
    }
}

#[tauri::command]
fn get_executable_icon(path: String) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let path_wide: Vec<u16> = OsStr::new(&path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut shfi = SHFILEINFOW::default();

        let result = unsafe {
            SHGetFileInfoW(
                PCWSTR(path_wide.as_ptr()),
                FILE_FLAGS_AND_ATTRIBUTES(0),
                Some(&mut shfi),                                      // ← Головне виправлення
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            )
        };

        if result == 0 || shfi.hIcon.is_invalid() {
            return Err("Could not extract icon".to_string());
        }

        let h_icon = shfi.hIcon;

        let (width, height, rgba_buf) = unsafe {
            let mut icon_info = ICONINFO::default();
            if GetIconInfo(h_icon, &mut icon_info).is_err() {
                let _ = DestroyIcon(h_icon);
                return Err("GetIconInfo failed".to_string());
            }

            let h_bitmap = icon_info.hbmColor;
            let mut bitmap = BITMAP::default();

            let _ = GetObjectW(
                h_bitmap.into(),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bitmap as *mut _ as *mut _),
            );

            let width = bitmap.bmWidth;
            let height = bitmap.bmHeight;

            let bi = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0 as u32,
                ..Default::default()
            };

            let hdc = CreateCompatibleDC(None);
            let buf_size = (width * height * 4) as usize;
            let mut buf = vec![0u8; buf_size];

            let mut bmi = BITMAPINFO {
                bmiHeader: bi,
                bmiColors: [RGBQUAD::default(); 1],
            };

            let _ = GetDIBits(
                hdc,
                h_bitmap,
                0,
                height as u32,
                Some(buf.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            let _ = DeleteDC(hdc);
            let _ = DeleteObject(h_bitmap.into());
            let _ = DeleteObject(icon_info.hbmMask.into());
            let _ = DestroyIcon(h_icon);

            for chunk in buf.chunks_exact_mut(4) {
                chunk.swap(0, 2);
            }

            (width as u32, height as u32, buf)
        };

        let img = image::RgbaImage::from_raw(width, height, rgba_buf)
            .ok_or("Failed to create image from raw data")?;

        let mut png_bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;

        let base64_string = general_purpose::STANDARD.encode(&png_bytes);
        Ok(format!("data:image/png;base64,{}", base64_string))
    }

    #[cfg(target_os = "linux")]
    {
        if path.ends_with(".desktop") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                for line in content.lines() {
                    if let Some(icon_name) = line.strip_prefix("Icon=") {
                        let icon_name = icon_name.trim();

                        let possible_paths = [
                            format!("/usr/share/icons/hicolor/256x256/apps/{}.png", icon_name),
                            format!("/usr/share/icons/hicolor/48x48/apps/{}.png", icon_name),
                            format!("/usr/share/pixmaps/{}.png", icon_name),
                            format!("{}/.local/share/icons/{}.png", std::env::var("HOME").unwrap_or_default(), icon_name),
                        ];

                        for p in possible_paths {
                            if Path::new(&p).exists() {
                                if let Ok(bytes) = std::fs::read(&p) {
                                    let base64 = general_purpose::STANDARD.encode(&bytes);
                                    return Ok(format!("data:image/png;base64,{}", base64));
                                }
                            }
                        }
                    }
                }
            }
        }
        Err("Icon not found on Linux".to_string())
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        Err("Platform not supported".to_string())
    }
}

#[tauri::command]
fn show_window(app: tauri::AppHandle) {
    let window = app.get_webview_window("main").unwrap();
    window.show().unwrap();
    window.set_focus().unwrap();
}

#[tauri::command]
fn launch_app(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", &path])
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        if path.ends_with(".desktop") {
            let _ = Command::new("gtk-launch").arg(&path).spawn();
        } else {
            let _ = Command::new(&path).spawn();
        }
    }

    Ok(())
}

#[tauri::command]
fn hide_window(app: tauri::AppHandle) {
    let window = app.get_webview_window("main").unwrap();
    window.hide().unwrap();
}

#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<bool, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => {
            update.download_and_install(|_, _| {}, || {})
                .await
                .map_err(|e| e.to_string())?;
            Ok(true)
        }
        Ok(None) => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

struct UpdateState(Mutex<Option<tauri_plugin_updater::Update>>);

#[tauri::command]
async fn get_update_info(app: tauri::AppHandle,state: State<'_, UpdateState>) -> Result<Option<serde_json::Value>, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    
    match updater.check().await {
        Ok(Some(update)) => {
            let info = serde_json::json!({
                "version": update.version,
                "body": update.body.clone().unwrap_or_default(),
            });
            *state.0.lock().unwrap() = Some(update);
            Ok(Some(info))
        },
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string())
    }
}

#[tauri::command]
async fn install_update(app: tauri::AppHandle,state: State<'_, UpdateState>) -> Result<(), String> {
    let update = state.0.lock().unwrap().take()
        .ok_or("No update available")?;
    
    update.download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    
    app.restart();  
}

#[tauri::command]
fn copy_shortcut(app: tauri::AppHandle, src_path: String) -> Result<String, String> {
    let app_dir = app.path().app_data_dir()
        .map_err(|e| e.to_string())?;
    
    let shortcuts_dir = app_dir.join("shortcuts");
    std::fs::create_dir_all(&shortcuts_dir)
        .map_err(|e| e.to_string())?;
    
    let file_name = std::path::Path::new(&src_path)
        .file_name()
        .ok_or("Invalid filename")?
        .to_string_lossy()
        .to_string();
    
    let dest_path = shortcuts_dir.join(&file_name);
    std::fs::copy(&src_path, &dest_path)
        .map_err(|e| e.to_string())?;
    
    Ok(dest_path.to_string_lossy().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                window.show().unwrap();
                window.set_focus().unwrap();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(UpdateState(Mutex::new(None)))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![launch_app, get_executable_icon, resize_and_center, show_window, hide_window, check_update, get_update_info, install_update, copy_shortcut])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();

            let quit = MenuItem::with_id(app, "quit", "Exit", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "Show App Panell", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            let tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Top Launcher")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .build(app)?;

            tray.on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event {
                    let app = tray.app_handle();
                    let window = app.get_webview_window("main").unwrap();
                    if window.is_visible().unwrap() {
                        window.hide().unwrap();
                    } else {
                        window.show().unwrap();
                        window.set_focus().unwrap();
                    }
                }
            });

            tray.on_menu_event(|app, event| {
                match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "show" => {
                        let window = app.get_webview_window("main").unwrap();
                        window.show().unwrap();
                        window.set_focus().unwrap();
                    }
                _ => {}
                }
            });

            let ctrl_shift_space = Shortcut::new(
                Some(Modifiers::CONTROL | Modifiers::SHIFT),
                Code::Space,
            );
            let _ = app.global_shortcut().unregister(ctrl_shift_space);
            app.global_shortcut().on_shortcut(ctrl_shift_space, move |app_handle, _shortcut, event| {
                if let ShortcutState::Pressed = event.state() {
                    let window = app_handle.get_webview_window("main").unwrap();
                    if window.is_visible().unwrap() {
                        window.hide().unwrap();
                    } else {
                        window.show().unwrap();
                        window.set_focus().unwrap();
                    }
                }
            }).unwrap_or_else(|e| eprintln!("Hockey: {}", e));

            if let Ok(Some(monitor)) = window.current_monitor() {
                let screen_size = monitor.size();
                let window_size = window.outer_size().unwrap();
                let x = (screen_size.width - window_size.width) / 2;
                window.set_position(tauri::PhysicalPosition::new(x, 0)).unwrap();
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error")
        .run(|_app, event| { 
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
