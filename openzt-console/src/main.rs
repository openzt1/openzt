use clap::Parser;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

/// OpenZT Lua Console - Interactive runtime scripting console for Zoo Tycoon
#[derive(Parser, Debug)]
#[command(name = "openzt-console")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Host address to connect to (default: 127.0.0.1:8080)
    #[arg(short = 'H', long, default_value = "127.0.0.1:8080")]
    host: String,

    /// Retry connection until successful
    #[arg(long)]
    wait: bool,

    /// Execute a single command and exit
    #[arg(long)]
    oneshot: Option<String>,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    if let Some(command) = cli.oneshot {
        run_oneshot(&cli.host, &command, cli.wait)
    } else {
        run_interactive(&cli.host, cli.wait)
    }
}

fn connect_with_wait(host: &str, wait: bool) -> io::Result<TcpStream> {
    if wait {
        loop {
            match TcpStream::connect(host) {
                Ok(stream) => {
                    eprintln!("Connected to {}", host);
                    return Ok(stream);
                }
                Err(e) => {
                    eprintln!("Failed to connect to {}: {}", host, e);
                    eprintln!("Retrying in 1 second...");
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
    } else {
        TcpStream::connect(host)
    }
}

/// Wait for the console server to be ready by sending ping() until we get pong
fn wait_for_ready(host: &str, wait: bool) -> io::Result<TcpStream> {
    loop {
        let mut stream = connect_with_wait(host, wait)?;

        // Send ping
        if let Err(e) = stream.write_all(b"ping()") {
            if wait {
                eprintln!("Failed to send ping: {}, retrying...", e);
                thread::sleep(Duration::from_secs(1));
                continue;
            } else {
                return Err(e);
            }
        }

        // Read response
        let mut buffer = [0; 1024];
        match stream.read(&mut buffer) {
            Ok(size) => {
                let response = String::from_utf8_lossy(&buffer[0..size]).trim().to_string();
                if response == "pong" {
                    // Server is ready
                    return Ok(stream);
                } else {
                    // Unexpected response
                    if wait {
                        eprintln!("Unexpected response: {}, retrying...", response);
                        thread::sleep(Duration::from_secs(1));
                        continue;
                    } else {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("Expected 'pong', got: {}", response)
                        ));
                    }
                }
            }
            Err(e) => {
                // Connection reset or other error
                if wait {
                    eprintln!("Server not ready yet: {}, retrying...", e);
                    thread::sleep(Duration::from_secs(1));
                    continue;
                } else {
                    return Err(e);
                }
            }
        }
    }
}

fn run_oneshot(host: &str, command: &str, wait: bool) -> io::Result<()> {
    let mut stream = wait_for_ready(host, wait)?;

    // Send command
    stream.write_all(command.as_bytes())?;

    // Read response
    let mut buffer = [0; 100024];
    let size = stream.read(&mut buffer)?;

    if size > 0 {
        let response = String::from_utf8_lossy(&buffer[0..size]);
        print!("{}", response);
    }

    Ok(())
}

fn run_interactive(host: &str, wait: bool) -> io::Result<()> {
    let mut stream = wait_for_ready(host, wait)?;
    println!("Connected to server at {}", host);

    loop {
        let mut input = String::new();
        print!("Enter a command: ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut input)?;

        let trimmed = input.trim();
        if trimmed.to_lowercase() == "quit" {
            break;
        } else if trimmed.is_empty() {
            continue;
        }

        match stream.write(trimmed.as_bytes()) {
            Ok(_) => {
                // Message sent successfully
            }
            Err(err) => {
                eprintln!("Error sending data to server: {}", err);
                break;
            }
        }

        let mut buffer = [0; 100024];
        match stream.read(&mut buffer) {
            Ok(size) => {
                if size == 0 {
                    // Connection closed by server
                    break;
                }

                // Print server response
                let response = String::from_utf8_lossy(&buffer[0..size]);
                println!("Server response: {}", response);
            }
            Err(err) => {
                eprintln!("Error reading data from server: {}", err);
                break;
            }
        }
    }

    Ok(())
}
