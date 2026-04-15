use crate::{JSBuffer, JSFunction};
use color_eyre::eyre::Result;
use slynx::middleend::{Context, IRType, Label, SlynxIR};
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

        let mut func = self.buffer.create_function(name, ty.get_args().len() as u8);
        let mut label = ir.get_context_labels(ctx).iter();

        if let Some(label) = label.next() {
            self.compile_entry_label(label, ir, &mut func);
        }

        for lbl in label {
            self.compile_label(lbl, ir, &mut func);
        }
        self.buffer.append_function(func);
    }

    pub fn compile_label(&mut self, lbl: &Label, ir: &SlynxIR, func: &mut JSFunction) {
        for instruction in ir.get_label_instructions(lbl) {
            func.compile_instruction(instruction, lbl, ir);
        }
    }

    pub fn compile_entry_label(&mut self, lbl: &Label, ir: &SlynxIR, func: &mut JSFunction) {
        self.compile_label(lbl, ir, func);
    }
}
