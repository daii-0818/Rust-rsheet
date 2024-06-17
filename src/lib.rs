//lib.rs
mod spreadsheet;
mod command_handle;

use rsheet_lib::connect::{Manager, Reader, Writer};
use rsheet_lib::replies::Reply;

use std::error::Error;
use crate::spreadsheet::Spreadsheet;
use std::sync::{Arc, Mutex};
use std::thread;
use std::process::exit;
use crate::command_handle::parse_command;

enum Command {
    Get(String),
    Set(String, String),
}

pub fn start_server<M>(mut manager: M) -> Result<(), Box<dyn Error>>
where
    M: Manager,
{
    
    let spreadsheet = Arc::new(Mutex::new(Spreadsheet::new()));
    let thread_num = Arc::new(Mutex::new(0));
    thread::scope(|s| {
        loop {
            match manager.accept_new_connection() {
                Ok((mut recv, mut send)) => {
                    let mut tn = thread_num.lock().unwrap();
                    *tn += 1;
                    let spreadsheet = spreadsheet.clone();
                    let thread_num_clone = thread_num.clone();
                    let _spawn = s.spawn(move || {
                        
                        loop {
                            let msg = match recv.read_message() {
                                Ok(msg) => msg,
                                Err(_) => {
                                    let mut tn = thread_num_clone.lock().unwrap();
                                    *tn -= 1;
                                    return
                                }
                            };
                            
                            let reply: Option<Reply> = match parse_command(&msg) {
                                Ok(Command::Get(cell_name)) => {
                                    Some(spreadsheet.lock().unwrap().get(&cell_name))
                                }
                                Ok(Command::Set(cell_name, expression)) => spreadsheet.lock().unwrap().set(&cell_name, &expression),
                                Err(err) => Option::from(Reply::Error(err)),
                            };

                        
                            if let Some(reply_value) = reply{   
                            
                                let _ = send.write_message(reply_value);
                            }
                        }
                    });
                }
                Err(_) => {
                    let tn = thread_num.lock().unwrap();
                    
                    if *tn > 0 { continue }
                    else { exit(0) };
                },
            };
        }
    });
    Ok(())
}
