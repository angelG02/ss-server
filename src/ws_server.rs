use crate::thread_pool::ThreadPool;

use websocket::native_tls::TlsAcceptor;
use websocket::OwnedMessage;
use websocket::{native_tls::Identity, sync::Server};

use tracing::info;

use notify::{RecursiveMode, Watcher};
use std::collections::HashSet;
use std::sync::mpsc::channel;
use std::time::Duration;

pub struct WsServer {
    _address: String,
}

impl WsServer {
    pub fn new(address: String) -> Self {
        let pool = ThreadPool::new(8);
        let addr = address.clone();

        let mut changed_asset_paths: HashSet<String> = HashSet::new();

        let (changed_assets_sender, changed_assets_receiver) = channel::<String>();

        let asset_folder = env!("ASSETS_PATH");

        // Set up file wath with notify crate
        // Automatically select the best implementation for your platform.
        let mut watcher = notify::recommended_watcher(
            move |res: Result<notify::Event, notify::Error>| match res {
                Ok(event) => {
                    info!("{event:?}");
                    match event.kind {
                        notify::EventKind::Modify(kind) => match kind {
                            notify::event::ModifyKind::Any => {
                                let asset_path = event.paths[0].to_string_lossy().to_string();
                                let index = asset_path.find("assets/").unwrap();
                                let asset_path =
                                    asset_path[index + 7..].to_string().replace("\\", "/");
                                changed_assets_sender.send(asset_path).unwrap();
                            }
                            notify::event::ModifyKind::Data(_) => {}
                            notify::event::ModifyKind::Metadata(_) => {}
                            notify::event::ModifyKind::Name(_) => {}
                            notify::event::ModifyKind::Other => {}
                        },
                        notify::EventKind::Any => {}
                        notify::EventKind::Access(_) => {}
                        notify::EventKind::Create(_) => {}
                        notify::EventKind::Remove(_) => {}
                        notify::EventKind::Other => {}
                    }
                }
                Err(e) => info!("watch error: {:?}", e),
            },
        )
        .unwrap();

        // Add a path to be watched. All files and directories at that path and
        // below will be monitored for changes.
        watcher
            .watch(std::path::Path::new(asset_folder), RecursiveMode::Recursive)
            .unwrap();

        let path = format!("{}{}", asset_folder, "cert/pkcs12.pfx");

        let mut file = std::fs::File::open(path).unwrap();
        let mut pkcs12 = vec![];
        std::io::Read::read_to_end(&mut file, &mut pkcs12).unwrap();
        let pkcs12 = Identity::from_pkcs12(&pkcs12, "asdfghj123").unwrap();

        let acceptor = TlsAcceptor::builder(pkcs12).build().unwrap();

        let server = Server::bind_secure(address, acceptor).unwrap();

        info!("Wss connection listening on address: {}{}", "wss://", addr);

        for request in server.filter_map(Result::ok) {
            let res = changed_assets_receiver.recv_timeout(Duration::from_secs_f32(0.1));
            match res {
                Ok(path) => {
                    changed_asset_paths.insert(path);
                }
                Err(_) => {
                    changed_asset_paths.clear();
                }
            }
            let changed_clone = changed_asset_paths.clone();
            pool.execute(move || {
                let mut client = request.accept().unwrap();

                let ip = client.peer_addr().unwrap();

                let mut send_message: Option<OwnedMessage> = None;

                for message in client.incoming_messages() {
                    let message = message.unwrap_or(OwnedMessage::Close(None));

                    match message {
                        OwnedMessage::Close(_) => {
                            send_message = Some(OwnedMessage::Close(None));
                            break;
                        }
                        // OwnedMessage::Ping(ping) => {
                        //     let message = OwnedMessage::Pong(ping);
                        //     sender.send_message(&message).unwrap();
                        // }
                        OwnedMessage::Text(text) => {
                            info!("Recieved request -> {:?}", text);

                            let request: Vec<&str> = text.split(' ').collect();

                            if request[0] == "get" {
                                if request.contains(&"changed") {
                                    if !changed_clone.is_empty() {
                                        let response = changed_clone
                                            .clone()
                                            .into_iter()
                                            .collect::<std::collections::HashSet<String>>()
                                            .into_iter()
                                            .map(|path| path)
                                            .collect::<Vec<String>>()
                                            .join(" ");
                                        send_message = Some(OwnedMessage::Text(response));
                                    }
                                } else {
                                    let asset_folder = env!("ASSETS_PATH");
                                    let path = format!("{}{}", asset_folder, request[1]);

                                    info!("{path}");

                                    if let Ok(file) = std::fs::read(path.clone()) {
                                        send_message = Some(OwnedMessage::Binary(file));
                                    } else {
                                        let response = format!("File not found: {}", request[1]);
                                        send_message = Some(OwnedMessage::Text(response));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    break;
                }
                if send_message.is_some() {
                    client.send_message(&send_message.unwrap()).unwrap_or(());
                }
                client.shutdown().unwrap();
                println!("Client {} disconnected", ip);
            });
        }

        Self { _address: addr }
    }
}
