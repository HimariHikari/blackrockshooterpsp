//! `bscr` battle scripts (the `.bms` files): one per enemy and per battle map.
//!
//! A script is a table of named functions over a stream of fixed-size instructions for
//! a small stack machine. The engine drives it through a few entry points it finds by
//! function type (an update run every frame, a "signal" for events), and the script
//! calls back into the engine through numbered functions.
//!
//! This follows the original's loader (0xCE3B8) and interpreter (step at 0xCC3A0, run
//! loop at 0xCE0B8); `docs/BATTLE_NOTES.md` has the details.

use anyhow::{bail, ensure, Context, Result};

use crate::reader::Reader;

pub mod signatures;

pub use signatures::Signature;

pub const MAGIC: &[u8; 4] = b"bscr";

// Header fields.
const NAME_AT: usize = 0x06;
const NAME_END: usize = 0x24;
const SIZE_AT: usize = 0x24;
const FUNCTION_COUNT_AT: usize = 0x28;
const FUNCTIONS_AT: usize = 0x2c;
const STRINGS_AT: usize = 0x34;
const CODE_SIZE_AT: usize = 0x38;
const CODE_AT: usize = 0x3c;
const HEADER_LEN: usize = 0x40;

// A function table entry: u16 size, u16 type, name, code offset, argument count, then
// eight bytes per argument.
const FUNCTION_NAME_LEN: usize = 32;
const FUNCTION_FIXED_LEN: usize = 0x2c;
const ARGUMENT_LEN: usize = 8;

const INSTRUCTION_LEN: usize = 16;

/// Globals of a script, and locals of each function.
pub const GLOBALS: usize = 128;
pub const LOCALS: usize = 16;
/// How deep calls can nest: the original keeps this many return slots.
const MAX_DEPTH: usize = 16;
/// The original's value stack has a fixed size given when a script is set up; no
/// script comes near this.
const MAX_STACK: usize = 4096;
/// Instructions one run may execute before it is taken for an endless loop.
const MAX_STEPS: usize = 4_000_000;

// Operand kinds.
const IMMEDIATE: u8 = 0xf0;
const POP: u8 = 0xf3;
const PUSH: u8 = 0xf2;
const GLOBAL: u8 = 0xf4;
const LOCAL: u8 = 0xf6;

// Opcodes.
const OP_ADD: u8 = 0x02;
const OP_SUB: u8 = 0x03;
const OP_MUL: u8 = 0x04;
const OP_DIV: u8 = 0x05;
const OP_MOD: u8 = 0x06;
const OP_AND: u8 = 0x08;
const OP_OR: u8 = 0x09;
const OP_XOR: u8 = 0x0a;
const OP_SHIFT_LEFT: u8 = 0x0b;
const OP_SHIFT_RIGHT: u8 = 0x0c;
const OP_NOT: u8 = 0x0e;
const OP_INCREMENT: u8 = 0x11;
const OP_DECREMENT: u8 = 0x12;
const OP_MOVE: u8 = 0x13;
const OP_PUSH: u8 = 0x18;
const OP_PUSH_STRING: u8 = 0x1a;
const OP_SAVE_STACK: u8 = 0x1c;
const OP_RESTORE_STACK: u8 = 0x1d;
const OP_LESS: u8 = 0x70;
const OP_NOT_EQUAL: u8 = 0x75;
const OP_SKIP_IF: u8 = 0x80;
const OP_CALL: u8 = 0x81;
const OP_ENGINE_1: u8 = 0x82;
const OP_RETURN: u8 = 0x83;
const OP_JUMP: u8 = 0x84;
const OP_ENGINE_1_HIGH: u8 = 0x86;
const OP_ENGINE_2: u8 = 0x87;
const OP_ENGINE_2_HIGH: u8 = 0x88;

/// What a function is for. The engine finds its entry points by these, not by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FunctionKind {
    Plain,
    /// Leaves a value on the stack for its caller.
    Returning,
    /// Run once when the script is loaded.
    Init,
    /// Entry points taking one argument, known only by their type so far.
    Event(u8),
    /// Called with three arguments when something happens; returns a value.
    Signal,
    /// Run every logic frame.
    Update,
}

impl FunctionKind {
    fn from_type(kind: u16) -> Result<FunctionKind> {
        Ok(match kind {
            0xe0 => FunctionKind::Plain,
            0xe1 => FunctionKind::Returning,
            0xe4 => FunctionKind::Init,
            0xe7..=0xe9 => FunctionKind::Event(kind as u8),
            0xec => FunctionKind::Signal,
            0xed => FunctionKind::Update,
            _ => bail!("function type {kind:#x} is not known"),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub kind: FunctionKind,
    /// Index of its first instruction.
    pub entry: usize,
    pub arguments: usize,
}

#[derive(Clone, Copy, Debug)]
struct Instruction {
    op: u8,
    kinds: [u8; 3],
    values: [i32; 3],
}

#[derive(Clone, Debug)]
pub struct Script {
    /// The source file's name, as the compiler recorded it.
    pub name: String,
    pub functions: Vec<Function>,
    code: Vec<Instruction>,
    /// The strings, zero-terminated and back to back. A script refers to one by its
    /// offset in here.
    strings: Vec<u8>,
}

/// What a string's offset is added to when a script pushes it. The original pushes
/// the string's address, which is never zero; scripts hand zero to a function for
/// "no string", and the first string's offset is zero too.
const STRING_BASE: i32 = 0x4000_0000;

impl Script {
    pub fn parse(d: &[u8]) -> Result<Script> {
        ensure!(d.starts_with(MAGIC), "not a bscr file");
        ensure!(d.len() >= HEADER_LEN, "file ends inside its header");
        let r = Reader(d);
        let field = |at: usize| -> Result<usize> { Ok(r.u32(at)? as usize) };
        ensure!(field(SIZE_AT)? == d.len(), "header says {} bytes, the file has {}", field(SIZE_AT)?, d.len());
        let (code_at, code_size, strings_at) = (field(CODE_AT)?, field(CODE_SIZE_AT)?, field(STRINGS_AT)?);
        ensure!(code_size % INSTRUCTION_LEN == 0, "code size {code_size:#x} is not a whole number of instructions");
        ensure!(code_at.checked_add(code_size) == Some(d.len()), "the code does not end where the file does");
        ensure!(strings_at <= code_at, "the strings start after the code");

        let mut functions = Vec::new();
        let mut at = field(FUNCTIONS_AT)?;
        for index in 0..field(FUNCTION_COUNT_AT)? {
            let name = r.bytes::<FUNCTION_NAME_LEN>(at + 4)?;
            let name = name.split(|&b| b == 0).next().unwrap_or_default();
            let entry = r.u32(at + 4 + FUNCTION_NAME_LEN)? as usize;
            let arguments = r.u32(at + 8 + FUNCTION_NAME_LEN)? as usize;
            ensure!(entry % INSTRUCTION_LEN == 0 && entry < code_size, "function {index} starts at {entry:#x}");
            ensure!(arguments <= LOCALS, "function {index} takes {arguments} arguments");
            functions.push(Function {
                name: String::from_utf8_lossy(name).into_owned(),
                kind: FunctionKind::from_type(r.u16(at + 2)?).with_context(|| format!("function {index}"))?,
                entry: entry / INSTRUCTION_LEN,
                arguments,
            });
            at += FUNCTION_FIXED_LEN + arguments * ARGUMENT_LEN;
        }
        ensure!(at <= strings_at, "the function table runs into the strings");

        let mut code = Vec::with_capacity(code_size / INSTRUCTION_LEN);
        for at in (code_at..code_at + code_size).step_by(INSTRUCTION_LEN) {
            let head = r.bytes::<4>(at)?;
            let value = |index: usize| -> Result<i32> { Ok(r.u32(at + 4 + index * 4)? as i32) };
            code.push(Instruction { op: head[0], kinds: [head[1], head[2], head[3]], values: [value(0)?, value(1)?, value(2)?] });
        }

        let name = d[NAME_AT..NAME_END].split(|&b| b == 0).next().unwrap_or_default();
        Ok(Script {
            name: String::from_utf8_lossy(name).into_owned(),
            functions,
            code,
            strings: d[strings_at..code_at].to_vec(),
        })
    }

    /// Index of the function called `name`.
    pub fn function(&self, name: &str) -> Option<usize> {
        self.functions.iter().position(|function| function.name == name)
    }

    /// Index of the first function of `kind`: how the engine finds a script's entry points.
    pub fn entry(&self, kind: FunctionKind) -> Option<usize> {
        self.functions.iter().position(|function| function.kind == kind)
    }

    /// The string a script pushed as `value`.
    pub fn string(&self, value: i32) -> Result<&str> {
        let offset = value.checked_sub(STRING_BASE).and_then(|offset| usize::try_from(offset).ok());
        let tail = offset.and_then(|at| self.strings.get(at..));
        let tail = tail.with_context(|| format!("{value:#x} is not a string of the script"))?;
        let len = tail.iter().position(|&b| b == 0).unwrap_or(tail.len());
        std::str::from_utf8(&tail[..len]).context("string is not UTF-8")
    }

    /// The string a script pushed as `value`, where a script may have pushed a zero
    /// instead to say there is none.
    pub fn string_or_none(&self, value: i32) -> Result<Option<&str>> {
        if value == 0 { Ok(None) } else { self.string(value).map(Some) }
    }

    /// Every string the script has, in the order they are in the file.
    pub fn strings(&self) -> impl Iterator<Item = &str> {
        self.strings.split(|&b| b == 0).filter_map(|string| std::str::from_utf8(string).ok()).filter(|string| !string.is_empty())
    }

    /// Number of instructions.
    pub fn len(&self) -> usize {
        self.code.len()
    }

    pub fn is_empty(&self) -> bool {
        self.code.is_empty()
    }
}

/// Which of the engine's two function tables a call goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Table {
    /// Opcodes 0x82 and 0x86.
    One,
    /// Opcodes 0x87 and 0x88.
    Two,
}

impl Table {
    /// How function `number` of this table is called, if there is one.
    pub fn signature(self, number: usize) -> Option<Signature> {
        let table: &[Signature] = match self {
            Table::One => &signatures::TABLE_1,
            Table::Two => &signatures::TABLE_2,
        };
        table.get(number).copied().filter(|_| number != 0)
    }
}

/// What an engine function tells the interpreter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reply {
    /// The result, pushed if the function is one that returns a value.
    pub value: i32,
    /// Stop running the script here; it carries on from the next instruction when it
    /// is next resumed.
    pub suspend: bool,
}

impl Reply {
    pub const NONE: Reply = Reply { value: 0, suspend: false };

    pub fn value(value: i32) -> Reply {
        Reply { value, suspend: false }
    }
}

/// The engine, as scripts see it.
pub trait Host {
    /// Runs engine function `number` of `table` with `arguments`, in the order the
    /// script pushed them. String arguments are resolved with [`Script::string`].
    fn engine(&mut self, table: Table, number: usize, arguments: &[i32], script: &Script) -> Result<Reply>;

    /// The same, for a host whose functions read the script's variables: a text can
    /// name one for its value to be put in (0x27438). What the machine calls; a host
    /// with no use for them leaves it as it is.
    fn engine_with(
        &mut self,
        table: Table,
        number: usize,
        arguments: &[i32],
        script: &Script,
        variables: Variables,
    ) -> Result<Reply> {
        let _ = variables;
        self.engine(table, number, arguments, script)
    }
}

/// The variables a script has in view where it calls the engine: the locals of the
/// function that is calling, and the script's globals.
#[derive(Clone, Copy, Debug)]
pub struct Variables<'a> {
    pub locals: &'a [i32],
    pub globals: &'a [i32],
}

impl Variables<'_> {
    /// For a call made with no script behind it.
    pub const NONE: Variables<'static> = Variables { locals: &[], globals: &[] };

    /// Local `index` of the calling function, or zero if there is none.
    pub fn local(&self, index: usize) -> i32 {
        self.locals.get(index).copied().unwrap_or(0)
    }

    pub fn global(&self, index: usize) -> i32 {
        self.globals.get(index).copied().unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// The function that was started has returned.
    Finished,
    /// An engine function suspended the script; [`Machine::resume`] continues it.
    Suspended,
}

#[derive(Clone, Copy, Debug)]
struct Frame {
    function: usize,
    /// The next instruction to run.
    pc: usize,
}

/// The running state of one script.
#[derive(Clone)]
pub struct Machine {
    stack: Vec<i32>,
    pub globals: [i32; GLOBALS],
    /// Each function has one set of locals, zeroed whenever it is called: a function
    /// that calls itself overwrites its own.
    locals: Vec<[i32; LOCALS]>,
    frames: Vec<Frame>,
    /// The stack height each call level last saved (opcode 0x1C).
    marks: [usize; MAX_DEPTH],
}

impl Machine {
    pub fn new(script: &Script) -> Machine {
        Machine {
            stack: Vec::new(),
            globals: [0; GLOBALS],
            locals: vec![[0; LOCALS]; script.functions.len()],
            frames: Vec::new(),
            marks: [0; MAX_DEPTH],
        }
    }

    /// True while a suspended call is waiting to be resumed.
    pub fn suspended(&self) -> bool {
        !self.frames.is_empty()
    }

    /// Calls function `function` with `arguments` and runs it until it returns or an
    /// engine function suspends the script. A function that returns a value leaves it
    /// to be taken with [`Machine::pop`].
    pub fn call(&mut self, script: &Script, function: usize, arguments: &[i32], host: &mut dyn Host) -> Result<Status> {
        ensure!(self.frames.is_empty(), "the script is suspended and must be resumed first");
        self.stack.extend_from_slice(arguments);
        self.enter(script, function)?;
        self.run(script, host)
    }

    /// Carries on after a suspension.
    pub fn resume(&mut self, script: &Script, host: &mut dyn Host) -> Result<Status> {
        if self.frames.is_empty() {
            return Ok(Status::Finished);
        }
        self.run(script, host)
    }

    /// Calls a function the way the engine sends a signal: on a copy of the state, so
    /// that it works while the script is suspended. Of what the call changes only the
    /// variables are kept. Returns the function's result; a signal that suspends
    /// itself is dropped where it stopped and answers zero, as in the original, where
    /// the copy it ran on is thrown away.
    pub fn signal(&mut self, script: &Script, function: usize, arguments: &[i32], host: &mut dyn Host) -> Result<i32> {
        let mut copy = self.clone();
        copy.frames.clear();
        copy.stack.clear();
        let status = copy.call(script, function, arguments, host)?;
        let result = if status == Status::Finished { copy.pop()? } else { 0 };
        self.globals = copy.globals;
        self.locals = copy.locals;
        Ok(result)
    }

    /// Throws away a suspended call, so that the next one starts afresh: what the
    /// original does to an actor's update when a signal cancels it (0xC98E4).
    pub fn abort(&mut self) {
        self.frames.clear();
        self.stack.clear();
    }

    /// Takes the top of the stack: the result of a returning function just called.
    pub fn pop(&mut self) -> Result<i32> {
        self.stack.pop().context("the script's stack is empty")
    }

    fn push(&mut self, value: i32) -> Result<()> {
        ensure!(self.stack.len() < MAX_STACK, "the script's stack overflowed");
        self.stack.push(value);
        Ok(())
    }

    /// Starts a call: the arguments are popped into the function's first locals, the
    /// last one first, and the rest of its locals are cleared.
    fn enter(&mut self, script: &Script, function: usize) -> Result<()> {
        let target = script.functions.get(function).with_context(|| format!("there is no function {function}"))?;
        ensure!(self.frames.len() < MAX_DEPTH, "calls nest deeper than {MAX_DEPTH}");
        let mut locals = [0; LOCALS];
        for index in (0..target.arguments).rev() {
            locals[index] = self.pop()?;
        }
        self.locals[function] = locals;
        self.frames.push(Frame { function, pc: target.entry });
        Ok(())
    }

    fn run(&mut self, script: &Script, host: &mut dyn Host) -> Result<Status> {
        for _ in 0..MAX_STEPS {
            let Some(frame) = self.frames.last().copied() else { return Ok(Status::Finished) };
            let at = frame.pc;
            let instruction = *script.code.get(at).with_context(|| format!("ran off the code at instruction {at}"))?;
            let depth = self.frames.len() - 1;
            self.frames[depth].pc = at + 1;
            let suspend = self
                .step(script, host, frame.function, depth, at, &instruction)
                .with_context(|| format!("in {} at {:#x}", script.functions[frame.function].name, at * INSTRUCTION_LEN))?;
            if suspend {
                return Ok(Status::Suspended);
            }
        }
        bail!("the script ran {MAX_STEPS} instructions without finishing")
    }

    /// Runs one instruction; true if the script is to stop here for now.
    fn step(
        &mut self,
        script: &Script,
        host: &mut dyn Host,
        function: usize,
        depth: usize,
        at: usize,
        instruction: &Instruction,
    ) -> Result<bool> {
        let Instruction { op, kinds, values } = *instruction;
        match op {
            OP_ADD | OP_SUB | OP_MUL | OP_DIV | OP_MOD | OP_AND | OP_OR | OP_XOR | OP_SHIFT_LEFT | OP_SHIFT_RIGHT => {
                let (a, b) = self.operands(kinds, values)?;
                let result = match op {
                    OP_ADD => a.wrapping_add(b),
                    OP_SUB => a.wrapping_sub(b),
                    OP_MUL => a.wrapping_mul(b),
                    // The original divides without checking; a script never divides by zero.
                    OP_DIV => a.checked_div(b).context("division by zero")?,
                    OP_MOD => a.checked_rem(b).context("division by zero")?,
                    OP_AND => a & b,
                    OP_OR => a | b,
                    OP_XOR => a ^ b,
                    OP_SHIFT_LEFT => a.wrapping_shl(b as u32),
                    _ => a.wrapping_shr(b as u32),
                };
                self.push(result)?;
            }
            OP_LESS..=OP_NOT_EQUAL => {
                let (a, b) = self.operands(kinds, values)?;
                let holds = match op - OP_LESS {
                    0 => a < b,
                    1 => a <= b,
                    2 => a > b,
                    3 => a >= b,
                    4 => a == b,
                    _ => a != b,
                };
                self.push(holds as i32)?;
            }
            OP_NOT => {
                let value = self.pop()?;
                self.push((value == 0) as i32)?;
            }
            OP_INCREMENT | OP_DECREMENT => {
                let step = if op == OP_INCREMENT { 1 } else { -1 };
                let variable = self.variable(kinds[0], values[0], function)?;
                *variable = variable.wrapping_add(step);
            }
            OP_MOVE => {
                let value = match kinds[0] {
                    IMMEDIATE => values[0],
                    POP => self.pop()?,
                    kind => *self.variable(kind, values[0], function)?,
                };
                match kinds[1] {
                    PUSH => self.push(value)?,
                    kind => *self.variable(kind, values[1], function)? = value,
                }
            }
            OP_PUSH => self.push(values[0])?,
            OP_PUSH_STRING => self.push(STRING_BASE.wrapping_add(values[0]))?,
            OP_SAVE_STACK => self.marks[depth] = self.stack.len(),
            OP_RESTORE_STACK => {
                ensure!(self.marks[depth] <= self.stack.len(), "the saved stack height is above the stack");
                self.stack.truncate(self.marks[depth]);
            }
            OP_SKIP_IF => {
                let condition = if kinds[0] == IMMEDIATE { values[0] } else { self.pop()? };
                if condition != 0 {
                    self.frames[depth].pc = at + 2;
                }
            }
            OP_JUMP => {
                ensure!(values[0] % INSTRUCTION_LEN as i32 == 0, "jump by {} bytes", values[0]);
                let target = at as i64 + (values[0] / INSTRUCTION_LEN as i32) as i64;
                self.frames[depth].pc = usize::try_from(target).ok().context("jump to before the code")?;
            }
            OP_CALL => self.enter(script, usize::try_from(values[0]).ok().context("call of a negative function")?)?,
            OP_RETURN => {
                self.frames.pop();
            }
            OP_ENGINE_1 | OP_ENGINE_1_HIGH | OP_ENGINE_2 | OP_ENGINE_2_HIGH => {
                let table = if matches!(op, OP_ENGINE_1 | OP_ENGINE_1_HIGH) { Table::One } else { Table::Two };
                let high = matches!(op, OP_ENGINE_1_HIGH | OP_ENGINE_2_HIGH);
                let number = kinds[0] as usize + if high { 0x100 } else { 0 };
                let signature =
                    table.signature(number).with_context(|| format!("there is no engine function {number} in {table:?}"))?;
                let first = self.stack.len().checked_sub(signature.arguments).context("too few arguments on the stack")?;
                let variables = Variables { locals: &self.locals[function], globals: &self.globals };
                let reply = host
                    .engine_with(table, number, &self.stack[first..], script, variables)
                    .with_context(|| format!("engine function {number} of {table:?}"))?;
                self.stack.truncate(first);
                if signature.returns {
                    self.push(reply.value)?;
                }
                return Ok(reply.suspend);
            }
            // Every other opcode does nothing in the original either; the compiler
            // emits 0x1B after pushes.
            _ => {}
        }
        Ok(false)
    }

    /// The two operands of an arithmetic or comparison instruction. With both popped,
    /// the second one is on top of the stack.
    fn operands(&mut self, kinds: [u8; 3], values: [i32; 3]) -> Result<(i32, i32)> {
        Ok(match (kinds[0], kinds[1]) {
            (IMMEDIATE, IMMEDIATE) => (values[0], values[1]),
            (IMMEDIATE, _) => (values[0], self.pop()?),
            (_, IMMEDIATE) => (self.pop()?, values[1]),
            _ => {
                let b = self.pop()?;
                (self.pop()?, b)
            }
        })
    }

    fn variable(&mut self, kind: u8, index: i32, function: usize) -> Result<&mut i32> {
        let slot = usize::try_from(index).ok();
        let variable = match kind {
            GLOBAL => slot.and_then(|slot| self.globals.get_mut(slot)),
            LOCAL => slot.and_then(|slot| self.locals[function].get_mut(slot)),
            _ => bail!("operand kind {kind:#x} is not a variable"),
        };
        variable.with_context(|| format!("there is no variable {index}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAD: u8 = 0x5a;

    /// Assembles a script from functions given as (name, type, argument count,
    /// instructions) and its strings.
    fn assemble(functions: &[(&str, u16, u32, Vec<[i32; 7]>)], strings: &[u8]) -> Vec<u8> {
        let mut table = Vec::new();
        let mut code = Vec::new();
        for (name, kind, arguments, instructions) in functions {
            let mut entry = vec![0u8; FUNCTION_FIXED_LEN + *arguments as usize * ARGUMENT_LEN];
            entry[2..4].copy_from_slice(&kind.to_le_bytes());
            entry[4..4 + name.len()].copy_from_slice(name.as_bytes());
            entry[36..40].copy_from_slice(&(code.len() as u32).to_le_bytes());
            entry[40..44].copy_from_slice(&arguments.to_le_bytes());
            table.extend(entry);
            for i in instructions {
                code.extend([i[0] as u8, i[1] as u8, i[2] as u8, i[3] as u8]);
                for value in &i[4..] {
                    code.extend(value.to_le_bytes());
                }
            }
        }
        let functions_at = HEADER_LEN;
        let strings_at = functions_at + table.len();
        let code_at = strings_at + strings.len();
        let mut d = vec![0u8; HEADER_LEN];
        d[..4].copy_from_slice(MAGIC);
        d[NAME_AT..NAME_AT + 8].copy_from_slice(b"test.bms");
        let mut field = |at: usize, value: usize| d[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
        field(SIZE_AT, code_at + code.len());
        field(FUNCTION_COUNT_AT, functions.len());
        field(FUNCTIONS_AT, functions_at);
        field(STRINGS_AT, strings_at);
        field(CODE_SIZE_AT, code.len());
        field(CODE_AT, code_at);
        d.extend(table);
        d.extend(strings);
        d.extend(code);
        d
    }

    fn op(op: u8, kinds: [u8; 3], values: [i32; 3]) -> [i32; 7] {
        [op as i32, kinds[0] as i32, kinds[1] as i32, kinds[2] as i32, values[0], values[1], values[2]]
    }

    fn push(value: i32) -> [i32; 7] {
        op(OP_PUSH, [IMMEDIATE, PAD, PAD], [value, 0, 0])
    }

    fn ret() -> [i32; 7] {
        op(OP_RETURN, [PAD; 3], [0; 3])
    }

    /// Records the calls it gets; function 6 of table 2 answers 7, and function 9
    /// suspends the script.
    #[derive(Default)]
    struct Recorder(Vec<(Table, usize, Vec<i32>)>);

    impl Host for Recorder {
        fn engine(&mut self, table: Table, number: usize, arguments: &[i32], _: &Script) -> Result<Reply> {
            self.0.push((table, number, arguments.to_vec()));
            Ok(Reply { value: 7, suspend: number == 9 })
        }
    }

    #[test]
    fn subtraction_and_comparison_take_the_earlier_push_first() {
        let main = vec![
            push(10),
            push(3),
            op(OP_SUB, [POP, POP, PUSH], [0; 3]),
            push(8),
            // 7 < 8
            op(OP_LESS, [POP, POP, PUSH], [0; 3]),
            // 5 - (that 1)
            op(OP_SUB, [IMMEDIATE, POP, PUSH], [5, 0, 0]),
            ret(),
        ];
        let script = Script::parse(&assemble(&[("main", 0xe1, 0, main)], b"")).unwrap();
        let mut machine = Machine::new(&script);
        assert_eq!(machine.call(&script, 0, &[], &mut Recorder::default()).unwrap(), Status::Finished);
        assert_eq!(machine.pop().unwrap(), 4);
    }

    #[test]
    fn a_true_condition_skips_the_jump_after_it() {
        // if (argument == 1) result = 20 else result = 30
        let main = vec![
            op(OP_MOVE, [LOCAL, PUSH, PAD], [0, 0, 0]),
            op(OP_LESS + 4, [IMMEDIATE, POP, PUSH], [1, 0, 0]),
            op(OP_SKIP_IF, [0xf1, PAD, PAD], [0; 3]),
            op(OP_JUMP, [IMMEDIATE, PAD, PAD], [0x30, 0, 0]),
            push(20),
            ret(),
            push(30),
            ret(),
        ];
        let script = Script::parse(&assemble(&[("main", 0xe1, 1, main)], b"")).unwrap();
        let mut machine = Machine::new(&script);
        for (argument, expected) in [(1, 20), (2, 30)] {
            machine.call(&script, 0, &[argument], &mut Recorder::default()).unwrap();
            assert_eq!(machine.pop().unwrap(), expected);
        }
    }

    #[test]
    fn calls_pass_arguments_into_locals_and_engine_calls_pop_theirs() {
        // helper(a, b): engine 2:8(b, a); returns engine 2:6(a)
        let helper = vec![
            op(OP_MOVE, [LOCAL, PUSH, PAD], [1, 0, 0]),
            op(OP_MOVE, [LOCAL, PUSH, PAD], [0, 0, 0]),
            op(OP_ENGINE_2, [8, PAD, PAD], [0; 3]),
            op(OP_MOVE, [LOCAL, PUSH, PAD], [0, 0, 0]),
            op(OP_ENGINE_2, [6, PAD, PAD], [0; 3]),
            ret(),
        ];
        // main: global 5 = helper(11, 22) + 1
        let main = vec![
            push(11),
            push(22),
            op(OP_CALL, [0, POP, PUSH], [1, 0, 0]),
            op(OP_ADD, [POP, IMMEDIATE, PUSH], [0, 1, 0]),
            op(OP_MOVE, [POP, GLOBAL, PAD], [0, 5, 0]),
            op(OP_INCREMENT, [GLOBAL, PAD, PAD], [5, 0, 0]),
            ret(),
        ];
        let script = Script::parse(&assemble(&[("main", 0xe0, 0, main), ("helper", 0xe1, 2, helper)], b"")).unwrap();
        let mut machine = Machine::new(&script);
        let mut host = Recorder::default();
        assert_eq!(machine.call(&script, 0, &[], &mut host).unwrap(), Status::Finished);
        assert_eq!(host.0, vec![(Table::Two, 8, vec![22, 11]), (Table::Two, 6, vec![11])]);
        assert_eq!(machine.globals[5], 9);
        assert!(machine.pop().is_err(), "the stack should be empty again");
    }

    #[test]
    fn a_suspended_script_resumes_after_the_call_that_stopped_it() {
        let inner = vec![push(1), op(OP_ENGINE_2, [9, PAD, PAD], [0; 3]), op(OP_INCREMENT, [GLOBAL, PAD, PAD], [0, 0, 0]), ret()];
        let outer = vec![op(OP_CALL, [0, POP, PUSH], [1, 0, 0]), op(OP_INCREMENT, [GLOBAL, PAD, PAD], [0, 0, 0]), ret()];
        // The signal reads and changes a global while the script is suspended.
        let signal = vec![op(OP_MOVE, [GLOBAL, PUSH, PAD], [0, 0, 0]), op(OP_MOVE, [IMMEDIATE, GLOBAL, PAD], [40, 0, 0]), ret()];
        let functions = [("outer", 0xe0, 0, outer), ("inner", 0xe0, 0, inner), ("signal", 0xec, 3, signal)];
        let script = Script::parse(&assemble(&functions, b"")).unwrap();
        let mut machine = Machine::new(&script);
        let mut host = Recorder::default();
        assert_eq!(machine.call(&script, 0, &[], &mut host).unwrap(), Status::Suspended);
        assert_eq!(machine.globals[0], 0);
        assert!(machine.suspended());

        let signal = script.entry(FunctionKind::Signal).unwrap();
        assert_eq!(machine.signal(&script, signal, &[1, 2, 3], &mut host).unwrap(), 0);
        assert_eq!(machine.globals[0], 40);

        assert_eq!(machine.resume(&script, &mut host).unwrap(), Status::Finished);
        assert_eq!(machine.globals[0], 42);
        assert!(!machine.suspended());
    }

    #[test]
    fn an_aborted_call_can_be_made_again_from_the_top() {
        let main = vec![
            op(OP_INCREMENT, [GLOBAL, PAD, PAD], [0, 0, 0]),
            push(1),
            op(OP_ENGINE_2, [9, PAD, PAD], [0; 3]),
            op(OP_INCREMENT, [GLOBAL, PAD, PAD], [1, 0, 0]),
            ret(),
        ];
        let script = Script::parse(&assemble(&[("main", 0xed, 0, main)], b"")).unwrap();
        let mut machine = Machine::new(&script);
        let mut host = Recorder::default();
        assert_eq!(machine.call(&script, 0, &[], &mut host).unwrap(), Status::Suspended);
        machine.abort();
        assert!(!machine.suspended());
        assert_eq!(machine.call(&script, 0, &[], &mut host).unwrap(), Status::Suspended);
        assert_eq!(machine.resume(&script, &mut host).unwrap(), Status::Finished);
        // The part before the wait ran twice, the part after it once.
        assert_eq!((machine.globals[0], machine.globals[1]), (2, 1));
    }

    #[test]
    fn a_signal_that_suspends_is_dropped_but_keeps_its_variables() {
        let update = vec![push(1), op(OP_ENGINE_2, [9, PAD, PAD], [0; 3]), ret()];
        let signal = vec![
            op(OP_MOVE, [IMMEDIATE, GLOBAL, PAD], [5, 0, 0]),
            push(1),
            op(OP_ENGINE_2, [9, PAD, PAD], [0; 3]),
            op(OP_MOVE, [IMMEDIATE, GLOBAL, PAD], [6, 0, 0]),
            push(1),
            ret(),
        ];
        let script = Script::parse(&assemble(&[("update", 0xed, 0, update), ("signal", 0xec, 3, signal)], b"")).unwrap();
        let mut machine = Machine::new(&script);
        let mut host = Recorder::default();
        machine.call(&script, 0, &[], &mut host).unwrap();
        assert_eq!(machine.signal(&script, 1, &[1, 0, 0], &mut host).unwrap(), 0);
        assert_eq!(machine.globals[0], 5);
        // The update is still waiting where it was.
        assert_eq!(machine.resume(&script, &mut host).unwrap(), Status::Finished);
    }

    #[test]
    fn strings_are_pushed_as_values_that_name_them() {
        let main = vec![
            op(OP_PUSH_STRING, [IMMEDIATE, 0xf1, PAD], [6, 0, 0]),
            op(0x1b, [0xf1, PAD, PAD], [0; 3]),
            op(OP_ENGINE_2, [1, PAD, PAD], [0; 3]),
            op(OP_PUSH_STRING, [IMMEDIATE, 0xf1, PAD], [0, 0, 0]),
            op(0x1b, [0xf1, PAD, PAD], [0; 3]),
            op(OP_ENGINE_2, [1, PAD, PAD], [0; 3]),
            ret(),
        ];
        let script = Script::parse(&assemble(&[("main", 0xe0, 0, main)], b"first\0second\0")).unwrap();
        let mut host = Recorder::default();
        Machine::new(&script).call(&script, 0, &[], &mut host).unwrap();
        assert_eq!(script.string(host.0[0].2[0]).unwrap(), "second");
        // The first string is not the zero a script hands over for no string at all.
        assert_ne!(host.0[1].2[0], 0);
        assert_eq!(script.string_or_none(host.0[1].2[0]).unwrap(), Some("first"));
        assert_eq!(script.string_or_none(0).unwrap(), None);
        assert!(script.string(0).is_err());
        assert_eq!(script.name, "test.bms");
    }
}
