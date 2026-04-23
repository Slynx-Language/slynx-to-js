use std::collections::HashMap;

use slynx::middleend::{
    IRPointer, IRTypeId,
    ir::{Context, Instruction, InstructionType, Label, SlynxIR, Value},
};

/// A single basic block in the CFG
#[derive(Debug, Clone)]
pub struct CFGBlock {
    /// Index of this label in the context's labels array
    pub label_idx: usize,
    /// Instructions in this block (excluding the final branch)
    pub instructions: Vec<Instruction>,
    /// Label indices that this block branches to
    pub successors: Vec<usize>,
    /// Label indices that branch to this block
    pub predecessors: Vec<usize>,
    /// Parameters of this label (type only - names not needed)
    pub params: Vec<IRTypeId>,
    /// Arguments passed to each successor label via branches
    pub branch_args: HashMap<usize, Vec<Value>>,
}

/// Control Flow Graph for a function context
pub struct CFG {
    /// Map from label index to its block
    blocks: HashMap<usize, CFGBlock>,
    /// Entry label index (always 0)
    entry: usize,
    /// Reference to all labels of the context (for looking them up)
    labels: Vec<Label>,
}

impl CFG {
    /// Build a CFG from a function context
    pub fn from_context(ctx: &Context, ir: &SlynxIR) -> Self {
        let labels = ir.get_context_labels(ctx).to_vec();
        let entry = 0;

        let mut blocks = HashMap::new();

        // First pass: create blocks for each label
        for (idx, label) in labels.iter().enumerate() {
            let all_instructions = ir.get_label_instructions(label);

            // Flatten: each label can have multiple instruction lists
            let mut instructions = Vec::new();
            for inst_list in all_instructions {
                instructions.extend_from_slice(inst_list);
            }

            // Extract parameters from label
            let params = label.arguments().to_vec();

            // Find the final branch instruction (if any)
            let final_branch = instructions
                .last()
                .and_then(|inst| match &inst.instruction_type {
                    InstructionType::Br(_) | InstructionType::Cbr { .. } => Some(inst.clone()),
                    _ => None,
                });

            // Remove the final branch from regular instructions
            let block_instructions = if final_branch.is_some() {
                instructions
                    .split_last()
                    .map(|(_, rest)| rest.to_vec())
                    .unwrap_or_default()
            } else {
                instructions.clone()
            };

            let block = CFGBlock {
                label_idx: idx,
                instructions: block_instructions,
                successors: Vec::new(),
                predecessors: Vec::new(),
                params,
                branch_args: HashMap::new(),
            };

            blocks.insert(idx, block);
        }

        // Second pass: fill in successors and branch_args
        // We need to map branch label pointers back to indices
        let label_ptrs: Vec<IRPointer<Label, 1>> =
            (0..labels.len()).map(|i| ctx.get_label(i)).collect();

        for (idx, label) in labels.iter().enumerate() {
            let block = blocks.get_mut(&idx).unwrap();
            let all_instructions = ir.get_label_instructions(label);

            // Look for the final branch in each instruction list
            for inst_list in all_instructions {
                if let Some(last_inst) = inst_list.last() {
                    match &last_inst.instruction_type {
                        InstructionType::Br(target) => {
                            // Find the index of the target label
                            let target_idx = label_ptrs
                                .iter()
                                .position(|p| p.ptr() == target.ptr())
                                .expect("Target label not found");
                            let args = ir
                                .get_values_by_pointer(last_inst.operands.clone())
                                .to_vec();
                            block.successors.push(target_idx);
                            block.branch_args.insert(target_idx, args);
                        }
                        InstructionType::Cbr {
                            then_label,
                            else_label,
                            then_args,
                            else_args,
                        } => {
                            let then_idx = label_ptrs
                                .iter()
                                .position(|p| p.ptr() == then_label.ptr())
                                .expect("Then label not found");
                            let else_idx = label_ptrs
                                .iter()
                                .position(|p| p.ptr() == else_label.ptr())
                                .expect("Else label not found");
                            let then_args_vals =
                                ir.get_values_by_pointer(then_args.clone()).to_vec();
                            let else_args_vals =
                                ir.get_values_by_pointer(else_args.clone()).to_vec();
                            block.successors.push(then_idx);
                            block.successors.push(else_idx);
                            block.branch_args.insert(then_idx, then_args_vals);
                            block.branch_args.insert(else_idx, else_args_vals);
                        }
                        _ => {}
                    }
                }
            }
        }

        // Third pass: fill in predecessors
        let mut predecessors: HashMap<usize, Vec<usize>> = HashMap::new();
        for (idx, block) in blocks.iter() {
            for succ in &block.successors {
                predecessors.entry(*succ).or_default().push(*idx);
            }
        }
        for (idx, preds) in predecessors {
            if let Some(block) = blocks.get_mut(&idx) {
                block.predecessors = preds;
            }
        }

        CFG {
            blocks,
            entry,
            labels,
        }
    }

    /// Get a block by label index
    pub fn get_block(&self, idx: &usize) -> Option<&CFGBlock> {
        self.blocks.get(idx)
    }

    /// Get the entry label index
    pub fn get_entry(&self) -> &usize {
        &self.entry
    }

    /// Get all blocks
    pub fn get_blocks(&self) -> &HashMap<usize, CFGBlock> {
        &self.blocks
    }

    /// Get the actual Label struct for an index
    pub fn get_label(&self, idx: &usize) -> &Label {
        &self.labels[*idx]
    }
}
