use std::collections::HashMap;

use slynx::slynx_ir::{Component, IRTypeId, Opcode, SlynxIR, Value};

use crate::js::InstructionCompiler;

pub struct JSComponent {
    name: String,
    buffer: String,
    variables: HashMap<Value, String>,
    child_var_map: HashMap<Value, String>,
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
    fn variables(&self) -> &HashMap<Value, String> {
        &self.variables
    }
    fn variables_mut(&mut self) -> &mut HashMap<Value, String> {
        &mut self.variables
    }
    fn append_content(&mut self, content: &str) {
        self.buffer.push_str(content);
    }
}

impl JSComponent {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            buffer: String::new(),
            arguments: Vec::new(),
            child_var_map: HashMap::new(),
            variables: HashMap::new(),
            identation: 0,
        }
    }

    pub fn compile(
        &mut self,
        fields: &[IRTypeId],
        child_types: &[IRTypeId],
        children: &[Value],
        component: &Component,
        ir: &SlynxIR,
    ) {
        let params: Vec<String> = (0..fields.len() + child_types.len())
            .map(|i| format!("p{i}"))
            .collect();
        self.arguments = params.clone();
        self.buffer.push_str(&format!("function {}(", self.name));
        self.buffer.push_str(&params.join(","));
        self.buffer.push_str("){\n");
        self.increase_identation();

        // Phase 1: create child elements, track variable names
        for (idx, child_val) in children.iter().enumerate() {
            let var_name = format!("c{idx}");
            self.compile_child(child_val, &var_name, ir, &params);
            self.child_var_map.insert(*child_val, var_name);
        }

        // Phase 2: emit init calls from the component preamble
        for i in component.instructions().range() {
            let inst_val = Value::instruction(i as u32);
            let inst = ir.get_instruction(inst_val);
            if let Opcode::InitCall(fptr) = &inst.opcode {
                let args = inst
                    .operands
                    .iter()
                    .map(|arg| {
                        if let Some(var_name) = self.child_var_map.get(arg) {
                            var_name.clone()
                        } else {
                            self.compile_value(*arg, ir)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let func_view = ir.get_view(*fptr);
                let name = func_view.get_name();
                self.buffer
                    .push_str(&self.ident(format!("{name}({args});\n")));
            }
        }

        let static_children = (0..children.len())
            .map(|i| {
                if i < child_types.len() {
                    format!("ui{i}: c{i}")
                } else {
                    format!("c{i}: c{i}")
                }
            })
            .collect::<Vec<_>>();
        let field_values = (0..fields.len()).map(|i| format!("f{i}: p{i}"));
        let component_children =
            (0..child_types.len()).map(|i| format!("c{i}: p{}", fields.len() + i));
        let ret_fields = static_children
            .into_iter()
            .chain(field_values)
            .chain(component_children)
            .collect::<Vec<_>>()
            .join(",");
        self.buffer
            .push_str(&self.ident(format!("return {{{ret_fields}}};\n",)));
    }

    fn compile_child(&mut self, value: &Value, var_name: &str, ir: &SlynxIR, params: &[String]) {
        let instruction = ir.get_instruction(*value);
        assert!(matches!(instruction.opcode, Opcode::Component));
        let out = match () {
            _ if ir.types.specialized_text_type() == instruction.value_type => {
                let values = &instruction.operands;
                if values.is_empty() {
                    // declaration-level child reference; content set by init function
                    format!("let {var_name} = document.createElement(\"p\");\n")
                } else {
                    assert_eq!(values.len(), 1);
                    let expr = &self.compile_values(values, ir)[0];
                    format!(
                        "let {var_name} = document.createElement(\"p\");\n {}",
                        self.ident(format!("{var_name}.textContent = {expr};\n")),
                    )
                }
            }
            _ if ir.types.specialized_div_type() == instruction.value_type => {
                let mut out = format!("let {var_name} = document.createElement(\"div\");\n",);

                for (i, child_val) in instruction.operands.iter().enumerate() {
                    let child_var = format!("{var_name}_c{i}");
                    self.compile_child(child_val, &child_var, ir, params);
                    out.push_str(&self.ident(format!("{var_name}.appendChild({child_var});\n")))
                }
                out
            }

            _ => {
                let expr = &self.compile_values(&[value.clone()], ir)[0];
                format!("let {var_name} = {expr};\n")
            }
        };
        self.buffer.push_str(&self.ident(out));
    }

    pub fn finish(mut self) -> String {
        self.buffer.push('}');
        self.buffer.push('\n');
        self.buffer
    }
}
