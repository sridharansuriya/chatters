use std::{
    collections::HashMap,
    io::{BufRead, BufReader, BufWriter},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

use super::{common::write_and_flush, message::MessageOptions};

pub struct Server {
    address: String,
    rooms: Mutex<HashMap<String, Vec<Arc<Client>>>>,
    client_rooms: Mutex<HashMap<String, String>>,
    handles: Mutex<Vec<JoinHandle<()>>>,
}

impl Server {
    pub fn new(address: String) -> Self {
        let clients = vec![];
        let mut rooms = HashMap::new();
        rooms.insert("default".to_owned(), clients);
        Self {
            address,
            rooms: Mutex::new(rooms),
            client_rooms: Mutex::new(HashMap::default()),
            handles: Mutex::new(vec![]),
        }
    }

    pub fn run(self: Arc<Self>) {
        let listener = TcpListener::bind(&self.address).unwrap();
        println!("Server running on: {}", self.address);
        let cleanup = thread::spawn({
            let server_clone = Arc::clone(&self);
            move || {
                loop {
                    {
                        let mut handles = server_clone.handles.lock().unwrap();
                        let mut remove_idx = None;
                        for (idx, handle) in handles.iter().enumerate() {
                            if handle.is_finished() {
                                remove_idx = Some(idx);
                            }
                        }
                        if let Some(idx) = remove_idx {
                            println!("a thread just finished execution");
                            let handle = handles.remove(idx);
                            let _ = handle.join();
                        }
                    }
                    thread::sleep(Duration::from_millis(100)); // to prevent infinite lock on join handle. there should be a better way to do this.
                }
            }
        });
        for conn in listener.incoming() {
            match conn {
                Ok(stream) => {
                    let server_clone = Arc::clone(&self);
                    let handle = thread::spawn(|| server_clone.handle_connection(stream));
                    let mut handles = self.handles.lock().unwrap();
                    handles.push(handle);
                }
                Err(_) => {
                    eprintln!("error accepting connection");
                    continue;
                }
            }
        }
        let _ = cleanup.join();
    }

    fn handle_connection(self: Arc<Self>, stream: TcpStream) {
        self.clone().register_client(stream.try_clone().unwrap());
        let read_stream = stream.try_clone().unwrap();
        let read_thread = thread::spawn(move || {
            let mut reader = BufReader::new(read_stream);
            loop {
                if let Err(err) = self.read_connection(&mut reader, &stream) {
                    eprintln!("{}", err);
                    stream.shutdown(std::net::Shutdown::Both).unwrap();
                    break;
                }
            }
        });

        let _ = read_thread.join();
    }

    fn read_connection(
        &self,
        reader: &mut BufReader<TcpStream>,
        stream: &TcpStream,
    ) -> Result<(), String> {
        let mut buf = String::new();
        match reader.read_line(&mut buf) {
            Ok(0) => Ok(()),
            Ok(_) => {
                // handle different commands
                // todo: what to do if message has less than one character.
                match MessageOptions::get_option(buf.split_whitespace().nth(0).unwrap()) {
                    MessageOptions::ROOMS => {
                        let mut writer = BufWriter::new(stream);
                        let rooms = self.rooms.lock().unwrap();
                        let rooms = rooms.keys();
                        for room in rooms {
                            write_and_flush(&mut writer, format!("{}\n", room).as_bytes());
                        }
                    }
                    MessageOptions::JOIN => {
                        let cmd: Vec<_> = buf.split_whitespace().collect();
                        if cmd.len() != 2 {
                            let mut writer = BufWriter::new(stream);
                            write_and_flush(
                                &mut writer,
                                "invalid usage for /join: expected usage /join <room_name>\n"
                                    .as_bytes(),
                            );
                            return Ok(());
                        }
                        //get original sender
                        let sender_room = self.get_active_room(stream).unwrap();

                        let sender = self.get_sender(stream.try_clone().unwrap(), &sender_room);

                        self.pop_sender_from_room(&sender_room, sender.clone());

                        let mut rooms = self.rooms.lock().unwrap();
                        let new_room = cmd.get(1).unwrap().to_string();
                        // check and see if room exists
                        if rooms.contains_key(&new_room) {
                            // a if exists, add user to the room
                            let rooms = rooms.get_mut(&new_room).unwrap();
                            rooms.push(sender);
                        }
                        // b if not, create room and user to the room
                        else {
                            let chat_room = vec![sender];
                            rooms.insert(new_room.clone(), chat_room);
                        }
                        // update user room to new room
                        let mut client_rooms = self.client_rooms.lock().unwrap();
                        client_rooms.insert(stream.peer_addr().unwrap().to_string(), new_room);
                    }
                    MessageOptions::LEAVE => {
                        let room = self.get_active_room(stream);
                        debug_assert!(room.is_some()); // a user should always be part of a room
                        let room = room.unwrap();
                        let mut writer = BufWriter::new(stream);
                        if &room.trim() == &"default" {
                            write_and_flush(
                                &mut writer,
                                format!(
                                    "cannot leave from {} room, call /quit instead\n",
                                    String::from(&room)
                                )
                                .as_bytes(),
                            );
                            return Ok(());
                        }

                        let sender = self.get_sender(stream.try_clone().unwrap(), &room);
                        self.pop_sender_from_room(&room, sender.clone());
                        // mutex is locked on client_rooms, so this will block
                        self.add_to_default_room(sender);
                    }
                    MessageOptions::WHICH => {
                        let room = self.get_active_room(stream);
                        debug_assert!(room.is_some()); // a user should always be part of a room
                        let mut writer = BufWriter::new(stream);
                        write_and_flush(&mut writer, format!("{}\n", room.unwrap()).as_bytes());
                    }
                    MessageOptions::MESSAGE => {
                        // way too much typing to get what I wanted. may be there's a better way to do this.
                        let sender_room = self.get_active_room(stream);
                        debug_assert!(sender_room.is_some()); //this should always succeed
                        let sender_room = sender_room.unwrap();
                        let sender =
                            self.get_sender(stream.try_clone().unwrap(), &sender_room);
                        let rooms = &self.rooms.lock().unwrap();
                        let client_rooms = rooms.get(&sender_room).unwrap();

                        for client in client_rooms.clone().iter() {
                            if client.stream.peer_addr().unwrap() != (stream.peer_addr().unwrap()) {
                                let mut broadcast_writer = BufWriter::new(&client.stream);
                                let msg = format!("{}: {}", sender.nickname, buf);
                                let msg = msg.as_bytes();

                                write_and_flush(&mut broadcast_writer, msg);
                            }
                        }
                    }
                    MessageOptions::QUIT => {
                        // todo remove client entries from map.
                        let sender_room = self.get_active_room(stream).unwrap();
                        let sender = self.get_sender(stream.try_clone().unwrap(), &sender_room);

                        self.pop_sender_from_room(&sender_room, sender.clone());
                        return Err("client disconnected!".to_owned());
                    }
                }
                Ok(())
            }
            Err(_) => Err("Error reading from connection".to_owned()),
        }
    }

    fn get_active_room(&self, stream: &TcpStream) -> Option<String> {
        let client_rooms = self.client_rooms.lock().unwrap();
        let room = client_rooms.get(&stream.peer_addr().unwrap().to_string());
        if room.is_some() {
            return Some(room.unwrap().to_owned())
        }
        return None
    }
    
    fn register_client(&self, stream: TcpStream) {
        let mut writer = BufWriter::new(stream.try_clone().unwrap());
        write_and_flush(&mut writer, "Please enter your nickname!\n".as_bytes());
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut buf = String::new();
        let _ = reader.read_line(&mut buf);
        let nickname = buf.trim().to_owned();
        write_and_flush(
            &mut writer,
            format!("Welcome to the chat {}\n", nickname).as_bytes(),
        );
        let client = Arc::new(Client::new(nickname, stream.try_clone().unwrap()));
        self.add_to_default_room(client);
    }

    fn add_to_default_room(&self, sender: Arc<Client>) {
        let mut rooms = self.rooms.lock().unwrap();
        let clients = rooms.get_mut("default");
        let addr = &sender.stream.peer_addr().unwrap().to_string();
        clients.unwrap().push(sender);
        let mut client_rooms = self.client_rooms.lock().unwrap();
        client_rooms.insert(addr.to_owned(), "default".to_owned());
    }

    fn get_sender(&self, stream: TcpStream, sender_room: &str) -> Arc<Client> {
        let rooms = self.rooms.lock().unwrap();
        let active_room = rooms.get(sender_room).unwrap();
        for client in active_room.iter() {
            if client.stream.peer_addr().unwrap() == stream.peer_addr().unwrap() {
                return client.clone();
            }
        }
        unreachable!("this shouldn't happen");
    }

    fn pop_sender_from_room(&self, sender_room: &str, sender: Arc<Client>) {
        let mut rooms = self.rooms.lock().unwrap();
        let chat_room = rooms.get_mut(sender_room).unwrap();
        chat_room.retain(|c| c.stream.peer_addr().unwrap() != sender.stream.peer_addr().unwrap());
    }
}

#[derive(Debug)]
struct Client {
    nickname: String,
    stream: TcpStream,
}

impl Client {
    fn new(nickname: String, stream: TcpStream) -> Self {
        Self { nickname, stream }
    }
}
