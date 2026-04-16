mod functions;
pub use functions::*;

pub struct JSBuffer {
    pub content: String,
}

impl JSBuffer {
    pub fn new() -> Self {
        Self {
            content: String::new(),
        }
    }

    pub fn create_function(&self, name: &str, param_quantity: u8) -> JSFunction {
        let args_vec: Vec<String> = (0..param_quantity).map(|p| format!("p{p}")).collect();
        let args = args_vec.join(",");
        JSFunction::new(format!("function {name}({args}){{"), args_vec)
    }

    pub fn append_function(&mut self, func: JSFunction) {
        self.content.push_str(&func.finish());
    }
}
