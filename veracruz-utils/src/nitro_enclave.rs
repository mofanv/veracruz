//! Nitro-Enclave-specific material for Veracruz
//!
//! ## Authors
//!
//! The Veracruz Development Team.
//!
//! ## Licensing and copyright notice
//!
//! See the `LICENSE.markdown` file in the Veracruz root directory for
//! information on licensing and copyright.

use std::os::unix::io::{ AsRawFd, RawFd};
use std::process::Command;
use serde_json::Value;
use err_derive::Error;
use std::thread::JoinHandle;
use nix::sys::socket::{
    AddressFamily, SockType, SockFlag, SockAddr, socket, bind, listen, accept,
    shutdown, Shutdown
};

#[derive(Debug, Error)]
pub enum NitroError {
    #[error(display = "Nitro: Serde Error")]
    SerdeError,
    #[error(display = "nitro: Serde JSON Error:{:?}", _0)]
    SerdeJsonError(#[error(source)] serde_json::error::Error),
    #[error(display = "Nitrno: Nix Error:{:?}", _0)]
    NixError(#[error(source)] nix::Error),
    #[error(display = "Nitro: IO Error:{:?}", _0)]
    IOError(#[error(source)] std::io::Error),
    #[error(display = "Nitro: Veracruz Socket Error:{:?}", _0)]
    VeracruzSocketError(#[error(source)] crate::VeracruzSocketError),
    #[error(display = "Nitro: Utf8Error:{:?}", _0)]
    Utf8Error(#[error(source)] std::str::Utf8Error),
    #[error(display = "Nitro: EC2 Error")]
    EC2Error,
    #[error(display = "nitro: Mutex Error")]
    MutexError,
    #[error(display = "nitro: Thread Error")]
    ThreadError,
    #[error(display = "Nitro: Unimplemented")]
    UnimplementedError,
}

pub struct NitroEnclave {
    enclave_id: String,
    //enclave_cid: u32,
    vsocksocket: crate::vsocket::VsockSocket,
    ocall_thread: Option<JoinHandle<()>>,
    ocall_terminate_sender: Option<std::sync::Mutex<std::sync::mpsc::Sender<bool>>>,
}

const VERACRUZ_PORT: u32 = 5005;
const VMADDR_CID_ANY: u32 = 0xFFFFFFFF;
const OCALL_PORT: u32 = 5006;
const BACKLOG: usize = 128;

pub type OCallHandler = fn(Vec<u8>) -> Result<Vec<u8>, NitroError>;
enum OcallWaitResult {
    FileDescriptor(RawFd),
    Terminate,
}

impl NitroEnclave {
    pub fn new(eif_path: &str, debug: bool, ocall_handler: Option<OCallHandler>) -> Result<Self, NitroError> {
        let mut args = vec!["run-enclave",
                        "--eif-path", eif_path,
                        "--cpu-count", "2", 
                        "--memory", "256",];
        if debug {
            args.push("--debug-mode=true");
        }
        let enclave_result = Command::new("/usr/sbin/nitro-cli")
            .args(&args)
            .output()?;
        //let enclave_result_stderr = std::str::from_utf8(&enclave_result.stderr);
        //println!("enclave_result_stderr:{:?}", enclave_result_stderr);

        let enclave_result_stdout = std::str::from_utf8(&enclave_result.stdout)?;
        println!("enclave_result_stdout:{:?}", enclave_result_stdout);

        let enclave_data: Value =
            serde_json::from_str(enclave_result_stdout)?;
        let cid:u32 = if !enclave_data["EnclaveCID"].is_number() {
            return Err(NitroError::SerdeError);
        } else {
            serde_json::from_value(enclave_data["EnclaveCID"].clone()).unwrap()
        };

        let (ocall_thread_opt, sender) = match ocall_handler {
            None => (None, None), // Do nothing, we don't need to support ocalls
            Some(handler) => {
                let (tx, rx): (std::sync::mpsc::Sender<bool>, std::sync::mpsc::Receiver<bool>) = std::sync::mpsc::channel();
                let ocall_thread = std::thread::spawn(move || { NitroEnclave::ocall_loop(handler, rx)});
                (Some(ocall_thread), Some(std::sync::Mutex::new(tx)))
            },
        };

        let enclave: Self = NitroEnclave {
            enclave_id: enclave_data["EnclaveID"].to_string().trim_matches('"').to_string(),
            //enclave_cid: cid,
            vsocksocket: crate::vsocket::vsock_connect(cid, VERACRUZ_PORT)?,
            ocall_thread: ocall_thread_opt,
            ocall_terminate_sender: sender,
        };
        return Ok(enclave);
    }

    fn ocall_loop(handler: OCallHandler, terminate_rx: std::sync::mpsc::Receiver<bool>) {
        let socket_fd = socket(AddressFamily::Vsock, SockType::Stream, SockFlag::SOCK_NONBLOCK, None)
            .expect("NitroEnclave::ocall_loop failed to create a socket");
        let sockaddr = SockAddr::new_vsock(VMADDR_CID_ANY, OCALL_PORT);

        let mut im_done: bool = false;

        while let Err(err) = bind(socket_fd, &sockaddr) {
            if err == nix::Error::Sys(nix::errno::Errno::EADDRINUSE) {
                // before we continue, check to see if we should terminate
                if let Ok(terminate) = terminate_rx.try_recv() {
                    if terminate {
                        im_done = true;
                        break;
                    }
                }
            } else {
                panic!("I don't know what to do here");
            }
        }

        if !im_done {
            listen(socket_fd, BACKLOG)
                .map_err(|err| NitroError::NixError(err)).expect("NitroEnclave::ocall_loop listen failed");
            loop {
                match accept(socket_fd) {
                    Ok(fd) => {
                        println!("NitroEnclave::ocall_loop calling receive_buffer");
                        let received_buffer = crate::nitro::receive_buffer(fd)
                            .expect("NitroEnclave::ocall_loop failed to receive buffer");
                        // call the handler
                        let return_buffer = handler(received_buffer).expect("NitroEnclave::ocall_loop handler failed");
                        println!("NitroEnclave::ocall_loop calling send_buffer");
                        crate::nitro::send_buffer(fd, &return_buffer)
                            .expect("NitroEnclave::ocall_loop failed to send buffer");
                    },
                    Err(err) => match err {
                        nix::Error::Sys(errno) => {
                            if let Ok(terminate) = terminate_rx.try_recv() {
                                if terminate {
                                    break;
                                }
                            }
                        },
                        _ => println!("NitroEnclave::ocall_loop received error:{:?}", err),
                    },
                }
            }
        }

        shutdown(socket_fd, Shutdown::Both);
        println!("ocall_loop terminating ?gracefully?");
    }

    pub fn send_buffer(&self, buffer: &Vec<u8>) -> Result<(), NitroError> {
        crate::nitro::send_buffer(self.vsocksocket.as_raw_fd(), buffer)
            .map_err(|err| NitroError::VeracruzSocketError(err))
    }

    pub fn receive_buffer(&self) -> Result<Vec<u8>, NitroError> {
        crate::nitro::receive_buffer(self.vsocksocket.as_raw_fd())
            .map_err(|err| NitroError::VeracruzSocketError(err))
    }
}

impl Drop for NitroEnclave {
    fn drop(&mut self) {
        // first, tell the ocall loop to terminate
        if let Some(tx_mutex) = &self.ocall_terminate_sender {
            let sender_guard = tx_mutex.lock().unwrap();
            sender_guard.send(true);
        }

        // second, wait for the ocall loop to terminate
        // This is referred to as the "Option dance" - https://users.rust-lang.org/t/spawn-threads-and-join-in-destructor/1613
        // we can only do the "take" because we are effectively a destructor
        if let Some(thread_handle) = self.ocall_thread.take() {
                thread_handle.join();
        }

        // now, shutdown the enclave
        let enclave_result = Command::new("/usr/sbin/nitro-cli")
            .args(&["terminate-enclave", "--enclave-id", &self.enclave_id])
            .output().unwrap();
        let stdout = std::str::from_utf8(&enclave_result.stdout).unwrap();
        let stderr = std::str::from_utf8(&enclave_result.stderr).unwrap();
        let exit_status = enclave_result.status;
    }
}
