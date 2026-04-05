use tauri::{
    Manager,
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
    menu::{Menu, MenuItem},
};
use std::process::Command;
use std::os::windows::process::CommandExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_updater::UpdaterExt;

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use winapi::um::shellapi::{SHGetFileInfoW, SHGFI_ICON, SHGFI_LARGEICON, SHFILEINFOW};
use winapi::um::winuser::{DestroyIcon, GetIconInfo, ICONINFO};
use winapi::um::wingdi::{GetDIBits, GetObjectW, BITMAP, BITMAPINFOHEADER, DIB_RGB_COLORS, CreateCompatibleDC, DeleteDC, DeleteObject};
use base64::{Engine as _, engine::general_purpose};
use std::sync::Mutex;
use tauri::State;

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
    let path_wide: Vec<u16> = OsStr::new(&path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    
    let mut shfi: SHFILEINFOW = unsafe { std::mem::zeroed() };

    let h_success = unsafe {
        SHGetFileInfoW(
            path_wide.as_ptr(),
            0,
            &mut shfi,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    };

    if h_success == 0 || shfi.hIcon.is_null() {
        return Err("The icon could not be found".to_string());
    }

    let h_icon = shfi.hIcon;

    let (width, height, rgba_buf) = unsafe {
        let mut icon_info: ICONINFO = std::mem::zeroed();
        if GetIconInfo(h_icon, &mut icon_info) == 0 {
            DestroyIcon(h_icon);
            return Err("GetIconInfo failed".to_string());
        }

        let h_bitmap = icon_info.hbmColor;
        let mut bitmap: BITMAP = std::mem::zeroed();
        GetObjectW(
            h_bitmap as *mut _,
            std::mem::size_of::<BITMAP>() as i32,
            &mut bitmap as *mut _ as *mut _,
        );

        let width = bitmap.bmWidth;
        let height = bitmap.bmHeight;

        let mut bi = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };

        let hdc = CreateCompatibleDC(ptr::null_mut());
        let buf_size = (width * height * 4) as usize;
        let mut buf: Vec<u8> = vec![0u8; buf_size];

        GetDIBits(
            hdc,
            h_bitmap,
            0,
            height as u32,
            buf.as_mut_ptr() as *mut _,
            &mut bi as *mut _ as *mut _,
            DIB_RGB_COLORS,
        );

        DeleteDC(hdc);
        DeleteObject(h_bitmap as *mut _);
        DeleteObject(icon_info.hbmMask as *mut _);
        DestroyIcon(h_icon);

        for chunk in buf.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }

        (width as u32, height as u32, buf)
    };

    let img = image::RgbaImage::from_raw(width, height, rgba_buf)
        .ok_or("Failed to create RgbaImage")?;

    let mut png_bytes: Vec<u8> = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut png_bytes),
        image::ImageFormat::Png,
    ).map_err(|e| e.to_string())?;

    let base64_string = general_purpose::STANDARD.encode(&png_bytes);
    Ok(format!("data:image/png;base64,{}", base64_string))
}

#[tauri::command]
fn show_window(app: tauri::AppHandle) {
    let window = app.get_webview_window("main").unwrap();
    window.show().unwrap();
    window.set_focus().unwrap();
}

#[tauri::command]
fn launch_app(path: String) -> Result<(), String> {
    Command::new("cmd")
        .args(["/C", "start", "", &path])
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| e.to_string())?;
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
async fn get_update_info(
    app: tauri::AppHandle,
    state: State<'_, UpdateState>
) -> Result<Option<serde_json::Value>, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    
    eprintln!("Checking for updates...");
    
    match updater.check().await {
        Ok(Some(update)) => {
            eprintln!("Update found: {} -> {}", update.current_version, update.version);
            let info = serde_json::json!({
                "version": update.version,
                "body": update.body.clone().unwrap_or_default(),
            });
            *state.0.lock().unwrap() = Some(update);
            Ok(Some(info))
        }
        Ok(None) => {
            eprintln!("No updates available");
            Ok(None)
        }
        Err(e) => {
            eprintln!("Updater error: {:#?}", e);
            Err(e.to_string())
        }
    }
}

#[tauri::command]
async fn install_update(
    app: tauri::AppHandle,
    state: State<'_, UpdateState>
) -> Result<(), String> {
    eprintln!("=== INSTALL UPDATE CALLED ===");
    
    let update = state.0.lock().unwrap().take()
        .ok_or("No update available")?;
    
    eprintln!("Starting download and install...");
    eprintln!("Update version: {}", update.version);
    
    match update.download_and_install(|_, _| {}, || {}).await {
        Ok(_) => {
            eprintln!("Install successful, restarting...");
            app.restart();
            Ok(())
        }
        Err(e) => {
            eprintln!("Install error: {:#?}", e);
            Err(e.to_string())
        }
    }
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
