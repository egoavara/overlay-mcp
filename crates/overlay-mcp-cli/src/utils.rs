use serde_json::{Map, Value};

// null 또는 빈 객체를 재귀적으로 제거하는 함수
pub fn clean_json(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let cleaned_map: Map<String, Value> = map
                .into_iter()
                .filter_map(|(k, v)| {
                    let cleaned_v = clean_json(v);
                    if cleaned_v.is_null()
                        || (cleaned_v.is_object() && cleaned_v.as_object().unwrap().is_empty())
                    {
                        None // null 이거나 빈 객체면 제거
                    } else {
                        Some((k, cleaned_v))
                    }
                })
                .collect();
            Value::Object(cleaned_map)
        }
        Value::Array(arr) => {
            // 배열 내의 각 요소에 대해 재귀적으로 정리
            let cleaned_arr = arr.into_iter().map(clean_json).collect();
            Value::Array(cleaned_arr)
        }
        // 다른 타입은 그대로 반환
        _ => value,
    }
}
