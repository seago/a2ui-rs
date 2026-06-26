use a2ui_core::prelude::*;
use serde_json::json;

/// 完整流程：createSurface → updateComponents → updateDataModel → deleteSurface
#[test]
fn test_full_surface_lifecycle() {
    // 1. createSurface
    let json = r#"{
        "version": "v1.0",
        "createSurface": {
            "surfaceId": "lifecycle-test",
            "catalogId": "basic",
            "sendDataModel": true,
            "components": [
                {"id":"root","component":"Column","children":{"children":["title","btn"]}},
                {"id":"title","component":"Text","text":"Hello"},
                {"id":"btn","component":"Button","child":"btn_label"},
                {"id":"btn_label","component":"Text","text":"Click me"}
            ],
            "dataModel": {"user": {"name": "Alice"}}
        }
    }"#;

    let envelope = ServerEnvelope::from_json(json).unwrap();
    match envelope {
        ServerEnvelope::V1_0(V1_0ServerMessage::CreateSurface(msg)) => {
            assert_eq!(msg.surface_id, "lifecycle-test");
            assert_eq!(msg.catalog_id, "basic");
            assert!(msg.send_data_model);
            assert_eq!(msg.components.as_ref().unwrap().len(), 4);
            assert_eq!(msg.data_model.as_ref().unwrap()["user"]["name"], "Alice");
        }
        _ => panic!("expected CreateSurface"),
    }

    // 2. updateComponents
    let update_json = r#"{
        "version": "v1.0",
        "updateComponents": {
            "surfaceId": "lifecycle-test",
            "components": [
                {"id": "new_comp", "component": "Text", "text": "Dynamic"}
            ]
        }
    }"#;
    let env2 = ServerEnvelope::from_json(update_json).unwrap();
    match env2 {
        ServerEnvelope::V1_0(V1_0ServerMessage::UpdateComponents(msg)) => {
            assert_eq!(msg.surface_id, "lifecycle-test");
            assert_eq!(msg.components.len(), 1);
        }
        _ => panic!("expected UpdateComponents"),
    }

    // 3. updateDataModel
    let dm_json = r#"{
        "version": "v1.0",
        "updateDataModel": {
            "surfaceId": "lifecycle-test",
            "path": "/user/name",
            "value": "Bob"
        }
    }"#;
    let env3 = ServerEnvelope::from_json(dm_json).unwrap();
    match env3 {
        ServerEnvelope::V1_0(V1_0ServerMessage::UpdateDataModel(msg)) => {
            assert_eq!(msg.surface_id, "lifecycle-test");
            assert_eq!(msg.path, Some("/user/name".into()));
        }
        _ => panic!("expected UpdateDataModel"),
    }

    // 4. deleteSurface
    let del_json = r#"{"version":"v1.0","deleteSurface":{"surfaceId":"lifecycle-test"}}"#;
    let env4 = ServerEnvelope::from_json(del_json).unwrap();
    match env4 {
        ServerEnvelope::V1_0(V1_0ServerMessage::DeleteSurface(msg)) => {
            assert_eq!(msg.surface_id, "lifecycle-test");
        }
        _ => panic!("expected DeleteSurface"),
    }
}

#[test]
fn test_data_model_round_trip() {
    let original = json!({
        "form": {
            "fields": [
                {"name": "email", "value": "a@b.com"},
                {"name": "age", "value": 30}
            ]
        }
    });

    let dm = DataModel::new(original.clone());
    assert_eq!(dm.get("/form/fields/0/name"), Some(&json!("email")));
    assert_eq!(dm.get("/form/fields/1/value"), Some(&json!(30)));
}

#[test]
fn test_state_machine_full_cycle() {
    let mut sm = StateMachine::new("s1".to_string());
    assert_eq!(sm.state(), SurfaceState::Pending);

    sm.create_surface().unwrap();
    assert_eq!(sm.state(), SurfaceState::Active);

    sm.update_components().unwrap();
    sm.update_data_model().unwrap();

    sm.delete_surface().unwrap();
    assert_eq!(sm.state(), SurfaceState::Deleted);

    // Deleted 后不能再创建
    assert!(sm.create_surface().is_err());
}

#[test]
fn test_component_tree_construction() {
    let root = Component::column(
        ComponentId::new("root").unwrap(),
        vec![
            ComponentId::new("header").unwrap(),
            ComponentId::new("body").unwrap(),
        ],
    );

    let header = Component::text(
        ComponentId::new("header").unwrap(),
        DynamicValue::Literal("Welcome".to_string()),
    );

    let body = Component::row(
        ComponentId::new("body").unwrap(),
        vec![ComponentId::new("left").unwrap()],
    );

    let components = vec![root, header, body];
    assert_eq!(components.len(), 3);
}

#[test]
fn test_client_action_message() {
    let action = ActionMessage::event("submit", "s1")
        .with_response("/result", "act-1")
        .with_context("value", DynamicValue::Literal("test".into()));

    let json = serde_json::to_value(&action).unwrap();
    assert_eq!(json["name"], "submit");
    assert_eq!(json["surfaceId"], "s1");
    assert!(json["wantResponse"].as_bool().unwrap());
    assert_eq!(json["responsePath"], "/result");
    assert_eq!(json["actionId"], "act-1");
}

#[test]
fn test_catalog_validation_integration() {
    let mut catalog = Catalog::new("a2ui://catalogs/basic/v1".to_string());
    catalog.add_component("Text", json!({"type": "object"}));
    catalog.add_function(
        "required",
        json!({"returnType":"boolean","callableFrom":"clientOnly"}),
    );
    assert!(CatalogValidator::validate(&catalog).is_ok());
}
