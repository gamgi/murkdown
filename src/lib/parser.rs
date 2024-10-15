use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "lib/parser_grammar.pest"]
struct RawParser;

impl Default for Rule {
    fn default() -> Self {
        Rule::Root
    }
}
