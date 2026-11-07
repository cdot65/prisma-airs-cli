//! Directional security profiles (DESIGN §13.17): the protection locations of a policy,
//! the `--direction` / `--ai-profile-index` selector and the layout rules of TypeScript
//! 7.3.0 `src/airs/profile-policy.ts`, and the recursive flag merge of
//! `builders/profile-builder.ts` (`mergeSettings`). Pure `serde_json::Value` surgery, no
//! I/O; unit-tested here.
//!
//! A policy holds AI entries (`ai-security-profiles[]`). A **legacy** entry keeps its
//! protections in `model-configuration`. A **directional** entry (`content-type-mode:
//! per_content_type`, or any `content-type-configurations` key) keeps them per direction
//! in `content-type-configurations`, and its `model-configuration` holds only the shared
//! settings (latency, storage masking, conversation inspection); protection blocks left
//! there are retained but inactive.

use crate::error::CliError;
use serde_json::{Map, Value};

/// The directions TypeScript 7.3.0 knows, in its order (`PROFILE_DIRECTIONS`). It is also
/// the set of keys the live service requires on every directional entry (each may be `{}`;
/// observed in B7, D335), which a directional flag create therefore writes in full.
pub const DIRECTIONS: [&str; 4] = ["prompt", "response", "tool-call", "tool-response"];

/// The one content-type mode whose behaviour is established.
pub const PER_CONTENT_TYPE: &str = "per_content_type";

const AI_PROFILES: &str = "ai-security-profiles";
const MODEL_CONFIGURATION: &str = "model-configuration";
const CONTENT_TYPE_MODE: &str = "content-type-mode";
const CONTENT_TYPE_CONFIGURATIONS: &str = "content-type-configurations";

/// The settings shared by every direction of an entry (TS `extractSharedSettings`).
pub const SHARED_KEYS: [&str; 3] = [
    "latency",
    "mask-data-in-storage",
    "enable-full-conversation-inspection",
];

/// The detector arrays merged by `name` (TS `mergeSettings`).
const NAMED_ARRAYS: [&str; 3] = ["model-protection", "agent-protection", "database-security"];

/// `--direction` and `--ai-profile-index` (validated; TS `ProfileSelector`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selector {
    pub direction: Option<&'static str>,
    pub ai_profile_index: Option<usize>,
}

impl Selector {
    /// The usage checks of TS `validateProfileSelector`, before the tenant is read.
    /// `--ai-profile-index` takes decimal digits only (D332).
    pub fn parse(direction: Option<&str>, index: Option<&str>) -> Result<Self, CliError> {
        let direction = match direction {
            None => None,
            Some(text) => Some(
                DIRECTIONS
                    .into_iter()
                    .find(|known| *known == text)
                    .ok_or_else(|| {
                        CliError::usage(
                            "--direction must be prompt, response, tool-call, or tool-response",
                        )
                    })?,
            ),
        };
        let ai_profile_index = match index {
            None => None,
            Some(text) => Some(
                (!text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
                    .then(|| text.parse::<usize>().ok())
                    .flatten()
                    .filter(|value| *value <= MAX_SAFE_INTEGER)
                    .ok_or_else(|| {
                        CliError::usage("--ai-profile-index must be a nonnegative integer")
                    })?,
            ),
        };
        Ok(Self {
            direction,
            ai_profile_index,
        })
    }

    /// Whether `--direction` or `--ai-profile-index` was given.
    pub fn is_given(&self) -> bool {
        self.direction.is_some() || self.ai_profile_index.is_some()
    }
}

/// `Number.MAX_SAFE_INTEGER`, the largest index TypeScript accepts.
const MAX_SAFE_INTEGER: usize = 9_007_199_254_740_991;

/// A layout refusal: a usage error (exit 2), as in TypeScript 7.3.0 (§13.17).
fn refuse(message: &str) -> CliError {
    CliError::usage(message)
}

/// TS `isDirectionalProfile`: `content-type-mode: per_content_type`, or a
/// `content-type-configurations` key (any value, `null` included).
pub fn is_directional(entry: &Map<String, Value>) -> bool {
    entry.get(CONTENT_TYPE_MODE).and_then(Value::as_str) == Some(PER_CONTENT_TYPE)
        || entry.contains_key(CONTENT_TYPE_CONFIGURATIONS)
}

/// Where a protection block lives: an AI entry and, for a directional block, the
/// direction key (`None` is the entry's `model-configuration`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub index: usize,
    pub direction: Option<String>,
}

/// One protection block of a policy (TS `profileProtectionLocations`).
#[derive(Clone, Debug)]
pub struct Location<'a> {
    pub place: Place,
    /// The entry's `model-type` value, as read.
    pub model_type: Option<&'a Value>,
    pub configuration: &'a Map<String, Value>,
    /// False for a `model-configuration` block of a directional entry (retained, not
    /// enforced as protections).
    pub active: bool,
}

/// The AI entries of a policy (absent or not an array: none).
fn entries(policy: Option<&Value>) -> &[Value] {
    policy
        .and_then(|policy| policy.get(AI_PROFILES))
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// Every protection block of a policy, in entry order; an entry's `model-configuration`
/// first, then its directions (the four known ones in [`DIRECTIONS`] order, then any other
/// key). Non-object entries, blocks and directions are skipped.
pub fn locations(policy: Option<&Value>) -> Vec<Location<'_>> {
    let mut found = Vec::new();
    for (index, entry) in entries(policy).iter().enumerate() {
        let Some(entry) = entry.as_object() else {
            continue;
        };
        let directional = is_directional(entry);
        let model_type = entry.get("model-type");
        if let Some(Value::Object(configuration)) = entry.get(MODEL_CONFIGURATION) {
            found.push(Location {
                place: Place {
                    index,
                    direction: None,
                },
                model_type,
                configuration,
                active: !directional,
            });
        }
        if let Some(Value::Object(directions)) = entry.get(CONTENT_TYPE_CONFIGURATIONS) {
            // The known directions in TypeScript's schema order, then any other key (TS
            // SDK 0.35.0 parses the known keys first; the rest keep the serializer's
            // order, D35).
            let known = DIRECTIONS
                .iter()
                .filter_map(|direction| directions.get_key_value(*direction));
            let other = directions
                .iter()
                .filter(|(direction, _)| !DIRECTIONS.contains(&direction.as_str()));
            for (direction, configuration) in known.chain(other) {
                if let Value::Object(configuration) = configuration {
                    found.push(Location {
                        place: Place {
                            index,
                            direction: Some(direction.clone()),
                        },
                        model_type,
                        configuration,
                        active: true,
                    });
                }
            }
        }
    }
    found
}

/// The label of a location: `AI[<i>] <model-type> / <direction|legacy>` (TS 7.3.0
/// `renderProfileDetail`; `unspecified` without a model type).
pub fn location_label(location: &Location<'_>) -> String {
    let model_type = match location.model_type {
        None | Some(Value::Null) => "unspecified".to_owned(),
        Some(Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
    };
    format!(
        "AI[{}] {model_type} / {}",
        location.place.index,
        location.place.direction.as_deref().unwrap_or("legacy")
    )
}

/// The mutable configuration object at `place`, when it exists.
pub fn configuration_at_mut<'a>(
    policy: &'a mut Value,
    place: &Place,
) -> Option<&'a mut Map<String, Value>> {
    let entry = policy.get_mut(AI_PROFILES)?.get_mut(place.index)?;
    match &place.direction {
        None => entry.get_mut(MODEL_CONFIGURATION)?.as_object_mut(),
        Some(direction) => entry
            .get_mut(CONTENT_TYPE_CONFIGURATIONS)?
            .get_mut(direction)?
            .as_object_mut(),
    }
}

/// TS `selectAiProfile`: the index of exactly one entry, never the first entry of a
/// multi-entry policy by guess. An absent or empty list without `--ai-profile-index`
/// gets one `{"model-type": "default"}` entry. The policy must be an object.
pub fn select_ai_profile(
    policy: &mut Map<String, Value>,
    selector: &Selector,
) -> Result<usize, CliError> {
    let profiles = policy
        .entry(AI_PROFILES)
        .or_insert_with(|| Value::Array(Vec::new()));
    if profiles.is_null() {
        *profiles = Value::Array(Vec::new());
    }
    let Value::Array(profiles) = profiles else {
        return Err(refuse(
            "--ai-profile-index does not identify an existing AI profile entry",
        ));
    };
    if selector.ai_profile_index.is_none() && profiles.len() > 1 {
        return Err(refuse(
            "Multiple AI profile entries: specify --ai-profile-index or use --config",
        ));
    }
    let index = selector.ai_profile_index.unwrap_or(0);
    if profiles.is_empty() && selector.ai_profile_index.is_none() {
        let mut entry = Map::new();
        entry.insert("model-type".into(), Value::String("default".into()));
        profiles.push(Value::Object(entry));
        return Ok(0);
    }
    match profiles.get(index) {
        Some(Value::Object(_)) => Ok(index),
        _ => Err(refuse(
            "--ai-profile-index does not identify an existing AI profile entry",
        )),
    }
}

/// The entry at `index` (checked by [`select_ai_profile`]).
pub fn entry_mut(policy: &mut Map<String, Value>, index: usize) -> &mut Map<String, Value> {
    policy
        .get_mut(AI_PROFILES)
        .and_then(|profiles| profiles.get_mut(index))
        .and_then(Value::as_object_mut)
        .expect("select_ai_profile checked the entry")
}

/// TS `selectProfileProtections`: the protection block a flag or topic edit changes.
/// A directional entry needs `direction`, a mode of `per_content_type` (or none), and
/// the direction present unless `allow_missing`; a legacy entry refuses a direction and
/// any content-type mode. The block is created when absent (`{}`).
pub fn select_protections<'a>(
    entry: &'a mut Map<String, Value>,
    direction: Option<&str>,
    allow_missing: bool,
) -> Result<&'a mut Map<String, Value>, CliError> {
    if is_directional(entry) {
        let Some(direction) = direction else {
            return Err(refuse("Directional protection edits require --direction"));
        };
        if entry
            .get(CONTENT_TYPE_MODE)
            .is_some_and(|mode| mode.as_str() != Some(PER_CONTENT_TYPE))
        {
            return Err(refuse(
                "Unknown content-type-mode: use a complete --config replacement",
            ));
        }
        let present = entry
            .get(CONTENT_TYPE_CONFIGURATIONS)
            .and_then(|directions| directions.get(direction))
            .is_some_and(Value::is_object);
        if !present && !allow_missing {
            return Err(refuse(
                "Selected direction is absent; add it with a complete --config replacement before editing protections or topics",
            ));
        }
        let directions = object_at(entry, CONTENT_TYPE_CONFIGURATIONS);
        return Ok(object_at(directions, direction));
    }
    if direction.is_some() {
        return Err(refuse(
            "A legacy profile cannot be converted by --direction; use a complete --config replacement",
        ));
    }
    if entry.contains_key(CONTENT_TYPE_MODE) {
        return Err(refuse(
            "Unknown content-type-mode: use a complete --config replacement",
        ));
    }
    Ok(object_at(entry, MODEL_CONFIGURATION))
}

/// TS `sharedProfileSettings`: the entry's `model-configuration` (created when absent).
pub fn shared_settings(entry: &mut Map<String, Value>) -> &mut Map<String, Value> {
    object_at(entry, MODEL_CONFIGURATION)
}

/// The object at `key`, created (`{}`) when absent or not an object.
fn object_at<'a>(map: &'a mut Map<String, Value>, key: &str) -> &'a mut Map<String, Value> {
    let value = map
        .entry(key.to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value.as_object_mut().expect("an object")
}

/// TS `extractSharedSettings`: move the shared keys out of a built configuration.
pub fn extract_shared(config: &mut Map<String, Value>) -> Map<String, Value> {
    let mut shared = Map::new();
    for key in SHARED_KEYS {
        if let Some(value) = config.remove(key) {
            shared.insert(key.to_owned(), value);
        }
    }
    shared
}

/// TS `mergeSettings`: overlay `patch` onto `base` recursively. The detector arrays
/// (`model-protection`, `agent-protection`, `database-security`) merge by `name`: a
/// match is merged recursively (its other fields, such as severities and topic lists,
/// kept), a new name is appended, and a name that occurs more than once is refused.
/// Other values replace. A `malicious-code-protection` object without a `name` gets
/// `malicious-code`.
pub fn merge_settings(
    base: &mut Map<String, Value>,
    patch: &Map<String, Value>,
) -> Result<(), CliError> {
    for (key, value) in patch {
        match value {
            Value::Array(items) if NAMED_ARRAYS.contains(&key.as_str()) => {
                let mut merged = match base.remove(key) {
                    Some(Value::Array(existing)) => existing,
                    _ => Vec::new(),
                };
                for item in items {
                    let name = item.get("name");
                    let matches: Vec<usize> = merged
                        .iter()
                        .enumerate()
                        .filter(|(_, entry)| entry.get("name") == name)
                        .map(|(at, _)| at)
                        .collect();
                    match matches.as_slice() {
                        [] => merged.push(item.clone()),
                        [at] => match (&mut merged[*at], item) {
                            (Value::Object(existing), Value::Object(fields)) => {
                                merge_settings(existing, fields)?;
                            }
                            (slot, _) => *slot = item.clone(),
                        },
                        _ => {
                            return Err(CliError::usage(format!(
                                "Ambiguous detector identity: {}",
                                name.map_or_else(|| "undefined".to_owned(), js_string)
                            )));
                        }
                    }
                }
                base.insert(key.clone(), Value::Array(merged));
            }
            Value::Object(fields) => {
                let mut target = match base.remove(key) {
                    Some(Value::Object(existing)) => existing,
                    _ => Map::new(),
                };
                merge_settings(&mut target, fields)?;
                if key == "malicious-code-protection" && !target.contains_key("name") {
                    target.insert("name".into(), Value::String("malicious-code".into()));
                }
                base.insert(key.clone(), Value::Object(target));
            }
            other => {
                base.insert(key.clone(), other.clone());
            }
        }
    }
    Ok(())
}

/// JavaScript `String(value)` for the scalar names a detector can carry.
fn js_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn object(value: Value) -> Map<String, Value> {
        value.as_object().cloned().unwrap()
    }

    fn usage(message: &str) -> CliError {
        CliError::usage(message)
    }

    #[test]
    fn selectors_are_checked_like_typescript_with_strict_numbers() {
        assert_eq!(
            Selector::parse(Some("tool-call"), Some("2")).unwrap(),
            Selector {
                direction: Some("tool-call"),
                ai_profile_index: Some(2)
            }
        );
        assert!(!Selector::parse(None, None).unwrap().is_given());
        for bad in ["invalid", "Prompt", "", "legacy"] {
            assert_eq!(
                Selector::parse(Some(bad), None),
                Err(usage(
                    "--direction must be prompt, response, tool-call, or tool-response"
                ))
            );
        }
        for bad in [
            "-1",
            "1.5",
            "abc",
            "",
            "1e1",
            "0x1",
            " 1",
            "9007199254740992",
        ] {
            assert_eq!(
                Selector::parse(None, Some(bad)),
                Err(usage("--ai-profile-index must be a nonnegative integer")),
                "{bad}"
            );
        }
        assert_eq!(
            Selector::parse(None, Some("0")).unwrap().ai_profile_index,
            Some(0)
        );
    }

    #[test]
    fn locations_keep_entry_direction_and_activity() {
        let policy = json!({"ai-security-profiles": [
            {"model-type": "default", "model-configuration": {"model-protection": []}},
            {"model-type": "other", "content-type-mode": "per_content_type",
             "model-configuration": {"latency": {}},
             "content-type-configurations": {"response": {"a": 1}, "prompt": {}, "future": {"x": true}, "bad": null}},
            "not an object",
            {"content-type-configurations": null}
        ]});
        let found = locations(Some(&policy));
        let labels: Vec<(String, bool)> = found
            .iter()
            .map(|location| (location_label(location), location.active))
            .collect();
        assert_eq!(
            labels,
            vec![
                ("AI[0] default / legacy".to_owned(), true),
                ("AI[1] other / legacy".to_owned(), false),
                ("AI[1] other / prompt".to_owned(), true),
                ("AI[1] other / response".to_owned(), true),
                ("AI[1] other / future".to_owned(), true),
            ]
        );
        assert_eq!(found[3].configuration, &object(json!({"a": 1})));
        assert!(locations(None).is_empty());
        assert!(locations(Some(&json!({"ai-security-profiles": "x"}))).is_empty());
        assert!(is_directional(&object(
            json!({"content-type-configurations": null})
        )));
        assert!(is_directional(&object(
            json!({"content-type-mode": "per_content_type"})
        )));
        assert!(!is_directional(&object(
            json!({"content-type-mode": "future_mode"})
        )));
    }

    #[test]
    fn selecting_an_entry_never_guesses() {
        let mut empty = Map::new();
        assert_eq!(select_ai_profile(&mut empty, &Selector::default()), Ok(0));
        assert_eq!(
            Value::Object(empty),
            json!({"ai-security-profiles": [{"model-type": "default"}]})
        );
        let mut indexed = Map::new();
        let first = Selector {
            ai_profile_index: Some(0),
            ..Selector::default()
        };
        assert_eq!(
            select_ai_profile(&mut indexed, &first),
            Err(usage(
                "--ai-profile-index does not identify an existing AI profile entry"
            ))
        );
        let mut two = object(json!({"ai-security-profiles": [{}, {}]}));
        assert_eq!(
            select_ai_profile(&mut two, &Selector::default()),
            Err(usage(
                "Multiple AI profile entries: specify --ai-profile-index or use --config"
            ))
        );
        let second = Selector {
            ai_profile_index: Some(1),
            ..Selector::default()
        };
        assert_eq!(select_ai_profile(&mut two, &second), Ok(1));
        let third = Selector {
            ai_profile_index: Some(2),
            ..Selector::default()
        };
        assert!(select_ai_profile(&mut two, &third).is_err());
    }

    #[test]
    fn protection_blocks_follow_the_layout_rules() {
        let mut legacy = object(json!({"model-type": "default"}));
        select_protections(&mut legacy, None, false)
            .unwrap()
            .insert("x".into(), json!(1));
        assert_eq!(legacy["model-configuration"], json!({"x": 1}));
        assert_eq!(
            select_protections(&mut legacy, Some("prompt"), true).map(|_| ()),
            Err(usage(
                "A legacy profile cannot be converted by --direction; use a complete --config replacement"
            ))
        );
        let mut moded = object(json!({"content-type-mode": "future_mode"}));
        assert_eq!(
            select_protections(&mut moded, None, false).map(|_| ()),
            Err(usage(
                "Unknown content-type-mode: use a complete --config replacement"
            ))
        );
        let mut unknown = object(json!({"content-type-mode": "future_mode",
            "content-type-configurations": {"prompt": {}}}));
        assert_eq!(
            select_protections(&mut unknown, Some("prompt"), false).map(|_| ()),
            Err(usage(
                "Unknown content-type-mode: use a complete --config replacement"
            ))
        );
        let mut directional = object(json!({"content-type-mode": "per_content_type",
            "content-type-configurations": {"prompt": {"a": 1}}}));
        assert_eq!(
            select_protections(&mut directional, None, false).map(|_| ()),
            Err(usage("Directional protection edits require --direction"))
        );
        assert_eq!(
            select_protections(&mut directional, Some("response"), false).map(|_| ()),
            Err(usage(
                "Selected direction is absent; add it with a complete --config replacement before editing protections or topics"
            ))
        );
        assert_eq!(
            select_protections(&mut directional, Some("prompt"), false)
                .unwrap()
                .clone(),
            object(json!({"a": 1}))
        );
        select_protections(&mut directional, Some("response"), true).unwrap();
        assert_eq!(
            directional["content-type-configurations"]["response"],
            json!({})
        );
    }

    #[test]
    fn merging_is_recursive_by_name_and_refuses_ambiguity() {
        let mut base = object(json!({
            "model-protection": [
                {"name": "toxic-content", "action": "block", "severity-by-confidence": {"high": "medium"}},
                {"name": "topic-guardrails", "topic-list": [{"action": "block"}]}
            ],
            "app-protection": {"malicious-code-protection": {"name": "malicious-code-detection", "action": "block", "severity": "high"},
                "allow-url-category": {"member": ["a"], "x": 1}},
            "data-protection": {"data-leak-detection": {"action": "block", "member": [{"text": "PII"}], "mask-data-inline": true}},
            "latency": {"inline-timeout-action": "block", "max-inline-latency": 5}
        }));
        let patch = object(json!({
            "model-protection": [{"name": "toxic-content", "action": "alert"}, {"name": "prompt-injection", "action": "block"}],
            "app-protection": {"malicious-code-protection": {"action": "allow"}, "allow-url-category": {"member": ["b"]}},
            "data-protection": {"data-leak-detection": {"mask-data-inline": false}},
            "latency": {"max-inline-latency": 9},
            "mask-data-in-storage": false
        }));
        merge_settings(&mut base, &patch).unwrap();
        assert_eq!(
            Value::Object(base),
            json!({
                "model-protection": [
                    {"name": "toxic-content", "action": "alert", "severity-by-confidence": {"high": "medium"}},
                    {"name": "topic-guardrails", "topic-list": [{"action": "block"}]},
                    {"name": "prompt-injection", "action": "block"}
                ],
                "app-protection": {"malicious-code-protection": {"name": "malicious-code-detection", "action": "allow", "severity": "high"},
                    "allow-url-category": {"member": ["b"], "x": 1}},
                "data-protection": {"data-leak-detection": {"action": "block", "member": [{"text": "PII"}], "mask-data-inline": false}},
                "latency": {"inline-timeout-action": "block", "max-inline-latency": 9},
                "mask-data-in-storage": false
            })
        );
        let mut fresh = Map::new();
        merge_settings(
            &mut fresh,
            &object(json!({"app-protection": {"malicious-code-protection": {"action": "block"}}})),
        )
        .unwrap();
        assert_eq!(
            fresh["app-protection"]["malicious-code-protection"],
            json!({"action": "block", "name": "malicious-code"})
        );
        let mut twice = object(json!({"database-security": [
            {"name": "database-security-read"}, {"name": "database-security-read"}]}));
        assert_eq!(
            merge_settings(
                &mut twice,
                &object(
                    json!({"database-security": [{"name": "database-security-read", "action": "allow"}]})
                )
            ),
            Err(usage("Ambiguous detector identity: database-security-read"))
        );
    }

    #[test]
    fn shared_settings_move_out_of_a_built_configuration() {
        let mut config = object(
            json!({"latency": {"max-inline-latency": 3}, "mask-data-in-storage": true,
            "enable-full-conversation-inspection": false, "model-protection": []}),
        );
        let shared = extract_shared(&mut config);
        assert_eq!(Value::Object(config), json!({"model-protection": []}));
        assert_eq!(
            Value::Object(shared),
            json!({"latency": {"max-inline-latency": 3}, "mask-data-in-storage": true,
                "enable-full-conversation-inspection": false})
        );
    }
}
