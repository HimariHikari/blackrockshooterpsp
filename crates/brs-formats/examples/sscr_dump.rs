//! Lists the symbols of a compiled script and dumps its data tables.
//!
//! Usage: `cargo run -p brs-formats --example sscr_dump -- <file> [symbol]`
//!
//! With no symbol, prints every symbol with the distance to the next one. With a
//! symbol, prints the 32-bit words from it to the next symbol; words that point into
//! the script are shown as the symbol they land on, or as the text they point at.

use anyhow::{Context, Result};
use brs_formats::script::Script;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args.next().context("usage: sscr_dump <file> [symbol]")?;
    let wanted = args.next();
    let script = Script::parse(&std::fs::read(&path).with_context(|| format!("reading {path}"))?)?;

    let mut symbols: Vec<_> = script.symbols.iter().collect();
    symbols.sort_by_key(|s| s.address);
    let end_of = |index: usize| symbols.get(index + 1).map(|next| next.address);

    for (index, symbol) in symbols.iter().enumerate() {
        match &wanted {
            None => {
                let size = end_of(index).map_or("?".to_string(), |end| format!("{:#x}", end - symbol.address));
                println!("{:#010x} {size:>8}  {}", symbol.address, symbol.name);
            }
            Some(name) if *name == symbol.name => {
                let data = script.data(symbol.address).context("symbol outside the image")?;
                let len = end_of(index).map_or(data.len(), |end| (end - symbol.address) as usize).min(data.len());
                for (i, word) in data[..len].chunks_exact(4).enumerate() {
                    let value = u32::from_le_bytes(word.try_into()?);
                    println!("{:+#06x}  {value:08x}  {}", i * 4, describe(&script, &symbols, value));
                }
            }
            Some(_) => {}
        }
    }
    if !script.imports.is_empty() && wanted.is_none() {
        println!("imports: {}", script.imports.join(", "));
    }
    Ok(())
}

/// A guess at what a word is: a pointer to a symbol or text, a float, or a number.
fn describe(script: &Script, symbols: &[&brs_formats::script::Symbol], value: u32) -> String {
    if script.is_address(value) {
        if let Some(symbol) = symbols.iter().rev().find(|s| s.address <= value) {
            let offset = value - symbol.address;
            let text = script.string(value).filter(|t| t.len() >= 2 && t.chars().all(|c| c.is_ascii_graphic() || c == ' '));
            return match (offset, text) {
                (0, _) => format!("-> {}", symbol.name),
                (_, Some(text)) => format!("-> \"{text}\""),
                _ => format!("-> {}+{offset:#x}", symbol.name),
            };
        }
    }
    let float = f32::from_bits(value);
    if value > 0xffff && float.is_finite() && (1e-4..1e7).contains(&float.abs()) {
        format!("{float}")
    } else {
        format!("{}", value as i32)
    }
}
