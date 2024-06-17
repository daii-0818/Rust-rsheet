// command_handler.rs

use crate::Command;

pub fn parse_command(input: &str) -> Result<Command, String> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    match parts.as_slice() {
        ["get", cell_name] => Ok(Command::Get(cell_name.to_string())),
        ["set", cell_name, ..] => {
            if parts.len() > 2 {
                let expression = parts[2..].join(" ");
                Ok(Command::Set(cell_name.to_string(), expression))
            } else {
                Err("set command needs the value.".to_owned())
            }
        }
        _ => Err("Unrecognized or invalid command".to_string()),
    }
}
