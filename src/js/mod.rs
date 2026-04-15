pub struct JSFunction<'a> {
    pub content: &'a mut String,
}

impl<'a> Drop for JSFunction<'a> {
    fn drop(&mut self) {
        self.content.push('}');
    }
}

pub struct JSBuffer {
    pub content: String,
}

impl JSBuffer {
    pub fn new() -> Self {
        Self {
            content: String::new(),
        }
    }

    pub fn write_function<'a>(&'a mut self, name: &str, param_quantity: u8) -> JSFunction<'a> {
        let args: Vec<String> = (0..param_quantity).map(|p| format!("p{p}")).collect();
        let args = args.join(",");
        self.content.push_str(&format!("function {name}({args}){{"));
        JSFunction {
            content: &mut self.content,
        }
    }
}
