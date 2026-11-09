mod component;
mod functions;
use std::collections::HashMap;

pub use component::*;
pub use functions::*;
use slynx::slynx_ir::{IRType, IRTypeId, Opcode, Operand, SlynxIR, Value};

/// Returns the JS identifier used to store an internal (non-extern) global.
pub fn js_global_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    format!("global_{sanitized}")
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
    fn resolve_materialized(&self, _value: Value) -> Option<String> {
        None
    }
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

    /// Produces a JS expression that represents the zero value of `ty`.
    fn zeroed_value(&self, ty: IRTypeId, ir: &SlynxIR) -> String {
        match ir.types.get_type(ty) {
            IRType::I(_)
            | IRType::U(_)
            | IRType::ISIZE
            | IRType::USIZE
            | IRType::F16
            | IRType::F32
            | IRType::F64 => "0".to_string(),
            IRType::BOOL => "false".to_string(),
            IRType::STR => "\"\"".to_string(),
            IRType::VOID => "null".to_string(),
            IRType::Array(inner, len) => {
                let value = self.zeroed_value(*inner, ir);
                format!("[{}]", vec![value; *len].join(","))
            }
            IRType::Vector(_) => "[]".to_string(),
            IRType::Struct(struct_id) => {
                let fields = ir.types.get_object_type(*struct_id).get_fields();
                let fields = fields
                    .iter()
                    .enumerate()
                    .map(|(idx, field)| format!("f{idx}: {}", self.zeroed_value(*field, ir)))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{{{fields}}}")
            }
            IRType::Component(component_id) => {
                let fields = ir.types.get_component_type(*component_id).fields();
                let fields = fields
                    .iter()
                    .enumerate()
                    .map(|(idx, field)| format!("f{idx}: {}", self.zeroed_value(*field, ir)))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{{{fields}}}")
            }
            IRType::Union(union_id) => {
                let variants = ir.types.get_union_type(*union_id).get_variants();
                match variants.first() {
                    Some(variant) => self.zeroed_value(*variant, ir),
                    None => "null".to_string(),
                }
            }
            IRType::Pointer(_)
            | IRType::Function(_)
            | IRType::GenericComponent
            | IRType::Specialized(_) => "null".to_string(),
        }
    }

    fn compile_field_read(&mut self, parent: Value, index: usize, ir: &SlynxIR) -> String {
        let ty = ir.get_instruction(parent).value_type;
        let parent = self.compile_value(parent, ir);

        match () {
            _ if ty == ir.types.specialized_text_type() => {
                format!("{parent}.textContent")
            }
            _ if ty == ir.types.specialized_div_type() => {
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
            _ if ty == ir.types.specialized_div_type() => {
                format!("{read}({value});\n")
            }
            _ => format!("{read} = {value};\n"),
        }
    }

    fn compile_component(&mut self, instruction: Value, name: &str, ir: &SlynxIR) -> String {
        let instruction = ir.get_instruction(instruction);

        match () {
            _ if ir.types.specialized_text_type() == instruction.value_type => {
                let values = &instruction.operands;
                let creation = self.ident(format!("let {name} = document.createElement(\"p\");\n"));
                if values.is_empty() {
                    creation
                } else {
                    debug_assert_eq!(values.len(), 1);
                    let expr = &self.compile_values(values, ir)[0];
                    let write = self.ident(format!("{name}.textContent = {expr};\n"));
                    format!("{creation}{write}")
                }
            }
            _ if ir.types.specialized_div_type() == instruction.value_type => {
                let mut out =
                    self.ident(format!("let {name} = document.createElement(\"div\");\n"));

                for child_val in instruction.operands.iter() {
                    let child_expr = self.compile_value(*child_val, ir);
                    out.push_str(&self.ident(format!("{name}.appendChild({child_expr});\n")))
                }
                out
            }

            _ if let IRType::Component(component_id) =
                ir.types.get_type(instruction.value_type) =>
            {
                let component = ir.types.get_component_type(*component_id);
                let name = ir.get_name(component.name());
                let operands = self.compile_values(&instruction.operands, ir).join(",");
                format!("{name}({operands})")
            }

            _ => unreachable!("Type of component instruction should be a component"),
        }
    }

    ///Compiles down the given `instruction`. This is basically recursive, since it must retrieve the values referenced by this `instruction`
    fn compile_value(&mut self, value: Value, ir: &SlynxIR) -> String {
        if let Some(materialized) = self.resolve_materialized(value) {
            return materialized;
        }
        if value.is_void() {
            return String::new();
        }
        let instruction = ir.get_instruction(value);
        match &instruction.opcode {
            Opcode::FieldRef(index) => {
                let parent = self.compile_value(instruction.operands[0], ir);
                format!("{parent}.f{index}")
            }
            Opcode::DerefWrite => {
                let values = self.compile_values(&instruction.operands, ir);
                self.ident(format!("{} = {};\n", values[0], values[1]))
            }
            Opcode::Move | Opcode::Cast | Opcode::Copy | Opcode::Ref | Opcode::Deref => {
                self.compile_value(instruction.operands[0], ir)
            }

            Opcode::GetChild(index) => {
                let parent = self.compile_value(instruction.operands[0], ir);
                format!("{parent}.c{index}")
            }
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
            Opcode::DynGetField(name) => {
                let parent = self.compile_value(instruction.operands[0], ir);
                format!("{parent}.{}", ir.get_name(*name))
            }
            Opcode::DynSetField(name) => {
                let values = self.compile_values(&instruction.operands, ir);
                self.ident(format!("{}.{} = {};\n", values[0], ir.get_name(*name), values[1]))
            }
            Opcode::DynMethodCall(name) => {
                let values = self.compile_values(&instruction.operands, ir);
                let object = &values[0];
                let args = values[1..].join(",");
                format!("{object}.{}({args})", ir.get_name(*name))
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
            Opcode::Array | Opcode::Vector => {
                let values = self.compile_values(&instruction.operands, ir).join(",");
                format!("[{values}]")
            }
            Opcode::ArrayGet => {
                let values = self.compile_values(&instruction.operands, ir);
                format!("{}[{}]", values[0], values[1])
            }
            Opcode::Zeroed => self.zeroed_value(instruction.value_type, ir),
            Opcode::Global(global) => {
                let global = ir.get_view(*global);
                js_global_name(global.raw_name())
            }
            Opcode::GlobalExtern(name) => ir.get_name(*name).to_string(),
            Opcode::Ret => {
                let operand = self.compile_value(instruction.operands[0], ir);
                format!("return {operand};")
            }
            Opcode::Call(fptr) => {
                let name = ir.get_view(*fptr).get_name().to_string();
                let args = self.compile_values(&instruction.operands, ir).join(",");
                format!("{name}({args})")
            }
            Opcode::InitCall(fptr) => {
                let name = ir.get_view(*fptr).get_name().to_string();
                let args = self.compile_values(&instruction.operands, ir).join(",");
                self.ident(format!("{name}({args});\n"))
            }

            Opcode::Component => {
                let inst = ir.get_instruction(value);
                let is_specialized = inst.value_type == ir.types.specialized_text_type()
                    || inst.value_type == ir.types.specialized_div_type();
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
        }
    }
}
