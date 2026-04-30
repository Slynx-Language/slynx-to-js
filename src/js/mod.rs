mod component;
mod functions;
use std::collections::HashMap;

pub use component::*;
pub use functions::*;
use slynx::middleend::{
    IRPointer, IRType, Instruction, InstructionType, Operand, Slot, SlynxIR, Value,
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
            let v = match val {
                Value::Raw(v) => {
                    self.compile_raw(&ir.get_operand_by_pointer(v.clone().with_length())[0], ir)
                }
                Value::FuncArg(n) => self.arguments()[*n].clone(),
                Value::Instruction(ptr) => {
                    let inst = &ir.get_instruction_by_pointer(ptr.clone().with_length())[0];
                    self.compile_instruction(inst, ir)
                }
                Value::Slot(s) => self.variables().get(s).cloned().unwrap(),
                Value::LabelArg(i) => self.resolve_label_arg(*i),
                u => unimplemented!("Not implemented {u:?}"),
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
                let v = self.compile_values(values, ir);
                format!("{}.f{f}", v[0])
            }
            InstructionType::SetField(f) => {
                let values = ir.get_values_by_pointer(instruction.operands);
                assert!(values.len() == 2);
                let v = self.compile_values(values, ir);
                self.ident(format!("{}.f{f} = {};\n", v[0], v[1]))
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
            InstructionType::FunctionCall(ctx) => {
                let ctx = &ir.contexts()[ctx.ptr()];
                let name = ir.string_pool().get_name(ctx.name());
                let args = ir.get_values_by_pointer(instruction.operands);
                let args = self.compile_values(args, ir).join(",");
                format!("{}({})", name, args)
            }
            InstructionType::Component => {
                let t = instruction.value_type;
                let IRType::Component(component_id) = ir.ir_types().get_type(t) else {
                    unreachable!("Expected type of component instruction to be a component")
                };
                let component = ir.ir_types().get_component_type(component_id);
                let name = ir.string_pool().get_name(component.name());
                let operands =
                    self.compile_values(ir.get_values_by_pointer(instruction.operands), ir);
                format!("{name}({})", operands.join(","))
            }
            InstructionType::Br(_) => "".to_string(),
            InstructionType::Cbr { .. } => "".to_string(),
            InstructionType::Reinterpret => "".to_string(),
        }
    }
}
