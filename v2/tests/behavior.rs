use super::*;
use crate::native_core::common::{CoreContext, CorePaths};
use crate::native_core::extensions::{v2_list_randomizer as adapter, v2_sdk as sdk};
use serde_json::json;
use std::sync::{Arc, Mutex};

type Calls = Arc<Mutex<Vec<(String, Vec<Value>)>>>;
fn context(result: Result<Value, String>) -> (tempfile::TempDir, CoreContext, Calls) {
    let dir = tempfile::tempdir().unwrap();
    let ctx = CoreContext::headless(CorePaths {
        data_dir: dir.path().join("data"),
        config_dir: dir.path().join("config"),
        resource_dir: dir.path().join("resources"),
        temp_dir: dir.path().join("tmp"),
        home_dir: dir.path().join("home"),
    });
    let calls = Arc::new(Mutex::new(vec![]));
    let record = calls.clone();
    ctx.set_router(Arc::new(move |channel, args| {
        let record = record.clone();
        let result = result.clone();
        Box::pin(async move {
            record.lock().unwrap().push((channel, args));
            result
        })
    }))
    .unwrap();
    (dir, ctx, calls)
}
async fn run(ctx: &CoreContext, state: &mut Value) -> Value {
    adapter::handle(ctx, "index", "extension:run-command", &json!({}), state)
        .await
        .unwrap()
}
async fn invoke(
    ctx: &CoreContext,
    state: &mut Value,
    id: &str,
    form: Value,
) -> Result<Value, String> {
    adapter::handle(
        ctx,
        "index",
        "extension:invoke-action",
        &json!({"actionId":id,"formValues":form}),
        state,
    )
    .await
}
fn action(view: &Value, title: &str) -> String {
    view["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["title"] == title)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}
fn titles(view: &Value) -> Vec<String> {
    view["root"]["children"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["type"] == "List.Item")
        .map(|r| r["props"]["title"].as_str().unwrap().into())
        .collect()
}
async fn add(ctx: &CoreContext, s: &mut Value, view: &Value, text: &str) -> Value {
    let form = invoke(ctx, s, &action(view, "Add"), json!({}))
        .await
        .unwrap();
    let field = form["root"]["children"][0]["props"]["id"].as_str().unwrap();
    invoke(ctx, s, &action(&form, "Add"), json!({field:text}))
        .await
        .unwrap()
}
fn path(ctx: &CoreContext) -> std::path::PathBuf {
    ctx.paths
        .data_dir
        .join("native-extensions-v2/list-randomizer/storage.json")
}
async fn seed(ctx: &CoreContext, content: Option<&str>) {
    let content = content.map(str::to_string);
    sdk::update_store(ctx, "list-randomizer", move |store| {
        Snapshot::read(store)?.write_if_current(store, content)
    })
    .await
    .unwrap();
}

#[test]
fn original_js_random_tape_oracle_and_restart_semantics() {
    let fixtures: Value = serde_json::from_str(include_str!("fixtures/oracle.json")).unwrap();
    for case in fixtures["cases"].as_array().unwrap() {
        let mut items: Vec<String> = serde_json::from_value(case["input"].clone()).unwrap();
        let mut tape = case["tape"].as_array().unwrap().iter();
        shuffle_with(&mut items, || tape.next().unwrap().as_f64().unwrap()).unwrap();
        assert!(tape.next().is_none());
        assert_eq!(json!(items), case["output"]);
        assert_eq!(items.join(", "), case["copy"]);
        let content = (!items.is_empty()).then(|| items.join(","));
        assert_eq!(json!(content), case["content"]);
        assert_eq!(
            json!(Snapshot {
                revision: None,
                content
            }
            .items()),
            case["restart"]
        );
    }
    assert_eq!(
        decode_history(&[b'a', 0xff, b',', b',']),
        vec!["a�", "", ""]
    );
    assert_eq!(decode_history(b""), vec![""]);
    assert!(Snapshot::default().items().is_empty());
    for bad in [f64::NAN, -0.1, 1.0, f64::INFINITY] {
        assert!(shuffle_with(&mut ["a".into(), "b".into()], || bad).is_err());
    }
    let original: Vec<String> = (0..20).map(|n| n.to_string()).collect();
    let mut outcomes = std::collections::HashSet::new();
    for _ in 0..32 {
        let mut items = original.clone();
        shuffle(&mut items);
        let mut sorted = items.clone();
        sorted.sort();
        let mut expected = original.clone();
        expected.sort();
        assert_eq!(sorted, expected);
        outcomes.insert(items);
    }
    assert!(
        outcomes.len() > 1,
        "production shuffle must consume a real RNG"
    );
}

#[tokio::test]
async fn real_sdk_full_workflow_preserves_items_copy_and_only_scoped_clear() {
    let (_dir, ctx, calls) = context(Ok(json!({"ok":true})));
    sdk::update_store(&ctx, "list-randomizer", |v| {
        v["unrelated"] = json!({"keep":true});
        Ok(())
    })
    .await
    .unwrap();
    // An installed V1 profile is deliberately not consulted or modified.
    let legacy = ctx
        .paths
        .data_dir
        .join("extensions/list-randomizer/list_history.txt");
    std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    std::fs::write(&legacy, b"V1 remains private").unwrap();
    let mut s = json!({});
    let mut view = run(&ctx, &mut s).await;
    assert_eq!(titles(&view), Vec::<String>::new());
    assert_eq!(view["actions"].as_array().unwrap().len(), 1);
    let expected = vec!["same", "same", " ", "a,b", "Қазақ 😀"];
    for text in &expected {
        view = add(&ctx, &mut s, &view, text).await;
    }
    assert_eq!(titles(&view), expected);
    for (title, modifier, key) in [
        ("Randomize", "cmd", "j"),
        ("Copy List", "cmd", "l"),
        ("Clear List", "ctrl", "x"),
    ] {
        let a = view["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["title"] == title)
            .unwrap();
        assert_eq!(a["shortcut"], json!({"modifiers":[modifier],"key":key}));
    }
    assert_eq!(
        view["root"]["children"][0]["props"]["actionIds"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    let before = s.clone();
    let search = adapter::handle(
        &ctx,
        "index",
        "extension:search-text-changed",
        &json!({"searchText":"does not match"}),
        &mut s,
    )
    .await
    .unwrap();
    assert_eq!(titles(&search), expected);
    assert_eq!(s, before);
    invoke(&ctx, &mut s, &action(&view, "Copy List"), json!({}))
        .await
        .unwrap();
    assert_eq!(
        *calls.lock().unwrap(),
        vec![("clipboard:write".into(), vec![json!(expected.join(", "))])]
    );
    let old_copy = action(&view, "Copy List");
    view = invoke(&ctx, &mut s, &action(&view, "Randomize"), json!({}))
        .await
        .unwrap();
    assert!(invoke(&ctx, &mut s, &old_copy, json!({})).await.is_err());
    let mut sorted = titles(&view);
    sorted.sort();
    let mut sorted_expected = expected.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    sorted_expected.sort();
    assert_eq!(sorted, sorted_expected);
    let content = titles(&view).join(",");
    assert_eq!(
        Snapshot::read(&sdk::read_store(&ctx, "list-randomizer").await.unwrap())
            .unwrap()
            .content,
        Some(content.clone())
    );
    let mut restarted = json!({});
    assert_eq!(
        titles(&run(&ctx, &mut restarted).await),
        content.split(',').collect::<Vec<_>>()
    );
    let old = action(&view, "Clear List");
    view = invoke(&ctx, &mut s, &old, json!({"paths":["/arbitrary"]}))
        .await
        .unwrap();
    assert!(titles(&view).is_empty());
    assert!(invoke(&ctx, &mut s, &old, json!({})).await.is_err());
    let store = sdk::read_store(&ctx, "list-randomizer").await.unwrap();
    assert_eq!(store["unrelated"], json!({"keep":true}));
    assert!(store[HISTORY_KEY]["content"].is_null());
    assert_eq!(std::fs::read(&legacy).unwrap(), b"V1 remains private");
    assert!(titles(&run(&ctx, &mut json!({})).await).is_empty());
}

#[tokio::test]
async fn form_cycles_queued_keystrokes_refresh_empty_noop_and_relaunch() {
    let (_dir, ctx, _) = context(Ok(json!(true)));
    let mut s = json!({});
    let list = run(&ctx, &mut s).await;
    let form = invoke(&ctx, &mut s, &action(&list, "Add"), json!({}))
        .await
        .unwrap();
    let field = form["root"]["children"][0]["props"].clone();
    let change = field["actionId"].as_str().unwrap();
    let submit = action(&form, "Add");
    for text in ["a", "a,", "a, b"] {
        let updated = invoke(&ctx, &mut s, change, json!({"value":text}))
            .await
            .unwrap();
        assert_eq!(updated["root"]["children"][0]["props"]["id"], field["id"]);
        assert_eq!(action(&updated, "Add"), submit);
    }
    let before = s.clone();
    let refreshed = adapter::handle(
        &ctx,
        "index",
        "extension:refresh-session",
        &json!({}),
        &mut s,
    )
    .await
    .unwrap();
    assert_eq!(s, before);
    assert_eq!(refreshed["root"]["children"][0]["props"]["value"], "a, b");
    let id = field["id"].as_str().unwrap();
    invoke(&ctx, &mut s, &submit, json!({id:""})).await.unwrap();
    assert_eq!(s, before);
    assert!(!path(&ctx).exists());
    let list = invoke(&ctx, &mut s, &submit, json!({id:"a, b"}))
        .await
        .unwrap();
    assert_eq!(titles(&list), vec!["a, b"]);
    assert!(invoke(&ctx, &mut s, change, json!({"value":"late"}))
        .await
        .is_err());
    assert!(invoke(&ctx, &mut s, &submit, json!({id:"late"}))
        .await
        .is_err());
    let copy = action(&list, "Copy List");
    adapter::handle(
        &ctx,
        "index",
        "extension:dispose-session",
        &json!({}),
        &mut s,
    )
    .await
    .unwrap();
    assert!(invoke(&ctx, &mut s, &copy, json!({})).await.is_err());
    let list = run(&ctx, &mut s).await;
    assert_eq!(titles(&list), vec!["a", " b"]);
    assert!(invoke(&ctx, &mut s, &copy, json!({})).await.is_err());
    let form = invoke(&ctx, &mut s, &action(&list, "Add"), json!({}))
        .await
        .unwrap();
    let change = form["root"]["children"][0]["props"]["actionId"]
        .as_str()
        .unwrap()
        .to_string();
    invoke(&ctx, &mut s, &change, json!({"value":"keep draft"}))
        .await
        .unwrap();
    let list = invoke(&ctx, &mut s, &action(&form, "Back"), json!({}))
        .await
        .unwrap();
    assert!(invoke(&ctx, &mut s, &change, json!({"value":"late"}))
        .await
        .is_err());
    let form = invoke(&ctx, &mut s, &action(&list, "Add"), json!({}))
        .await
        .unwrap();
    assert_eq!(form["root"]["children"][0]["props"]["value"], "keep draft");
}

#[tokio::test]
async fn concurrent_instances_reject_stale_write_then_refresh_and_retry() {
    let (_dir, ctx, calls) = context(Ok(json!({"ok":true})));
    seed(&ctx, Some("initial")).await;
    let mut a = json!({});
    let mut b = json!({});
    let av = run(&ctx, &mut a).await;
    let bv = run(&ctx, &mut b).await;
    let af = invoke(&ctx, &mut a, &action(&av, "Add"), json!({}))
        .await
        .unwrap();
    let bf = invoke(&ctx, &mut b, &action(&bv, "Add"), json!({}))
        .await
        .unwrap();
    let aid = action(&af, "Add");
    let bid = action(&bf, "Add");
    let ak = af["root"]["children"][0]["props"]["id"].as_str().unwrap();
    let bk = bf["root"]["children"][0]["props"]["id"].as_str().unwrap();
    let (ar, br) = tokio::join!(
        invoke(&ctx, &mut a, &aid, json!({ak:"A"})),
        invoke(&ctx, &mut b, &bid, json!({bk:"B"}))
    );
    assert_ne!(ar.is_ok(), br.is_ok());
    let (winner, loser) = if ar.is_ok() {
        (&mut a, &mut b)
    } else {
        (&mut b, &mut a)
    };
    let stale_submit = if ar.is_ok() { bid } else { aid };
    let stale_field = Session::deserialize(&*loser).unwrap().change_id();
    let latest = adapter::handle(
        &ctx,
        "index",
        "extension:refresh-session",
        &json!({}),
        loser,
    )
    .await
    .unwrap();
    assert!(invoke(&ctx, loser, &stale_submit, json!({})).await.is_err());
    assert!(invoke(&ctx, loser, &stale_field, json!({"value":"late"}))
        .await
        .is_err());
    let k = latest["root"]["children"][0]["props"]["id"]
        .as_str()
        .unwrap();
    let latest = invoke(&ctx, loser, &action(&latest, "Add"), json!({k:"retry"}))
        .await
        .unwrap();
    assert_eq!(titles(&latest).len(), 3);
    let stale_copy = Session::deserialize(&*winner).unwrap().action_id("copy");
    assert!(invoke(&ctx, winner, &stale_copy, json!({})).await.is_err());
    assert!(calls.lock().unwrap().is_empty());
    // Refresh sees remote changes; clear invalidates both old mutation and copy IDs.
    let latest = adapter::handle(
        &ctx,
        "index",
        "extension:refresh-session",
        &json!({}),
        winner,
    )
    .await
    .unwrap();
    invoke(&ctx, winner, &action(&latest, "Clear List"), json!({}))
        .await
        .unwrap();
    assert!(
        invoke(&ctx, loser, &action(&latest, "Randomize"), json!({}))
            .await
            .is_err()
    );
    let s = Snapshot::read(&sdk::read_store(&ctx, "list-randomizer").await.unwrap()).unwrap();
    assert!(s.items().is_empty());
}

#[tokio::test]
async fn failure_is_retryable_and_malformed_records_are_never_reset() {
    let (_dir, ctx, calls) = context(Ok(json!({"ok":false,"error":"clipboard denied"})));
    seed(&ctx, Some("")).await;
    let mut s = json!({});
    let view = run(&ctx, &mut s).await;
    assert_eq!(titles(&view), vec![""]);
    let before = s.clone();
    for _ in 0..2 {
        assert_eq!(
            invoke(&ctx, &mut s, &action(&view, "Copy List"), json!({}))
                .await
                .unwrap_err(),
            "clipboard denied"
        );
        assert_eq!(s, before);
    }
    assert_eq!(calls.lock().unwrap().len(), 2);
    let form = invoke(&ctx, &mut s, &action(&view, "Add"), json!({}))
        .await
        .unwrap();
    let field = form["root"]["children"][0]["props"].clone();
    invoke(
        &ctx,
        &mut s,
        field["actionId"].as_str().unwrap(),
        json!({"value":"retry me"}),
    )
    .await
    .unwrap();
    let before = s.clone();
    let saved = std::fs::read(path(&ctx)).unwrap();
    std::fs::write(path(&ctx), b"{broken").unwrap();
    let id = field["id"].as_str().unwrap();
    let submit = action(&form, "Add");
    assert!(invoke(&ctx, &mut s, &submit, json!({id:"retry me"}))
        .await
        .is_err());
    assert_eq!(s, before);
    assert_eq!(std::fs::read(path(&ctx)).unwrap(), b"{broken");
    assert!(adapter::handle(
        &ctx,
        "index",
        "extension:refresh-session",
        &json!({}),
        &mut s
    )
    .await
    .is_err());
    assert_eq!(s, before);
    std::fs::write(path(&ctx), saved).unwrap();
    let view = invoke(&ctx, &mut s, &submit, json!({id:"retry me"}))
        .await
        .unwrap();
    assert_eq!(titles(&view), vec!["", "retry me"]);
    for malformed in [
        json!([]),
        json!({HISTORY_KEY:[]}),
        json!({HISTORY_KEY:{"schema":9,"revision":"bad","content":"x"}}),
        json!({HISTORY_KEY:{"schema":1,"revision":"0123456789abcdef0123456789abcdef","content":[]}}),
    ] {
        std::fs::write(path(&ctx), malformed.to_string()).unwrap();
        let bytes = std::fs::read(path(&ctx)).unwrap();
        assert!(adapter::handle(
            &ctx,
            "index",
            "extension:run-command",
            &json!({}),
            &mut json!({})
        )
        .await
        .is_err());
        assert!(
            invoke(&ctx, &mut s, &action(&view, "Clear List"), json!({}))
                .await
                .is_err()
        );
        assert_eq!(std::fs::read(path(&ctx)).unwrap(), bytes);
    }
}

#[tokio::test]
async fn invalid_command_operation_payload_never_mutates_or_routes() {
    let (_dir, ctx, calls) = context(Err("clipboard unavailable".into()));
    let mut s = json!({});
    let view = run(&ctx, &mut s).await;
    let before = s.clone();
    for (cmd, op, payload) in [
        ("wrong", "extension:run-command", json!({})),
        ("index", "unknown", json!({})),
        (
            "index",
            "extension:invoke-action",
            json!({"actionId":"clear"}),
        ),
        (
            "index",
            "extension:search-text-changed",
            json!({"searchText":2}),
        ),
    ] {
        assert!(adapter::handle(&ctx, cmd, op, &payload, &mut s)
            .await
            .is_err());
        assert_eq!(s, before);
    }
    assert!(calls.lock().unwrap().is_empty());
    assert!(!path(&ctx).exists());
    let form = invoke(&ctx, &mut s, &action(&view, "Add"), json!({}))
        .await
        .unwrap();
    let before = s.clone();
    assert!(invoke(&ctx, &mut s, &action(&form, "Add"), json!({}))
        .await
        .is_err());
    assert_eq!(s, before);
    seed(&ctx, Some("x")).await;
    let view = run(&ctx, &mut s).await;
    let before = s.clone();
    assert_eq!(
        invoke(&ctx, &mut s, &action(&view, "Copy List"), json!({}))
            .await
            .unwrap_err(),
        "clipboard unavailable"
    );
    assert_eq!(s, before);
}

#[tokio::test]
async fn emit_renderer_wire() {
    let (_dir, ctx, _) = context(Ok(json!({"ok":true})));
    seed(&ctx, Some(",same,same")).await;
    let mut state = json!({});
    let list = run(&ctx, &mut state).await;
    let list = add(&ctx, &mut state, &list, "a,b").await;
    let list = add(&ctx, &mut state, &list, "Қазақ 😀").await;
    let form = invoke(&ctx, &mut state, &action(&list, "Add"), json!({}))
        .await
        .unwrap();
    let change = form["root"]["children"][0]["props"]["actionId"]
        .as_str()
        .unwrap();
    let form = invoke(&ctx, &mut state, change, json!({"value":"queued draft"}))
        .await
        .unwrap();
    let list_back = invoke(&ctx, &mut state, &action(&form, "Back"), json!({}))
        .await
        .unwrap();
    let empty = invoke(
        &ctx,
        &mut state,
        &action(&list_back, "Clear List"),
        json!({}),
    )
    .await
    .unwrap();
    if let Ok(output) = std::env::var("LIST_RANDOMIZER_WIRE") {
        std::fs::write(
            output,
            serde_json::to_vec_pretty(&json!({"list":list,"form":form,"empty":empty})).unwrap(),
        )
        .unwrap();
    }
}

#[cfg(unix)]
#[tokio::test]
async fn atomic_write_failure_keeps_draft_and_revision_retryable() {
    use std::os::unix::fs::PermissionsExt;
    let (_dir, ctx, _) = context(Ok(json!({"ok":true})));
    seed(&ctx, Some("original")).await;
    let mut state = json!({});
    let list = run(&ctx, &mut state).await;
    let form = invoke(&ctx, &mut state, &action(&list, "Add"), json!({}))
        .await
        .unwrap();
    let field = form["root"]["children"][0]["props"].clone();
    invoke(
        &ctx,
        &mut state,
        field["actionId"].as_str().unwrap(),
        json!({"value":"preserved"}),
    )
    .await
    .unwrap();
    let before = state.clone();
    let disk = std::fs::read(path(&ctx)).unwrap();
    let directory = path(&ctx).parent().unwrap().to_path_buf();
    let permissions = std::fs::metadata(&directory).unwrap().permissions();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500)).unwrap();
    let id = field["id"].as_str().unwrap();
    let result = invoke(
        &ctx,
        &mut state,
        &action(&form, "Add"),
        json!({id:"preserved"}),
    )
    .await;
    std::fs::set_permissions(directory, permissions).unwrap();
    assert!(
        result.is_err(),
        "SDK atomic write must fail in an unwritable directory"
    );
    assert_eq!(state, before);
    assert_eq!(std::fs::read(path(&ctx)).unwrap(), disk);
    let list = invoke(
        &ctx,
        &mut state,
        &action(&form, "Add"),
        json!({id:"preserved"}),
    )
    .await
    .unwrap();
    assert_eq!(titles(&list), vec!["original", "preserved"]);
}
