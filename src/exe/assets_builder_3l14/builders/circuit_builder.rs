use std::error::Error;
use crate::core::{AssetBuilder, AssetBuilderMeta, BuildOutputs, SourceInput, VersionBuilder};
use logos::{Lexer, Logos};
use std::collections::HashMap;
use std::fmt::Display;
use std::io::Read;
use smallvec::SmallVec;
use unicase::UniCase;
use latch_3l14::block_meta::BlockRegistration;
use latch_3l14::Inlet;

pub struct CircuitBuilder
{
    block_types: HashMap<UniCase<&'static str>, &'static BlockRegistration>,
}
impl CircuitBuilder
{
    #[must_use]
    pub fn new() -> Self
    {
        let mut block_types = HashMap::new();
        for bty in inventory::iter::<BlockRegistration>()
        {
            block_types.insert(UniCase::unicode(bty.name), bty);
        }

        Self
        {
            block_types
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
        let parsed = lex_circuit_dsl(&str);
        println!("{:#?}", parsed);
        todo!()
    }
}

#[derive(Logos, Debug)]
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

    #[regex(r"=\s*[^\n]+")]
    KeyValue,

    #[token("\n", newline_callback)]
    NewLine,

    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Identifier(&'p str),
}
fn newline_callback<'p>(lexer: &mut Lexer<'p, Token<'p>>)
{
    lexer.extras.line += 1;
    lexer.extras.column = 1;
    lexer.extras.line_offset = lexer.span().start;
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
    DuplicateSignalEntry { signal: &'p str },
    ExpectedSignalName,
    DuplicateAutoEntry,
    UnexpectedKeyValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexerError<'p>
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
    line_offset: usize,
}
impl Default for FilePos
{
    fn default() -> FilePos
    {
        FilePos { line: 1, column: 1, line_offset: 0 }
    }
}

#[derive(Debug, Clone)]
pub struct PlugRef<'p>
{
    pub target_block_name: UniCase<&'p str>,
    pub inlet: Inlet,
}

pub type Outlets<'p> = HashMap<UniCase<&'p str>, Vec<PlugRef<'p>>>;

#[derive(Debug, Clone)]
pub struct BlockDef<'p>
{
    pub type_name: &'p str,
    pub name: UniCase<&'p str>,
    pub pulsed_outlets: Outlets<'p>,
    pub latching_outlets: Outlets<'p>,
    pub properties: toml::Table,
}

#[derive(Debug)]
pub struct CircuitDef<'p>
{
    pub metadata: toml::Table,
    pub impulses: Vec<BlockDef<'p>>,
    pub latches: Vec<BlockDef<'p>>,
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
                Ok(PlugRef { target_block_name: UniCase::new(target_block_name), inlet: Inlet::Pulse })
            },
            Some(Ok(Token::PowerOff)) =>
            {
                let Some(Ok(Token::Identifier(target_block_name))) = lexer.next()
                    else { error!(LexerErrorKind::ExpectedTargetBlock); };
                Ok(PlugRef { target_block_name: UniCase::new(target_block_name), inlet: Inlet::PowerOff })
            }
            _ => error!(LexerErrorKind::ExpectedTargetBlock)
        }
    } }

    macro_rules! declare_block { ($is_latch:expr) =>
    {{
        let Some(Ok(Token::Identifier(block_type))) = lexer.next()
            else { error!(LexerErrorKind::ExpectedBlockType) };

        // cleaner way to do this?f
        if $is_latch
        {
            let Some(Ok(Token::LatchDefEnd)) = lexer.next()
                else { error!(LexerErrorKind::ExpectedBlockDefTerminator) };
        }
        else
        {
            let Some(Ok(Token::ImpulseDefEnd_PulseOutlet)) = lexer.next()
                else { error!(LexerErrorKind::ExpectedBlockDefTerminator) };
        }

        let Some(Ok(Token::Identifier(block_name))) = lexer.next()
            else { error!(LexerErrorKind::ExpectedBlockName) };

        Ok(BlockDef
        {
            type_name: block_type,
            name: UniCase::new(block_name),

            pulsed_outlets: Default::default(),
            latching_outlets: Default::default(),
            properties: Default::default(),
        })
    }} }

    let mut metadata = toml::Table::new();
    let mut impulses = Vec::new();
    let mut latches = Vec::new();
    let mut auto_entries = Vec::new();
    let mut signal_entries: HashMap<_, SmallVec<_>> = HashMap::new();

    let mut curr_state = LexerState::Metadata;

    // make a macro?
    macro_rules! set_state { ($new_state:expr) =>
    {
        match std::mem::replace(&mut curr_state, $new_state)
        {
            LexerState::ImpulseBlock(impulse) => impulses.push(impulse),
            LexerState::LatchBlock(latch) => latches.push(latch),
            LexerState::SignalEntry(signal, entries) =>
            {
                let sig = signal_entries.entry(signal).or_default();
                sig.extend(entries);
            },
            _ => {}
        }
    } };

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
                        auto_entries.push(UniCase::new(id));
                    }
                    LexerState::SignalEntry(signal, entries) =>
                    {
                        entries.push(UniCase::new(id));
                    }

                    // a <> b, a > b, a = b
                    s => match (lexer.next(), s)
                    {
                        (Some(Ok(Token::ImpulseDefEnd_PulseOutlet)), LexerState::ImpulseBlock(block)) =>
                        {
                            let plug = parse_plug!()?;
                            let outlet = block.pulsed_outlets.entry(UniCase::new(id))
                                .or_insert(Vec::new());
                            outlet.push(plug);
                        }

                        (Some(Ok(Token::ImpulseDefEnd_PulseOutlet)), LexerState::LatchBlock(block)) =>
                        {
                            let plug = parse_plug!()?;
                            let outlet = block.pulsed_outlets.entry(UniCase::new(id))
                                .or_insert(Vec::new());
                            outlet.push(plug);
                        }
                        (Some(Ok(Token::LatchOutlet)), LexerState::LatchBlock(block)) =>
                        {
                            let plug = parse_plug!()?;
                            let outlet = block.latching_outlets.entry(UniCase::new(id))
                                .or_insert(Vec::new());
                            outlet.push(plug);
                        }

                        (Some(Ok(Token::KeyValue)), ls) =>
                        {
                            let line = &lexer.source()[line_start.start+1..lexer.span().end];
                            let table: toml::Table = match toml::from_str(line)
                            {
                                Ok(t) => t,
                                Err(e) => error!(LexerErrorKind::InvalidTomlValue { value: line, error: e })
                            };

                            match ls
                            {
                                LexerState::ImpulseBlock(impulse) => impulse.properties.extend(table),
                                LexerState::LatchBlock(latch) => latch.properties.extend(table),
                                LexerState::Metadata => metadata.extend(table),
                                _ => error!(LexerErrorKind::UnexpectedKeyValue)
                            }
                        }
                        
                        _ => error!(LexerErrorKind::UnknownToken),
                    }
                }
            }

            Some(Ok(Token::ImpulseDefBegin)) =>
            {
                set_state!(LexerState::ImpulseBlock(declare_block!(false)?));
            }
            Some(Ok(Token::LatchDefBegin)) =>
            {
                set_state!(LexerState::LatchBlock(declare_block!(true)?));
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
    Ok(CircuitDef { metadata, impulses, latches, auto_entries, signal_entries })
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn basic()
    {
        let input =
r#"
meta="value"
[ConditionalLatch] Cond1 # comment
OnTrue > Print1 # comment
# comment
True <> Sub1
False <> -Sub1
x = 5
<DebugLog> Print1
Text = "Hola!"
~ Sig1 # comment
Cond1 # comment
~ Sig1 # comment
Print1 # comment
@ # comment
Print1"#;

        let lexed = lex_circuit_dsl(input);
        println!("!!! {:#?}", lexed);
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
*/