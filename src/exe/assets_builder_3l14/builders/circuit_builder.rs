use bitcode::Decode;
use crate::core::{AssetBuilder, AssetBuilderMeta, BuildOutputs, SourceInput, VersionBuilder};
use indexmap::IndexMap;
use latch_3l14::block_meta::{BlockMeta, HydrateBlock};
use latch_3l14::{BlockKind, BlockVisitor, ImpulseActions, ImpulseBlock, Inlet, PulsedOutlet, Scope};
use logos::{Lexer, Logos};
use proc_macros_3l14::CircuitBlock;
use serde::Deserialize;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Debug, Display};
use std::io::Read;
use unicase::UniCase;

pub struct CircuitBuilder
{
}
impl CircuitBuilder
{
    #[must_use]
    pub fn new() -> Self
    {
        // let mut block_types = HashMap::new();
        // for bty in inventory::iter::<BlockMeta>()
        // {
        //     block_types.insert(UniCase::unicode(bty.type_name), bty);
        // }

        // Self
        // {
        //     block_types
        // }
        Self
        {

        }
    }
}
impl AssetBuilderMeta for CircuitBuilder
{
    fn supported_input_file_extensions() -> &'static [&'static str]
    {
        todo!()
    }

    fn builder_version(vb: &mut VersionBuilder)
    {
        vb.append(&[
            b"Initial"
        ]);
    }

    fn format_version(vb: &mut VersionBuilder)
    {
        vb.append(&[
            b"Initial"
        ]);
    }
}
impl AssetBuilder for CircuitBuilder
{
    type BuildConfig = ();

    fn build_assets(&self, config: Self::BuildConfig, input: &mut SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        // todo: lex DSL
        // order blocks based on depth
        // generate properties for outlets on blocks
        // deserialize blocks
        // deserialize circuit
        // split out debug data

        let mut str = String::new();
        input.read_to_string(&mut str)?;
        let lexed = lex_circuit_dsl(&str);
        todo!()
    }
}

#[derive(Logos)]
#[logos(skip r"[ \t\r\f]+")]
#[logos(skip r"#[^\n]*")] // TODO: should this consume the newline?
#[logos(extras = FilePos)]
enum Token<'p>
{
    #[token("[")]
    LatchDefBegin,
    #[token("]")]
    LatchDefEnd,
    #[token("<")]
    ImpulseDefBegin,
    #[allow(non_camel_case_types)]
    #[token(">")]
    ImpulseDefEnd_PulseOutlet,
    #[token("<>")]
    LatchOutlet,

    #[token("-")]
    PowerOff,
    #[token("~")]
    SignalEntry,
    #[token("@")]
    AutoEntry,

    #[token("=", lex_toml)]
    Assignment(Result<Box<dyn erased_serde::Deserializer<'p> + 'p>, LexerError<'p>>),

    #[token("\n", newline_callback)]
    NewLine,

    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Identifier(&'p str),
}
fn lex_toml<'p>(lex: &mut Lexer<'p, Token<'p>>) -> Result<Box<dyn erased_serde::Deserializer<'p> + 'p>, LexerError<'p>>
{
    // todo: this would be cleaner as a nested lexer

    let sub = lex.remainder().trim_start();
    let mut chars = sub.chars();

    let mut n = 1;
    let closer = match chars.next()
    {
        Some('{') => '}',
        Some('[') => ']',
        // todo: escaping
        Some('"') => '"',
        Some('\'') => '\'',
        Some(other) =>
        {
            let s = sub.split_whitespace().next().unwrap();
            // todo: dedupe
            let parsed = match toml::de::ValueDeserializer::parse(s)
            {
                Ok(v) => v,
                Err(e) => return Err(LexerError
                {
                    kind: LexerErrorKind::InvalidTomlValue { value: s, error: e },
                    line: lex.extras.line,
                    column: lex.extras.column,
                    token: lex.slice(),
                })
            };
            lex.bump(s.len() + 1);
            return Ok(Box::new(<dyn erased_serde::Deserializer>::erase(parsed)));
        }
        None => return Err(LexerError
        {
            kind: LexerErrorKind::ExpectedFieldValue,
            line: lex.extras.line,
            column: lex.extras.column,
            token: lex.slice(),
        })
    };

    for char in chars
    {
        if char == closer
        {
            let s = &sub[0..=n];
            let parsed = match toml::de::ValueDeserializer::parse(s)
            {
                Ok(v) => v,
                Err(e) => return Err(LexerError
                {
                    kind: LexerErrorKind::InvalidTomlValue { value: s, error: e },
                    line: lex.extras.line,
                    column: lex.extras.column,
                    token: lex.slice(),
                })
            };
            lex.bump(n + 2);
            return Ok(Box::new(<dyn erased_serde::Deserializer>::erase(parsed)));
        }
        n += 1
    }

    return Err(LexerError
    {
        kind: LexerErrorKind::ExpectedTomlValueTerminator,
        line: lex.extras.line,
        column: lex.extras.column,
        token: lex.slice(),
    });
}
fn newline_callback<'p>(lexer: &mut Lexer<'p, Token<'p>>)
{
    lexer.extras.line += 1;
    lexer.extras.column = lexer.span().start;
}

#[derive(Debug, Clone, PartialEq)]
pub enum LexerErrorKind<'p>
{
    UnknownToken,
    InvalidTomlValue { value: &'p str, error: toml::de::Error },
    MissingPropertyValue { property: &'p str },
    ExpectedTargetBlock,
    ExpectedBlockType,
    ExpectedBlockName,
    ExpectedBlockDefTerminator,
    ImpulseBlockLatchedOutlet { block_name: &'p str },
    ExpectedEndOfLine,
    ExpectedSignalName,
    DuplicateAutoEntry,
    DuplicateBlockName { block_name: &'p str },
    DuplicateField { field: &'p str },
    ExpectedTomlValueTerminator,
    UnexpectedKeyValue,
    ExpectedFieldValue,
}

#[derive(Debug, Clone, PartialEq)]
struct LexerError<'p>
{
    pub kind: LexerErrorKind<'p>,
    pub line: usize,
    pub column: usize,
    pub token: &'p str,
}
impl Display for LexerError<'_>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { std::fmt::Debug::fmt(&self, f) }
}
impl std::error::Error for LexerError<'_> { }

struct FilePos
{
    line: usize,
    column: usize,
}
impl Default for FilePos
{
    fn default() -> FilePos
    {
        FilePos { line: 1, column: 1 }
    }
}

struct PlugRef<'p>
{
    pub target_block_name: UniCase<&'p str>,
    pub inlet: Inlet,
}

type Outlets<'p> = HashMap<UniCase<&'p str>, Vec<PlugRef<'p>>>;
type Fields<'p> = HashMap<UniCase<&'p str>, Box<dyn erased_serde::Deserializer<'p> + 'p>>;

struct BlockDef<'p>
{
    pub type_name: UniCase<&'p str>,
    pub kind: BlockKind,
    pub name: UniCase<&'p str>,
    pub pulsed_outlets: Outlets<'p>,
    pub latching_outlets: Outlets<'p>,
    pub fields: Fields<'p>,
}

struct CircuitDef<'p>
{
    pub metadata: Fields<'p>,
    pub blocks: IndexMap<UniCase<&'p str>, BlockDef<'p>>,
    pub auto_entries: Vec<UniCase<&'p str>>,
    pub signal_entries: HashMap<&'p str, SmallVec<[UniCase<&'p str>; 4]>>,
}

enum LexerState<'p>
{
    Metadata,
    ImpulseBlock(BlockDef<'p>),
    LatchBlock(BlockDef<'p>),
    AutoEntry,
    SignalEntry(&'p str, SmallVec<[UniCase<&'p str>; 4]>),
}

type CircuitLexer<'p> = Lexer<'p, Token<'p>>;

pub fn lex_circuit_dsl(input: &str) -> Result<CircuitDef<'_>, LexerError<'_>>
{
    let mut lexer = CircuitLexer::with_extras(&input, FilePos::default());

    // // debug print all tokens
    // {
    //     println!("{:#?}", CircuitLexer::with_extras(&input, FilePos::default()).collect::<Vec<_>>());
    // }

    // it would be nice if these were functions not macros, but borrow checker is dumb

    // TODO: could probably make the function a nested function that just returns the LexerErrorKind, then wrap it
    macro_rules! error { ($err:expr) =>
    {
        return Err(LexerError
        {
            kind: $err,
            line: lexer.extras.line,
            column: lexer.extras.column,
            token: lexer.slice(),
        })
    } }

    macro_rules! parse_plug { () =>
    {
        match lexer.next()
        {
            Some(Ok(Token::Identifier(target_block_name))) =>
            {
                Ok(PlugRef { target_block_name: UniCase::unicode(target_block_name), inlet: Inlet::Pulse })
            },
            Some(Ok(Token::PowerOff)) =>
            {
                let Some(Ok(Token::Identifier(target_block_name))) = lexer.next()
                    else { error!(LexerErrorKind::ExpectedTargetBlock); };
                Ok(PlugRef { target_block_name: UniCase::unicode(target_block_name), inlet: Inlet::PowerOff })
            }
            _ => error!(LexerErrorKind::ExpectedTargetBlock)
        }
    } }

    macro_rules! declare_block { ($block_kind:expr) =>
    {{
        let Some(Ok(Token::Identifier(block_type))) = lexer.next()
            else { error!(LexerErrorKind::ExpectedBlockType) };

        // cleaner way to do this?f
        match $block_kind
        {
            BlockKind::Impulse =>
            {
            let Some(Ok(Token::ImpulseDefEnd_PulseOutlet)) = lexer.next()
                else { error!(LexerErrorKind::ExpectedBlockDefTerminator) };
            }
            BlockKind::Latch =>
            {
                let Some(Ok(Token::LatchDefEnd)) = lexer.next()
                    else { error!(LexerErrorKind::ExpectedBlockDefTerminator) };
            }
        }

        let Some(Ok(Token::Identifier(block_name))) = lexer.next()
            else { error!(LexerErrorKind::ExpectedBlockName) };

        Ok(BlockDef
        {
            type_name: UniCase::unicode(block_type),
            kind: $block_kind,
            name: UniCase::unicode(block_name),
            pulsed_outlets: Default::default(),
            latching_outlets: Default::default(),
            fields: Default::default(),
        })
    }} }

    let mut metadata = Fields::default();
    let mut blocks = IndexMap::new();
    let mut auto_entries = Vec::new();
    let mut signal_entries: HashMap<_, SmallVec<_>> = HashMap::new();

    let mut curr_state = LexerState::Metadata;

    // make a macro?
    macro_rules! set_state { ($new_state:expr) =>
    {
        match std::mem::replace(&mut curr_state, $new_state)
        {
            LexerState::ImpulseBlock(impulse) =>
            {
                let name = impulse.name;
                if let Some(_) = blocks.insert(name, impulse)
                {
                    error!(LexerErrorKind::DuplicateBlockName { block_name: name.into_inner() });
                }
            },
            LexerState::LatchBlock(latch) =>
            {
                let name = latch.name;
                if let Some(_) = blocks.insert(name, latch)
                {
                    error!(LexerErrorKind::DuplicateBlockName { block_name: name.into_inner() });
                }
            },
            LexerState::SignalEntry(signal, entries) =>
            {
                let sig = signal_entries.entry(signal).or_default();
                sig.extend(entries);
            },
            _ => {}
        }
    } }

    'lexer: loop
    {
        let line_start = lexer.span();
        match lexer.next()
        {
            None => {},
            Some(Ok(Token::NewLine)) => continue 'lexer,

            Some(Ok(Token::Identifier(id))) =>
            {
                match &mut curr_state
                {
                    LexerState::AutoEntry =>
                    {
                        auto_entries.push(UniCase::unicode(id));
                    }
                    LexerState::SignalEntry(signal, entries) =>
                    {
                        entries.push(UniCase::unicode(id));
                    }

                    // a <> b, a > b, a = b
                    s => match (lexer.next(), s)
                    {
                        (Some(Ok(Token::ImpulseDefEnd_PulseOutlet)), LexerState::ImpulseBlock(block)) =>
                        {
                            let plug = parse_plug!()?;
                            let outlet = block.pulsed_outlets.entry(UniCase::unicode(id))
                                .or_insert(Vec::new());
                            outlet.push(plug);
                        }

                        (Some(Ok(Token::ImpulseDefEnd_PulseOutlet)), LexerState::LatchBlock(block)) =>
                        {
                            let plug = parse_plug!()?;
                            let outlet = block.pulsed_outlets.entry(UniCase::unicode(id))
                                .or_insert(Vec::new());
                            outlet.push(plug);
                        }
                        (Some(Ok(Token::LatchOutlet)), LexerState::LatchBlock(block)) =>
                        {
                            let plug = parse_plug!()?;
                            let outlet = block.latching_outlets.entry(UniCase::unicode(id))
                                .or_insert(Vec::new());
                            outlet.push(plug);
                        }

                        (Some(Ok(Token::Assignment(val))), ls) =>
                        {
                            let fid = UniCase::unicode(id);
                            // todo: assert no dupes
                            let existing = match ls
                            {
                                LexerState::Metadata =>
                                {
                                    let fucker = <dyn ::erased_serde::Deserializer>::erase(val?);
                                    metadata.insert(fid, Box::new(fucker))
                                },
                                LexerState::ImpulseBlock(impulse) => impulse.fields.insert(fid, val?),
                                LexerState::LatchBlock(latch) => latch.fields.insert(fid, val?),
                                _ => error!(LexerErrorKind::UnexpectedKeyValue),
                            };
                            if existing.is_some()
                            {
                                error!(LexerErrorKind::DuplicateField { field: id });
                            }
                        }

                        _ => error!(LexerErrorKind::UnknownToken),
                    }
                }
            }

            Some(Ok(Token::ImpulseDefBegin)) =>
            {
                set_state!(LexerState::ImpulseBlock(declare_block!(BlockKind::Impulse)?));
            }
            Some(Ok(Token::LatchDefBegin)) =>
            {
                set_state!(LexerState::LatchBlock(declare_block!(BlockKind::Latch)?));
            }

            Some(Ok(Token::SignalEntry)) =>
            {
                let Some(Ok(Token::Identifier(signal))) = lexer.next()
                    else { error!(LexerErrorKind::ExpectedSignalName) };

                set_state!(LexerState::SignalEntry(signal, SmallVec::new()));
            }

            Some(Ok(Token::AutoEntry)) =>
            {
                set_state!(LexerState::AutoEntry);
            }

            _ => error!(LexerErrorKind::ExpectedBlockName) // todo: distinct error
        }

        match lexer.next()
        {
            Some(Ok(Token::NewLine)) => { },
            Some(Ok(_)) => error!(LexerErrorKind::ExpectedEndOfLine),
            Some(Err(_)) => error!(LexerErrorKind::UnknownToken),
            None => break,
        }
    }

    set_state!(LexerState::Metadata);
    Ok(CircuitDef { metadata, blocks, auto_entries, signal_entries })
}

#[cfg(test)]
mod tests
{
    use super::*;
    use latch_3l14::{BlockId, Circuit, ImpulseBlock, LatchBlock, LatchingOutlet, Plug};
    use nab_3l14::utils::alloc_slice::alloc_slice_uninit;

    #[test]
    fn basic()
    {
        let input =
// r#"
// meta="value" # test

// [ConditionalLatch] Cond1
// OnTrue > Print2
// True <> Sub1
// False <> -Sub1
// x = 5

// <DebugLog> Print1
// Text = "Hola!"

// <DebugLog> Print2
// Text = "Hola!"
// outlet > Print3

// <DebugLog> Print3

// [Something] Sub1

// ~ Sig1 # signaled entries
// Cond1 # comment

// @ # auto entries
// Print1
// "#;
r#"
<DebugLog> Print1
message = "Hello!"
Outlet > Print2
<DebugLog> Print2
message = "Goodbye!"
@
Print1
"#;

        let lexed = lex_circuit_dsl(input).unwrap();
        println!("{:#?}", parse(lexed).unwrap());

        #[derive(Debug)]
        enum ParseError<'p>
        {
            AutoEntryPointsToUndefinedBlock { block_name: &'p str },
            SignalEntryPointsToUndefinedBlock { signal: &'p str, block_name: &'p str },
            OutletPointsToUndefinedBlock { block_name: &'p str },
            ImpulseBlockContainsLatchingOutlets { block_name: &'p str },
            PulsedPlugPointsToUndefinedBlock { block_name: &'p str, outlet_name: &'p str, target_block_name: &'p str },
            LatchingPlugPointsToUndefinedBlock { block_name: &'p str, outlet_name: &'p str, target_block_name: &'p str },
            ImpulsesDoNotHavePowerOffInlets { block_name: &'p str, outlet_name: &'p str, target_block_name: &'p str },
            BlockDeserializeError { block_name: &'p str, error: erased_serde::Error },
            UnknownBlockType { type_name: UniCase<&'p str> },
        }

        fn parse(mut lexed: CircuitDef) -> Result<Circuit, ParseError>
        {
            let mut depths = HashMap::new();
            let mut stack = Vec::new();
            for entry in lexed.auto_entries
            {
                if !lexed.blocks.contains_key(&entry)
                {
                    return Err(ParseError::AutoEntryPointsToUndefinedBlock { block_name: entry.into_inner() });
                }

                depths.entry(entry).or_insert(0u32);
                stack.push(entry);
            }
            for (signal, entries) in lexed.signal_entries
            {
                for entry in entries
                {
                    if !lexed.blocks.contains_key(&entry)
                    {
                        return Err(ParseError::SignalEntryPointsToUndefinedBlock { signal, block_name: entry.into_inner() });
                    }

                    depths.entry(entry).or_insert(0u32);
                    stack.push(entry);
                }
            }

            while let Some(block_name) = stack.pop()
            {
                let block = lexed.blocks.get(&block_name)
                    .expect("Block should not be pushed onto depth stack if it doesn't exist");

                if let BlockKind::Impulse = block.kind &&
                    !block.latching_outlets.is_empty()
                {
                    return Err(ParseError::ImpulseBlockContainsLatchingOutlets { block_name: block_name.into_inner() });
                }

                let depth = depths.get(&block_name).unwrap() + 1;
                for (outlet_name, plugs) in block.pulsed_outlets.iter()
                {
                    for PlugRef { target_block_name, inlet } in plugs.iter()
                    {
                        let Some(target_block) = lexed.blocks.get(target_block_name)
                            else { return Err(ParseError::PulsedPlugPointsToUndefinedBlock
                            {
                                block_name: block_name.into_inner(),
                                outlet_name: outlet_name.into_inner(),
                                target_block_name: target_block_name.into_inner(),
                            }) };

                        if let Inlet::PowerOff = inlet &&
                            let BlockKind::Impulse = target_block.kind
                        {
                            return Err(ParseError::ImpulsesDoNotHavePowerOffInlets
                            {
                                block_name: block_name.into_inner(),
                                outlet_name: outlet_name.into_inner(),
                                target_block_name: target_block_name.into_inner(),
                            });
                        }

                        let target_depth = depths.entry(*target_block_name).or_insert(u32::MAX);
                        if *target_depth > depth
                        {
                            *target_depth = depth;
                            stack.push(*target_block_name);
                        }
                    }
                }

                for (outlet_name, plugs) in block.latching_outlets.iter()
                {
                    for PlugRef { target_block_name, inlet } in plugs.iter()
                    {
                        let Some(target_block) = lexed.blocks.get(target_block_name)
                            else { return Err(ParseError::LatchingPlugPointsToUndefinedBlock
                            {
                                block_name: block_name.into_inner(),
                                outlet_name: outlet_name.into_inner(),
                                target_block_name: target_block_name.into_inner(),
                            }) };

                        if let Inlet::PowerOff = inlet &&
                            let BlockKind::Impulse = target_block.kind
                        {
                            return Err(ParseError::ImpulsesDoNotHavePowerOffInlets
                            {
                                block_name: block_name.into_inner(),
                                outlet_name: outlet_name.into_inner(),
                                target_block_name: target_block_name.into_inner(),
                            });
                        }

                        let target_depth = depths.entry(*target_block_name).or_insert(u32::MAX);
                        if *target_depth > depth
                        {
                            *target_depth = depth;
                            stack.push(*target_block_name);
                        }
                    }
                }
            }

            let mut impulses = Vec::new();
            let mut latches = Vec::new();
            for (block_name, block) in lexed.blocks.iter()
            {
                let Some(depth) = depths.get(block_name) else { continue };
                match block.kind
                {
                    BlockKind::Impulse => impulses.push((*block_name, depth)),
                    BlockKind::Latch => latches.push((*block_name, depth)),
                }
            }
            // these will sort stable and fallback to insertion order (maintained by the indexmap) if the same
            impulses.sort_by(|a, b| a.1.cmp(&b.1));
            latches.sort_by(|a, b| a.1.cmp(&b.1));

            let block_ids =
            {
                let mut blids = HashMap::new();
                for (i, (name, _)) in impulses.iter().enumerate()
                {
                    blids.insert(name, BlockId::impulse(i as u32));
                }
                for (i, (name, _)) in latches.iter().enumerate()
                {
                    blids.insert(name, BlockId::latch(i as u32));
                }
                blids
            };

            let impulse_types: HashMap<_, _> = inventory::iter::<BlockMeta<0>>()
                .map(|b| (UniCase::unicode(b.type_name), b)).collect();
            let latch_types: HashMap<_, _> = inventory::iter::<BlockMeta<1>>()
                .map(|b| (UniCase::unicode(b.type_name), b)).collect();

            let map_plugs = |plugs: &Vec<PlugRef>|
            {
                plugs.iter().map(|plug|
                    {
                        Plug
                        {
                            block: *block_ids.get(&plug.target_block_name).unwrap(), // guaranteed to exist earlier
                            inlet: plug.inlet,
                        }
                    }).collect()
            };

            let mut impulse_blocks = Vec::with_capacity(impulses.len());
            for (block_name, _) in impulses.iter()
            {
                let mut block = lexed.blocks.swap_remove(block_name).unwrap();
                let mut hydrate = HydrateBlock
                {
                    pulsed_outlets: block.pulsed_outlets.iter().map(|(k,v)|
                    {
                        (*k, PulsedOutlet { plugs: map_plugs(v)  })
                    }).collect(),
                    latching_outlets: Default::default(),
                    fields: block.fields
                };

                let Some(meta) = impulse_types.get(&block.type_name)
                    else { return Err(ParseError::UnknownBlockType { type_name: block.type_name }); };

                let hydrated = (meta.hydrate_fn)(&mut hydrate)
                    .map_err(|e| ParseError::BlockDeserializeError { block_name, error: e })?;

                impulse_blocks.push(hydrated);
            }

            let mut latch_blocks = Vec::with_capacity(latches.len());
            for (block_name, _) in latches.iter()
            {
                for (block_name, _) in impulses.iter()
                {
                    let mut block = lexed.blocks.swap_remove(block_name).unwrap();
                    let mut hydrate = HydrateBlock
                    {
                        pulsed_outlets: block.pulsed_outlets.iter().map(|(k,v)|
                        {
                            (*k, PulsedOutlet { plugs: map_plugs(v) })
                        }).collect(),
                        latching_outlets: block.latching_outlets.iter().map(|(k,v)|
                        {
                            (*k, LatchingOutlet { plugs: map_plugs(v) })
                        }).collect(),
                        fields: block.fields
                    };

                    let Some(meta) = latch_types.get(&block.type_name)
                    else { return Err(ParseError::UnknownBlockType { type_name: block.type_name }); };

                    let hydrated = (meta.hydrate_fn)(&mut hydrate)
                        .map_err(|e| ParseError::BlockDeserializeError { block_name, error: e })?;

                    latch_blocks.push(hydrated);
                }
            }

            Ok(Circuit
            {
                auto_entries: Box::new([]),
                signaled_entries: Box::new([]),
                impulses: impulse_blocks.into_boxed_slice(),
                latches: latch_blocks.into_boxed_slice(),
                num_local_vars: 0,
            })
        }
    }

    #[test]
    fn lex_toml()
    {
        lex_circuit_dsl("a = \"value\"").unwrap();
        lex_circuit_dsl("a = 5.123").unwrap();
        lex_circuit_dsl("a = { x = 1, y = true }").unwrap();
        lex_circuit_dsl("a = [ 1, 2, 3 ]").unwrap();

    }
}

#[derive(Debug, CircuitBlock, Decode)]
pub struct DebugLog
{
    pub message: String,
    // todo: format strings

    pub outlet: PulsedOutlet,
}
impl ImpulseBlock for DebugLog
{
    fn pulse(&self, scope: Scope, actions: ImpulseActions) {
        todo!()
    }

    fn inspect(&self, visit: BlockVisitor) {
        todo!()
    }
}

/* TODO: test cases:
pulsed and latching outlets on latches
fail if latching outlets on impulses
power-off plugs
require block type
require block name
key value properties
comments
signals
combine duplicate signals
auto entries
combine all auto entries
discard unlinked blocks (and should not get IDs)
impulses cannot have latching outlets
impulses cannot have power-off inlets
circular wires have correct depth
output block ordering is depth then insertion order
impulses and blocks are indexed independently
end file mid-line
*/
