use super::block::BasicBlock;
use super::inst::{BlockId, InstData, InstId, Instruction, Operand};
use std::collections::{HashMap, HashSet, VecDeque};

pub fn optimize_ir(
    blocks: &mut Vec<BasicBlock>,
    instructions: &mut Vec<InstData>,
    entry_block: &mut BlockId,
) {
    // --- Step 0: Constant Folding ---
    fold_constants(instructions);

    // --- Step 1: Unreachable Block Elimination ---
    let mut reachable = HashSet::new();
    let mut queue = VecDeque::new();

    reachable.insert(*entry_block);
    queue.push_back(*entry_block);

    while let Some(block_id) = queue.pop_front() {
        if (block_id.0 as usize) < blocks.len() {
            let block = &blocks[block_id.0 as usize];
            // Find successors
            for &inst_id in &block.instructions {
                if (inst_id.0 as usize) < instructions.len() {
                    let inst_data = &instructions[inst_id.0 as usize];
                    match &inst_data.inst {
                        Instruction::Br(target) => {
                            if reachable.insert(*target) {
                                queue.push_back(*target);
                            }
                        }
                        Instruction::CondBr(_, true_target, false_target) => {
                            if reachable.insert(*true_target) {
                                queue.push_back(*true_target);
                            }
                            if reachable.insert(*false_target) {
                                queue.push_back(*false_target);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Create mapping of old BlockId to new BlockId
    let mut block_map = HashMap::new();
    let mut new_blocks = Vec::new();
    for (old_idx, block) in blocks.iter().enumerate() {
        let old_id = BlockId(old_idx as u32);
        if reachable.contains(&old_id) {
            let new_id = BlockId(new_blocks.len() as u32);
            block_map.insert(old_id, new_id);
            new_blocks.push(block.clone());
        }
    }

    // Update block IDs in instructions and entry block
    for block in &mut new_blocks {
        for &inst_id in &block.instructions {
            if (inst_id.0 as usize) < instructions.len() {
                map_instruction_blocks(&mut instructions[inst_id.0 as usize].inst, &block_map);
            }
        }
    }
    map_block_id(entry_block, &block_map);

    // Update block IDs of the basic blocks themselves
    for (new_idx, block) in new_blocks.iter_mut().enumerate() {
        block.id = BlockId(new_idx as u32);
    }
    *blocks = new_blocks;

    // --- Step 2: Dead Instruction Elimination ---
    let mut alive = HashSet::new();
    let mut worklist = VecDeque::new();

    // Identify intrinsically alive instructions
    for block in blocks.iter() {
        for &inst_id in &block.instructions {
            if (inst_id.0 as usize) < instructions.len() {
                let inst_data = &instructions[inst_id.0 as usize];
                if is_intrinsically_alive(&inst_data.inst) {
                    if alive.insert(inst_id) {
                        worklist.push_back(inst_id);
                    }
                }
            }
        }
    }

    // Propagate liveness
    while let Some(inst_id) = worklist.pop_front() {
        let inst_data = &instructions[inst_id.0 as usize];
        let mut used = Vec::new();
        get_used_registers(&inst_data.inst, &mut used);
        for used_id in used {
            if (used_id.0 as usize) < instructions.len() {
                if alive.insert(used_id) {
                    worklist.push_back(used_id);
                }
            }
        }
    }

    // Create mapping of old InstId to new InstId
    let mut inst_map = HashMap::new();
    let mut new_instructions = Vec::new();
    for (old_idx, inst_data) in instructions.iter().enumerate() {
        let old_id = InstId(old_idx as u32);
        if alive.contains(&old_id) {
            let new_id = InstId(new_instructions.len() as u32);
            inst_map.insert(old_id, new_id);
            new_instructions.push(inst_data.clone());
        }
    }

    // Update instruction IDs inside operands of new instructions
    for inst_data in &mut new_instructions {
        map_instruction_registers(&mut inst_data.inst, &inst_map);
    }

    // Filter and update instruction IDs in basic blocks
    for block in blocks.iter_mut() {
        let mut new_block_insts = Vec::new();
        for &old_inst_id in &block.instructions {
            if let Some(&new_inst_id) = inst_map.get(&old_inst_id) {
                new_block_insts.push(new_inst_id);
            }
        }
        block.instructions = new_block_insts;
    }

    *instructions = new_instructions;
}

fn is_intrinsically_alive(inst: &Instruction) -> bool {
    matches!(
        inst,
        Instruction::Store(..)
            | Instruction::Return(..)
            | Instruction::Br(..)
            | Instruction::CondBr(..)
            | Instruction::Call(..)
    )
}

fn map_block_id(block_id: &mut BlockId, map: &HashMap<BlockId, BlockId>) {
    if let Some(&new_id) = map.get(block_id) {
        *block_id = new_id;
    }
}

fn map_instruction_blocks(inst: &mut Instruction, map: &HashMap<BlockId, BlockId>) {
    match inst {
        Instruction::Br(target) => {
            map_block_id(target, map);
        }
        Instruction::CondBr(_, true_target, false_target) => {
            map_block_id(true_target, map);
            map_block_id(false_target, map);
        }
        _ => {}
    }
}

fn map_operand(op: &mut Operand, map: &HashMap<InstId, InstId>) {
    if let Operand::Reg(id) = op {
        if let Some(&new_id) = map.get(id) {
            *op = Operand::Reg(new_id);
        }
    }
}

fn map_instruction_registers(inst: &mut Instruction, map: &HashMap<InstId, InstId>) {
    match inst {
        Instruction::Add(op1, op2)
        | Instruction::Sub(op1, op2)
        | Instruction::Mul(op1, op2)
        | Instruction::Div(op1, op2)
        | Instruction::Mod(op1, op2)
        | Instruction::Eq(_, op1, op2)
        | Instruction::NotEq(_, op1, op2)
        | Instruction::Lt(_, op1, op2)
        | Instruction::LtEq(_, op1, op2)
        | Instruction::Gt(_, op1, op2)
        | Instruction::GtEq(_, op1, op2)
        | Instruction::And(op1, op2)
        | Instruction::Or(op1, op2)
        | Instruction::BitAnd(op1, op2)
        | Instruction::BitOr(op1, op2)
        | Instruction::BitXor(op1, op2)
        | Instruction::Index(op1, op2)
        | Instruction::GetIndexPtr(op1, op2)
        | Instruction::Store(_, op1, op2) => {
            map_operand(op1, map);
            map_operand(op2, map);
        }
        Instruction::Neg(op)
        | Instruction::Not(op)
        | Instruction::Cast(op)
        | Instruction::Load(op)
        | Instruction::CondBr(op, _, _)
        | Instruction::GetMemberPtr(op, _) => {
            map_operand(op, map);
        }
        Instruction::Call(callee, args) => {
            map_operand(callee, map);
            for arg in args.iter_mut() {
                map_operand(arg, map);
            }
        }
        Instruction::Return(Some(op)) => {
            map_operand(op, map);
        }
        Instruction::Alloca(_) | Instruction::Br(_) | Instruction::Return(None) => {}
    }
}

fn get_used_registers(inst: &Instruction, used: &mut Vec<InstId>) {
    let mut add_op = |op: &Operand| {
        if let Operand::Reg(id) = op {
            used.push(*id);
        }
    };
    match inst {
        Instruction::Add(op1, op2)
        | Instruction::Sub(op1, op2)
        | Instruction::Mul(op1, op2)
        | Instruction::Div(op1, op2)
        | Instruction::Mod(op1, op2)
        | Instruction::Eq(_, op1, op2)
        | Instruction::NotEq(_, op1, op2)
        | Instruction::Lt(_, op1, op2)
        | Instruction::LtEq(_, op1, op2)
        | Instruction::Gt(_, op1, op2)
        | Instruction::GtEq(_, op1, op2)
        | Instruction::And(op1, op2)
        | Instruction::Or(op1, op2)
        | Instruction::BitAnd(op1, op2)
        | Instruction::BitOr(op1, op2)
        | Instruction::BitXor(op1, op2)
        | Instruction::Index(op1, op2)
        | Instruction::GetIndexPtr(op1, op2)
        | Instruction::Store(_, op1, op2) => {
            add_op(op1);
            add_op(op2);
        }
        Instruction::Neg(op)
        | Instruction::Not(op)
        | Instruction::Cast(op)
        | Instruction::Load(op)
        | Instruction::CondBr(op, _, _)
        | Instruction::GetMemberPtr(op, _) => {
            add_op(op);
        }
        Instruction::Call(callee, args) => {
            add_op(callee);
            for arg in args.iter() {
                add_op(arg);
            }
        }
        Instruction::Return(Some(op)) => {
            add_op(op);
        }
        Instruction::Alloca(_) | Instruction::Br(_) | Instruction::Return(None) => {}
    }
}

pub fn fold_constants(instructions: &mut [InstData]) {
    for inst_data in instructions {
        match &inst_data.inst {
            Instruction::Add(Operand::Int(a), Operand::Int(b)) => {
                inst_data.inst = Instruction::Cast(Operand::Int(a.wrapping_add(*b)));
            }
            Instruction::Sub(Operand::Int(a), Operand::Int(b)) => {
                inst_data.inst = Instruction::Cast(Operand::Int(a.wrapping_sub(*b)));
            }
            Instruction::Mul(Operand::Int(a), Operand::Int(b)) => {
                inst_data.inst = Instruction::Cast(Operand::Int(a.wrapping_mul(*b)));
            }
            Instruction::Div(Operand::Int(a), Operand::Int(b)) => {
                if *b != 0 {
                    inst_data.inst = Instruction::Cast(Operand::Int(a / b));
                }
            }
            Instruction::Mod(Operand::Int(a), Operand::Int(b)) => {
                if *b != 0 {
                    inst_data.inst = Instruction::Cast(Operand::Int(a % b));
                }
            }
            Instruction::Eq(_, Operand::Int(a), Operand::Int(b)) => {
                inst_data.inst = Instruction::Cast(Operand::Bool(a == b));
            }
            Instruction::NotEq(_, Operand::Int(a), Operand::Int(b)) => {
                inst_data.inst = Instruction::Cast(Operand::Bool(a != b));
            }
            Instruction::Lt(_, Operand::Int(a), Operand::Int(b)) => {
                inst_data.inst = Instruction::Cast(Operand::Bool((*a as i32) < (*b as i32)));
            }
            Instruction::LtEq(_, Operand::Int(a), Operand::Int(b)) => {
                inst_data.inst = Instruction::Cast(Operand::Bool((*a as i32) <= (*b as i32)));
            }
            Instruction::Gt(_, Operand::Int(a), Operand::Int(b)) => {
                inst_data.inst = Instruction::Cast(Operand::Bool((*a as i32) > (*b as i32)));
            }
            Instruction::GtEq(_, Operand::Int(a), Operand::Int(b)) => {
                inst_data.inst = Instruction::Cast(Operand::Bool((*a as i32) >= (*b as i32)));
            }
            _ => {}
        }
    }
}
