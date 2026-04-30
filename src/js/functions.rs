use std::collections::{HashMap, HashSet};

use slynx::middleend::{
    ControlFlowGraph, IRPointer, InstructionType, Label, Slot, SlynxIR, petgraph::graph::NodeIndex,
};

use crate::InstructionCompiler;

pub struct JSFunction {
    pub content: String,
    arguments: Vec<String>,
    variables: HashMap<IRPointer<Slot, 1>, String>,
    /// Maps (label_ptr, arg_index) → JS variable name
    label_args: HashMap<(IRPointer<Label, 1>, usize), String>,
    /// The label currently being compiled, needed to resolve LabelArg values
    current_label: Option<IRPointer<Label, 1>>,
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
    fn variables(&self) -> &HashMap<IRPointer<Slot, 1>, String> {
        &self.variables
    }
    fn variables_mut(&mut self) -> &mut HashMap<IRPointer<Slot, 1>, String> {
        &mut self.variables
    }
    fn resolve_label_arg(&self, index: usize) -> String {
        let lbl = self.current_label.expect("current_label not set");
        self.label_args
            .get(&(lbl, index))
            .cloned()
            .unwrap_or_else(|| panic!("LabelArg({index}) not found for current label"))
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
            identation: 1,
        }
    }

    pub fn compile_from_cfg(&mut self, cfg: &ControlFlowGraph, ir: &SlynxIR) {
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

        let label = ir.get_label(label_ptr);
        let all_insts: Vec<_> = ir
            .get_label_instructions(label)
            .into_iter()
            .flatten()
            .collect();
        let (body, terminator) = match all_insts.split_last() {
            Some((t, body)) => (body, Some(*t)),
            None => (&[][..], None),
        };

        for inst in body {
            let s = self.compile_instruction(inst, ir);
            self.content.push_str(&s);
        }
        let Some(term) = terminator else { return };

        match &term.instruction_type {
            InstructionType::Ret => {
                let s = self.compile_instruction(term, ir);
                self.content.push_str(&self.ident(s));
            }
            InstructionType::Br(target_ptr) => {
                let target_label = ir.get_label(*target_ptr);
                if !target_label.arguments().is_empty() {
                    let args = ir.get_values_by_pointer(term.operands.clone());
                    let compiled = self.compile_values(args, ir);
                    for (i, val) in compiled.into_iter().enumerate() {
                        let var = self
                            .label_args
                            .get(&(*target_ptr, i))
                            .cloned()
                            .unwrap_or_else(|| panic!("label arg var not allocated"));
                        self.content
                            .push_str(&self.ident(format!("{var} = {val};\n")));
                    }
                }
            }
            InstructionType::Cbr {
                then_label,
                else_label,
                ..
            } => {
                let mappings = cfg.label_mappings();
                let then_node = mappings[then_label];
                let else_node = mappings[else_label];

                if let Some(end_ptr) = self.find_end_label(then_node, else_node, cfg, ir) {
                    let end_label = ir.get_label(end_ptr);
                    for i in 0..end_label.arguments().len() {
                        if !self.label_args.contains_key(&(end_ptr, i)) {
                            let var_name =
                                format!("v{}", self.variables.len() + self.label_args.len() + 1);
                            let decl = self.ident(format!("let {var_name};\n"));
                            self.content.push_str(&decl);
                            self.label_args.insert((end_ptr, i), var_name);
                        }
                    }
                }

                let cond_vals = ir.get_values_by_pointer(term.operands.clone());
                // restore current_label after compile_values (it may change in recursive emit)
                self.current_label = Some(label_ptr);
                let cond = self.compile_values(cond_vals, ir).remove(0);

                self.content
                    .push_str(&self.ident(format!("if({cond}){{\n")));
                self.increase_identation();
                self.emit_node(then_node, cfg, ir, emitted);
                self.decrease_identation();
                self.content.push_str(&self.ident("} else {\n".to_string()));
                self.increase_identation();
                self.emit_node(else_node, cfg, ir, emitted);
                self.decrease_identation();
                self.content.push_str(&self.ident("}\n".to_string()));
            }
            _ => {
                let s = self.compile_instruction(term, ir);
                self.content.push_str(&self.ident(s));
            }
        }
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
            let label = ir.get_label(label_ptr);
            let insts = ir
                .get_label_instructions(label)
                .into_iter()
                .flatten()
                .last();
            if let InstructionType::Br(target) = insts?.instruction_type {
                let target_label = ir.get_label(target);
                if !target_label.arguments().is_empty() {
                    return Some(target);
                }
            }
            None
        };
        let a = br_target(then_node)?;
        let b = br_target(else_node)?;
        (a == b).then_some(a)
    }

    pub fn append(&mut self, content: String) {
        self.content.push_str(&content);
    }

    pub fn finish(mut self) -> String {
        self.content.push_str("\n}\n");
        self.content
    }
}
