use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use std::thread::{self, spawn, JoinHandle};
use std::time::Duration;

// Secure TCP connection module
pub struct STCPSession {
    host: String,      // ssh remote host name, must be defined in ~/.ssh/config
    server_port: u32,  // port to connect and listen to
    client_port: u32,  // port to connect and listen to
    workspace: String, // remote workspace to monitor

    ssh_thread: Option<JoinHandle<()>>,
    ssh_restart_process: Arc<AtomicBool>,
}

impl STCPSession {
    pub fn new(host: String, server_port: u32, client_port: u32, workspace: String) -> STCPSession {
        let mut session = STCPSession {
            host,
            server_port,
            client_port,
            workspace,
            ssh_thread: None,
            ssh_restart_process: Arc::new(AtomicBool::new(true)),
        };
        session.start_ssh();
        session
    }

    pub fn start_ssh(&mut self) {
        if self.ssh_thread.is_some() {
            return; // ssh thread already started
        }

        let host = self.host.clone();
        let workspace = self.workspace.clone();
        let server_port = self.server_port;
        let client_port = self.client_port;
        let ssh_restart_process = self.ssh_restart_process.clone();

        self.ssh_thread = Some(spawn(move || {
            let mut retries: i32 = 0;
            while ssh_restart_process.load(Relaxed) {
                let mut child = Command::new("ssh")
                    .arg("-L")
                    .arg(format!("{}:localhost:{}", client_port, server_port))
                    .arg(host.clone())
                    .arg(format!(
                        "~/.emacs_remote/bin/emacs-remote-server -w {} -p {}",
                        workspace, server_port,
                    ))
                    .spawn()
                    .expect("Failed to start ssh server");

                thread::sleep(Duration::new(2, 0));

                loop {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            if status.success() {
                                break;
                            } else if retries > 5 {
                                println!("Failed to start ssh server 5 times.");
                                return;
                            } else {
                                retries += 1;
                            }
                        }
                        Ok(None) => {
                            retries = 0;
                            if !ssh_restart_process.load(Relaxed) {
                                child.kill().expect("Failed to kill child process");
                                return;
                            }
                            thread::sleep(Duration::new(1, 0));
                        }
                        Err(e) => {
                            println!("error attempting to wait: {}", e);
                            return;
                        }
                    }
                }
            }
        }));
    }
}

impl Drop for STCPSession {
    fn drop(&mut self) {
        self.ssh_restart_process.store(false, Relaxed);

        if self.ssh_thread.is_none() {
            return;
        }
        let _ = self
            .ssh_thread
            .take()
            .map(JoinHandle::join)
            .expect("Unable to join thread");
    }
}
