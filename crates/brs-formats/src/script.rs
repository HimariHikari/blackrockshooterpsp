//! Compiled scripts (`SSCR`) and the virtual machine that runs them.
//!
//! Each field character, map and event carries one. A script is a symbol table, a
//! relocation table, an import table, and a blob of code and data. The instruction set
//! and calling convention were read out of the game's interpreter (the function at
//! 0x112790 in the executable); see `docs/FORMAT_NOTES.md`.

use std::cell::{Cell, RefCell};

use anyhow::{bail, ensure, Context, Result};

use crate::reader::Reader;

pub const MAGIC: &[u8; 4] = b"SSCR";

/// The header is 0x28 bytes: the magic, a version word, the symbol and relocation
/// counts, then offsets, the first of which is that of the symbol table, which always
/// follows the header directly.
const HEADER_LEN: usize = 0x28;
/// Nine of the game's scripts are of the format's first version (the word at 4 is
/// 0x10000, not 0x20000), whose header ends after the offsets of the symbols and of
/// the relocations (0x1122B0, 0x1122D8): such a script has no imports.
const FIRST_VERSION: u16 = 1;
const FIRST_VERSION_HEADER_LEN: usize = 0x14;

/// Where a script's image and the stack sit in the machine's address space. Anything
/// outside them and the memory the host has mapped (a null pointer, most obviously)
/// is an error rather than a wild read.
pub const IMAGE_BASE: u32 = 0x1000_0000;
const STACK_BASE: u32 = 0x7000_0000;
const STACK_LEN: usize = 0x4000;

/// Bit 0 of a call target marks an engine function; the rest is its number.
const NATIVE_BIT: u32 = 1;
/// Numbers given to a script's imports, above any fixed engine function number.
const IMPORT_NUMBER_BASE: u32 = 0x1_0000;

/// Instructions one call may execute before it is treated as stuck.
const STEP_LIMIT: usize = 10_000_000;

#[derive(Clone, Debug)]
pub struct Script {
    /// The file with relocations applied, addressed from `base`.
    image: Vec<u8>,
    /// The machine address of the image's first byte.
    base: u32,
    /// Where the code and data end, as an offset in the image.
    data_end: usize,
    pub symbols: Vec<Symbol>,
    /// Functions the script expects the engine or another script to supply, by name.
    pub imports: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Symbol {
    pub name: String,
    /// Machine address of the function or data.
    pub address: u32,
}

impl Script {
    pub fn parse(d: &[u8]) -> Result<Script> {
        Script::parse_at(d, IMAGE_BASE)
    }

    /// Parses a script whose image is to sit at machine address `base`: for a file
    /// of tables that is mapped into another script's machine next to that script's
    /// own image (see [`Machine::map`]).
    pub fn parse_at(d: &[u8], base: u32) -> Result<Script> {
        ensure!(d.starts_with(MAGIC), "not a script");
        ensure!(d.len() >= FIRST_VERSION_HEADER_LEN, "file ends inside its header");
        let mut image = d.to_vec();

        let (symbols, relocations, imports, fixups, data_end) = if Reader(&image).u16(0x06)? <= FIRST_VERSION {
            let r = Reader(&image);
            let (symbol_count, relocation_count) = (r.u16(0x08)? as usize, r.u16(0x0a)? as usize);
            let (symbols_at, relocations_at) = (r.u32(0x0c)? as usize, r.u32(0x10)? as usize);
            let mut symbols = Vec::with_capacity(symbol_count);
            for at in (0..symbol_count).map(|index| symbols_at + index * 8) {
                symbols.push(Symbol { name: c_string(&image, r.u32(at + 4)? as usize)?, address: base + r.u32(at)? });
            }
            let mut relocations = Vec::with_capacity(relocation_count);
            for at in (0..relocation_count).map(|index| relocations_at + index * 8) {
                relocations.push((r.u32(at)? as usize, r.u32(at + 4)?));
            }
            (symbols, relocations, Vec::new(), Vec::new(), image.len())
        } else {
            ensure!(d.len() >= HEADER_LEN, "file ends inside its header");
            let r = Reader(&image);
            let symbols_at = r.u32(0x0c)? as usize;
            let relocations_at = r.u32(0x10)? as usize;
            let code_at = r.u32(0x14)? as usize;
            let imports_at = r.u32(0x18)? as usize;
            let import_count = r.u16(0x1c)? as usize;
            let fixup_count = r.u16(0x1e)? as usize;
            let fixups_at = r.u32(0x24)? as usize;
            ensure!(
                symbols_at <= relocations_at && relocations_at <= code_at && code_at <= image.len(),
                "script header offsets are out of order"
            );

            // Symbols are (value, name offset) pairs running up to the first name.
            let mut symbols = Vec::new();
            let (mut at, mut names_at) = (symbols_at, relocations_at);
            while at < names_at {
                let name_at = r.u32(at + 4)? as usize;
                names_at = names_at.min(name_at);
                symbols.push(Symbol { name: c_string(&image, name_at)?, address: base + r.u32(at)? });
                at += 8;
            }

            // Relocations are (location of a 32-bit immediate, target offset) pairs.
            let mut relocations = Vec::new();
            for at in (relocations_at..code_at.saturating_sub(7)).step_by(8) {
                relocations.push((r.u32(at)? as usize, r.u32(at + 4)?));
            }

            // Imports are (zero, name offset) pairs; fixups are (location, import index).
            let mut imports = Vec::with_capacity(import_count);
            for i in 0..import_count {
                imports.push(c_string(&image, r.u32(imports_at + i * 8 + 4)? as usize)?);
            }
            let mut fixups = Vec::with_capacity(fixup_count);
            for i in 0..fixup_count {
                let index = r.u32(fixups_at + i * 8 + 4)?;
                ensure!((index as usize) < import_count, "import fixup {i} refers to import {index}");
                fixups.push((r.u32(fixups_at + i * 8)? as usize, index));
            }
            (symbols, relocations, imports, fixups, imports_at.clamp(code_at, image.len()))
        };

        let mut patch = |location: usize, value: u32| -> Result<()> {
            let slot = image.get_mut(location..location + 4).context("relocation outside the script")?;
            slot.copy_from_slice(&value.to_le_bytes());
            Ok(())
        };
        for (location, target) in relocations {
            patch(location, base + target)?;
        }
        for (location, index) in fixups {
            patch(location, ((IMPORT_NUMBER_BASE + index) << 8) | NATIVE_BIT)?;
        }
        Ok(Script { image, base, data_end, symbols, imports })
    }

    /// The machine address of the image's first byte.
    pub fn base(&self) -> u32 {
        self.base
    }

    /// The whole image, with its pointers made machine addresses.
    pub fn image(&self) -> &[u8] {
        &self.image
    }

    /// Address of a function or data symbol.
    pub fn symbol(&self, name: &str) -> Option<u32> {
        self.symbols.iter().find(|s| s.name == name).map(|s| s.address)
    }

    /// Reads `N` consecutive floats of a data symbol, e.g. a table of speeds.
    pub fn floats<const N: usize>(&self, name: &str) -> Option<[f32; N]> {
        let at = (self.symbol(name)? - self.base) as usize;
        Reader(&self.image).f32s(at).ok()
    }

    /// Reads a 32-bit integer data symbol.
    pub fn integer(&self, name: &str) -> Option<u32> {
        let at = (self.symbol(name)? - self.base) as usize;
        Reader(&self.image).u32(at).ok()
    }

    /// What a data symbol holds: the image from it up to the next symbol, or to the
    /// end of the code and data if it is the last.
    pub fn extent(&self, name: &str) -> Option<&[u8]> {
        let address = self.symbol(name)?;
        let next = self.symbols.iter().map(|symbol| symbol.address).filter(|&other| other > address).min();
        let end = next.map_or(self.data_end, |next| (next - self.base) as usize);
        self.image.get((address - self.base) as usize..end)
    }

    /// The image from machine address `address` to its end, for reading tables. Pointers
    /// stored in the image are machine addresses too (relocations have been applied).
    pub fn data(&self, address: u32) -> Option<&[u8]> {
        self.image.get(address.checked_sub(self.base)? as usize..)
    }

    /// The zero-terminated string at machine address `address`.
    pub fn string(&self, address: u32) -> Option<String> {
        let tail = self.data(address)?;
        let len = tail.iter().position(|&b| b == 0)?;
        Some(String::from_utf8_lossy(&tail[..len]).into_owned())
    }

    /// Whether `value` is a machine address inside the image.
    pub fn is_address(&self, value: u32) -> bool {
        value >= self.base && ((value - self.base) as usize) < self.image.len()
    }
}

fn c_string(d: &[u8], at: usize) -> Result<String> {
    let tail = d.get(at..).context("name offset outside the script")?;
    let len = tail.iter().position(|&b| b == 0).unwrap_or(tail.len());
    Ok(String::from_utf8_lossy(&tail[..len]).into_owned())
}

/// What a script can call out to.
pub trait Host {
    /// Runs engine function `number` and returns its result (0 if it has none).
    /// Imports are numbered from [`Machine::import_number`].
    fn native(&mut self, number: u32, call: &Call) -> Result<u32>;
}

/// The arguments of an engine call in progress.
pub struct Call<'m> {
    machine: &'m Machine,
    /// Stack pointer at the call: arguments sit here, first argument lowest.
    arguments_at: u32,
    resumed: bool,
    wait: Cell<bool>,
    /// What the engine function has put into the machine's memory: written once it
    /// returns, as it only reads the machine while it runs.
    writes: RefCell<Vec<(u32, Vec<u8>)>>,
}

impl Call<'_> {
    /// Has `bytes` written at a machine address as the engine function returns: for
    /// the functions that answer through a pointer they are given.
    pub fn write(&self, address: u32, bytes: &[u8]) {
        self.writes.borrow_mut().push((address, bytes.to_vec()));
    }

    /// False the first time an engine function is called, true each time it is called
    /// again because it asked to [`wait`](Call::wait) (the original's 0x113510).
    pub fn resumed(&self) -> bool {
        self.resumed
    }

    /// Stops the script inside this call: when the script is next resumed the engine
    /// function is called again, with the same arguments, and the script only goes on
    /// once it returns without asking this (0x1134A4; an engine function that does not
    /// ask it has, in the original's terms, set the script running again, 0x1134B8).
    pub fn wait(&self) {
        self.wait.set(true);
    }

    pub fn int(&self, index: u32) -> Result<i32> {
        Ok(self.machine.read_u32(self.arguments_at + index * 4)? as i32)
    }

    pub fn float(&self, index: u32) -> Result<f32> {
        Ok(f32::from_bits(self.machine.read_u32(self.arguments_at + index * 4)?))
    }

    /// The text an argument points at.
    pub fn string(&self, index: u32) -> Result<String> {
        self.machine.string(self.int(index)? as u32)
    }

    /// The machine, for following a pointer among the arguments.
    pub fn machine(&self) -> &Machine {
        self.machine
    }
}

/// Which of a machine's stretches of memory an address falls in.
#[derive(Clone, Copy)]
enum Region {
    Image,
    Stack,
    Mapped(usize),
}

// Register numbers. The interpreter keeps them in its call context; scripts use the
// first few for fixed purposes.
const PC: usize = 0;
const BP: usize = 1;
const SP: usize = 2;
const RV: usize = 3;
const REGISTER_COUNT: usize = 16;

const FLAG_EQUAL: u32 = 1;
const FLAG_LESS: u32 = 2;

/// One running instance of a script: its own copy of the image (scripts write to their
/// data) and a stack.
pub struct Machine {
    image: Vec<u8>,
    /// The machine address of the image's first byte.
    base: u32,
    stack: Vec<u8>,
    /// Memory the host has put into the address space, by its first address: the
    /// game's tables, and records it hands the script by address.
    mapped: Vec<(u32, Vec<u8>)>,
    registers: [u32; REGISTER_COUNT],
    flags: u32,
    /// The engine function the script is waiting in, and where its arguments are.
    waiting_in: Option<(u32, u32)>,
}

/// How a run of a script ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Run {
    /// The function returned this.
    Returned(u32),
    /// An engine function is waiting; [`Machine::resume`] calls it again.
    Waiting,
}

impl Machine {
    pub fn new(script: &Script) -> Machine {
        Machine {
            image: script.image.clone(),
            base: script.base,
            stack: vec![0; STACK_LEN],
            mapped: Vec::new(),
            registers: [0; REGISTER_COUNT],
            flags: 0,
            waiting_in: None,
        }
    }

    /// Puts `bytes` into the machine's address space from `base` on, in place of
    /// whatever the host had put there before. The script can read and write them.
    pub fn map(&mut self, base: u32, bytes: Vec<u8>) {
        match self.mapped.iter_mut().find(|(at, _)| *at == base) {
            Some(mapping) => mapping.1 = bytes,
            None => self.mapped.push((base, bytes)),
        }
    }

    /// The memory the host has mapped at `base`, as the script has left it.
    pub fn mapped(&self, base: u32) -> Option<&[u8]> {
        self.mapped.iter().find(|(at, _)| *at == base).map(|(_, bytes)| &bytes[..])
    }

    /// The zero-terminated text at a machine address.
    pub fn string(&self, address: u32) -> Result<String> {
        let mut bytes = Vec::new();
        loop {
            let byte = self.read_bytes::<1>(address + bytes.len() as u32)?[0];
            if byte == 0 {
                return Ok(String::from_utf8_lossy(&bytes).into_owned());
            }
            bytes.push(byte);
        }
    }

    /// The number a host sees for the script's `index`-th import.
    pub fn import_number(index: usize) -> u32 {
        IMPORT_NUMBER_BASE + index as u32
    }

    /// Calls the function at `address` with 32-bit arguments (use `f32::to_bits` for
    /// floats) and returns the 32-bit result register.
    pub fn call(&mut self, address: u32, arguments: &[u32], host: &mut dyn Host) -> Result<u32> {
        match self.start(address, arguments, host)? {
            Run::Returned(value) => Ok(value),
            Run::Waiting => bail!("the script waits in an engine function, which a plain call cannot do"),
        }
    }

    /// Starts the function at `address`, dropping whatever the machine was doing, and
    /// runs it until it returns or an engine function waits.
    pub fn start(&mut self, address: u32, arguments: &[u32], host: &mut dyn Host) -> Result<Run> {
        self.waiting_in = None;
        self.registers[SP] = STACK_BASE + STACK_LEN as u32;
        self.registers[BP] = self.registers[SP];
        // The first argument ends up lowest; a zero return address ends the run.
        for &argument in arguments.iter().rev() {
            self.push(argument)?;
        }
        self.push(0)?;
        self.registers[PC] = address;
        self.run(host)
    }

    /// Carries on a script that is waiting: the engine function it waits in is called
    /// again, and the script runs on if that no longer waits (the top of 0x112790).
    pub fn resume(&mut self, host: &mut dyn Host) -> Result<Run> {
        if let Some((number, arguments_at)) = self.waiting_in.take() {
            self.native_at(number, arguments_at, true, host)?;
        }
        self.run(host)
    }

    /// True while an engine function is waiting.
    pub fn waiting(&self) -> bool {
        self.waiting_in.is_some()
    }

    fn run(&mut self, host: &mut dyn Host) -> Result<Run> {
        for _ in 0..STEP_LIMIT {
            if self.waiting_in.is_some() {
                return Ok(Run::Waiting);
            }
            if self.registers[PC] == 0 {
                return Ok(Run::Returned(self.registers[RV]));
            }
            self.step(host)?;
        }
        bail!("script ran {STEP_LIMIT} instructions without returning");
    }

    /// Where `len` bytes at `address` are: the stretch of memory, and the offset in it.
    fn locate(&self, address: u32, len: usize) -> Result<(Region, usize)> {
        let within = |base: u32, size: usize| address.checked_sub(base).map(|at| at as usize).filter(|at| at + len <= size);
        if let Some(at) = within(STACK_BASE, self.stack.len()) {
            return Ok((Region::Stack, at));
        }
        if let Some(at) = within(self.base, self.image.len()) {
            return Ok((Region::Image, at));
        }
        for (index, (base, bytes)) in self.mapped.iter().enumerate() {
            if let Some(at) = within(*base, bytes.len()) {
                return Ok((Region::Mapped(index), at));
            }
        }
        bail!("address {address:#x} is outside the script's memory")
    }

    fn memory(&self, address: u32, len: usize) -> Result<(&[u8], usize)> {
        let (region, at) = self.locate(address, len)?;
        let bytes = match region {
            Region::Image => &self.image,
            Region::Stack => &self.stack,
            Region::Mapped(index) => &self.mapped[index].1,
        };
        Ok((bytes, at))
    }

    /// The 32-bit word at a machine address.
    pub fn read_u32(&self, address: u32) -> Result<u32> {
        let (region, at) = self.memory(address, 4)?;
        Ok(u32::from_le_bytes(region[at..at + 4].try_into().unwrap()))
    }

    fn read_bytes<const N: usize>(&self, address: u32) -> Result<[u8; N]> {
        let (region, at) = self.memory(address, N)?;
        Ok(region[at..at + N].try_into().unwrap())
    }

    /// `len` bytes at a machine address.
    pub fn bytes(&self, address: u32, len: usize) -> Result<&[u8]> {
        let (region, at) = self.memory(address, len)?;
        Ok(&region[at..at + len])
    }

    /// Writes into the machine's memory: its image, its stack, or what is mapped.
    pub fn write(&mut self, address: u32, bytes: &[u8]) -> Result<()> {
        let (region, at) = self.locate(address, bytes.len())?;
        let region = match region {
            Region::Image => &mut self.image,
            Region::Stack => &mut self.stack,
            Region::Mapped(index) => &mut self.mapped[index].1,
        };
        region[at..at + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }

    fn push(&mut self, value: u32) -> Result<()> {
        self.registers[SP] -= 4;
        self.write(self.registers[SP], &value.to_le_bytes())
    }

    fn pop(&mut self) -> Result<u32> {
        let value = self.read_u32(self.registers[SP])?;
        self.registers[SP] += 4;
        Ok(value)
    }

    fn native(&mut self, number: u32, host: &mut dyn Host) -> Result<()> {
        self.native_at(number, self.registers[SP], false, host)
    }

    fn native_at(&mut self, number: u32, arguments_at: u32, resumed: bool, host: &mut dyn Host) -> Result<()> {
        let call = Call { machine: self, arguments_at, resumed, wait: Cell::new(false), writes: RefCell::new(Vec::new()) };
        let result = host.native(number, &call)?;
        let (waits, writes) = (call.wait.get(), call.writes.take());
        for (address, bytes) in writes {
            self.write(address, &bytes)?;
        }
        if waits {
            self.waiting_in = Some((number, arguments_at));
        } else {
            self.registers[RV] = result;
        }
        Ok(())
    }

    fn compare(&mut self, equal: bool, less: bool) {
        self.flags = (self.flags & !(FLAG_EQUAL | FLAG_LESS))
            | if equal { FLAG_EQUAL } else { 0 }
            | if less { FLAG_LESS } else { 0 };
    }

    /// Executes one instruction. Operand order follows the encoding: for two-register
    /// forms the first byte is the source and the second the destination.
    fn step(&mut self, host: &mut dyn Host) -> Result<()> {
        let pc = self.registers[PC];
        let opcode = self.read_bytes::<1>(pc)?[0];
        let byte = |m: &Machine, offset: u32| -> Result<usize> { Ok(m.read_bytes::<1>(pc + offset)?[0] as usize) };
        let register = |m: &Machine, offset: u32| -> Result<usize> {
            let index = byte(m, offset)?;
            ensure!(index < REGISTER_COUNT, "register {index} at {pc:#x}");
            Ok(index)
        };
        let immediate = |m: &Machine, offset: u32| -> Result<u32> { Ok(u32::from_le_bytes(m.read_bytes(pc + offset)?)) };

        // Most instructions fall through to the next one; jumps overwrite this.
        let next = |m: &mut Machine, len: u32| m.registers[PC] = pc + len;

        match opcode {
            0x00 => {
                let r = register(self, 1)?;
                next(self, 2);
                self.push(self.registers[r])?;
            }
            0x01 => {
                let r = register(self, 1)?;
                next(self, 2);
                self.registers[r] = self.pop()?;
            }
            0x02 => {
                let (src, dst) = (register(self, 1)?, register(self, 2)?);
                self.registers[dst] = self.registers[src];
                next(self, 3);
            }
            0x03 => {
                let (value, dst) = (immediate(self, 1)?, register(self, 5)?);
                self.registers[dst] = value;
                next(self, 6);
            }
            // Loads: destination = memory[base + offset], sign-extended for the narrow ones.
            0x04 | 0x08 | 0x0c => {
                let (offset, base, dst) = (immediate(self, 1)?, register(self, 5)?, register(self, 6)?);
                let address = self.registers[base].wrapping_add(offset);
                self.registers[dst] = match opcode {
                    0x04 => self.read_u32(address)?,
                    0x08 => i16::from_le_bytes(self.read_bytes(address)?) as u32,
                    _ => self.read_bytes::<1>(address)?[0] as i8 as u32,
                };
                next(self, 7);
            }
            // Stores: memory[base + offset] = source.
            0x05 | 0x09 | 0x0d => {
                let (src, offset, base) = (register(self, 1)?, immediate(self, 2)?, register(self, 6)?);
                let address = self.registers[base].wrapping_add(offset);
                let bytes = self.registers[src].to_le_bytes();
                let len = match opcode {
                    0x05 => 4,
                    0x09 => 2,
                    _ => 1,
                };
                self.write(address, &bytes[..len])?;
                next(self, 7);
            }
            0x07 => {
                let value = i16::from_le_bytes(self.read_bytes(pc + 1)?) as u32;
                let dst = register(self, 3)?;
                self.registers[dst] = value;
                next(self, 4);
            }
            0x0b => {
                let value = byte(self, 1)? as u8 as i8 as u32;
                let dst = register(self, 2)?;
                self.registers[dst] = value;
                next(self, 3);
            }
            // Two-register arithmetic: destination = destination op source.
            0x10..=0x24 => {
                let (src, dst) = (register(self, 1)?, register(self, 2)?);
                let (a, b) = (self.registers[dst], self.registers[src]);
                let (fa, fb) = (f32::from_bits(a), f32::from_bits(b));
                self.registers[dst] = match opcode {
                    0x10 | 0x11 => a.wrapping_add(b),
                    0x12 => (fa + fb).to_bits(),
                    0x13 | 0x14 => a.wrapping_sub(b),
                    0x15 => (fa - fb).to_bits(),
                    0x16 | 0x17 => a.wrapping_mul(b),
                    0x18 => (fa * fb).to_bits(),
                    0x19 => (a as i32).checked_div(b as i32).context("script divided by zero")? as u32,
                    0x1a => a.checked_div(b).context("script divided by zero")?,
                    0x1b => (fa / fb).to_bits(),
                    0x1c => (a as i32).checked_rem(b as i32).context("script divided by zero")? as u32,
                    0x1d => a.checked_rem(b).context("script divided by zero")?,
                    0x1e | 0x1f => a.wrapping_shl(b),
                    0x20 => ((a as i32).wrapping_shr(b)) as u32,
                    0x21 => a.wrapping_shr(b),
                    0x22 => a & b,
                    0x23 => a | b,
                    _ => a ^ b,
                };
                next(self, 3);
            }
            0x25..=0x28 => {
                let r = register(self, 1)?;
                let value = self.registers[r];
                self.registers[r] = match opcode {
                    0x25 => !value,
                    0x26 => (value == 0) as u32,
                    0x27 => value.wrapping_neg(),
                    _ => (-f32::from_bits(value)).to_bits(),
                };
                next(self, 2);
            }
            0x29 => {
                let (src, dst) = (register(self, 1)?, register(self, 2)?);
                self.registers[dst] = f32::from_bits(self.registers[src]) as i32 as u32;
                next(self, 3);
            }
            0x2a => {
                let (src, dst) = (register(self, 1)?, register(self, 2)?);
                self.registers[dst] = (self.registers[src] as i32 as f32).to_bits();
                next(self, 3);
            }
            // Compares set "equal" and "second operand less than first".
            0x30..=0x32 => {
                let (first, second) = (self.registers[register(self, 1)?], self.registers[register(self, 2)?]);
                match opcode {
                    0x30 => self.compare(second == first, (second as i32) < (first as i32)),
                    0x31 => self.compare(second == first, second < first),
                    _ => {
                        let difference = f32::from_bits(second) - f32::from_bits(first);
                        self.compare(difference == 0.0, difference < 0.0);
                    }
                }
                next(self, 3);
            }
            0x33..=0x38 => {
                let r = register(self, 1)?;
                let (equal, less) = (self.flags & FLAG_EQUAL != 0, self.flags & FLAG_LESS != 0);
                self.registers[r] = match opcode {
                    0x33 => equal as u32,
                    0x34 => !equal as u32,
                    // These two store the flag bit itself, so "true" is 2.
                    0x35 => self.flags & FLAG_LESS,
                    0x38 => !self.flags & FLAG_LESS,
                    0x36 => (!equal && !less) as u32,
                    _ => (equal || less) as u32,
                };
                next(self, 2);
            }
            0x39 => self.registers[PC] = immediate(self, 1)?,
            0x3a | 0x3b => {
                let taken = (self.flags & FLAG_EQUAL != 0) == (opcode == 0x3a);
                self.registers[PC] = if taken { immediate(self, 1)? } else { pc + 5 };
            }
            0x3c => {
                let target = immediate(self, 1)?;
                self.push(pc + 5)?;
                self.registers[PC] = target;
            }
            0x3d => self.registers[PC] = self.pop()?,
            0x3e => {
                let target = self.registers[register(self, 1)?];
                if target & NATIVE_BIT != 0 {
                    next(self, 2);
                    self.native(target >> 8, host)?;
                } else {
                    self.push(pc + 2)?;
                    self.registers[PC] = target;
                }
            }
            0x40 => {
                let number = immediate(self, 1)?;
                next(self, 5);
                self.native(number, host)?;
            }
            other => bail!("unknown script instruction {other:#04x} at {pc:#x}"),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A host that records calls and answers every one with zero.
    #[derive(Default)]
    struct Recorder(Vec<(u32, i32, f32)>);

    impl Host for Recorder {
        fn native(&mut self, number: u32, call: &Call) -> Result<u32> {
            self.0.push((number, call.int(0)?, call.float(1)?));
            Ok(0)
        }
    }

    /// A script of one function, `main`, with this code and four bytes of data after
    /// it under the name `table`.
    fn assemble(code: &[u8]) -> Script {
        Script::parse(&file(code)).unwrap()
    }

    /// The file of such a script.
    fn file(code: &[u8]) -> Vec<u8> {
        const SYMBOLS_AT: u32 = 0x28;
        const NAMES_AT: u32 = SYMBOLS_AT + 16;
        const CODE_AT: u32 = NAMES_AT + 16;
        let end = CODE_AT + code.len() as u32 + 4;
        let mut d = MAGIC.to_vec();
        d.extend(0x0002_0000u32.to_le_bytes());
        d.extend([2, 0, 0, 0]);
        for word in [SYMBOLS_AT, CODE_AT, CODE_AT, end, 0, end, end] {
            d.extend(word.to_le_bytes());
        }
        for word in [CODE_AT, NAMES_AT, CODE_AT + code.len() as u32, NAMES_AT + 8] {
            d.extend(word.to_le_bytes());
        }
        d.extend(b"main\0\0\0\0table\0\0\0");
        d.extend(code);
        d.extend(7u32.to_le_bytes());
        d
    }

    /// Waits in engine function 9 until it has been called `calls_needed` times.
    struct Waiter {
        calls_needed: u32,
        /// The argument and whether the call was a resumed one, for each call.
        seen: Vec<(i32, bool)>,
    }

    impl Host for Waiter {
        fn native(&mut self, number: u32, call: &Call) -> Result<u32> {
            assert_eq!(number, 9);
            self.seen.push((call.int(0)?, call.resumed()));
            if (self.seen.len() as u32) < self.calls_needed {
                call.wait();
            }
            Ok(42)
        }
    }

    #[test]
    fn a_waiting_engine_function_is_called_again_until_it_is_done() {
        // main: return engine9(5)
        let script = assemble(&[
            0x03, 5, 0, 0, 0, 3, // rv = 5
            0x00, 3, // push rv
            0x03, 1, 9, 0, 0, 3, // rv = engine function 9
            0x3e, 3, // call it
            0x03, 4, 0, 0, 0, 4, // r4 = 4
            0x10, 4, 2, // sp += r4
            0x3d, // return
        ]);
        let main = script.symbol("main").unwrap();
        let mut machine = Machine::new(&script);
        let mut host = Waiter { calls_needed: 3, seen: Vec::new() };
        assert_eq!(machine.start(main, &[], &mut host).unwrap(), Run::Waiting);
        assert!(machine.waiting());
        assert_eq!(machine.resume(&mut host).unwrap(), Run::Waiting);
        assert_eq!(machine.resume(&mut host).unwrap(), Run::Returned(42));
        assert!(!machine.waiting());
        assert_eq!(host.seen, [(5, false), (5, true), (5, true)]);

        // A plain call cannot wait, and starting afresh drops a wait in progress.
        let mut host = Waiter { calls_needed: 2, seen: Vec::new() };
        assert!(machine.call(main, &[], &mut host).is_err());
        let mut host = Waiter { calls_needed: 1, seen: Vec::new() };
        assert_eq!(machine.call(main, &[], &mut host).unwrap(), 42);
    }

    #[test]
    fn a_script_reads_and_writes_the_memory_its_host_maps() {
        // main(p): p[1] = p[0] + 1; return engine9(p[1])
        let script = assemble(&[
            0x04, 4, 0, 0, 0, 2, 4, // r4 = sp[4]: the argument
            0x04, 0, 0, 0, 0, 4, 3, // rv = r4[0]
            0x03, 1, 0, 0, 0, 5, // r5 = 1
            0x10, 5, 3, // rv += r5
            0x05, 3, 4, 0, 0, 0, 4, // r4[4] = rv
            0x00, 3, // push rv
            0x03, 1, 9, 0, 0, 3, // rv = engine function 9
            0x3e, 3, // call it
            0x03, 4, 0, 0, 0, 4, // r4 = 4
            0x10, 4, 2, // sp += r4
            0x3d, // return
        ]);
        const RECORD: u32 = 0x3000_0000;
        let mut machine = Machine::new(&script);
        machine.map(RECORD, [41u32.to_le_bytes(), 0u32.to_le_bytes()].concat());
        let mut host = Waiter { calls_needed: 1, seen: Vec::new() };
        assert_eq!(machine.call(script.symbol("main").unwrap(), &[RECORD], &mut host).unwrap(), 42);
        assert_eq!(host.seen, [(42, false)]);
        assert_eq!(machine.read_u32(RECORD + 4).unwrap(), 42);
        assert_eq!(machine.mapped(RECORD).unwrap()[4], 42);
        // Memory nobody has mapped is an error, not a wild read.
        assert!(machine.call(script.symbol("main").unwrap(), &[0x4000_0000], &mut host).is_err());
    }

    #[test]
    fn a_script_can_be_placed_anywhere_in_the_address_space() {
        let code = [0x03, 7, 0, 0, 0, 3, 0x3d]; // main: return 7
        let moved = Script::parse_at(&file(&code), 0x2000_0000).unwrap();
        assert_eq!(moved.base(), 0x2000_0000);
        let main = moved.symbol("main").unwrap();
        assert_eq!(main - moved.base(), assemble(&code).symbol("main").unwrap() - IMAGE_BASE);
        assert!(moved.is_address(main) && !moved.is_address(IMAGE_BASE));
        assert_eq!(moved.extent("table"), Some(&7u32.to_le_bytes()[..]));
        assert_eq!(Machine::new(&moved).call(main, &[], &mut Recorder::default()).unwrap(), 7);
    }

    #[test]
    fn a_data_symbol_runs_up_to_the_next_symbol_or_the_end_of_the_data() {
        let script = assemble(&[0x3d, 0, 0, 0]);
        assert_eq!(script.extent("main"), Some(&[0x3d, 0, 0, 0][..]));
        assert_eq!(script.extent("table"), Some(&7u32.to_le_bytes()[..]));
        assert_eq!(script.extent("missing"), None);
    }

    /// The player's field script, if the game data has been unpacked here.
    fn player_script() -> Option<Script> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked/FLD/FCHR/FCHR_P_BRS00.BIN/00.sscr");
        let data = std::fs::read(path).ok()?;
        Some(Script::parse(&data).expect("the player's script parses"))
    }

    #[test]
    fn player_script_reports_its_speeds() {
        let Some(script) = player_script() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let function = script.symbol("get_move_speed").expect("get_move_speed exists");
        let speed = |status: u32| {
            let mut machine = Machine::new(&script);
            f32::from_bits(machine.call(function, &[status], &mut Recorder::default()).unwrap())
        };
        assert!((speed(0) - 40.0 / 7.0).abs() < 1e-4);
        assert_eq!(speed(1), 32.0);
        assert_eq!(speed(0x40), 96.0);
        assert_eq!(speed(0x41), 1.0);
        // Statuses with no case of their own run.
        assert_eq!(speed(7), 32.0);
    }

    #[test]
    fn player_script_set_status_calls_the_engine() {
        let Some(script) = player_script() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let function = script.symbol("set_status").unwrap();
        let mut host = Recorder::default();
        Machine::new(&script).call(function, &[1], &mut host).unwrap();
        // For any status but 3: set parameter 2 to 0, set float 0 to 1.0, set parameter 0 to 0.
        assert_eq!(host.0, vec![(8, 2, 0.0), (10, 0, 1.0), (8, 0, 0.0)]);

        let mut host = Recorder::default();
        Machine::new(&script).call(function, &[3], &mut host).unwrap();
        assert_eq!(host.0, vec![(8, 0, 0.0)]);
    }

    #[test]
    fn player_script_data_symbols_are_readable() {
        let Some(script) = player_script() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        assert_eq!(script.integer("num_gimmick"), Some(4));
        assert_eq!(script.floats::<3>("move"), Some([40.0 / 7.0, 32.0, 64.0]));
        assert_eq!(script.imports, ["fchr_z_rsd_set_face", "fchr_z_rsd_update_foot_se"]);
    }
}
