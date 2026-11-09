use std::collections::{HashMap, HashSet};

use slynx::slynx_ir::{
    ControlFlowGraph, IRPointer, IRTypeId, Label, Opcode, SlynxIR, Value,
    petgraph::graph::NodeIndex,
};

use crate::InstructionCompiler;

pub struct JSFunction {
    pub content: String,
    arguments: Vec<String>,
    variables: HashMap<Value, String>,
    /// Maps (label_ptr, arg_index) → JS variable name
    label_args: HashMap<(IRPointer<Label, 1>, usize), String>,
    /// The label currently being compiled, needed to resolve LabelArg values
    current_label: Option<IRPointer<Label, 1>>,
    /// Number of times each instruction is evaluated when expressions are
    /// inlined. The instruction itself contributes one evaluation.
    use_counts: HashMap<Value, usize>,
    /// Deferred impure values that had to be materialized because they are
    /// evaluated more than once.
    materialized: HashMap<Value, String>,
    /// Void calls that were already emitted as statements. Their value must
    /// not be evaluated again by a following `Ret`.
    emitted_values: HashSet<Value>,
    temp_counter: usize,
    identation: usize,
}

impl InstructionCompiler for JSFunction {
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
    fn resolve_label_arg(&self, index: usize) -> String {
        let lbl = self.current_label.expect("current_label not set");
        self.label_args
            .get(&(lbl, index))
            .cloned()
            .unwrap_or_else(|| panic!("LabelArg({index}) not found for current label"))
    }
    fn append_content(&mut self, content: &str) {
        self.content.push_str(content);
    }
    fn resolve_materialized(&self, value: Value) -> Option<String> {
        self.materialized.get(&value).cloned()
    }
}

impl JSFunction {
    pub fn new(initial_content: String, arguments: Vec<String>) -> Self {
        Self {
            content: initial_content,
            arguments,
            variables: HashMap::new(),
            label_args: HashMap::new(),
            current_label: None,
            use_counts: HashMap::new(),
            materialized: HashMap::new(),
            emitted_values: HashSet::new(),
            temp_counter: 0,
            identation: 1,
        }
    }

    pub fn compile_from_cfg(&mut self, cfg: &ControlFlowGraph, ir: &SlynxIR) {
        self.use_counts = Self::compute_use_counts(cfg, ir);
        let order = cfg
            .topological_order()
            .expect("CFG has cycles (while loops not yet supported)");
        let mut emitted: HashSet<NodeIndex> = HashSet::new();

        for node in &order {
            if emitted.contains(node) {
                continue;
            }
            self.emit_node(*node, cfg, ir, &mut emitted);
        }
    }

    fn compute_use_counts(cfg: &ControlFlowGraph, ir: &SlynxIR) -> HashMap<Value, usize> {
        let mut counts = HashMap::new();
        let mut stack = Vec::new();
        for block in cfg.graph().node_weights() {
            for value in ir.get_view(block.label()).get_instructions() {
                stack.push((*value, 1usize));
            }
        }
        while let Some((value, multiplicity)) = stack.pop() {
            *counts.entry(value).or_insert(0) += multiplicity;
            let instruction = ir.get_instruction(value);
            for operand in &instruction.operands {
                stack.push((*operand, multiplicity));
            }
        }
        counts
    }

    fn emit_deferred(
        &mut self,
        value: Value,
        instruction: &slynx::slynx_ir::Instruction,
        ir: &SlynxIR,
    ) -> bool {
        let uses = self
            .use_counts
            .get(&value)
            .copied()
            .unwrap_or(0)
            .saturating_sub(1);
        match &instruction.opcode {
            Opcode::Call(_) | Opcode::DynMethodCall(_) => {
                if uses == 0 {
                    let expr = self.compile_value(value, ir);
                    self.content.push_str(&self.ident(format!("{expr};\n")));
                } else if uses == 1 {
                    // Inlined at its single use site.
                } else if !self.materialized.contains_key(&value) {
                    let expr = self.compile_value(value, ir);
                    let var = format!("tmp{}", self.temp_counter);
                    self.temp_counter += 1;
                    self.content
                        .push_str(&self.ident(format!("let {var} = {expr};\n")));
                    self.materialized.insert(value, var);
                }
                true
            }
            Opcode::Component => uses > 0,
            _ => false,
        }
    }

    fn emit_node(
        &mut self,
        node: NodeIndex,
        cfg: &ControlFlowGraph,
        ir: &SlynxIR,
        emitted: &mut HashSet<NodeIndex>,
    ) {
        emitted.insert(node);
        let label_ptr = cfg.graph().node_weight(node).unwrap().label();
        self.current_label = Some(label_ptr);

        let all_insts = ir.get_view(label_ptr).get_instructions().to_vec();
        let (body, terminator) = match all_insts.last() {
            Some(last)
                if matches!(
                    ir.get_instruction(*last).opcode,
                    Opcode::Ret | Opcode::Br(_) | Opcode::Cbr { .. }
                ) =>
            {
                (&all_insts[..all_insts.len() - 1], Some(*last))
            }
            _ => (&all_insts[..], None),
        };

        for inst in body {
            let instruction = ir.get_instruction(*inst);
            if Self::is_expression(&instruction.opcode, instruction.value_type, ir) {
                continue;
            }
            if self.emit_deferred(*inst, instruction, ir) {
                continue;
            }
            let s = self.compile_value(*inst, ir);
            self.content.push_str(&s);
        }
        let Some(term) = terminator else { return };
        let term_instruction = ir.get_instruction(term);

        match &term_instruction.opcode {
            Opcode::Ret => {
                let value = term_instruction.operands.first().copied();
                if value.map_or(true, |v| self.emitted_values.contains(&v)) {
                    self.content.push_str(&self.ident("return;\n".to_string()));
                } else {
                    let s = self.compile_value(term, ir);
                    self.content.push_str(&self.ident(s));
                }
            }
            Opcode::Br(target_ptr) => {
                if !ir.get_view(*target_ptr).arguments().is_empty() {
                    self.ensure_label_args(*target_ptr, ir);
                    let compiled = self.compile_values(&term_instruction.operands, ir);
                    for (i, val) in compiled.into_iter().enumerate() {
                        let var = self.label_arg_var(*target_ptr, i, ir);
                        self.content
                            .push_str(&self.ident(format!("{var} = {val};\n")));
                    }
                }
            }
            Opcode::Cbr {
                then_label,
                else_label,
                ..
            } => {
                let mappings = cfg.label_mappings();
                let then_node = mappings.get(then_label).copied();
                let else_node = mappings.get(else_label).copied();

                if let (Some(then_node), Some(else_node)) = (then_node, else_node) {
                    if let Some(end_ptr) = self.find_end_label(then_node, else_node, cfg, ir) {
                        self.ensure_label_args(end_ptr, ir);
                    }
                }
                self.ensure_label_args(*then_label, ir);
                self.ensure_label_args(*else_label, ir);

                let then_arg_count = ir.get_view(*then_label).arguments().len();
                let condition = term_instruction.operands[0];
                let cond = if condition.is_void() {
                    "false".to_string()
                } else {
                    self.compile_value(condition, ir)
                };
                // restore current_label after compiling values
                self.current_label = Some(label_ptr);

                self.content
                    .push_str(&self.ident(format!("if({cond}){{\n")));
                self.increase_identation();
                let then_args = &term_instruction.operands[1..1 + then_arg_count];
                let compiled_then = self.compile_values(then_args, ir);
                self.current_label = Some(label_ptr);
                for (i, val) in compiled_then.into_iter().enumerate() {
                    let var = self.label_arg_var(*then_label, i, ir);
                    self.content
                        .push_str(&self.ident(format!("{var} = {val};\n")));
                }
                if let Some(then_node) = then_node {
                    self.emit_node(then_node, cfg, ir, emitted);
                }
                self.decrease_identation();
                self.content.push_str(&self.ident("} else {\n".to_string()));
                self.increase_identation();
                let else_args = &term_instruction.operands[1 + then_arg_count..];
                let compiled_else = self.compile_values(else_args, ir);
                self.current_label = Some(label_ptr);
                for (i, val) in compiled_else.into_iter().enumerate() {
                    let var = self.label_arg_var(*else_label, i, ir);
                    self.content
                        .push_str(&self.ident(format!("{var} = {val};\n")));
                }
                if let Some(else_node) = else_node {
                    self.emit_node(else_node, cfg, ir, emitted);
                }
                self.decrease_identation();
                self.content.push_str(&self.ident("}\n".to_string()));
            }
            _ => {
                let s = self.compile_value(term, ir);
                self.content.push_str(&self.ident(s));
            }
        }
    }

    /// Declares JS variables for a label's block arguments if they have not
    /// been declared yet. The declaration is emitted at the current scope.
    fn ensure_label_args(&mut self, label: IRPointer<Label, 1>, ir: &SlynxIR) {
        let count = ir.get_view(label).arguments().len();
        for index in 0..count {
            self.label_arg_var(label, index, ir);
        }
    }

    fn label_arg_var(
        &mut self,
        label: IRPointer<Label, 1>,
        index: usize,
        _ir: &SlynxIR,
    ) -> String {
        if let Some(var) = self.label_args.get(&(label, index)) {
            return var.clone();
        }
        let var = format!("v{}", self.variables.len() + self.label_args.len() + 1);
        self.content.push_str(&self.ident(format!("let {var};\n")));
        self.label_args.insert((label, index), var.clone());
        var
    }

    fn find_end_label(
        &self,
        then_node: NodeIndex,
        else_node: NodeIndex,
        cfg: &ControlFlowGraph,
        ir: &SlynxIR,
    ) -> Option<IRPointer<Label, 1>> {
        let br_target = |node: NodeIndex| -> Option<IRPointer<Label, 1>> {
            let label_ptr = cfg.graph().node_weight(node)?.label();
            let last_instruction = ir
                .get_view(label_ptr)
                .get_instructions()
                .last()
                .map(|v| ir.get_instruction(*v));

            if let Opcode::Br(target) = last_instruction?.opcode {
                if !ir.get_view(target).arguments().is_empty() {
                    return Some(target);
                }
            }
            None
        };
        let a = br_target(then_node)?;
        let b = br_target(else_node)?;
        (a == b).then_some(a)
    }

    fn is_expression(opcode: &Opcode, value_type: IRTypeId, ir: &SlynxIR) -> bool {
        match opcode {
            Opcode::Component => {
                value_type != ir.types.specialized_text_type()
                    && value_type != ir.types.specialized_div_type()
            }
            // Calls are handled by `emit_deferred`, which decides whether they
            // are emitted as statements, inlined, or materialized.
            Opcode::Call(_) | Opcode::DynMethodCall(_) | Opcode::InitCall(_) => false,
            _ => !opcode.is_impure(),
        }
    }

    pub fn append(&mut self, content: String) {
        self.content.push_str(&content);
    }

    pub fn finish(mut self) -> String {
        self.content.push_str("\n}\n");
        self.content
    }
}
