pub mod ast;
pub mod parser;
pub mod preprocessor;
pub mod types;

pub type Error = pest::error::Error<parser::Rule>;
