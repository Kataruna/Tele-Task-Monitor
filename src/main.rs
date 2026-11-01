#![cfg_attr(not(test), windows_subsystem = "windows")]

// Portable core (no OS-specific code).
mod aggregate;
mod config;
mod lan;
mod model;
mod sampler;
mod server;
mod settings;
mod viewers;

// Everything OS-specific: one backend per target OS (see src/platform/mod.rs).
mod platform;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;

use platform::dialog::show_message_box;
use platform::{priority, tray};

const INTERVAL: Duration = Duration::from_millis(1000);

fn main() {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let explicit_port = raw_args.iter().any(|a| a == "--port");
    let cfg = match config::parse(raw_args.into_iter()) {
        Ok(c) => c,
        Err(e) => {
            let msg = if e.is_empty() {
                config::USAGE.to_string()
            } else {
                format!("Error: {e}\n\n{}", config::USAGE)
            };
            show_message_box("Tele-Task-Monitor", &msg, !e.is_empty());
            std::process::exit(if e.is_empty() { 0 } else { 2 });
        }
    };
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let web_dir = config::resolve_web_dir(cfg.web_dir.clone(), exe_dir.as_deref());
    if !web_dir.join("index.html").is_file() {
        eprintln!(
            "warning: no index.html in {} (the page will 404; /api/stream still works)",
            web_dir.display()
        );
    }

    let settings_store = settings::SettingsStore::load_or_default(exe_dir.as_deref());
    if explicit_port {
        let mut s = settings_store.get();
        s.port = cfg.port;
        let _ = settings_store.update(s);
    }
    let active_port = settings_store.get().port;

    priority::lower_self();

    // Bind the TCP socket upfront so we can report port conflicts immediately.
    let std_listener = match std::net::TcpListener::bind((cfg.bind, active_port)) {
        Ok(l) => {
            let _ = l.set_nonblocking(true);
            l
        }
        Err(e) => {
            show_message_box(
                "Tele-Task-Monitor",
                &format!(
                    "Cannot listen on {}:{}: {e}\n\nTele-Task-Monitor may already be running in your system tray.",
                    cfg.bind, active_port
                ),
                true,
            );
            std::process::exit(1);
        }
    };

    let viewers = viewers::Viewers::new();
    let (tx, rx) = watch::channel(None);
    let info = sampler::spawn(Arc::clone(&viewers), tx, INTERVAL);
    let info_json: Arc<str> = serde_json::to_string(&info).expect("Info serializes").into();

    let (port_tx, port_rx) = tokio::sync::mpsc::unbounded_channel::<std::net::TcpListener>();
    let port_rx = Arc::new(tokio::sync::Mutex::new(port_rx));
    let state = server::AppState {
        info_json,
        samples: rx,
        viewers,
        settings: settings_store,
        bind_ip: cfg.bind,
        port_change_tx: Some(port_tx),
    };

    let lan_urls = lan::collect_lan_urls(active_port);
    println!("Tele-Task-Monitor serving {}", web_dir.display());
    for (url, iface) in &lan_urls {
        println!("  Open on iPad: {url}   ({iface})");
    }

    // Run the HTTP/SSE server on a lightweight single-threaded Tokio runtime thread,
    // seamlessly rebinding if the user changes the port in Settings.
    std::thread::Builder::new()
        .name("tele-http".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            rt.block_on(async move {
                let mut current_std = std_listener;
                loop {
                    let listener = tokio::net::TcpListener::from_std(current_std)
                        .expect("tokio TcpListener::from_std");
                    let app = server::router(state.clone(), web_dir.clone());
                    let next_std_cell = Arc::new(std::sync::Mutex::new(None));
                    let next_std_clone = Arc::clone(&next_std_cell);
                    let rx_clone = Arc::clone(&port_rx);

                    let serve_res = axum::serve(listener, app)
                        .with_graceful_shutdown(async move {
                            if let Some(new_l) = rx_clone.lock().await.recv().await {
                                *next_std_clone.lock().unwrap() = Some(new_l);
                            }
                        })
                        .await;

                    if let Err(e) = serve_res {
                        eprintln!("error: server stopped: {e}");
                        std::process::exit(1);
                    }

                    if let Some(new_l) = next_std_cell.lock().unwrap().take() {
                        current_std = new_l;
                    } else {
                        break;
                    }
                }
            });
        })
        .expect("spawn http thread");

    // Run the tray (Windows) / menu-bar (macOS) UI on the main thread until the user clicks "Quit".
    tray::run(active_port, lan_urls);
}
