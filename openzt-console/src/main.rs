use std::io::{self, Read, Write};
use std::net::TcpStream;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let server_address = "127.0.0.1:8080";

    // Check for --oneshot flag
    if let Some(pos) = args.iter().position(|arg| arg == "--oneshot") {
        if pos + 1 >= args.len() {
            eprintln!("Error: --oneshot requires a command argument");
            eprintln!("Usage: openzt-console --oneshot <command>");
            std::process::exit(1);
        }

        let command = &args[pos + 1];
        return run_oneshot(server_address, command);
    }

    // Interactive mode (existing behavior)
    run_interactive(server_address)
}

fn run_oneshot(server_address: &str, command: &str) -> io::Result<()> {
    let mut stream = TcpStream::connect(server_address)?;

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

fn run_interactive(server_address: &str) -> io::Result<()> {
    let mut stream = TcpStream::connect(server_address)?;
    println!("Connected to server at {}", server_address);

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
