pub fn json_num_to_bool(val: &serde_json::Value) -> Option<bool> {
    match val {
        serde_json::Value::Bool(x) => Some(*x),
        serde_json::Value::Number(x) => {
            #[allow(clippy::manual_map)]
            if let Some(x) = x.as_u64() {
                Some(x != 0)
            } else if let Some(x) = x.as_i64() {
                Some(x != 0)
            } else if let Some(x) = x.as_f64() {
                Some(x != 0.0)
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn identity_copy<T: Copy>(x: &T) -> T {
    *x
}
