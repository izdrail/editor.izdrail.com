//! End-to-end runtime test for the WASI plugin host: a real prebuilt WASI
//! plugin is discovered, activated, started and answers a request through
//! the full plugin catalog.

use std::{collections::HashMap, thread, time::Duration, time::Instant};

use lsp_types::request::Request as _;
use lapce_rpc::{
    RequestId, RpcError,
    core::{CoreNotification, CoreRpc, CoreRpcHandler},
    proxy::{ProxyHandler, ProxyNotification, ProxyRequest, ProxyRpcHandler},
};

use super::super::{PluginCatalog, PluginCatalogRpcHandler};

/// Answers the one request the catalog makes while a plugin loads.
struct AnsweringProxy {
    proxy_rpc: ProxyRpcHandler,
}

impl ProxyHandler for AnsweringProxy {
    fn handle_notification(&mut self, _rpc: ProxyNotification) {}

    fn handle_request(&mut self, id: RequestId, rpc: ProxyRequest) {
        let result = match rpc {
            ProxyRequest::GetOpenFilesContent {} => {
                Ok(lapce_rpc::proxy::ProxyResponse::GetOpenFilesContentResponse {
                    items: Vec::new(),
                })
            }
            _ => Err(RpcError {
                code: -32601,
                message: "not supported by the test proxy".into(),
            }),
        };
        self.proxy_rpc.handle_response(id, result);
    }
}

#[test]
fn wasi_volt_is_discovered_started_and_answers() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::DEBUG.into()),
        )
        .with_test_writer()
        .try_init();
    let fixture = std::env::current_dir()
        .expect("can't get lapce-proxy directory")
        .join("src")
        .join("plugin")
        .join("wasi")
        .join("plugins")
        .join("some_author.test-plugin-one");
    assert!(fixture.join("lapce.wasm").is_file());

    // Give the fixture an activation trigger in a scratch copy.
    let temp = tempfile::tempdir().unwrap();
    let volt_dir = temp.path().join("some_author.test-plugin-one");
    std::fs::create_dir(&volt_dir).unwrap();
    std::fs::copy(fixture.join("lapce.wasm"), volt_dir.join("lapce.wasm"))
        .unwrap();
    std::fs::write(
        volt_dir.join("volt.toml"),
        r#"name = "some-useful-plugin"
version = "0.1.56"
display-name = "Some Useful Plugin Name"
author = "some_author"
description = "very useful plugin"
wasm = "lapce.wasm"

[activation]
language = ["rust"]
"#,
    )
    .unwrap();

    let core_rpc = CoreRpcHandler::new();
    let proxy_rpc = ProxyRpcHandler::new();
    let catalog_rpc =
        PluginCatalogRpcHandler::new(core_rpc.clone(), proxy_rpc.clone());

    let answering_proxy = AnsweringProxy {
        proxy_rpc: proxy_rpc.clone(),
    };
    let proxy_loop = proxy_rpc.clone();
    thread::spawn(move || {
        let mut handler = answering_proxy;
        proxy_loop.mainloop(&mut handler);
    });

    let plugin_rpc = catalog_rpc.clone();
    thread::spawn(move || {
        let mut catalog = PluginCatalog::new(
            None,
            Vec::new(),
            vec![volt_dir],
            HashMap::new(),
            plugin_rpc.clone(),
        );
        plugin_rpc.mainloop(&mut catalog);
    });

    // The catalog announces discovered executable volts to the core.
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut announced = false;
    while Instant::now() < deadline && !announced {
        while let Ok(msg) = core_rpc.rx().try_recv() {
            if let CoreRpc::Notification(notification) = msg {
                if let CoreNotification::VoltInstalled { volt, .. } =
                    *notification
                {
                    assert_eq!(volt.name, "some-useful-plugin");
                    assert!(volt.wasm.is_some());
                    announced = true;
                }
            }
        }
        if !announced {
            thread::sleep(Duration::from_millis(10));
        }
    }
    assert!(announced, "catalog never announced the WASI volt");

    // Opening a rust document activates the volt: the WASI module is
    // instantiated and its guest starts.
    catalog_rpc.did_open_document(
        std::path::Path::new("/test/main.rs"),
        "rust".to_string(),
        0,
        String::new(),
    );

    // The guest answers an initialize request: full host -> guest -> host
    // roundtrip through the WASI pipes.
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut answered = None;
    while Instant::now() < deadline {
        let (result_tx, result_rx) = crossbeam_channel::bounded(1);
        catalog_rpc.send_request(
            None,
            None,
            lsp_types::request::Initialize::METHOD,
            serde_json::json!({
                "processId": null,
                "rootUri": null,
                "capabilities": {},
            }),
            None,
            None,
            false,
            move |_, result| {
                let _ = result_tx.send(result);
            },
        );
        match result_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(value)) => {
                answered = Some(value);
                break;
            }
            // No plugin loaded yet (or the guest is still booting): retry.
            _ => thread::sleep(Duration::from_millis(500)),
        }
    }
    let response = answered.expect("WASI plugin never answered initialize");
    assert!(
        response.get("capabilities").is_some(),
        "unexpected initialize response: {response}"
    );

    catalog_rpc.shutdown();
}
