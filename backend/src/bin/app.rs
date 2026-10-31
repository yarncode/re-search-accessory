//! Desktop wrapper: starts the axum API on an ephemeral loopback port, serves the
//! UI bundled inside this executable, and shows it in a native WebView2 window.
//!
//! `cargo build --release --bin re-search-accessory-app` produces an .exe that can
//! be double-clicked (no terminal, no separate server). With the MSVC toolchain
//! WebView2Loader is linked statically; the GNU toolchain needs the DLL next to it.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::TcpListener as StdTcpListener;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{SystemTime, UNIX_EPOCH};

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

fn make_connect_secret() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);

    format!("{nanos:x}-{:x}", std::process::id())
}

/// Runs on thegioiic pages while the user signs in. Once the website stores its
/// session token, send it to the local backend and return to the BOM page.
/// The random per-run secret keeps other websites from posting a token to the
/// loopback API.
fn thegioiic_login_script(origin: &str, secret: &str) -> String {
    let template = r#"
(() => {
  const LOCAL = '__ORIGIN__';
  const SECRET = '__SECRET__';
  const host = location.hostname;

  if (host === 'accounts.google.com') {
    document.addEventListener('keydown', event => {
      if (event.key === 'Escape') {
        event.preventDefault();
        location.href = LOCAL + '/bom?thegioiic=cancelled';
      }
    }, true);

    return;
  }

  if (host !== 'www.thegioiic.com' && host !== 'thegioiic.com')
    return;

  const back = reason => {
    location.href = LOCAL + '/bom?thegioiic=' + reason;
  };

  const params = new URLSearchParams(location.search);

  // The app opens /gio-hang?res_cart=1 after pushing a BOM. Rebuild the
  // website's local cart from the server cart so the page shows the items.
  if (params.get('res_cart') === '1') {
    fetch(LOCAL + '/api/thegioiic/cart-state', {
      headers: { 'X-RES-Connect': SECRET },
      cache: 'no-store',
    })
      .then(response => response.ok ? response.json() : null)
      .then((data) => {
        if (data && Array.isArray(data.items)) {
          localStorage.setItem('cart', JSON.stringify(data.items));
          params.delete('res_cart');

          const query = params.toString();

          location.replace(location.pathname + (query ? '?' + query : ''));
        }
      })
      .catch(() => {});

    return;
  }

  if (location.pathname.startsWith('/auth/')) {
    let sent = false;
    let timer = null;

    const check = async () => {
      if (sent)
        return;

      let token = null;

      try {
        token = localStorage.getItem('token');
      }
      catch (e) {
        return;
      }

      if (!token)
        return;

      sent = true;

      if (timer)
        clearInterval(timer);

      let ok = false;

      try {
        const response = await fetch(LOCAL + '/api/thegioiic/connect', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            'X-RES-Connect': SECRET,
          },
          body: JSON.stringify({ token }),
        });

        ok = response.ok;
      }
      catch (e) {}

      back(ok ? 'connected' : 'error');
    };

    check();
    timer = setInterval(check, 700);
    setTimeout(() => { if (timer) clearInterval(timer); }, 180000);
  }

  const addBackButton = () => {
    if (document.getElementById('res-back-app'))
      return;

    const button = document.createElement('button');

    button.id = 'res-back-app';
    button.textContent = '← Quay lại app';
    button.addEventListener('click', () => back('closed'));
    button.style.cssText = 'position:fixed;right:16px;bottom:16px;z-index:2147483647;padding:10px 14px;border:0;border-radius:8px;background:#7c3aed;color:#fff;font:600 13px system-ui,sans-serif;cursor:pointer;box-shadow:0 4px 14px rgba(0,0,0,.25)';
    document.body.appendChild(button);
  };

  if (document.readyState === 'loading')
    document.addEventListener('DOMContentLoaded', addBackButton);
  else
    addBackButton();
})();
"#;

    template
        .replace("__ORIGIN__", origin)
        .replace("__SECRET__", secret)
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

    let connect_secret = make_connect_secret();
    server::set_connect_secret(connect_secret.clone());
    let login_script = thegioiic_login_script(&origin, &connect_secret);
    let login_mode = Arc::new(AtomicBool::new(false));

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

    // The SPA normally never leaves the local origin. During thegioiic sign-in
    // we allow the official login flow, then the injected script returns to /bom.
    let nav_origin = origin.clone();
    let nav_login = login_mode.clone();
    let popup_login = login_mode.clone();

    let mut builder = WebViewBuilder::new()
        .with_url(&url)
        .with_initialization_script(&login_script)
        .with_navigation_handler(move |target| {
            if is_local_url(&target, &nav_origin) {
                nav_login.store(false, Ordering::Relaxed);

                return true;
            }

            if target.starts_with("https://api.thegioiic.com/v1/auth/") {
                nav_login.store(true, Ordering::Relaxed);

                return true;
            }

            if target.starts_with("https://www.thegioiic.com/gio-hang")
                || target.starts_with("https://thegioiic.com/gio-hang")
            {
                nav_login.store(true, Ordering::Relaxed);

                return true;
            }

            if nav_login.load(Ordering::Relaxed)
                && (target.starts_with("https://www.thegioiic.com/")
                    || target.starts_with("https://thegioiic.com/")
                    || target.starts_with("https://accounts.google.com/")
                    || target.starts_with("https://accounts.youtube.com/"))
            {
                return true;
            }

            open_external(&target);
            false
        })
        .with_new_window_req_handler(move |target, _features| {
            if popup_login.load(Ordering::Relaxed)
                && (target.starts_with("https://accounts.google.com/")
                    || target.starts_with("https://www.thegioiic.com/")
                    || target.starts_with("https://thegioiic.com/"))
            {
                return NewWindowResponse::Allow;
            }

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
