//! Desktop wrapper: starts the axum API on an ephemeral loopback port, serves the
//! UI bundled inside this executable, and shows it in a native WebView2 window.
//!
//! `cargo build --release --bin re-search-accessory-app` produces an .exe that can
//! be double-clicked (no terminal, no separate server). With the MSVC toolchain
//! WebView2Loader is linked statically; the GNU toolchain needs the DLL next to it.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::TcpListener as StdTcpListener;

use axum::Router;
use re_search_accessory_backend::{assets::embedded, routes, server};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Icon, Window, WindowBuilder},
};
use wry::{NewWindowResponse, Rect, WebViewBuilder, dpi as wry_dpi};

const WINDOW_WIDTH: f64 = 1440.0;
const WINDOW_HEIGHT: f64 = 900.0;

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    // Release builds have no console: leave a breadcrumb next to the executable.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let _ = std::fs::write(dir.join("re-search-accessory-error.log"), message);
        }
    }
    std::process::exit(1);
}

/// The webview is a child window, so its bounds must be set explicitly. Before
/// the event loop runs the window may not report a usable size yet; fall back to
/// the requested size scaled for the current DPI.
fn full_bounds(window: &Window) -> Rect {
    let mut width = window.inner_size().width;
    let mut height = window.inner_size().height;

    if width < 100 || height < 100 {
        let scale = window.scale_factor();
        width = (WINDOW_WIDTH * scale).round() as u32;
        height = (WINDOW_HEIGHT * scale).round() as u32;
    }

    Rect {
        position: wry_dpi::Position::Physical(wry_dpi::PhysicalPosition::new(0, 0)),
        size: wry_dpi::Size::Physical(wry_dpi::PhysicalSize::new(width, height)),
    }
}

/// Only pages served by our own ephemeral loopback origin stay inside the webview.
fn is_local_url(target: &str, origin: &str) -> bool {
    match target.strip_prefix(origin) {
        Some(rest) => {
            rest.is_empty() || rest.starts_with('/') || rest.starts_with('?') || rest.starts_with('#')
        }
        None => {
            target == "about:blank" || target.starts_with("data:") || target.starts_with("blob:")
        }
    }
}

/// Hand http(s)/mailto links to the user's default browser instead of the
/// embedded WebView.
fn open_external(target: &str) {
    if !(target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:"))
    {
        return;
    }

    #[cfg(target_os = "windows")]
    {
        // rundll32 takes the URL as a single argument, so no cmd.exe quoting traps.
        let _ = std::process::Command::new("rundll32.exe")
            .arg("url.dll,FileProtocolHandler")
            .arg(target)
            .spawn();
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("xdg-open").arg(target).spawn();
    }
}

fn main() {
    // Bind synchronously so the port is known before the webview opens; the TCP
    // backlog holds the first request until the async runtime starts accepting.
    let listener = match StdTcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(err) => fail(&format!("cannot bind loopback port: {err}")),
    };

    if let Err(err) = listener.set_nonblocking(true) {
        fail(&format!("cannot configure loopback listener: {err}"));
    }

    let addr = match listener.local_addr() {
        Ok(addr) => addr,
        Err(err) => fail(&format!("cannot read loopback address: {err}")),
    };

    let url = format!("http://{addr}/");
    let origin = format!("http://{addr}");

    // API + bundled UI in a background tokio runtime.
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(err) => {
                eprintln!("cannot start async runtime: {err}");
                return;
            }
        };

        runtime.block_on(async move {
            let listener = match tokio::net::TcpListener::from_std(listener) {
                Ok(listener) => listener,
                Err(err) => {
                    eprintln!("cannot adopt loopback listener: {err}");
                    return;
                }
            };

            let app = server::with_layers(
                Router::new()
                    .nest("/api", routes::router(server::state()))
                    .fallback(embedded),
            );

            if let Err(err) = axum::serve(listener, app).await {
                eprintln!("server error: {err}");
            }
        });
    });

    // Native window around the local app.
    let event_loop = EventLoop::new();

    let window_icon = Icon::from_rgba(
        include_bytes!("../../assets/app-icon-64.rgba").to_vec(),
        64,
        64,
    )
    .ok();

    let window = match WindowBuilder::new()
        .with_title("re-search-accessory")
        .with_window_icon(window_icon)
        .with_inner_size(LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT))
        .with_min_inner_size(LogicalSize::new(920.0, 620.0))
        .build(&event_loop)
    {
        Ok(window) => window,
        Err(err) => fail(&format!("cannot create window: {err}")),
    };

    // The SPA never leaves the local origin; anything else (shop/datasheet
    // links, target=_blank) belongs in the user's real browser.
    let nav_origin = origin.clone();

    let mut builder = WebViewBuilder::new()
        .with_url(&url)
        .with_navigation_handler(move |target| {
            if is_local_url(&target, &nav_origin) {
                return true;
            }

            open_external(&target);
            false
        })
        .with_new_window_req_handler(move |target, _features| {
            open_external(&target);
            NewWindowResponse::Deny
        })
        .with_bounds(full_bounds(&window))
        .with_background_color((250, 250, 252, 255));

    if cfg!(debug_assertions) {
        builder = builder.with_devtools(true);
    }

    let webview = match builder.build_as_child(&window) {
        Ok(webview) => webview,
        Err(err) => fail(&format!(
            "cannot create webview (WebView2 runtime missing?): {err}"
        )),
    };

    let mut fitted = false;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            // The window only reports its real size once the loop is running.
            Event::MainEventsCleared if !fitted => {
                fitted = true;
                let _ = webview.set_bounds(full_bounds(&window));
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                let _ = webview.set_bounds(full_bounds(&window));
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}
