#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TextInput {
    text: String,
}

impl TextInput {
    pub fn push(&mut self, c: char) {
        self.text.push(c);
    }

    pub fn pop(&mut self) {
        self.text.pop();
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}
