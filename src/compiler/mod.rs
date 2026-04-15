use crate::JSBuffer;
use color_eyre::eyre::Result;
use slynx::middleend::{Context, IRType, SlynxIR};
use std::path::PathBuf;

pub struct JsCompiler {
    buffer: JSBuffer,
}

impl JsCompiler {
    pub fn compile(ir: SlynxIR, path: PathBuf) -> Result<()> {
        let mut s = Self {
            buffer: JSBuffer::new(),
        };

        for ctx in ir.contexts() {
            s.compile_context(ctx, &ir);
        }

        std::fs::write(path, &s.buffer.content)?;
        Ok(())
    }

    pub fn compile_context(&mut self, ctx: &Context, ir: &SlynxIR) {
        let name = ctx.name();
        let name = ir.string_pool().get_name(name);
        let types = ir.ir_types();
        let IRType::Function(ty) = types.get_type(ctx.ty()) else {
            unreachable!();
        };
        let ty = types.get_function_type(ty);

        self.buffer.write_function(name, ty.get_args().len() as u8);
        for label in ir.get_context_labels(ctx) {}
    }
}
