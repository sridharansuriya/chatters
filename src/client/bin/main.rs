use std::env;

use chatters::Client;

fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Invalid usage! chatters connect ipaddress:port");
        return;
    }
    match args[1].as_str() {
        "connect" => {
            Client::connect(args[2].clone());
        },
        x => {
            eprintln!("Unknown option provided: {}", x);
        }
    }
}
