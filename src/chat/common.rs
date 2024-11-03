use std::io::{ BufWriter, Write };


pub fn write_and_flush<T: std::io::Write>(writer: &mut BufWriter<T>, message: &[u8]) {
    let _ = writer.write(message);
    let _ = writer.flush();
}