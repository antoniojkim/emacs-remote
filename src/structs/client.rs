extern crate rmp_serde as rmps;
extern crate ssh;

use std::convert::TryFrom;
use std::io::{Read, Write};
use std::net::{Incoming, TcpListener, TcpStream};
use std::{fs, io};

use serde::{Deserialize, Serialize};

use crate::handle::HandleClientDaemon;
use crate::messages::index::IndexRequest;
use crate::messages::messagetype::{MessageType, MessageTypeTrait};

pub struct ClientDaemon {
    pub workspace: String,
    // connection info
    pub client_path: String,
    client_port: String,
    server_port: String,

    // streams
    server: TcpStream,

    // state
    current_index_hash: u64,
}

impl ClientDaemon {
    pub fn new(
        workspace: String,
        client_path: String,
        client_port: String,
        server_port: String,
    ) -> ClientDaemon {
        let result = fs::create_dir_all(client_path.clone());
        assert!(result.is_ok());

        ClientDaemon {
            workspace,
            client_path,
            client_port: client_port.clone(),
            server_port: server_port.clone(),
            // initialize streams
            server: TcpStream::connect(format!("localhost:{}", server_port)).unwrap(),
            // initialize state
            current_index_hash: 0,
        }
    }

    pub fn server_send<T: Serialize>(&mut self, message: &T) -> Result<(), ()> {
        let buffer = rmps::encode::to_vec(&message).unwrap();
        if self.server.write(&buffer).is_err() {
            return Err(());
        }
        if self.server.flush().is_err() {
            return Err(());
        }
        Ok(())
    }

    pub fn server_recv<'a, T>(&mut self) -> Result<T, ()>
    where
        T: Deserialize<'a> + MessageTypeTrait + Clone,
    {
        let mut buf = [0; 1024];
        self.server.read(&mut buf).unwrap();

        let value: rmpv::Value = rmps::decode::from_read_ref(&buf).unwrap();
        println!("Request: {}", serde_json::to_string(&value).unwrap());

        if !value.is_array() || !value[0].is_u64() {
            return Err(());
        }
        let msgtype = MessageType::try_from(value[0].as_u64().unwrap()).unwrap();
        if msgtype != T::messagetype() {
            return Err(());
        }

        // let result: T = rmp_serde::from_read_ref(&buf).unwrap();
        // return Ok(result.clone());
        return Err(());
    }

    pub fn update_index_hash(&mut self, hash: u64) {
        self.current_index_hash = hash;
    }

    pub fn listen(&mut self) {
        let receiver = TcpListener::bind(format!("localhost:{}", self.client_port)).unwrap();

        for stream in receiver.incoming() {
            let mut stream = stream.unwrap();

            if self.handle(&mut stream).is_err() {
                println!("Failed to handle stream");
            }
        }
    }

    fn handle(&mut self, stream: &mut TcpStream) -> Result<(), ()> {
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
                return request.handle(stream, self);
            }
            _ => {
                println!("Invalid type: {:?}", msgtype);
                return Err(());
            }
        }
    }
}

// fn test_connection() {
//     let mut client = TcpStream::connect("localhost:9130").unwrap();

//     let request = IndexRequest::new(
//         0,                                                               // prev_hash
//         "/Users/antoniokim/Documents/Projects/emacs-remote".to_string(), // index_path
//     );

//     let now = Instant::now();

//     let buffer = rmps::encode::to_vec(&request).unwrap();
//     client.write(&buffer).unwrap();

//     if handle_response(&mut &client).is_err() {
//         panic!("Unable to handle response");
//     }

//     println!(
//         "Request handled in {} milliseconds",
//         now.elapsed().as_millis()
//     );
// }

// fn test_ssh() {
//     let now = Instant::now();

//     let mut session = ssh::Session::new().unwrap();
//     session.set_host("cerebras").unwrap();
//     session.parse_config(None).unwrap();
//     session.connect().unwrap();
//     println!("{:?}", session.is_server_known());
//     session.userauth_publickey_auto(None).unwrap();

//     println!(
//         "ssh connection established in {} milliseconds",
//         now.elapsed().as_millis()
//     );

//     for _i in 0..10 {
//         thread::sleep(Duration::from_millis(10000));

//         let now = Instant::now();

//         let mut scp = session
//             .scp_new(
//                 ssh::READ,
//                 "/net/antonio-dev/srv/nfs/antonio-data/ws/.emacs_remote/server/index_17645410072557185842.mp",
//             )
//             .unwrap();
//         scp.init().unwrap();
//         loop {
//             match scp.pull_request().unwrap() {
//                 ssh::Request::NEWFILE => {
//                     let mut buf: Vec<u8> = vec![];
//                     scp.accept_request().unwrap();
//                     scp.reader().read_to_end(&mut buf).unwrap();

//                     println!(
//                         "Took {} milliseconds to read file",
//                         now.elapsed().as_millis()
//                     );
//                     break;
//                 }
//                 ssh::Request::WARNING => {
//                     scp.deny_request().unwrap();
//                     break;
//                 }
//                 _ => scp.deny_request().unwrap(),
//             }
//         }
//     }
// }
