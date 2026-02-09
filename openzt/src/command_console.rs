use std::{
    error::Error,
    fmt,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::Mutex,
    thread,
};

use openzt_detour_macro::detour_mod;
use std::sync::LazyLock;
use tracing::{error, info};

/// Error type for command execution (kept for backward compatibility with existing command implementations)
#[derive(Debug)]
pub struct CommandError {
    message: String,
}

impl CommandError {
    pub fn new(message: String) -> Self {
        CommandError { message }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CommandError: {}", self.message)
    }
}

impl Error for CommandError {}

impl From<std::str::ParseBoolError> for CommandError {
    fn from(err: std::str::ParseBoolError) -> Self {
        CommandError {
            message: format!("Failed to parse bool: {}", err),
        }
    }
}

impl From<std::num::ParseIntError> for CommandError {
    fn from(err: std::num::ParseIntError) -> Self {
        CommandError {
            message: format!("Failed to parse int: {}", err),
        }
    }
}

impl From<std::num::ParseFloatError> for CommandError {
    fn from(err: std::num::ParseFloatError) -> Self {
        CommandError {
            message: format!("Failed to parse float: {}", err),
        }
    }
}

impl From<String> for CommandError {
    fn from(err: String) -> Self {
        CommandError { message: err }
    }
}

impl From<&str> for CommandError {
    fn from(err: &str) -> Self {
        CommandError { message: err.to_string() }
    }
}

#[detour_mod]
pub mod zoo_console {
    use openzt_detour::generated::ztapp::UPDATE_SIM;
    use tracing::error;

    use super::call_next_command;

    #[detour(UPDATE_SIM)]
    unsafe extern "thiscall" fn zoo_zt_app_update_game(_this_ptr: u32, param_2: i32) {
        call_next_command();
        unsafe { UPDATE_SIM_DETOUR.call(_this_ptr, param_2) }
    }

    pub fn init() {
        unsafe {
            if init_detours().is_err() {
                error!("Failed to initialize console detours");
            }
        };
    }
}

pub fn init() {
    info!("Initializing Lua console on 127.0.0.1:8080");
    zoo_console::init();
}

static COMMAND_THREAD: LazyLock<Mutex<std::thread::JoinHandle<()>>> = LazyLock::new(|| {
    Mutex::new(std::thread::spawn(|| {
        start_server();
    }))
});

static COMMAND_RESULTS: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::<String>::new()));

static COMMAND_QUEUE: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::<String>::new()));

/// Executes the next Lua code from the command queue on the game thread
pub fn call_next_command() {
    let _unused = COMMAND_THREAD.lock().unwrap();
    let Some(lua_code) = get_from_command_queue() else {
        return;
    };

    info!("Executing Lua: {}", lua_code.clone());

    let result = match crate::scripting::execute_lua(&lua_code) {
        Ok(result) => result,
        Err(err) => err,
    };

    let mut result_mutex = COMMAND_RESULTS.lock().unwrap();
    result_mutex.push(result);
}

pub fn get_next_result() -> Option<String> {
    let mut data_mutex = COMMAND_RESULTS.lock().unwrap();
    data_mutex.pop()
}

fn add_to_command_queue(command: String) {
    info!("Adding Lua code to queue: {}", command);
    let mut data_mutex = COMMAND_QUEUE.lock().unwrap();
    data_mutex.push(command);
}

pub fn get_from_command_queue() -> Option<String> {
    let mut data_mutex = COMMAND_QUEUE.lock().unwrap();
    data_mutex.pop()
}

fn handle_client(mut stream: TcpStream) {
    let mut buffer = [0; 1024];

    loop {
        match stream.read(&mut buffer) {
            Ok(size) => {
                if size == 0 {
                    // Connection closed by client
                    break;
                }

                // Received Lua code to execute
                let received_string = String::from_utf8_lossy(&buffer[0..size]);
                add_to_command_queue(received_string.to_string());
                info!("Received Lua code: {}", received_string);

                loop {
                    if let Some(result) = get_next_result() {
                        if let Err(err) = stream.write_all(result.as_bytes()) {
                            info!("Error sending data: {}", err);
                        }
                        break;
                    }
                }
            }
            Err(err) => {
                info!("Error reading data: {}", err);
                break;
            }
        }
    }
}

pub fn start_server() {
    let Ok(listener) = TcpListener::bind("127.0.0.1:8080") else {
        error!("Failed to bind socket 127.0.0.1:8080, console will not work");
        return;
    };

    info!("Listening on 127.0.0.1:8080...");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // Spawn a new thread for each incoming connection
                thread::spawn(move || {
                    handle_client(stream);
                });
            }
            Err(err) => {
                info!("Error accepting connection: {}", err);
            }
        }
    }
}
