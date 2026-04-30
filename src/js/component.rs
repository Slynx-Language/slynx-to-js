use std::collections::HashMap;

use crate::js::InstructionCompiler;
use slynx::middleend::ir::Value;
use slynx::middleend::{IRPointer, IRSpecializedComponent, IRTypeId, Slot, SlynxIR};

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
            identation: 0,
        }
    }

    pub fn compile(&mut self, fields: &[IRTypeId], children: IRPointer<Value>, ir: &SlynxIR) {
        let params: Vec<String> = (0..fields.len()).map(|i| format!("p{}", i)).collect();
        self.buffer.push_str(&format!("function {}(", self.name));
        self.buffer.push_str(&params.join(","));
        self.buffer.push_str("){\n");
        self.increase_identation();

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
            .push_str(&self.ident(format!("return {{{ret_fields}}};\n",)));
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
                        self.buffer.push_str(
                            &self.ident(format!(
                                "let {var_name} = document.createElement(\"p\");\n",
                            )),
                        );

                        self.buffer.push_str(
                            &self.ident(format!("{var_name}.textContent = {};\n", expr[0])),
                        );
                    }
                    IRSpecializedComponent::Div(children_vals) => {
                        self.buffer.push_str(&self.ident(format!(
                            "let {} = document.createElement(\"div\");\n",
                            var_name
                        )));
                        let child_vals = ir.get_values_by_pointer(children_vals.clone());
                        for (i, child_val) in child_vals.iter().enumerate() {
                            let child_var = format!("{var_name}_c{}", i + 1);
                            self.compile_child(child_val, &child_var, ir, params);
                            self.buffer.push_str(
                                &self.ident(format!("{var_name}.appendChild({child_var});\n")),
                            );
                        }
                    }
                }
            }
            _ => {
                let expr = self.compile_values(&[value.clone()], ir);
                self.buffer
                    .push_str(&self.ident(format!("let {var_name} = {};\n", expr[0])));
            }
        }
    }

    pub fn finish(mut self) -> String {
        self.buffer.push('}');
        self.buffer.push('\n');
        self.buffer
    }
}
