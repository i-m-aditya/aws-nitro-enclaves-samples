pub mod command_parser;
pub mod protocol_helpers;
pub mod utils;

use command_parser::{ClientArgs, ServerArgs};
use protocol_helpers::{recv_loop, recv_u64, send_loop, send_u64};

use nix::sys::socket::listen as listen_vsock;
use nix::sys::socket::{accept, bind, connect, shutdown, socket};
use nix::sys::socket::{AddressFamily, Shutdown, SockAddr, SockFlag, SockType};
use nix::unistd::close;
use std::convert::TryInto;
use std::os::unix::io::{AsRawFd, RawFd};

const VMADDR_CID_ANY: u32 = 0xFFFFFFFF;
const BUF_MAX_LEN: usize = 8192;
// Maximum number of outstanding connections in the socket's
// listen queue
const BACKLOG: usize = 128;
// Maximum number of connection attempts
const MAX_CONNECTION_ATTEMPTS: usize = 5;

#[derive(Debug)]
struct VsockSocket {
    socket_fd: RawFd,
}

impl VsockSocket {
    fn new(socket_fd: RawFd) -> Self {
        VsockSocket { socket_fd }
    }
}

impl Drop for VsockSocket {
    fn drop(&mut self) {
        shutdown(self.socket_fd, Shutdown::Both)
            .unwrap_or_else(|e| eprintln!("Failed to shut socket down: {:?}", e));
        close(self.socket_fd).unwrap_or_else(|e| eprintln!("Failed to close socket: {:?}", e));
    }
}

impl AsRawFd for VsockSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.socket_fd
    }
}

/// Initiate a connection on an AF_VSOCK socket
fn vsock_connect(cid: u32, port: u32) -> Result<VsockSocket, String> {
    let sockaddr = SockAddr::new_vsock(cid, port);
    let mut err_msg = String::new();

    for i in 0..MAX_CONNECTION_ATTEMPTS {
        let vsocket = VsockSocket::new(
            socket(
                AddressFamily::Vsock,
                SockType::Stream,
                SockFlag::empty(),
                None,
            )
            .map_err(|err| format!("Failed to create the socket: {:?}", err))?,
        );
        match connect(vsocket.as_raw_fd(), &sockaddr) {
            Ok(_) => return Ok(vsocket),
            Err(e) => err_msg = format!("Failed to connect: {}", e),
        }

        // Exponentially backoff before retrying to connect to the socket
        std::thread::sleep(std::time::Duration::from_secs(1 << i));
    }

    Err(err_msg)
}

/// Send 'Hello, world!' to the server
pub fn client(args: ClientArgs) -> Result<(), String> {
    let vsocket = vsock_connect(args.cid, args.port)?;

    println!("Vsock connected : {:?}", vsocket);
    let fd = vsocket.as_raw_fd();
    std::thread::sleep(std::time::Duration::from_secs(10));
    let data = "Hello, world!".to_string();
    let buf = data.as_bytes();
    let len: u64 = buf.len().try_into().map_err(|err| format!("{:?}", err))?;
    send_u64(fd, len)?;
    send_loop(fd, buf, len)?;
    // **********
    send_u64(fd, 4)?;

    // send_loop(fd, "Dean".as_bytes(), 4);
    // send_loop(fd, buf, len)?;
    // **********

    // std::thread::sleep(std::time::Duration::from_secs(5));

    // let new_data = "Dean eingermane".to_string();
    // let new_buf = new_data.as_bytes();
    // let new_len: u64 = new_buf
    //     .len()
    //     .try_into()
    //     .map_err(|err| format!("{:?}", err))?;
    // send_loop(fd, new_buf, new_len);

    Ok(())
}

/// Accept connections on a certain port and print
/// the received data
pub fn server(args: ServerArgs) -> Result<(), String> {
    let socket_fd = socket(
        AddressFamily::Vsock,
        SockType::Stream,
        SockFlag::empty(),
        None,
    )
    .map_err(|err| format!("Create socket failed: {:?}", err))?;

    println!("Socket created: {:?}", socket_fd);

    let sockaddr = SockAddr::new_vsock(VMADDR_CID_ANY, args.port);

    println!("Binding to {:?}", sockaddr);

    bind(socket_fd, &sockaddr).map_err(|err| format!("Bind failed: {:?}", err))?;

    listen_vsock(socket_fd, BACKLOG).map_err(|err| format!("Listen failed: {:?}", err))?;

    println!("Started listening again");
    let fd = accept(socket_fd).map_err(|err| format!("Accept failed: {:?}", err))?;
    loop {
        println!("Go again");
        std::thread::sleep(std::time::Duration::from_secs(5));
        let len = recv_u64(fd)?;
        println!("Buf length: {:?}", len);
        let mut buf = [0u8; BUF_MAX_LEN];
        recv_loop(fd, &mut buf, len)?;

        println!("{:?}", buf);
        println!("Length of buf: {}", buf.len());
        println!("Received: {}", String::from_utf8(buf.to_vec()).unwrap());
        std::thread::sleep(std::time::Duration::from_secs(5));

        // *************
        let new_len = recv_u64(fd)?;
        println!("New Length: {:?}", new_len);
        // break;

        // let mut new_buf = [0u8; BUF_MAX_LEN];
        // recv_loop(fd, &mut new_buf, len)?;
        // println!(
        //     "Received: {:?}",
        //     String::from_utf8(new_buf.to_vec()[0..new_len as usize])
        // );
        // *************
        // health_check();
        // println!(
        //     "{}",
        //     String::from_utf8(buf.to_vec())
        //         .map_err(|err| format!("The received bytes are not UTF-8: {:?}", err))?
        // );
    }
}

pub fn health_check() {
    println!("Health, Check!");
}
