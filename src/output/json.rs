use serde::Serialize;

/// Wrap any serializable value in a standard JSON envelope.
#[derive(Serialize)]
pub struct JsonEnvelope<T: Serialize> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> JsonEnvelope<T> {
    pub fn success(data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }
}

impl JsonEnvelope<()> {
    pub fn error(message: String) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_envelope() {
        let env = JsonEnvelope::success("hello");
        assert!(env.ok);
        assert_eq!(env.data, Some("hello"));
        assert!(env.error.is_none());
    }

    #[test]
    fn test_error_envelope() {
        let env = JsonEnvelope::<()>::error("something broke".into());
        assert!(!env.ok);
        assert!(env.data.is_none());
        assert_eq!(env.error.as_deref(), Some("something broke"));
    }

    #[test]
    fn test_success_serialization_omits_none_fields() {
        let env = JsonEnvelope::success(42);
        let json = serde_json::to_value(&env).unwrap();
        assert_eq!(json["ok"], true);
        assert_eq!(json["data"], 42);
        assert!(json.get("error").is_none());
    }

    #[test]
    fn test_error_serialization_omits_none_fields() {
        let env = JsonEnvelope::<()>::error("fail".into());
        let json = serde_json::to_value(&env).unwrap();
        assert_eq!(json["ok"], false);
        assert!(json.get("data").is_none());
        assert_eq!(json["error"], "fail");
    }

    #[test]
    fn test_success_with_struct() {
        #[derive(Serialize, PartialEq, Debug)]
        struct Info {
            count: u32,
        }
        let env = JsonEnvelope::success(Info { count: 5 });
        assert!(env.ok);
        assert_eq!(env.data.unwrap().count, 5);
    }
}
