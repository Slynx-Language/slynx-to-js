mod component;
mod functions;
use std::{collections::HashMap, ops::Deref};

pub use component::*;
pub use functions::*;
use slynx::slynx_ir::{
    IRPointer, IRSpecializedComponentType, IRType, Instruction, InstructionType, Operand, Slot,
    SlynxIR, StyleProperty, UIInstruction, Value, ValueKind,
};

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
    fn variables(&self) -> &HashMap<IRPointer<Slot, 1>, String>;
    fn variables_mut(&mut self) -> &mut HashMap<IRPointer<Slot, 1>, String>;
    fn resolve_label_arg(&self, _index: usize) -> String {
        unimplemented!("LabelArg not supported in this context")
    }
    /// Whether this compiler should materialize multi-use SSA instruction results into variables.
    /// When true, `compile_values` will detect `Component` instructions referenced more than once
    /// and emit `let vN = <expr>;` on first use, reusing the variable on subsequent uses.
    /// This is needed because some instructions (e.g. `document.createElement('p')`) create
    /// new objects on every evaluation — inlining them at multiple use-sites produces wrong output.
    fn should_materialize_instructions(&self) -> bool {
        false
    }
    /// Push raw code directly to the output buffer (used for emitting materialized variable declarations).
    fn emit_raw(&mut self, _code: String) {
        unimplemented!("emit_raw not implemented — enable should_materialize_instructions first")
    }
    /// Returns a map from instruction pointer to its materialized variable name.
    fn materialized_instructions(&self) -> &HashMap<IRPointer<Instruction, 1>, String> {
        unimplemented!("materialized_instructions not implemented")
    }
    /// Returns a mutable reference to the materialized instruction map.
    fn materialized_instructions_mut(&mut self) -> &mut HashMap<IRPointer<Instruction, 1>, String> {
        unimplemented!("materialized_instructions_mut not implemented")
    }

    ///Compiles the given `raw` operand. Operands are primitives
    fn compile_raw(&mut self, raw: &Operand, ir: &SlynxIR) -> String {
        match raw {
            Operand::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            Operand::Int(i) => i.to_string(),
            Operand::Float(f) => f.to_string(),
            Operand::String(ptr) => format!("\"{}\"", ir.string_pool().get_name(*ptr)),
        }
    }

    ///Compiles the given `values` and returns a vector with the string for each of them
    fn compile_values(&mut self, values: &[Value], ir: &SlynxIR) -> Vec<String> {
        let mut out = Vec::with_capacity(values.len());
        for val in values {
            let v = match val.deref() {
                ValueKind::Raw(v) => {
                    self.compile_raw(&ir.get_operand_by_pointer(v.clone().with_length())[0], ir)
                }
                ValueKind::FuncArg(n) => self.arguments()[*n].clone(),
                ValueKind::Instruction(ptr) => {
                    // If materialization is enabled, check if this instruction has already been
                    // materialized into a variable (it was referenced by a previous value).
                    if self.should_materialize_instructions() {
                        if let Some(var) = self.materialized_instructions().get(ptr) {
                            var.clone()
                        } else {
                            let inst =
                                &ir.get_instruction_by_pointer(ptr.clone().with_length())[0];
                            let expr = self.compile_instruction(inst, ir);
                            // Materialize Component instructions (createElement) into variables
                            // to prevent duplicate DOM element creation when the same instruction
                            // is referenced by multiple consumers (e.g. textContent + appendChild).
                            if matches!(inst.instruction_type, InstructionType::Component) {
                                let var_name = format!(
                                    "v{}",
                                    self.variables().len()
                                        + self.materialized_instructions().len()
                                        + 1
                                );
                                self.emit_raw(self.ident(format!("let {var_name} = {expr};\n")));
                                self.materialized_instructions_mut()
                                    .insert(ptr.clone(), var_name.clone());
                                var_name
                            } else {
                                expr
                            }
                        }
                    } else {
                        let inst =
                            &ir.get_instruction_by_pointer(ptr.clone().with_length())[0];
                        self.compile_instruction(inst, ir)
                    }
                }
                ValueKind::Slot(s) => self.variables().get(s).cloned().unwrap(),
                ValueKind::LabelArg(i) => self.resolve_label_arg(*i),
                ValueKind::Void => "null".to_string(),
                ValueKind::StructLiteral(ptr) => {
                    let args = self
                        .compile_values(ir.get_values_by_pointer(*ptr), ir)
                        .iter()
                        .enumerate()
                        .map(|(idx, arg)| format!("f{idx}: {arg}"))
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("{{{args}}}")
                }
                ValueKind::ComponentChild(n) => format!("c{n}"),
            };
            out.push(v);
        }
        out
    }

    ///Compiles the given `values`, knowing its a pointer 2 specifically 2 values.
    ///When compiling it will use the `operator` as an operator for it and `()` to certify
    ///the order is correct
    fn compile_binary(&mut self, values: IRPointer<Value>, operator: &str, ir: &SlynxIR) -> String {
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
    ///Compiles the a struct with the given `values`. The fields are named as `fN` where `N` is the index of the value, and thus, the field
    fn compile_struct_expression(&mut self, values: IRPointer<Value, 0>, ir: &SlynxIR) -> String {
        let values = ir.get_values_by_pointer(values);
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
    fn compile_allocation(&mut self, slot: IRPointer<Slot, 1>) -> String {
        let variable_name = format!("v{}", self.variables().len() + 1);
        let out = format!("let {variable_name};\n");
        self.variables_mut().insert(slot, variable_name);
        self.ident(out)
    }

    fn compile_write(
        &mut self,
        slot: IRPointer<Slot, 1>,
        value: IRPointer<Value, 0>,
        ir: &SlynxIR,
    ) -> String {
        debug_assert!(self.variables().contains_key(&slot));
        let values = ir.get_values_by_pointer(value);
        debug_assert!(value.len() == 1);
        let value = self.compile_values(values, ir);
        let variable = self.variables().get(&slot).unwrap();
        self.ident(format!("{variable} = {};\n", &value[0]))
    }

    fn compile_sapply(
        &mut self,
        ir: &SlynxIR,
        property_code: &StyleProperty,
        operands: IRPointer<Value>,
    ) -> String {
        let operands = ir.get_values_by_pointer(operands);
        let (component, value) = {
            let v = self.compile_values(operands, ir);
            (v[0].clone(), v[1].clone())
        };
        let component_field = format!("{component}.style");
        self.ident(match property_code {
            StyleProperty::BackgroundColor => format!(
                "{component_field}.backgroundColor = {};\n",
                format!("'#'+{value}.toString(16)")
            ),
            StyleProperty::ForegroundColor => format!(
                "{component_field}.color = {};\n",
                format!("'#'+{value}.toString(16)")
            ),
        })
    }

    fn compile_append_child(&self, args: &[String], _ir: &SlynxIR) -> String {
        let [parent, child] = args else {
            panic!("expected exactly 2 arguments");
        };
        self.ident(format!("{parent}.appendChild({child});\n"))
    }

    fn compile_field_access(&mut self, index: usize, values: &[Value], ir: &SlynxIR) -> String {
        assert!(values.len() == 2);
        let compiled_values = self.compile_values(values, ir);
        let field = match ir.ir_types().get_type(values[0].ir_type()) {
            IRType::Specialized(IRSpecializedComponentType::Div) => {
                return self.compile_append_child(&compiled_values, ir);
            }
            IRType::Specialized(IRSpecializedComponentType::Text) => "textContent".to_string(),
            _ => format!("f{index}"),
        };

        self.ident(format!(
            "{}.{field} = {};\n",
            compiled_values[0], compiled_values[1]
        ))
    }

    ///Compiles down the given `instruction`. This is basically recursive, since it must retrieve the values referenced by this `instruction`
    fn compile_instruction(&mut self, instruction: &Instruction, ir: &SlynxIR) -> String {
        match &instruction.instruction_type {
            InstructionType::RawValue => {
                let values = ir.get_values_by_pointer(instruction.operands.clone());
                let values = self.compile_values(values, ir);
                values.join(",")
            }
            InstructionType::Allocate(slot) => self.compile_allocation(*slot),
            InstructionType::Write(slot) => {
                self.compile_write(slot.clone(), instruction.operands, ir)
            }
            InstructionType::GetField(f) => {
                let values = ir.get_values_by_pointer(instruction.operands);
                assert!(values.len() == 1);
                let values = self.compile_values(values, ir);
                format!("{}.f{f}", values[0])
            }
            InstructionType::SetField(f) => {
                let values = ir.get_values_by_pointer(instruction.operands);
                self.compile_field_access(*f, values, ir)
            }
            InstructionType::Read => {
                let values = ir.get_values_by_pointer(instruction.operands);
                assert!(values.len() == 1);
                let mut values = self.compile_values(values, ir);
                values.swap_remove(0)
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
            InstructionType::Struct => self.compile_struct_expression(instruction.operands, ir),
            InstructionType::Ret => {
                let operand = self
                    .compile_values(ir.get_values_by_pointer(instruction.operands.clone()), ir)
                    .join(",");
                format!("return {operand};")
            }
            InstructionType::FunctionCall(ctx)
            | InstructionType::UI(UIInstruction::InitCall(ctx)) => {
                let ctx = &ir.contexts()[ctx.ptr()];
                let name = ir.string_pool().get_name(ctx.name());
                let args = ir.get_values_by_pointer(instruction.operands);
                let args = self.compile_values(args, ir).join(",");
                format!("{name}({args})")
            }
            InstructionType::Component => {
                let t = instruction.value_type;

                match ir.ir_types().get_type(t) {
                    IRType::Component(component_id) => {
                        let component = ir.ir_types().get_component_type(component_id);
                        let name = ir.string_pool().get_name(component.name());
                        let operands = self
                            .compile_values(ir.get_values_by_pointer(instruction.operands), ir)
                            .join(",");
                        format!("{name}({operands})")
                    }
                    IRType::Specialized(IRSpecializedComponentType::Div) => {
                        "document.createElement('div')".to_string()
                    }
                    IRType::Specialized(IRSpecializedComponentType::Text) => {
                        "document.createElement('p')".to_string()
                    }
                    _ => unreachable!(
                        "Type of component should be a component, or specialized component"
                    ),
                }
            }
            InstructionType::Br(_) => "".to_string(),
            InstructionType::Cbr { .. } => "".to_string(),
            InstructionType::Reinterpret => "".to_string(),
            InstructionType::UI(UIInstruction::SApply { property_code }) => {
                self.compile_sapply(ir, property_code, instruction.operands)
            }
        }
    }
}
