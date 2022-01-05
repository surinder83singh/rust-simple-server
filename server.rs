// Updated example from http://rosettacode.org/wiki/Hello_world/Web_server#Rust
// to work with Rust 1.0 beta

use std::net::{TcpStream, TcpListener};
use std::io::{Read, Write};
use std::thread;
use std::fs::{self, File};
use std::io::{self, BufRead};
use std::collections::HashMap;
use std::path::Path;
use std::env;
mod encoder;
use encoder::Encoder;


fn handle_read(mut stream: &TcpStream)->HashMap<String, String>{
    const BUF_LENGTH:usize = 4000;
    let mut buf = [0u8; BUF_LENGTH];
    let mut data = HashMap::new();
    match stream.read(&mut buf) {
        Ok(read_length) => {
            //println!("____: {}", read_length);
            //let req_str = String::from_utf8_lossy(&buf);
            //println!("REQ: {}", req_str);
            //let s = format!("{}", req_str);
            let cursor = io::Cursor::new(&buf);
            let mut lines:Vec<String> = cursor.lines().map(|l| l.unwrap()).collect();
            //println!("lines: {:?}", lines);
            let mut content_started = false;
            let mut content_length:usize = 0;
            let mut header_length = 0;

            //println!("line: {:?}", line);
            let first_line = &lines[0];
            header_length += first_line.len() + 2;
            let list: Vec<&str> = first_line.split(' ').collect();
            let method = list[0];
            let path = list[1];
            //println!("info: {}, {}, {:?}", method, path, list);
            data.insert("method".to_string(), method.to_string());
            data.insert("path".to_string(), path.to_string());
            lines.remove(0);

            let mut content_index = 0;
            for i in 0..lines.len() {
                let line = &lines[i];
                header_length += line.len() + 2;
                if line.len() == 0 {
                    content_started = true;
                    if data.contains_key("Content-Length"){
                        let cl = data.get("Content-Length").unwrap().parse::<usize>().unwrap_or(0);
                        //println!("cccccc: {:?}, header_length:{}", cl, header_length);
                        content_length = cl;
                        let mut more_to_read = content_length - (read_length - header_length);
                        let mut read_count = 0;
                        while more_to_read > 0 || read_count > 10 {
                            read_count += 1;
                            //println!("more_to_read: {}", more_to_read);
                            let mut data_buf = [0u8; 1000];
                            match stream.read(&mut data_buf) {
                                Ok(read_len) => {
                                    more_to_read -= read_len;
                                    //println!("data_buf: {:?}", data_buf);
                                    let cursor = io::Cursor::new(&data_buf);
                                    let mut data_lines:Vec<String> = cursor.lines().map(|l| l.unwrap()).collect();
                                    println!("data_lines: {:?}", data_lines);
                                    lines.append(&mut data_lines);
                                },
                                Err(e) => {
                                    println!("Unable to read stream: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                }else if content_started {
                    content_index = i;
                }else{
                    let mut list: Vec<&str> = line.split(": ").collect();
                    let key = list[0];
                    list.remove(0);
                    //println!("key: {}:{:?}", key, list.join(" "));
                    data.insert(key.to_string(), list.join(" ").to_string());
                }
            }

            for i in content_index..lines.len(){
                let line = &lines[i];
                //println!("Content: {}, line_len:{}", line, line.len());
                //println!("content_length: {}", content_length);
                let mut c = line.to_string();
                c.replace_range(content_length.., "");
                data.insert("contents".to_string(), c);

                //println!("data: {:?}", data);
            }
        },
        Err(e) => println!("Unable to read stream: {}", e),
    }

    data
}
fn ext_to_content_type(ext:&str)->(&str, bool){
    match ext {
        "js"=>("application/javascript", false),
        "css"=>("text/css", false),
        "html"=>("text/html", false),
        "jpg"=>("image/jpeg", true),
        "ico"=>("image/x-icon", true),
        _=>("text/html", false)
    }
}

fn build_response(file_path:&Path, content_type:&str)->Vec<u8>{
    let mut buf = Vec::new();
    let mut file = File::open(file_path).unwrap();
    file.read_to_end(&mut buf).unwrap();

    let mut encoded = Vec::new();
    {
        let mut encoder = Encoder::with_chunks_size(&mut encoded, 8);
        encoder.write_all(&buf).unwrap();
    }

    let headers = format!("HTTP/1.1 200 OK\r\nContent-Type: {};\r\nTransfer-Encoding: chunked\r\n\r\n", content_type);
    let mut response = headers.into_bytes();
    response.extend(encoded);
    response
}

fn handle_write(args:HashMap<String, String>, mut stream: TcpStream) {
    let mut file = "index.html";
    let mut method = "GET";
    if let Some(m) = args.get("method"){
        method = m;
    }
    if let Some(path) = args.get("path"){
        file = match path.as_str() {
            "/" => "/index.html",
            a=>a
        };
    }

    println!("file:{:?}, method:{:?}", file, method);

    let file_path = "./http".to_string()+file;
    let path = Path::new(&file_path);
    let res:Vec<u8>;
    let res_str:String;
    let response;
    if !path.exists(){
        res_str = format!(
            "HTTP/1.1 404 OK\r\nContent-Type: text/html; charset=UTF-8\r\nContent-Length: {}\r\n\r\n",
            0
        );
        response = res_str.as_bytes();
    }else{
        let ext = path.extension().unwrap().to_str().unwrap();
        let (content_type, is_binary) = ext_to_content_type(ext);
        if is_binary{
            res = build_response(path, content_type);
            response = &res;
        }else{
            let contents = fs::read_to_string(path)
                .expect("Something went wrong reading the file");
            res_str = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {};\r\nContent-Length: {}\r\n\r\n{}",
                content_type,
                contents.len(),
                contents
            );
            response = res_str.as_bytes();
        }
    }

    match stream.write(response) {
        Ok(_) => {
            //println!("Response sent")
        },
        Err(e) => println!("Failed sending response: {}", e),
    }

    stream.flush().unwrap();
}

fn handle_client(stream: TcpStream) {
    let req_args = handle_read(&stream);
    handle_write(req_args, stream);
}

fn main() {
    let mut port = 8080;
    let mut args = env::args();
    args.next();
    if let Some(p) = args.next(){
        port = p.parse::<u16>().unwrap();
    }
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
    println!("Listening for connections on port {}", port);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(|| {
                    handle_client(stream)
                });
            }
            Err(e) => {
                println!("Unable to connect: {}", e);
            }
        }
    }
}
