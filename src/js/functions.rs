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

        let all_insts = ir.get_view(label_ptr).get_instructions().to_vec();
        let (body, terminator) = match all_insts.split_last() {
            Some((t, body)) => (body, Some(*t)),
            None => (&[][..], None),
        };

        for inst in body {
            let instruction = ir.get_instruction(*inst);
            if Self::is_expression(&instruction.opcode, instruction.value_type, ir) {
                continue;
            }
            let s = self.compile_value(*inst, ir);
            self.content.push_str(&s);
        }
        let Some(term) = terminator else { return };
        let term_instruction = ir.get_instruction(term);

        match &term_instruction.opcode {
            Opcode::Ret => {
                let s = self.compile_value(term, ir);
                self.content.push_str(&self.ident(s));
            }
            Opcode::Br(target_ptr) => {
                if !ir.get_view(*target_ptr).arguments().is_empty() {
                    let compiled = self.compile_values(&term_instruction.operands, ir);
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
            Opcode::Cbr {
                then_label,
                else_label,
                ..
            } => {
                let mappings = cfg.label_mappings();
                let then_node = mappings[then_label];
                let else_node = mappings[else_label];

                if let Some(end_ptr) = self.find_end_label(then_node, else_node, cfg, ir) {
                    for i in 0..ir.get_view(end_ptr).arguments().len() {
                        if !self.label_args.contains_key(&(end_ptr, i)) {
                            let var_name =
                                format!("v{}", self.variables.len() + self.label_args.len() + 1);
                            let decl = self.ident(format!("let {var_name};\n"));
                            self.content.push_str(&decl);
                            self.label_args.insert((end_ptr, i), var_name);
                        }
                    }
                }

                let mut cond_vals = self.compile_values(&term_instruction.operands, ir);
                let cond = cond_vals.remove(0);
                // restore current_label after compile_values (it may change in recursive emit)
                self.current_label = Some(label_ptr);

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
                let s = self.compile_value(term, ir);
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
                value_type != ir.specialized_text_type() && value_type != ir.specialized_div_type()
            }
            Opcode::Call(_) | Opcode::InitCall(_) => value_type != ir.void_type(),
            _ => !matches!(
                opcode,
                Opcode::Allocate
                    | Opcode::Write
                    | Opcode::SetField(_)
                    | Opcode::SApply { .. }
                    | Opcode::Br(_)
                    | Opcode::Cbr { .. }
                    | Opcode::Ret
            ),
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
