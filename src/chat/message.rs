pub enum MessageOptions {
    ROOMS,
    JOIN,
    LEAVE,
    MESSAGE,
    WHICH,
    QUIT,
}

impl MessageOptions {
    pub fn get_option(option: &str) -> MessageOptions {
        match option {
            "/rooms" => MessageOptions::ROOMS,
            "/join" => MessageOptions::JOIN,
            "/leave" => MessageOptions::LEAVE,
            "/which" => MessageOptions::WHICH,
            "/quit" => MessageOptions::QUIT,
            _ => MessageOptions::MESSAGE
        }   
    }
}