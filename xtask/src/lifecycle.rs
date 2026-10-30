//! The R2 lifecycle rule that expires rendered Kokoro audio. The R2 API replaces the whole rule list on PUT, so the
//! existing rules (for example Cloudflare's default multipart-abort rule) are kept and only ours is added or replaced.

use serde_json::{Value, json};

use crate::names::{KOKORO_AUDIO_MAX_AGE_DAYS, KOKORO_AUDIO_PREFIX};

pub const RULE_ID: &str = "nolabs-expire-kokoro-audio";

pub fn max_age_seconds() -> u64 {
    KOKORO_AUDIO_MAX_AGE_DAYS * 24 * 60 * 60
}

pub fn our_rule() -> Value {
    json!({
        "id": RULE_ID,
        "enabled": true,
        "conditions": { "prefix": KOKORO_AUDIO_PREFIX },
        "deleteObjectsTransition": { "condition": { "type": "Age", "maxAge": max_age_seconds() } },
    })
}

/// True when `rule` already does what [`our_rule`] does. Compares only the fields that matter, since Cloudflare may
/// echo rules back with extra or reordered fields.
fn is_current(rule: &Value) -> bool {
    rule.get("id").and_then(Value::as_str) == Some(RULE_ID)
        && rule.get("enabled").and_then(Value::as_bool) == Some(true)
        && rule.pointer("/conditions/prefix").and_then(Value::as_str) == Some(KOKORO_AUDIO_PREFIX)
        && rule.pointer("/deleteObjectsTransition/condition/type").and_then(Value::as_str) == Some("Age")
        && rule.pointer("/deleteObjectsTransition/condition/maxAge").and_then(Value::as_u64) == Some(max_age_seconds())
}

/// The rule list to PUT, or `None` when `existing` already contains the current rule.
pub fn merge(existing: &[Value]) -> Option<Vec<Value>> {
    if existing.iter().any(is_current) {
        return None;
    }
    let mut rules: Vec<Value> =
        existing.iter().filter(|r| r.get("id").and_then(Value::as_str) != Some(RULE_ID)).cloned().collect();
    rules.push(our_rule());
    Some(rules)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn default_rule() -> Value {
        json!({"id":"Default Multipart Abort Rule","enabled":true,"conditions":{"prefix":""},
               "abortMultipartUploadsTransition":{"condition":{"maxAge":604800,"type":"Age"}}})
    }

    #[test]
    fn ninety_days() {
        assert_eq!(max_age_seconds(), 7_776_000);
        assert_eq!(our_rule()["conditions"]["prefix"], "audio/kokoro/");
    }

    #[test]
    fn adds_rule_and_keeps_others() {
        let merged = merge(&[default_rule()]).unwrap();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], default_rule());
        assert_eq!(merged[1], our_rule());
        assert_eq!(merge(&[]).unwrap(), vec![our_rule()]);
    }

    #[test]
    fn current_rule_is_left_alone_even_with_extra_fields() {
        let mut echoed = our_rule();
        echoed["storageClassTransitions"] = json!([]);
        assert_eq!(merge(&[default_rule(), echoed]), None);
    }

    #[test]
    fn stale_rule_with_our_id_is_replaced() {
        let mut stale = our_rule();
        stale["deleteObjectsTransition"]["condition"]["maxAge"] = json!(86_400);
        let merged = merge(&[stale, default_rule()]).unwrap();
        assert_eq!(merged, vec![default_rule(), our_rule()]);
        let mut disabled = our_rule();
        disabled["enabled"] = json!(false);
        assert_eq!(merge(&[disabled]).unwrap(), vec![our_rule()]);
    }
}
