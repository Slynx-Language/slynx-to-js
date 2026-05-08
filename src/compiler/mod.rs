use crate::{JSBuffer, JSComponent};
use color_eyre::eyre::Result;
use slynx::slynx_ir::{
    IRType,
    ir::{Context, SlynxIR},
};

pub struct JsCompiler {
    buffer: JSBuffer,
}

impl JsCompiler {
    pub fn compile(ir: SlynxIR) -> Result<String> {
        let mut s = Self {
            buffer: JSBuffer::new(),
        };
        for comp in ir.components() {
            s.compile_component(comp, &ir);
        }
        for ctx in ir.contexts() {
            s.compile_context(ctx, &ir);
        }

        let main_returns_component = ir.contexts().iter().any(|ctx| {
            ir.string_pool().get_name(ctx.name()) == "main" && {
                let IRType::Function(fid) = ir.ir_types().get_type(ctx.ty()) else {
                    return false;
                };
                matches!(
                    ir.ir_types()
                        .get_type(ir.ir_types().get_function_type(fid).get_return_type()),
                    IRType::GenericComponent | IRType::Component(_)
                )
            }
        });

        if main_returns_component {
            s.buffer.content.push_str(include_str!("../../js/start.js"));
        }

        println!("Final output:\n{}", s.buffer.content);

        Ok(s.buffer.content)
    }

    pub fn compile_context(&mut self, ctx: &Context, ir: &SlynxIR) {
        let name = ctx.name();
        let name = ir.string_pool().get_name(name);
        let types = ir.ir_types();
        let IRType::Function(ty) = types.get_type(ctx.ty()) else {
            unreachable!();
        };
        let ty = types.get_function_type(ty);

        let mut func = self.buffer.create_function(name, ty.get_args().len() as u8);
        let cfg = ir.generate_context_cfg(ctx);
        func.compile_from_cfg(&cfg, ir);
        self.buffer.append_function(func);
    }

    pub fn compile_component(&mut self, component: &slynx::slynx_ir::Component, ir: &SlynxIR) {
        let ty = component.ir_type();
        let types = ir.ir_types();
        let IRType::Component(component_id) = types.get_type(ty) else {
            unreachable!("Type of component should be an ir component");
        };
        let component_type = types.get_component_type(component_id);
        let mut js_component = JSComponent::new(ir.string_pool().get_name(component_type.name()));
        js_component.compile(component_type.fields(), component.values(), ir);
        self.buffer.append_component(js_component);
    }
}
