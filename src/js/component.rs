use std::collections::HashMap;

use crate::js::InstructionCompiler;
use slynx::slynx_ir::ir::Value;
use slynx::slynx_ir::{IRPointer, IRTypeId, Instruction, Slot, SlynxIR};

pub struct JSComponent {
    name: String,
    buffer: String,
    variables: HashMap<IRPointer<Slot, 1>, String>,
    arguments: Vec<String>,
    identation: usize,
}

impl InstructionCompiler for JSComponent {
    fn identation_value(&self) -> usize {
        self.identation
    }

    fn increase_identation(&mut self) {
        self.identation += 1;
    }

    fn decrease_identation(&mut self) {
        self.identation -= 1;
    }

    fn arguments(&self) -> &Vec<String> {
        &self.arguments
    }
    fn variables(&self) -> &std::collections::HashMap<IRPointer<slynx::slynx_ir::Slot, 1>, String> {
        &self.variables
    }
    fn variables_mut(
        &mut self,
    ) -> &mut std::collections::HashMap<IRPointer<slynx::slynx_ir::Slot, 1>, String> {
        &mut self.variables
    }
}

impl JSComponent {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            buffer: String::new(),
            arguments: Vec::new(),
            variables: HashMap::new(),
            identation: 0,
        }
    }

    pub fn compile(
        &mut self,
        fields: &[IRTypeId],
        children: IRPointer<Value>,
        instructions: IRPointer<Instruction>,
        ir: &SlynxIR,
    ) {
        let params: Vec<String> = (0..fields.len()).map(|i| format!("p{}", i)).collect();
        self.buffer.push_str(&format!("function {}(", self.name));
        self.buffer.push_str(&params.join(","));
        self.buffer.push_str("){\n");
        self.increase_identation();

        let children_vals = ir.get_values_by_pointer(children);

        for (idx, child_val) in children_vals.iter().enumerate() {
            let var_name = format!("c{}", idx);
            self.compile_child(child_val, &var_name, ir);
        }
        let child_count = children_vals.len();
        let ret_fields = (1..=child_count)
            .map(|i| format!("c{i}"))
            .chain(params)
            .collect::<Vec<_>>()
            .join(",");

        for instruction in ir.get_instruction_by_pointer(instructions) {
            let instruction = self.compile_instruction(instruction, ir);
            self.buffer.push_str(&self.ident(instruction));
            self.buffer.push_str(";\n");
        }
        self.buffer
            .push_str(&self.ident(format!("return {{{ret_fields}}};\n",)));
        self.decrease_identation();
    }

    fn compile_child(&mut self, value: &Value, var_name: &str, ir: &SlynxIR) {
        let expr = self.compile_values(&[value.clone()], ir);
        let out = format!("let {var_name} = {};\n", expr[0]);

        self.buffer.push_str(&self.ident(out));
    }

    pub fn finish(mut self) -> String {
        self.buffer.push('}');
        self.buffer.push('\n');
        self.buffer
    }
}
