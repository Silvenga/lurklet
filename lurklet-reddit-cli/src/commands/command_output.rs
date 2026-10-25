use serde_json::Value;

pub struct CommandOutput {
    pub result: Value,
    pub error: Option<Value>,
}
