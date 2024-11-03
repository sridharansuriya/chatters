use std::{
    io::{self, BufRead, BufReader, BufWriter, Write}, net::{Shutdown, TcpStream}, sync::mpsc, thread
};

pub struct Client {}

impl Client {
    pub fn connect(address: String) {
        let stream = TcpStream::connect(address).unwrap();
        let read_stream = stream.try_clone().unwrap();

        let (tx, rx) = mpsc::channel();

        // Handle reading from the stream
        let read_thread = thread::spawn(move || {
            let mut buf = String::new();
            let mut reader = BufReader::new(&read_stream);
            loop {
                // Check if the peer address is still valid
                if read_stream.peer_addr().is_err() {
                    break;
                }
                match reader.read_line(&mut buf) {
                    Ok(0) => {
                        // Connection closed by the server
                        println!("Connection closed by server.");
                        let _ = tx.send(());
                        break;
                    },
                    Ok(_) => {
                        println!("{}", buf.trim());
                    },
                    Err(e) => {
                        eprintln!("Error reading from stream: {}", e);
                        break;
                    },
                }
                buf.clear();
            }
        });

        // Handle writing to the stream
        let write_thread = thread::spawn(move || {
            let mut buf = String::new();
            let mut reader = BufReader::new(io::stdin());
            let mut writer = BufWriter::new(&stream);
            loop {
                if rx.try_recv().is_ok() {
                    break;
                }
                match reader.read_line(&mut buf) {
                    Ok(0) => continue,
                    Ok(_) => {
                        if writer.write(buf.as_bytes()).is_err() {
                            eprintln!("Error writing to stream.");
                            break;
                        }
                        if writer.flush().is_err() {
                            eprintln!("Error flushing writer.");
                            break;
                        }
                    }
                    Err(_) => {
                        eprintln!("Error reading from stdin.");
                        break;
                    }
                }
                buf.clear();
            }

            // Shut down the write side when done
            if let Err(e) = stream.shutdown(Shutdown::Write) {
                eprintln!("Failed to shut down the stream: {}", e);
            }
        });

        // Wait for both threads to finish
        let _ = read_thread.join();
        let _ = write_thread.join();
    }
}

