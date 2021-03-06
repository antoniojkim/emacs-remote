extern crate dirs;
extern crate rmp_serde as rmps;
extern crate rmpv;
extern crate serde_json;

use std::convert::TryFrom;
use std::env;
use std::fs;
use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;

use clap::{App, Arg};

use emacs_remote::handle::HandleServerDaemon;
use emacs_remote::messages::index::IndexRequest;
use emacs_remote::messages::messagetype::MessageType;
use emacs_remote::structs::server::ServerDaemon;
use emacs_remote::version::VERSION;

fn handle_connection(stream: &mut TcpStream, server_daemon: &mut ServerDaemon) -> Result<(), ()> {
    let mut buf = [0; 1024];
    stream.read(&mut buf).unwrap();

    let value: rmpv::Value = rmps::decode::from_read_ref(&buf).unwrap();
    println!("Request: {}", serde_json::to_string(&value).unwrap());

    assert!(value.is_array());
    assert!(value[0].is_u64());
    let msgtype = MessageType::try_from(value[0].as_u64().unwrap()).unwrap();

    match msgtype {
        MessageType::IndexRequest => {
            let request: IndexRequest = rmp_serde::from_read_ref(&buf).unwrap();
            return request.handle(stream, server_daemon);
        }
        _ => {
            println!("Invalid type: {:?}", msgtype);
            return Err(());
        }
    }
}

fn main() {
    // Set up default emacs_remote path
    let mut default_path = PathBuf::new();
    default_path.push(dirs::home_dir().unwrap());
    default_path.push(".emacs_remote");

    let app = App::new("emacs-remote-server-daemon")
        .version(VERSION)
        .author("antoniojkim <contact@antoniojkim.com>")
        .about("Starts emacs remote server daemon")
        .arg(
            Arg::with_name("workspace")
                .short("w")
                .long("workspace")
                .takes_value(true)
                .required(true)
                .help("Specifies the path to workspace on the remote server"),
        )
        .arg(
            Arg::with_name("emacs_remote_path")
                .short("r")
                .long("emacs_remote_path")
                .default_value(default_path.to_str().unwrap())
                .help("Path to emacs-remote directory"),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .default_value("9130")
                .help("Specifies the port that the server is listening on"),
        );

    let matches = app.get_matches_from(env::args_os());

    let mut server_daemon = ServerDaemon::new(
        matches.value_of("emacs_remote_path").unwrap().to_string(),
        matches.value_of("port").unwrap().to_string(),
        matches.value_of("workspace").unwrap().to_string(),
    );

    server_daemon.init();
    server_daemon.listen();
}
