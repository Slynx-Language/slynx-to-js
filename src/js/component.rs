use std::collections::HashMap;

use crate::js::InstructionCompiler;
use slynx::middleend::ir::{Instruction, InstructionType, Operand, Value};
use slynx::middleend::{IRPointer, IRSpecializedComponent, IRType, IRTypeId, Slot, SlynxIR};

pub struct JSComponent {
    name: String,
    buffer: String,
    variables: HashMap<IRPointer<Slot, 1>, String>,
    arguments: Vec<String>,
}

impl InstructionCompiler for JSComponent {
    fn arguments(&self) -> &Vec<String> {
        &self.arguments
    }
    fn variables(
        &self,
    ) -> &std::collections::HashMap<IRPointer<slynx::middleend::Slot, 1>, String> {
        &self.variables
    }
    fn variables_mut(
        &mut self,
    ) -> &mut std::collections::HashMap<IRPointer<slynx::middleend::Slot, 1>, String> {
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
        }
    }

    pub fn compile(&mut self, fields: &[IRTypeId], children: IRPointer<Value>, ir: &SlynxIR) {
        let params: Vec<String> = (0..fields.len()).map(|i| format!("p{}", i)).collect();
        self.buffer.push_str(&format!("function {}(", self.name));
        self.buffer.push_str(&params.join(","));
        self.buffer.push_str("){\n");

        let children_vals = ir.get_values_by_pointer(children);

        for (idx, child_val) in children_vals.iter().enumerate() {
            let var_name = format!("c{}", idx + 1);
            self.compile_child(child_val, &var_name, ir, &params);
        }
        let child_count = children_vals.len();
        let ret_fields = (1..=child_count)
            .map(|i| format!("c{i}"))
            .chain(params)
            .collect::<Vec<_>>()
            .join(",");
        self.buffer
            .push_str(&format!("return {{{ret_fields}}};\n",));

        self.buffer.push('}');
    }

    fn compile_child(&mut self, value: &Value, var_name: &str, ir: &SlynxIR, params: &[String]) {
        match value {
            Value::Specliazed(ptr) => {
                let spec = ir.get_specialized(ptr.clone());
                match spec {
                    IRSpecializedComponent::Text(v) => {
                        let v_vals = ir.get_values_by_pointer(v.with_length::<0>());
                        assert_eq!(v_vals.len(), 1);
                        let expr = self.compile_values(v_vals, ir);
                        self.buffer.push_str(&format!(
                            "let {} = document.createElement(\"p\");\n",
                            var_name
                        ));

                        self.buffer
                            .push_str(&format!("{}.textContent = {};\n", var_name, expr[0]));
                    }
                    IRSpecializedComponent::Div(children_vals) => {
                        self.buffer.push_str(&format!(
                            "let {} = document.createElement(\"div\");\n",
                            var_name
                        ));
                        let child_vals = ir.get_values_by_pointer(children_vals.clone());
                        for (i, child_val) in child_vals.iter().enumerate() {
                            let child_var = format!("{}_c{}", var_name, i + 1);
                            self.compile_child(child_val, &child_var, ir, params);
                            self.buffer
                                .push_str(&format!("{}.appendChild({});\n", var_name, child_var));
                        }
                    }
                }
            }
            _ => {
                let expr = self.compile_values(&[value.clone()], ir);
                self.buffer
                    .push_str(&format!("let {} = {};\n", var_name, expr[0]));
            }
        }
    }

    pub fn finish(mut self) -> String {
        self.buffer.push('}');
        self.buffer.push('\n');
        self.buffer
    }
}
