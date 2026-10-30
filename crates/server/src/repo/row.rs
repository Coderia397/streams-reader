//! Column accessors for JSON rows. D1 hands numbers to Rust as JS doubles, so integers may arrive as `3.0`; every
//! integer accessor accepts both forms.

use serde_json::Value;

use crate::ports::PortError;

pub use crate::ports::Row;

fn missing(col: &str) -> PortError {
    PortError::new(format!("column {col} missing or of the wrong type"))
}

pub fn opt_str(row: &Row, col: &str) -> Result<Option<String>, PortError> {
    match row.get(col) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(missing(col)),
    }
}

pub fn str(row: &Row, col: &str) -> Result<String, PortError> {
    opt_str(row, col)?.ok_or_else(|| missing(col))
}

pub fn opt_i64(row: &Row, col: &str) -> Result<Option<i64>, PortError> {
    match row.get(col) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                Ok(Some(i))
            } else {
                // Integral doubles below 2^53 convert exactly.
                n.as_f64()
                    .filter(|f| f.fract() == 0.0 && f.abs() < 9.007_199_254_740_992e15)
                    .map(|f| Some(f as i64))
                    .ok_or_else(|| missing(col))
            }
        }
        Some(_) => Err(missing(col)),
    }
}

pub fn i64(row: &Row, col: &str) -> Result<i64, PortError> {
    opt_i64(row, col)?.ok_or_else(|| missing(col))
}

pub fn opt_u32(row: &Row, col: &str) -> Result<Option<u32>, PortError> {
    opt_i64(row, col)?.map(|v| u32::try_from(v).map_err(|_| missing(col))).transpose()
}

pub fn u32(row: &Row, col: &str) -> Result<u32, PortError> {
    opt_u32(row, col)?.ok_or_else(|| missing(col))
}

pub fn opt_u64(row: &Row, col: &str) -> Result<Option<u64>, PortError> {
    opt_i64(row, col)?.map(|v| u64::try_from(v).map_err(|_| missing(col))).transpose()
}

pub fn u64(row: &Row, col: &str) -> Result<u64, PortError> {
    opt_u64(row, col)?.ok_or_else(|| missing(col))
}

pub fn bool(row: &Row, col: &str) -> Result<bool, PortError> {
    Ok(i64(row, col)? != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(v: Value) -> Row {
        v.as_object().cloned().unwrap()
    }

    #[test]
    fn integers_accept_doubles_from_d1() {
        let r = row(json!({"a": 3.0, "b": 4, "c": null, "d": 1.5, "e": "x"}));
        assert_eq!(i64(&r, "a").unwrap(), 3);
        assert_eq!(u32(&r, "b").unwrap(), 4);
        assert_eq!(opt_i64(&r, "c").unwrap(), None);
        assert_eq!(opt_i64(&r, "zz").unwrap(), None);
        assert!(i64(&r, "d").is_err());
        assert!(i64(&r, "e").is_err());
        assert_eq!(str(&r, "e").unwrap(), "x");
        assert!(bool(&r, "b").unwrap());
    }

    #[test]
    fn negative_values_do_not_fit_unsigned() {
        let r = row(json!({"a": -1}));
        assert!(u64(&r, "a").is_err());
    }
}
