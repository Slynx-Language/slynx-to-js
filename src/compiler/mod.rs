use crate::{
    InstructionCompiler, JSBuffer, JSComponent, JSFunction,
    cfg::{CFG, CFGBlock},
};
use color_eyre::eyre::Result;
use slynx::middleend::{
    IRPointer, IRType,
    ir::{Context, InstructionType, SlynxIR, Value},
};
use std::collections::HashSet;
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
        for comp in ir.components() {
            s.compile_component(comp, &ir);
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

        // Build CFG for this context
        let cfg = CFG::from_context(ctx, ir);

        // Track which blocks have been compiled to avoid re-compilation
        let mut compiled = HashSet::new();

        // Start compilation from the entry block
        self.compile_cfg_block(*cfg.get_entry(), &cfg, ir, &mut func, &mut compiled);

        self.buffer.append_function(func);
    }

    /// Compile a single CFG block and handle its branch
    fn compile_cfg_block(
        &mut self,
        idx: usize,
        cfg: &CFG,
        ir: &SlynxIR,
        func: &mut JSFunction,
        compiled: &mut HashSet<usize>,
    ) {
        // Avoid infinite loops - skip if already compiled
        if compiled.contains(&idx) {
            return;
        }
        compiled.insert(idx);

        let block = cfg.get_block(&idx).unwrap();

        // Compile all non-branch instructions in this block
        for inst in &block.instructions {
            let result = func.compile_instruction(inst, ir);
            if !result.is_empty() {
                func.append(result);
            }
        }

        // Get the final branch instruction (if any) by looking at the original instructions
        let label = cfg.get_label(&idx);
        let all_instructions = ir.get_label_instructions(label);
        let final_branch = all_instructions
            .iter()
            .filter_map(|list| list.last())
            .last();

        match final_branch.map(|inst| &inst.instruction_type) {
            Some(InstructionType::Br(_target)) => {
                self.handle_unconditional_branch(idx, &block, cfg, ir, func, compiled);
            }
            Some(InstructionType::Cbr {
                then_label: _,
                else_label: _,
                then_args,
                else_args,
            }) => {
                let condition_values =
                    ir.get_values_by_pointer(final_branch.unwrap().operands.clone());
                self.handle_conditional_branch(
                    condition_values,
                    idx,
                    *then_args,
                    *else_args,
                    cfg,
                    ir,
                    func,
                    compiled,
                );
            }
            _ => {
                // No branch - block ends naturally
                // Check if we need to compile successors that haven't been compiled
                for succ in &block.successors {
                    if !compiled.contains(succ) {
                        self.compile_cfg_block(*succ, cfg, ir, func, compiled);
                    }
                }
            }
        }
    }

    /// Handle unconditional branch (br)
    fn handle_unconditional_branch(
        &mut self,
        _source_idx: usize,
        source_block: &CFGBlock,
        cfg: &CFG,
        ir: &SlynxIR,
        func: &mut JSFunction,
        _compiled: &mut HashSet<usize>,
    ) {
        // Get the target index from successors
        let target_idx = if !source_block.successors.is_empty() {
            source_block.successors[0]
        } else {
            return;
        };

        // Get arguments passed by this branch
        if let Some(args) = source_block.branch_args.get(&target_idx) {
            // Assign arguments to variables
            for (i, arg) in args.iter().enumerate() {
                let arg: Value = arg.clone();
                let arg_strs = func.compile_values(&[arg], ir);
                if !arg_strs.is_empty() {
                    let var_name = format!("v{}", func.variables().len() + 1 + i);
                    func.append(format!("let {} = {};\n", var_name, arg_strs[0]));
                }
            }
        }

        // Continue with target block
        self.compile_cfg_block(target_idx, cfg, ir, func, _compiled);
    }

    /// Handle conditional branch (cbr) - detects if-else pattern
    fn handle_conditional_branch(
        &mut self,
        condition: &[Value],
        _source_idx: usize,
        _then_args: IRPointer<Value>,
        _else_args: IRPointer<Value>,
        cfg: &CFG,
        ir: &SlynxIR,
        func: &mut JSFunction,
        compiled: &mut HashSet<usize>,
    ) {
        let source_block = cfg.get_block(&_source_idx).unwrap();
        let then_idx = source_block.successors[0];
        let else_idx = source_block.successors[1];

        let then_block = cfg.get_block(&then_idx).unwrap();
        let else_block = cfg.get_block(&else_idx).unwrap();

        // Check if this is an if-else pattern: both branches go to the same merge block
        let is_if_else = {
            let then_targets = &then_block.successors;
            let else_targets = &else_block.successors;
            then_targets.len() == 1 && else_targets.len() == 1 && then_targets == else_targets
        };

        if is_if_else {
            // If-else pattern: compile as structured if-else
            let merge_idx = then_block.successors[0];
            let then_args = then_block
                .branch_args
                .get(&merge_idx)
                .cloned()
                .unwrap_or_default();
            let else_args = else_block
                .branch_args
                .get(&merge_idx)
                .cloned()
                .unwrap_or_default();

            // Compile condition
            let cond_strs = func.compile_values(condition, ir);
            let cond = cond_strs.join(", ");

            func.append(format!("if ({}) {{\n", cond));

            // In then block: assign if there are args for the merge
            if !then_args.is_empty() {
                for (i, arg) in then_args.iter().enumerate() {
                    let arg: Value = arg.clone();
                    let val = func.compile_values(&[arg], ir);
                    if !val.is_empty() {
                        let var_name = format!("v{}", func.variables().len() + 1 + i);
                        func.append(format!("let {} = {};\n", var_name, val[0]));
                    }
                }
            }
            // Compile then block content (without its final branch)
            for inst in &then_block.instructions {
                let result = func.compile_instruction(inst, ir);
                if !result.is_empty() {
                    func.append(result);
                }
            }

            func.append("else {\n".to_string());

            // In else block: assign if there are args for the merge
            if !else_args.is_empty() {
                for (i, arg) in else_args.iter().enumerate() {
                    let arg: Value = arg.clone();
                    let val = func.compile_values(&[arg], ir);
                    if !val.is_empty() {
                        let var_name = format!("v{}", func.variables().len() + 1 + i);
                        func.append(format!("let {} = {};\n", var_name, val[0]));
                    }
                }
            }
            // Compile else block content (without its final branch)
            for inst in &else_block.instructions {
                let result = func.compile_instruction(inst, ir);
                if !result.is_empty() {
                    func.append(result);
                }
            }

            func.append("}\n".to_string());

            // Continue with merge block
            if !compiled.contains(&merge_idx) {
                self.compile_cfg_block(merge_idx, cfg, ir, func, compiled);
            }
        } else {
            // Non-structured control flow - use plain if-else
            let cond_strs = func.compile_values(condition, ir);
            let cond = cond_strs.join(", ");

            func.append(format!("if ({}) {{\n", cond));
            if !compiled.contains(&then_idx) {
                self.compile_cfg_block(then_idx, cfg, ir, func, compiled);
            }
            func.append("else {\n".to_string());
            if !compiled.contains(&else_idx) {
                self.compile_cfg_block(else_idx, cfg, ir, func, compiled);
            }
            func.append("}\n".to_string());
        }
    }

    pub fn compile_component(&mut self, component: &slynx::middleend::Component, ir: &SlynxIR) {
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
