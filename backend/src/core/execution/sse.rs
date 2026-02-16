use serde_json::Value;

#[derive(Debug, Clone)]
pub enum SseParsedEvent {
    TextToken(String),
    FunctionCall { name: String, args: Value, raw_part: Value },
}

pub struct SseParser {
    pub buffer: String,
}

impl SseParser {
    pub fn new() -> Self {
        Self { buffer: String::new() }
    }

    pub fn parse_parts(json_val: &Value) -> Vec<SseParsedEvent> {
        let mut events = Vec::new();
        if let Some(parts) = json_val["candidates"][0]["content"]["parts"].as_array() {
            for part in parts {
                if let Some(text) = part["text"].as_str() {
                    if !text.is_empty() {
                        events.push(SseParsedEvent::TextToken(text.to_string()));
                    }
                }
                if let Some(fc) = part.get("functionCall") {
                    if let Some(name) = fc["name"].as_str() {
                        events.push(SseParsedEvent::FunctionCall {
                            name: name.to_string(),
                            args: fc["args"].clone(),
                            raw_part: part.clone(),
                        });
                    }
                }
            }
        }
        events
    }

    pub fn feed(&mut self, chunk: &str) -> Vec<SseParsedEvent> {
        self.buffer.push_str(chunk);
        let mut events = Vec::new();
        while let Some(pos) = self.buffer.find("\n\n") {
            let block = self.buffer[..pos].to_string();
            self.buffer = self.buffer[pos + 2..].to_string();
            for line in block.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if data != "[DONE]" && !data.is_empty() {
                        if let Ok(jv) = serde_json::from_str::<Value>(data) {
                            events.extend(Self::parse_parts(&jv));
                        }
                    }
                }
            }
        }
        events
    }

    pub fn flush(&mut self) -> Vec<SseParsedEvent> {
        let mut events = Vec::new();
        for line in self.buffer.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data != "[DONE]" && !data.is_empty() {
                    if let Ok(jv) = serde_json::from_str::<Value>(data) {
                        events.extend(Self::parse_parts(&jv));
                    }
                }
            }
        }
        self.buffer.clear();
        events
    }
}
