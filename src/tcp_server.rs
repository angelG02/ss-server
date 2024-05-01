use std::{io::BufReader, sync::Arc};

use crate::thread_pool::ThreadPool;
use tokio::{
    io::{split, AsyncReadExt, AsyncWriteExt, ReadHalf},
    net::{TcpListener, TcpStream},
};
use tokio_rustls::server::TlsStream;
use tracing::{error, info};

pub struct TcpServer {
    _address: String,
}

const MAX_REQUEST_SIZE: usize = 100 * 1024 * 1024;
const AUTH_TOKEN: &'static str = "genkata";

impl TcpServer {
    pub async fn run(address: String) {
        let addr = address;
        let listener = TcpListener::bind(addr.as_str()).await.unwrap();

        let asset_folder = env!("ASSETS_PATH");

        let cert_path = format!("{}{}", asset_folder, "cert/cert.pem");
        // let root_cert_path = format!("{}{}", asset_folder, "cert/origin_ca_rsa_root.pem");
        let pk_path = format!("{}{}", asset_folder, "cert/pk.pem");

        let cert = std::fs::File::open(cert_path).unwrap();
        // let root_cert = std::fs::File::open(root_cert_path).unwrap();

        let pk = std::fs::File::open(pk_path).unwrap();

        let certs = rustls_pemfile::certs(&mut BufReader::new(cert))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        let private_key = rustls_pemfile::private_key(&mut BufReader::new(pk))
            .unwrap()
            .unwrap();

        // let mut root_cert_pem = BufReader::new(root_cert);
        // let mut root_cert_store = rustls::RootCertStore::empty();

        // for cert in rustls_pemfile::certs(&mut root_cert_pem) {
        //     root_cert_store.add(cert.unwrap()).unwrap();
        // }

        // let verifier = WebPkiClientVerifier::builder(Arc::new(root_cert_store))
        //     .build()
        //     .unwrap();

        let config =
            rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS12])
                .with_no_client_auth()
                .with_single_cert(certs, private_key)
                .unwrap();

        info!(
            "Http connection listening on address: {}{}",
            "http://", addr
        );

        let thread_pool = ThreadPool::new(8);

        loop {
            let Ok((stream, _peer_addr)) = listener.accept().await else {
                continue;
            };

            let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config.clone()));

            thread_pool.execute(|| {
                pollster::block_on(TcpServer::handle_connection(stream, acceptor));
            });
        }
    }

    async fn handle_connection(stream: TcpStream, tls_acceptor: tokio_rustls::TlsAcceptor) {
        let stream = tls_acceptor.accept(stream).await;

        match stream {
            Ok(stream) => {
                let (mut reader, mut writer) = split(stream);

                let request_size = match validate_size(&mut reader).await {
                    Some(value) => value,
                    None => return,
                };

                let mut request: Vec<u8> = vec![0; request_size];
                reader.read_exact(&mut request).await.unwrap_or_else(|err| {
                    error!("{err}");
                    request = [0x65, 0x72, 0x72].to_vec();
                    0
                });

                info!("{}", request.len());

                let command = String::from_utf8(request[0..3].to_vec())
                    .unwrap_or("Could not read command".into());

                info!("Client Command: {:?}", command);

                match command.as_str() {
                    "get" => {
                        let request = String::from_utf8(request).unwrap_or(
                            "Could not parse get request. Non-valid utf8 characters encountered"
                                .into(),
                        );

                        let request: Vec<&str> = request.split(' ').collect();
                        info!("{request:?}");

                        if !request.contains(&"changed") {
                            let path = format!(
                                "{}{}",
                                env!("ASSETS_PATH"),
                                request.get(1).unwrap_or(&"err")
                            );

                            if let Ok(file) = std::fs::read(&path) {
                                writer.write_all(&file).await.unwrap_or_else(|err| {
                                    error!("{err}");
                                    ()
                                });
                            } else {
                                let response = format!("File not found: {}", path);
                                writer
                                    .write_all(response.as_bytes())
                                    .await
                                    .unwrap_or_else(|err| {
                                        error!("{err}");
                                        ()
                                    });
                            }
                        } else {
                            error!("PLEASE EXTEND NATIVE FILE WATCH AND CHANGE FUNCTIONALITY");
                        }
                    }
                    "put" => {
                        let request = String::from_utf8(request).unwrap_or(
                            "Could not parse put request. Non-valid utf8 characters encountered"
                                .into(),
                        );
                        info!("{request:?}");

                        let request: Vec<&str> = request.split(" ").collect();
                        let _asset_name = request[3];

                        match request[1] {
                            "model" => {
                                let asset_path =
                                    format!("{}{}", env!("ASSETS_PATH"), "models/test/duck.glb");

                                if request[2] == AUTH_TOKEN {
                                    writer
                                        .write_all(&b"OK".len().to_ne_bytes())
                                        .await
                                        .unwrap_or_else(|err| {
                                            error!("{err}");
                                            ()
                                        });

                                    writer.write_all(b"OK").await.unwrap_or_else(|err| {
                                        error!("{err}");
                                        ()
                                    });

                                    let model_size = match validate_size(&mut reader).await {
                                        Some(value) => value,
                                        None => return,
                                    };

                                    let mut model_data: Vec<u8> = vec![0; model_size];
                                    reader.read_exact(&mut model_data).await.unwrap_or_else(
                                        |err| {
                                            error!("{err}");
                                            0
                                        },
                                    );

                                    std::fs::write(asset_path, model_data).unwrap_or_else(|err| {
                                        error!("{err}");
                                        ()
                                    });
                                } else {
                                    writer.write_all(&b"Auth token rejected! Please try again or contact the developer".len().to_ne_bytes()).await.unwrap_or_else(|err| {
                                        error!("{err}");
                                        ()
                                    });
                                    writer.write_all(b"Auth token rejected! Please try again or contact the developer").await.unwrap_or_else(|err| {
                                        error!("{err}");
                                        ()
                                    });
                                }
                            }
                            "texture" => {}
                            "shader" => {}
                            _ => {}
                        }
                    }
                    "len" => {
                        let request = String::from_utf8(request).unwrap_or(
                            "Could not parse get request. Non-valid utf8 characters encountered"
                                .into(),
                        );

                        let request: Vec<&str> = request.split(' ').collect();

                        let path = format!(
                            "{}{}",
                            env!("ASSETS_PATH"),
                            request.get(1).unwrap_or(&"err")
                        );

                        if let Ok(file) = std::fs::read(&path) {
                            let size = std::mem::size_of_val(&file);
                            writer.write_all(&size.to_ne_bytes()).await.unwrap();
                        } else {
                            let response = format!("File not found: {}", path);
                            writer.write_all(response.as_bytes()).await.unwrap();
                        }
                    }
                    "err" => {
                        error!("Error parsing first 3 characters of request to determin command");
                    }
                    _ => {}
                }
                reader
                    .unsplit(writer)
                    .shutdown()
                    .await
                    .unwrap_or_else(|err| {
                        error!("{err}");
                        ()
                    });
            }
            Err(err) => {
                error!("Could not connect to client! {err}");
                return;
            }
        }
    }
}

async fn validate_size(reader: &mut ReadHalf<TlsStream<TcpStream>>) -> Option<usize> {
    let mut request_size: [u8; 8] = [0; 8];
    reader
        .read_exact(&mut request_size)
        .await
        .unwrap_or_else(|err| {
            error!("{err}");
            0
        });
    info!("Bytes: {:?}", request_size);
    let request_size = usize::from_ne_bytes(request_size);
    info!("Request size: {}", request_size);
    if request_size > MAX_REQUEST_SIZE {
        error!(
            "Maximum request size is: {}mb",
            MAX_REQUEST_SIZE / 1024 / 1024
        );
        return None;
    }
    Some(request_size)
}
