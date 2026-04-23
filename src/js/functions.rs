use std::collections::HashMap;

use slynx::middleend::{IRPointer, Slot};

use crate::InstructionCompiler;

pub struct JSFunction {
    pub content: String,
    arguments: Vec<String>,
    variables: HashMap<IRPointer<Slot, 1>, String>,
}

impl InstructionCompiler for JSFunction {
    fn arguments(&self) -> &Vec<String> {
        &self.arguments
    }
    fn variables(&self) -> &HashMap<IRPointer<Slot, 1>, String> {
        &self.variables
    }
    fn variables_mut(&mut self) -> &mut HashMap<IRPointer<Slot, 1>, String> {
        &mut self.variables
    }
}

impl JSFunction {
    pub fn new(initial_content: String, arguments: Vec<String>) -> Self {
        Self {
            content: initial_content,
            arguments,
            variables: HashMap::new(),
        }
    }

    ///Appends the given `content` on the body of this function
    pub fn append(&mut self, content: String) {
        self.content.push_str(&content);
    }

    ///Finishes this function and returns its contents
    pub fn finish(mut self) -> String {
        self.content.push_str("\n}\n");
        self.content
    }
}
