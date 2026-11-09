//! Drives the real project commands through Tauri's IPC layer (mock runtime): argument names,
//! JSON shapes, app-data path resolution and a simulated application restart.
use serde_json::{json, Value};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::webview::InvokeRequest;
use tauri::{App, Manager, WebviewWindow, WebviewWindowBuilder};

fn launch(identifier: &str) -> (App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let mut context = mock_context(noop_assets());
    context.config_mut().identifier = identifier.to_owned();
    let app = optic_rift_lib::with_commands(mock_builder())
        .build(context)
        .expect("mock app builds");
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock window builds");
    (app, window)
}

fn call(window: &WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, Value> {
    get_ipc_response(
        window,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        },
    )
    .map(|body| body.deserialize::<Value>().unwrap())
}

#[test]
fn project_commands_survive_app_restart() {
    let identifier = format!("studio.mythologiq.opticrift.ipctest-{}", std::process::id());
    let (app, window) = launch(&identifier);
    let data_dir = app.path().app_data_dir().unwrap();
    assert!(data_dir.ends_with(&identifier), "{data_dir:?}");
    let _ = std::fs::remove_dir_all(&data_dir);

    let empty = call(&window, "project_list", json!({})).unwrap();
    assert_eq!(empty, json!({"projects": [], "problems": []}));
    let created = call(&window, "project_create", json!({"name": "  Coastline  "})).unwrap();
    assert_eq!(created["name"], "Coastline");
    assert_eq!(created["schema_version"], 1);
    let id = created["id"].as_str().unwrap().to_owned();
    let bad = call(&window, "project_create", json!({"name": "a\u{7}b"})).unwrap_err();
    assert!(bad.as_str().unwrap().contains("control characters"));
    assert!(call(&window, "project_open", json!({"id": "../x"})).is_err());
    drop(window);
    drop(app);

    // "Restart": a new app instance with no shared memory finds the project on disk.
    let (app, window) = launch(&identifier);
    let listing = call(&window, "project_list", json!({})).unwrap();
    assert_eq!(listing["projects"].as_array().unwrap().len(), 1);
    assert_eq!(listing["projects"][0]["id"], id.as_str());
    let reopened = call(&window, "project_open", json!({"id": id})).unwrap();
    assert_eq!(reopened, created);
    let status = call(&window, "runtime_status", json!({})).unwrap();
    assert_eq!(status["project_storage_ready"], false);
    drop(window);
    drop(app);
    std::fs::remove_dir_all(&data_dir).unwrap();
}
