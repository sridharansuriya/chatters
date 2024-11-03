use std::{env, sync::Arc};

use chatters::Server;

fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Invalid usage! chatters serve ipaddress:port");
        return;
    }
    match args[1].as_str() {
        "serve" => {
            let server = Server::new(args[2].clone());
            let server = Arc::new(server);
            server.run();
        },
        x => {
            eprintln!("Unknown option provided: {}", x);
        }
    }
}
