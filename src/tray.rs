use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};

pub fn run(api_key: String, server_config: crate::config::Config) {
    let event_loop = EventLoopBuilder::new().build();

    let menu = Menu::new();

    let status_item = MenuItem::new("AiMessage Server", false, None);
    let copy_key_item = MenuItem::new(
        format!("Copy API Key ({}...)", &api_key[..8]),
        true,
        None,
    );
    let open_config_item = MenuItem::new("Open Config File", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

    let copy_key_id = copy_key_item.id().clone();
    let open_config_id = open_config_item.id().clone();
    let quit_id = quit_item.id().clone();

    menu.append(&status_item).unwrap();
    menu.append(&PredefinedMenuItem::separator()).unwrap();
    menu.append(&copy_key_item).unwrap();
    menu.append(&open_config_item).unwrap();
    menu.append(&PredefinedMenuItem::separator()).unwrap();
    menu.append(&quit_item).unwrap();

    let icon = create_status_icon(true);

    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("AiMessage Server")
        .with_icon(icon)
        .build()
        .unwrap();

    // Spawn the server on a background thread
    let server_api_key = api_key.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(crate::run_server(server_config));
    });

    let config_path = crate::config::Config::config_path();
    let menu_api_key = api_key.clone();

    // Run the macOS event loop on main thread
    event_loop.run(move |_event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(100),
        );

        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == copy_key_id {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(&menu_api_key);
                    tracing::info!("API key copied to clipboard");
                }
            } else if event.id == open_config_id {
                let _ = std::process::Command::new("open").arg(&config_path).spawn();
            } else if event.id == quit_id {
                tracing::info!("Quit requested from menu");
                std::process::exit(0);
            }
        }
    });
}

fn create_status_icon(connected: bool) -> Icon {
    let (r, g, b) = if connected {
        (0u8, 200u8, 0u8)
    } else {
        (200u8, 0u8, 0u8)
    };

    let size = 22usize;
    let mut rgba = vec![0u8; size * size * 4];

    let center = size as f32 / 2.0;
    let radius = size as f32 / 2.0 - 1.0;

    for y in 0..size {
        for x in 0..size {
            let idx = (y * size + x) * 4;
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist <= radius {
                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = 255;
            }
        }
    }

    Icon::from_rgba(rgba, size as u32, size as u32).unwrap()
}
