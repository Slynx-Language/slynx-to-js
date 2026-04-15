use slynx::middleend::{Instruction, InstructionType, Label, Operand, SlynxIR, Value};

pub struct JSFunction {
    pub content: String,
}

impl JSFunction {
    pub fn compile_raw(&mut self, raw: &Operand) -> String {
        match raw {
            Operand::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            Operand::Int(i) => i.to_string(),
            Operand::Float(f) => f.to_string(),
            Operand::String(_) => unimplemented!(),
        }
    }

    pub fn compile_values(&mut self, values: &[Value], ir: &SlynxIR) -> Vec<String> {
        let mut out = Vec::with_capacity(values.len());
        for val in values {
            let v = match val {
                Value::Raw(v) => {
                    self.compile_raw(&ir.get_operand_by_pointer(v.clone().with_length())[0])
                }
                u => unimplemented!("Not implemented {u:?}"),
            };
            out.push(v);
        }
        out
    }

    pub fn compile_instruction(&mut self, instruction: &Instruction, lbl: &Label, ir: &SlynxIR) {
        match &instruction.instruction_type {
            InstructionType::RawValue => {
                let values = ir.get_values_by_pointer(instruction.operands.clone());
                let values = self.compile_values(values, ir);
                for val in values {
                    self.content.push_str(&val);
                }
            }
            InstructionType::Struct => {}
            i => unimplemented!("{i:?}"),
        }
    }

    pub fn finish(mut self) -> String {
        self.content.push('}');
        self.content
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

    pub fn create_function(&self, name: &str, param_quantity: u8) -> JSFunction {
        let args: Vec<String> = (0..param_quantity).map(|p| format!("p{p}")).collect();
        let args = args.join(",");
        JSFunction {
            content: format!("function {name}({args}){{"),
        }
    }

    pub fn append_function(&mut self, func: JSFunction) {
        self.content.push_str(&func.finish());
    }
}
