use crate::command::CommandType;
use clap::Parser;
use std::error;

pub mod ast;
pub mod command;
pub mod db;
pub mod image;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<CommandType>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    let args = Cli::parse();
    CommandType::parse_command(args.command).await?;
    Ok(())
}
