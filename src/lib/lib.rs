pub mod ast;
pub mod parser;
pub mod types;

pub type Error = pest::error::Error<parser::Rule>;
