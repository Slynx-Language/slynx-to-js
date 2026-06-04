mod component;
mod functions;
use std::collections::HashMap;

pub use component::*;
pub use functions::*;
use slynx::slynx_ir::{IRType, Opcode, Operand, SlynxIR, StyleProperty, Value};

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

        JSFunction::new(format!("function {name}({args}){{\n"), args_vec)
    }

    pub fn append_function(&mut self, func: JSFunction) {
        self.content.push_str(&func.finish());
    }
    pub fn append_component(&mut self, comp: JSComponent) {
        self.content.push_str(&comp.finish());
    }
}

pub trait InstructionCompiler {
    fn identation_multiplier() -> usize {
        4
    }
    fn identation_value(&self) -> usize;
    fn increase_identation(&mut self);
    fn decrease_identation(&mut self);
    fn identation_string(&self) -> String {
        " ".repeat(Self::identation_multiplier() * self.identation_value())
    }
    fn ident(&self, name: String) -> String {
        format!(
            "{}{name}",
            " ".repeat(Self::identation_multiplier() * self.identation_value())
        )
    }
    fn arguments(&self) -> &Vec<String>;
    fn variables(&self) -> &HashMap<Value, String>;
    fn variables_mut(&mut self) -> &mut HashMap<Value, String>;
    fn append_content(&mut self, content: &str);
    fn next_variable_name(&self) -> String {
        format!("v{}", self.variables().len())
    }
    fn resolve_label_arg(&self, _index: usize) -> String {
        unimplemented!("LabelArg not supported in this context")
    }
    ///Compiles the given `raw` operand. Operands are primitives
    fn compile_raw(&mut self, raw: &Operand, ir: &SlynxIR) -> String {
        match raw {
            Operand::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            Operand::Int(i) => i.to_string(),
            Operand::Float(f) => f.to_string(),
            Operand::String(ptr) => format!("\"{}\"", ir.get_name(*ptr)),
        }
    }

    ///Compiles the given `values` and returns a vector with the string for each of them
    fn compile_values(&mut self, values: &[Value], ir: &SlynxIR) -> Vec<String> {
        let mut out = Vec::with_capacity(values.len());
        for val in values {
            let v = self.compile_value(*val, ir);
            out.push(v);
        }
        out
    }

    ///Compiles the given `values`, knowing its a pointer 2 specifically 2 values.
    ///When compiling it will use the `operator` as an operator for it and `()` to certify
    ///the order is correct
    fn compile_binary(&mut self, values: &[Value], operator: &str, ir: &SlynxIR) -> String {
        debug_assert_eq!(values.len(), 2);
        let values = self.compile_values(values, ir);
        let mut out = String::from("(");
        out.push_str(&values[0]);
        out.push_str(operator);
        out.push_str(&values[1]);
        out.push(')');
        out
    }
    ///Compiles the a struct with the given `values`. The fields are named as `fN` where `N` is the index of the value, and thus, the field
    fn compile_struct_expression(&mut self, values: &[Value], ir: &SlynxIR) -> String {
        let values = self
            .compile_values(values, ir)
            .into_iter()
            .enumerate()
            .map(|(idx, value)| format!("f{idx}: {value}"))
            .collect::<Vec<_>>()
            .join(",");
        format!("{{{values}}}")
    }

    ///Compiles an allocation that maps to the given `slot`, and maps the name of the variable to it
    fn compile_allocation(&mut self, slot: Value) -> String {
        let variable_name = format!("v{}", self.variables().len() + 1);
        let out = format!("let {variable_name};\n");
        self.variables_mut().insert(slot, variable_name.clone());
        self.ident(out)
    }

    /// Look up an already-allocated slot's variable name (no re-declaration).
    fn resolve_allocation(&self, slot: Value) -> String {
        self.variables()
            .get(&slot)
            .cloned()
            .unwrap_or_else(|| panic!("slot {:?} not allocated", slot))
    }

    fn compile_write(&mut self, slot: Value, value: Value, ir: &SlynxIR) -> String {
        debug_assert!(self.variables().contains_key(&slot));
        let value = self.compile_value(value, ir);
        let variable = self.variables().get(&slot).unwrap();
        self.ident(format!("{variable} = {value};\n"))
    }

    fn compile_sapply(
        &mut self,
        property: &StyleProperty,
        values: &[Value],
        ir: &SlynxIR,
    ) -> String {
        let component = self.compile_value(values[0], ir);
        let value = self.compile_value(values[1], ir);
        let partial_content = format!("{component}.style");
        match property {
            StyleProperty::BackgroundColor => self.ident(format!(
                "{partial_content}.backgroundColor = '#'+{value}.toString(16).padStart(6, '0');\n"
            )),
            StyleProperty::ForegroundColor => self.ident(format!(
                "{partial_content}.color = '#'+{value}.toString(16).padStart(6, '0');\n"
            )),
        }
    }
    fn compile_field_read(&mut self, parent: Value, index: usize, ir: &SlynxIR) -> String {
        let ty = ir.get_instruction(parent).value_type;
        let parent = self.compile_value(parent, ir);

        match () {
            _ if ty == ir.specialized_text_type() => {
                format!("{parent}.textContent")
            }
            _ if ty == ir.specialized_div_type() => {
                format!("{parent}.appendChild")
            }
            _ => format!("{parent}.f{index}"),
        }
    }
    fn compile_field_write(
        &mut self,
        parent: Value,
        index: usize,
        value: Value,
        ir: &SlynxIR,
    ) -> String {
        let ty = ir.get_instruction(parent).value_type;
        let read = self.compile_field_read(parent, index, ir);
        let value = self.compile_value(value, ir);
        match () {
            _ if ty == ir.specialized_div_type() => {
                format!("{read}({value});\n")
            }
            _ => format!("{read} = {value};\n"),
        }
    }

    fn compile_component(&mut self, instruction: Value, name: &str, ir: &SlynxIR) -> String {
        let instruction = ir.get_instruction(instruction);

        match () {
            _ if ir.specialized_text_type() == instruction.value_type => {
                let values = &instruction.operands;
                debug_assert_eq!(values.len(), 1);
                let expr = &self.compile_values(values, ir)[0];
                let creation = self.ident(format!("let {name} = document.createElement(\"p\");\n"));
                let write = self.ident(format!("{name}.textContent = {expr};\n"));
                format!("{creation}{write}")
            }
            _ if ir.specialized_div_type() == instruction.value_type => {
                let mut out =
                    self.ident(format!("let {name} = document.createElement(\"div\");\n"));

                for child_val in instruction.operands.iter() {
                    let child_expr = self.compile_value(*child_val, ir);
                    out.push_str(&self.ident(format!("{name}.appendChild({child_expr});\n")))
                }
                out
            }

            _ if let IRType::Component(component_id) = ir.get_type(instruction.value_type) => {
                let component = ir.get_component_type(component_id);
                let name = ir.get_name(component.name());
                let operands = self.compile_values(&instruction.operands, ir).join(",");
                format!("{name}({operands})")
            }

            _ => unreachable!("Type of component instruction should be a component"),
        }
    }

    ///Compiles down the given `instruction`. This is basically recursive, since it must retrieve the values referenced by this `instruction`
    fn compile_value(&mut self, value: Value, ir: &SlynxIR) -> String {
        let instruction = ir.get_instruction(value);
        match &instruction.opcode {
            Opcode::GetChild(_) => unimplemented!("Get child not implement yet"),
            Opcode::BlockParam(i) => self.resolve_label_arg(*i as usize),
            Opcode::Arg(idx) => self.arguments()[*idx as usize].clone(),
            Opcode::Const(c) => self.compile_raw(c, ir),
            Opcode::RawValue => self.compile_values(&instruction.operands, ir).join(","),
            Opcode::Allocate => {
                if self.variables().contains_key(&value) {
                    self.resolve_allocation(value)
                } else {
                    self.compile_allocation(value)
                }
            }
            Opcode::Write => {
                self.compile_write(instruction.operands[0], instruction.operands[1], ir)
            }
            Opcode::GetField(f) => {
                assert!(instruction.operands.len() == 1);
                let out = self.compile_field_read(instruction.operands[0], *f as usize, ir);
                out
            }
            Opcode::SetField(f) => {
                assert!(instruction.operands.len() == 2);
                let out = self.compile_field_write(
                    instruction.operands[0],
                    *f as usize,
                    instruction.operands[1],
                    ir,
                );
                self.ident(out)
            }

            Opcode::Read => {
                assert!(instruction.operands.len() == 1);
                self.variables()
                    .get(&instruction.operands[0])
                    .cloned()
                    .unwrap_or_else(|| {
                        panic!("Read: slot {:?} not allocated", instruction.operands[0])
                    })
            }
            Opcode::Add => self.compile_binary(&instruction.operands, "+", ir),
            Opcode::Sub => self.compile_binary(&instruction.operands, "-", ir),
            Opcode::Mul => self.compile_binary(&instruction.operands, "*", ir),
            Opcode::Div => self.compile_binary(&instruction.operands, "/", ir),
            Opcode::Cmp => self.compile_binary(&instruction.operands, "==", ir),
            Opcode::Gt => self.compile_binary(&instruction.operands, ">", ir),
            Opcode::Lt => self.compile_binary(&instruction.operands, "<", ir),
            Opcode::Lte => self.compile_binary(&instruction.operands, "<=", ir),
            Opcode::Gte => self.compile_binary(&instruction.operands, ">=", ir),
            Opcode::And => self.compile_binary(&instruction.operands, "&", ir),
            Opcode::Or => self.compile_binary(&instruction.operands, "|", ir),
            Opcode::Xor => self.compile_binary(&instruction.operands, "^", ir),
            Opcode::Shr => self.compile_binary(&instruction.operands, ">>", ir),
            Opcode::AShr => self.compile_binary(&instruction.operands, ">>>", ir),
            Opcode::Shl => self.compile_binary(&instruction.operands, "<<", ir),
            Opcode::Struct => self.compile_struct_expression(&instruction.operands, ir),
            Opcode::Ret => {
                let operand = self.compile_value(instruction.operands[0], ir);
                format!("return {operand};")
            }
            Opcode::Call(fptr) | Opcode::InitCall(fptr) => {
                let func_view = ir.get_view(*fptr);
                let name = func_view.get_name();
                let args = self.compile_values(&instruction.operands, ir).join(",");
                if instruction.value_type == ir.void_type() {
                    self.ident(format!("{name}({args});\n"))
                } else {
                    format!("{name}({args})")
                }
            }
            Opcode::SApply { property_code } => {
                self.compile_sapply(property_code, &instruction.operands, ir)
            }
            Opcode::Component => {
                let inst = ir.get_instruction(value);
                let is_specialized = inst.value_type == ir.specialized_text_type()
                    || inst.value_type == ir.specialized_div_type();
                if is_specialized {
                    if self.variables().contains_key(&value) {
                        self.resolve_allocation(value)
                    } else {
                        let name = self.next_variable_name();
                        self.variables_mut().insert(value, name.clone());
                        let out = self.compile_component(value, &name, ir);
                        self.append_content(&out);
                        name
                    }
                } else {
                    self.compile_component(value, "", ir)
                }
            }
            Opcode::Br(_) => "".to_string(),
            Opcode::Cbr { .. } => "".to_string(),
            Opcode::Reinterpret => "".to_string(),
        }
    }
}
