use slynx::middleend::{IRPointer, Instruction, InstructionType, Operand, SlynxIR, Value};

pub struct JSFunction {
    pub content: String,
    arguments: Vec<String>,
}

impl JSFunction {
    pub fn new(initial_content: String, arguments: Vec<String>) -> Self {
        Self {
            content: initial_content,
            arguments,
        }
    }

    ///Compiles the given `raw` operand. Operands are primitives
    pub fn compile_raw(&mut self, raw: &Operand, ir: &SlynxIR) -> String {
        match raw {
            Operand::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            Operand::Int(i) => i.to_string(),
            Operand::Float(f) => f.to_string(),
            Operand::String(_) => unimplemented!(),
        }
    }

    ///Compiles the given `values` and returns a vector with the string for each of them
    pub fn compile_values(&mut self, values: &[Value], ir: &SlynxIR) -> Vec<String> {
        let mut out = Vec::with_capacity(values.len());
        for val in values {
            let v = match val {
                Value::Raw(v) => {
                    self.compile_raw(&ir.get_operand_by_pointer(v.clone().with_length())[0], ir)
                }
                Value::FuncArg(n) => self.arguments[*n].clone(),
                Value::Instruction(ptr) => {
                    let inst = &ir.get_instruction_by_pointer(ptr.clone().with_length())[0];
                    self.compile_instruction(inst, ir)
                }
                u => unimplemented!("Not implemented {u:?}"),
            };
            out.push(v);
        }
        out
    }

    ///Compiles the given `values`, knowing its a pointer 2 specifically 2 values.
    ///When compiling it will use the `operator` as an operator for it and `()` to certify
    ///the order is correct
    pub fn compile_binary(
        &mut self,
        values: IRPointer<Value>,
        operator: &str,
        ir: &SlynxIR,
    ) -> String {
        let values = ir.get_values_by_pointer(values);
        debug_assert_eq!(values.len(), 2);
        let values = self.compile_values(values, ir);
        let mut out = String::from("(");
        out.push_str(&values[0]);
        out.push_str(operator);
        out.push_str(&values[1]);
        out.push(')');
        out
    }

    ///Compiles down the given `instruction`. This is basically recursive, since it must retrieve the values referenced by this `instruction`
    pub fn compile_instruction(&mut self, instruction: &Instruction, ir: &SlynxIR) -> String {
        match &instruction.instruction_type {
            InstructionType::RawValue => {
                let values = ir.get_values_by_pointer(instruction.operands.clone());
                let values = self.compile_values(values, ir);
                values.join(",")
            }
            InstructionType::Add => self.compile_binary(instruction.operands.clone(), "+", ir),
            InstructionType::Sub => self.compile_binary(instruction.operands.clone(), "-", ir),
            InstructionType::Mul => self.compile_binary(instruction.operands.clone(), "*", ir),
            InstructionType::Div => self.compile_binary(instruction.operands.clone(), "/", ir),
            InstructionType::Cmp => self.compile_binary(instruction.operands.clone(), "==", ir),
            InstructionType::Gt => self.compile_binary(instruction.operands.clone(), ">", ir),
            InstructionType::Lt => self.compile_binary(instruction.operands.clone(), "<", ir),
            InstructionType::Lte => self.compile_binary(instruction.operands.clone(), "<=", ir),
            InstructionType::Gte => self.compile_binary(instruction.operands.clone(), ">=", ir),
            InstructionType::And => self.compile_binary(instruction.operands.clone(), "&", ir),
            InstructionType::Or => self.compile_binary(instruction.operands.clone(), "|", ir),
            InstructionType::Xor => self.compile_binary(instruction.operands.clone(), "^", ir),
            InstructionType::Shr => self.compile_binary(instruction.operands.clone(), ">>", ir),
            InstructionType::AShr => self.compile_binary(instruction.operands.clone(), ">>>", ir),
            InstructionType::Shl => self.compile_binary(instruction.operands.clone(), "<<", ir),
            InstructionType::Ret => {
                let operand = self
                    .compile_values(ir.get_values_by_pointer(instruction.operands.clone()), ir)
                    .join(",");
                format!("return {operand};")
            }
            i => unimplemented!("{i:?}"),
        }
    }

    ///Appends the given `content` on the body of this function
    pub fn append(&mut self, content: String) {
        self.content.push_str(&content);
    }

    ///Finishes this function and returns its contents
    pub fn finish(mut self) -> String {
        self.content.push('}');
        self.content
    }
}
