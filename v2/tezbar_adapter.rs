//! Compiled-in beta Native V2 adapter; pair with the host's real v2_sdk.
use super::v2_sdk as sdk;
use crate::native_core::common::{CoreContext, CoreResult};
use list_randomizer_v2::{shuffle, token, Session, Snapshot};
use serde_json::{json, Value};

const SLUG: &str = "list-randomizer";
// Host serializes per-session state. This lane also serializes independent
// instances through effects/reads, while SDK update_store checks disk revisions.
static LANE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub fn commands() -> &'static [&'static str] {
    &["index"]
}

fn view(s: &Session) -> Value {
    let action = |verb: &str, title: &str, kind: &str| json!({"id":s.action_id(verb),"title":title,"kind":kind});
    if s.form {
        return sdk::view(
            "Add Item",
            json!({"type":"Form","props":{"backActionId":s.action_id("back")},"children":[
                {"type":"Form.TextField","props":{"id":s.field_id(),"title":"Item","placeholder":"Add items to the list","value":s.draft,"actionId":s.change_id()}}
            ]}),
            vec![
                action("submit", "Add", "submit-form"),
                action("back", "Back", "action"),
            ],
        );
    }
    let mut actions = vec![action("add", "Add", "action")];
    if !s.items.is_empty() {
        for (verb, title, modifiers, key) in [
            ("randomize", "Randomize", vec!["cmd"], "j"),
            ("copy", "Copy List", vec!["cmd"], "l"),
            ("clear", "Clear List", vec!["ctrl"], "x"),
        ] {
            let mut a = action(verb, title, "action");
            a["shortcut"] = json!({"modifiers":modifiers,"key":key});
            actions.push(a);
        }
    }
    let ids: Vec<_> = actions.iter().map(|a| a["id"].clone()).collect();
    let mut rows: Vec<_> = s.items.iter().enumerate().map(|(i, item)| {
        json!({"type":"List.Item","props":{"id":format!("item:{i}"),"title":item,"actionIds":ids}})
    }).collect();
    if rows.is_empty() {
        rows.push(json!({"type":"List.EmptyView","props":{"title":"No list items","description":"Use Actions → Add to enter an item."}}));
    }
    sdk::view(
        "Randomize List",
        json!({"type":"List","props":{"__hasServerSearch":true,"searchBarPlaceholder":"Use Add to enter an item"},"children":rows}),
        actions,
    )
}

async fn persist(ctx: &CoreContext, s: &mut Session, items: Vec<String>) -> Result<(), String> {
    let expected = s.snapshot.clone();
    // None is the scoped equivalent of trashing the V1 history file; keeping a
    // revision tombstone prevents an old instance from resurrecting that list.
    let content = (!items.is_empty()).then(|| items.join(","));
    let store = sdk::update_store(ctx, SLUG, move |store| {
        expected.write_if_current(store, content)
    })
    .await?;
    let snapshot = Snapshot::read(&store)?;
    // Do not round-trip items through commas until restart or a remote revision.
    s.items = items;
    s.snapshot = snapshot;
    s.generation = token();
    s.cycle = token();
    s.form = false;
    Ok(())
}

fn effect_success(value: Value) -> Result<(), String> {
    if value.get("ok") == Some(&Value::Bool(false)) {
        return Err(value
            .get("error")
            .or_else(|| value.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("Clipboard effect failed")
            .to_string());
    }
    Ok(())
}

pub async fn handle(
    ctx: &CoreContext,
    command: &str,
    operation: &str,
    payload: &Value,
    state: &mut Value,
) -> CoreResult {
    if !commands().contains(&command) {
        return Err("Unsupported List Randomizer command".into());
    }
    if !matches!(
        operation,
        "extension:run-command"
            | "extension:refresh-session"
            | "extension:invoke-action"
            | "extension:search-text-changed"
            | "extension:dispose-session"
            | "extension:load-more"
    ) {
        return Err("Unsupported List Randomizer operation".into());
    }
    if !state.is_object() {
        return Err("Invalid List Randomizer session state".into());
    }
    let _lane = LANE.lock().await;
    if operation == "extension:dispose-session" {
        *state = json!({});
        return Ok(sdk::unchanged());
    }
    if operation == "extension:run-command" {
        let snapshot = Snapshot::read(&sdk::read_store(ctx, SLUG).await?)?;
        let s = Session::new(snapshot);
        let result = view(&s);
        *state = serde_json::to_value(s).map_err(|e| e.to_string())?;
        return Ok(result);
    }
    let mut s: Session = serde_json::from_value(state.clone())
        .map_err(|_| "List Randomizer session is not active")?;
    if operation == "extension:invoke-action" {
        let id = payload["actionId"].as_str().ok_or("Missing action ID")?;
        // Validate before storage reads, clipboard calls or state mutation.
        let verb = if s.form && id == s.change_id() {
            "change"
        } else {
            let allowed: &[&str] = if s.form {
                &["submit", "back"]
            } else if s.items.is_empty() {
                &["add"]
            } else {
                &["add", "randomize", "copy", "clear"]
            };
            allowed
                .iter()
                .copied()
                .find(|verb| id == s.action_id(verb))
                .ok_or("Unknown or stale List Randomizer action")?
        };
        match verb {
            "change" => {
                s.draft = payload["formValues"]["value"]
                    .as_str()
                    .ok_or("Item value must be text")?
                    .to_string();
            }
            "back" => {
                s.form = false;
                s.generation = token();
                s.cycle = token();
            }
            _ => {
                let snapshot = Snapshot::read(&sdk::read_store(ctx, SLUG).await?)?;
                if snapshot != s.snapshot {
                    return Err("List changed in another instance; refresh and retry".into());
                }
                match verb {
                    "add" => {
                        s.form = true;
                        s.generation = token();
                        s.cycle = token();
                    }
                    "submit" => {
                        let draft = payload["formValues"]
                            .get(s.field_id())
                            .and_then(Value::as_str)
                            .ok_or("Missing item text")?
                            .to_string();
                        if draft.is_empty() {
                            return Ok(view(&s));
                        }
                        let mut items = s.items.clone();
                        items.push(draft);
                        persist(ctx, &mut s, items).await?;
                        s.draft.clear();
                    }
                    "randomize" => {
                        let mut items = s.items.clone();
                        shuffle(&mut items);
                        persist(ctx, &mut s, items).await?;
                    }
                    "clear" => persist(ctx, &mut s, vec![]).await?,
                    "copy" => effect_success(sdk::copy(ctx, &s.items.join(", ")).await?)?,
                    _ => unreachable!(),
                }
            }
        }
    } else if operation == "extension:refresh-session" {
        s.sync(Snapshot::read(&sdk::read_store(ctx, SLUG).await?)?);
    } else if operation == "extension:search-text-changed" {
        // Native List search is not an item draft and never filters this list.
        payload["searchText"]
            .as_str()
            .ok_or("Missing search text")?;
    } else {
        return Ok(sdk::unchanged());
    }
    let result = view(&s);
    *state = serde_json::to_value(s).map_err(|e| e.to_string())?;
    Ok(result)
}
