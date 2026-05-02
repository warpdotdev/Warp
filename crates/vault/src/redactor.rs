#[cfg(test)]
#[path = "redactor_tests.rs"]
mod tests;

use std::collections::HashSet;
use zeroize::Zeroizing;

pub struct Redactor {
    known_values: Vec<Zeroizing<String>>,
    known_set: HashSet<String>,
}

impl Redactor {
    pub fn new() -> Self {
        Self {
            known_values: Vec::new(),
            known_set: HashSet::new(),
        }
    }

    pub fn register(&mut self, value: String) {
        self.known_set.insert(value.clone());
        self.known_values.push(Zeroizing::new(value));
    }

    pub fn redact(&self, output: &str) -> String {
        let mut result = output.to_string();
        for secret in &self.known_set {
            if !secret.is_empty() {
                result = result.replace(secret.as_str(), "[REDACTED]");
            }
        }
        result
    }
}

impl Default for Redactor {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Redactor {
    fn drop(&mut self) {
        self.known_set.clear();
    }
}
