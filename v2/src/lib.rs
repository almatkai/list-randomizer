//! Behavior and storage model for quuu's List Randomizer (MIT).
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const HISTORY_KEY: &str = "listRandomizerHistory";

/// None means no history, Some("") means an empty history file, as in V1.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub revision: Option<String>,
    pub content: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: u8,
    revision: String,
    content: Option<String>,
}

impl Snapshot {
    pub fn read(store: &Value) -> Result<Self, String> {
        if !store.is_object() {
            return Err("Native storage must be an object".into());
        }
        let Some(raw) = store.get(HISTORY_KEY) else {
            return Ok(Self::default());
        };
        if raw.get("content").is_none() {
            return Err("History record is missing content".into());
        }
        let record: Record = serde_json::from_value(raw.clone()).map_err(|e| e.to_string())?;
        if record.schema != 1
            || record.revision.len() != 32
            || !record.revision.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("Invalid List Randomizer history schema or revision".into());
        }
        Ok(Self {
            revision: Some(record.revision),
            content: record.content,
        })
    }

    pub fn items(&self) -> Vec<String> {
        self.content
            .as_ref()
            .map(|s| s.split(',').map(str::to_owned).collect())
            .unwrap_or_default()
    }

    /// Called only inside the real SDK's atomic update_store transaction.
    pub fn write_if_current(
        &self,
        store: &mut Value,
        content: Option<String>,
    ) -> Result<(), String> {
        if &Self::read(store)? != self {
            return Err("List changed in another instance; refresh and retry".into());
        }
        store[HISTORY_KEY] = serde_json::to_value(Record {
            schema: 1,
            revision: token(),
            content,
        })
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Same UTF-8 replacement decoding as Buffer.toString(); no legacy file is read.
pub fn decode_history(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .split(',')
        .map(str::to_owned)
        .collect()
}

pub fn token() -> String {
    format!("{:032x}", rand::thread_rng().gen::<u128>())
}

/// Production uses Rust's RNG. The tape seam compares the *original* JS loop.
/// This preserves the source's algorithm, without claiming cryptographic use.
pub fn shuffle_with<F>(items: &mut [String], mut random: F) -> Result<(), String>
where
    F: FnMut() -> f64,
{
    for i in (1..items.len()).rev() {
        let sample = random();
        if !(0.0..1.0).contains(&sample) {
            return Err("Shuffle random sample must be in [0, 1)".into());
        }
        let j = (sample * (i + 1) as f64).floor() as usize;
        items.swap(i, j);
    }
    Ok(())
}

pub fn shuffle(items: &mut [String]) {
    let mut rng = rand::thread_rng();
    // gen::<f64>() is finite and in [0, 1).
    shuffle_with(items, || rng.gen::<f64>()).expect("Rust RNG returned an invalid f64");
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub snapshot: Snapshot,
    pub items: Vec<String>,
    pub draft: String,
    pub generation: String,
    pub cycle: String,
    pub form: bool,
}

impl Session {
    pub fn new(snapshot: Snapshot) -> Self {
        Self {
            items: snapshot.items(),
            snapshot,
            draft: String::new(),
            generation: token(),
            cycle: token(),
            form: false,
        }
    }

    pub fn sync(&mut self, snapshot: Snapshot) {
        if self.snapshot != snapshot {
            self.items = snapshot.items();
            self.snapshot = snapshot;
            self.generation = token();
            self.cycle = token();
        }
    }

    pub fn action_id(&self, verb: &str) -> String {
        format!("list-randomizer:{}:{verb}", self.generation)
    }

    pub fn field_id(&self) -> String {
        format!("item-{}", self.cycle)
    }

    pub fn change_id(&self) -> String {
        format!("list-randomizer:field:{}", self.cycle)
    }
}

// Compile the unchanged shipping adapter against byte-exact beta SDK/common.
#[cfg(test)]
extern crate self as list_randomizer_v2;
#[cfg(test)]
extern crate self as tauri;
#[cfg(test)]
pub use native_core::{AppHandle, Emitter};
#[cfg(test)]
#[path = "../tests/beta_host/mod.rs"]
pub mod native_core;
#[cfg(test)]
#[path = "../tests/behavior.rs"]
mod tests;
