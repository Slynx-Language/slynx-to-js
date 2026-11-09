use crate::{InstructionCompiler, JSBuffer, JSComponent, JSFunction, js_global_name};
use color_eyre::eyre::Result;
use slynx::slynx_ir::{Component, Function, GlobalValue, IRStorage, IRType, InitValue, SlynxIR};
pub struct JsCompiler {
    buffer: JSBuffer,
}

impl JsCompiler {
    pub fn main_func_ret_component(ir: &SlynxIR) -> bool {
        for func in (ir as &dyn IRStorage<Function>).get_storage() {
            if ir.get_name(func.name()) == "main" {
                let IRType::Function(ty) = ir.types.get_type(func.ty()) else {
                    //this is a bug, so we just ignore
                    continue;
                };
                let ret = ir.types.get_function_type(*ty).get_return_type();
                match ir.types.get_type(ret) {
                    IRType::Component(_) | IRType::GenericComponent => return true,
                    _ => continue,
                }
            }
        }
        false
    }

    pub fn compile(ir: SlynxIR) -> Result<String> {
        let mut s = Self {
            buffer: JSBuffer::new(),
        };

        s.compile_globals(&ir);

        for ctx in (&ir as &dyn IRStorage<Function>).get_storage() {
            if ctx.is_external() {
                continue;
            }
            s.compile_context(ctx, &ir);
        }
        for comp in ir.get_storage() {
            s.compile_component(comp, &ir);
        }

        Ok(s.buffer.content)
    }

    pub fn compile_globals(&mut self, ir: &SlynxIR) {
        let mut init = JSFunction::new(String::new(), Vec::new());
        for global in ir.view_all::<GlobalValue>().iter() {
            let name = js_global_name(global.raw_name());
            let value = match global.value().initial_value() {
                InitValue::ZeroInit(ty) => init.zeroed_value(ty, ir),
                InitValue::Constant(value) => init.compile_value(value, ir),
                InitValue::Lazy(func) => {
                    format!("{}()", ir.get_view(func.with_length()).get_name())
                }
            };
            self.buffer
                .content
                .push_str(&format!("let {name} = {value};\n"));
        }
    }

    pub fn compile_context(&mut self, ctx: &Function, ir: &SlynxIR) {
        let name = ctx.name();
        let name = ir.get_name(name);
        let IRType::Function(ty) = ir.types.get_type(ctx.ty()) else {
            unreachable!();
        };
        let ty = ir.types.get_function_type(*ty);

        let mut func = self.buffer.create_function(name, ty.get_args().len() as u8);
        let cfg = ir.generate_function_cfg(ctx);
        func.compile_from_cfg(&cfg, ir);
        self.buffer.append_function(func);
    }

    pub fn compile_component(&mut self, component: &Component, ir: &SlynxIR) {
        let ty = component.ir_type();
        let IRType::Component(component_id) = ir.types.get_type(ty) else {
            unreachable!("Type of component should be an ir component");
        };
        let component_type = ir.types.get_component_type(*component_id);
        let mut js_component = JSComponent::new(ir.get_name(component_type.name()));
        js_component.compile(
            component_type.fields(),
            component_type.children(),
            component.child_values(),
            component,
            ir,
        );
        self.buffer.append_component(js_component);
    }
}
