//! Real project-photo command routing through the same registered Tauri IPC
//! handler as production. Uses a mock runtime and a unique local app-data root.
use base64::Engine;
use serde_json::{json, Value};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::webview::InvokeRequest;
use tauri::{App, Manager, WebviewWindow, WebviewWindowBuilder};

fn launch(identifier: &str) -> (App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let mut context = mock_context(noop_assets());
    context.config_mut().identifier = identifier.to_owned();
    let app = optic_rift_lib::with_commands(mock_builder()).build(context)
        .expect("mock project photo app builds");
    let window = WebviewWindowBuilder::new(&app, "main", Default::default()).build()
        .expect("mock window builds");
    (app, window)
}
fn call(window: &WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, Value> {
    get_ipc_response(
        window,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0), error: CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        },
    ).map(|body| body.deserialize::<Value>().unwrap())
}
fn tiny_png() -> String {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 2, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[255u8; 16]).unwrap();
    }
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
fn recipe() -> Value {
    json!({
        "schema_version": 1,
        "brightness": 120, "contrast": 100, "saturation": 90,
        "rotation": 90, "straighten_tenths": -25,
        "resize_long_edge": null, "crop": null,
        "flip_horizontal": true, "flip_vertical": false
    })
}
#[test]
fn explicit_png_project_save_and_reopen_survive_new_app_instance() {
    let identifier = format!("studio.mythologiq.opticrift.photoipctest-{}", std::process::id());
    let (app, window) = launch(&identifier);
    let root = app.path().app_data_dir().unwrap();
    let _ = std::fs::remove_dir_all(&root);
    let project = call(&window, "project_create", json!({"name":"Research images"})).unwrap();
    let id = project["id"].as_str().unwrap().to_owned();
    let saved = call(&window, "project_photo_save", json!({
        "request": {
            "project_id": id, "asset_id": null, "expected_revision": null,
            "source_name": "Source.png", "source_png_base64": tiny_png(),
            "edits": recipe()
        }
    })).unwrap();
    assert_eq!(saved["revision"], 1);
    assert_eq!(saved["edits"]["flip_horizontal"], true);
    let asset = saved["asset_id"].as_str().unwrap().to_string();
    assert_eq!(call(&window, "project_photo_list", json!({"projectId":id}))
        .unwrap()["photos"].as_array().unwrap().len(), 1);
    drop(window);
    drop(app);

    let (app, window) = launch(&identifier);
    let reopened = call(&window, "project_photo_open", json!({
        "projectId": id, "assetId": asset
    })).unwrap();
    assert_eq!(reopened["document"]["revision"], 1);
    assert_eq!(reopened["document"]["edits"]["straighten_tenths"], -25);
    assert_eq!(reopened["source_png_base64"], tiny_png());
    assert!(call(&window, "project_photo_open", json!({
        "projectId": id, "assetId": "../../escape"
    })).is_err());
    let stale = call(&window, "project_photo_save", json!({
        "request": {
            "project_id": id, "asset_id": asset, "expected_revision": 0,
            "source_name": "Source.png", "source_png_base64": tiny_png(),
            "edits": recipe()
        }
    }));
    assert!(stale.is_err(), "stale revisions must not silently overwrite");
    drop(window);
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
