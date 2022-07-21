mod utils;

extern crate ev3dev_lang_rust;

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::str::FromStr;
use std::sync::Mutex;
use std::thread::sleep;
use std::time::Duration;
use ev3dev_lang_rust::{Ev3Button};
use scoped_threadpool::Pool;
use crate::utils::PcConnection;

const INCOMING_PREFIX:&str = "pc2ev-";
const OUTGOING_PREFIX:&str = "ev2pc-";

fn main() {
    // todo: keepalive system
    //let connections:Mutex<HashMap<SocketAddr, PcConnection>> = Mutex::new(HashMap::new());

    let listen = "0.0.0.0:42069";
    let target = &SocketAddr::from_str("192.168.0.2:6969").unwrap();
    let mut pool = Pool::new(4);

    let mut socket = UdpSocket::bind(listen).expect("Couldn't bind to address");

    pool.scoped(|scope| {
        scope.execute(|| socket_thread(&socket)); // this thread is responsible for receiving messages
        scope.execute(|| init_connection_thread(&socket, target)); // this thread closes immediately
        scope.execute(|| input_thread(&socket, target)); // this thread is responsible for receiving input and sending messages
        scope.execute(|| keepalive_thread(&socket, target)); // todo: finish keepalive system
        scope.join_all();
    });
}

fn init_connection_thread(socket: &UdpSocket, target:&SocketAddr) {
    sleep(Duration::from_millis(500));
    send_to_pc(&socket, target, format!("connect?{}", hostname::get().unwrap().to_str().unwrap()).as_str());
}

fn socket_thread(socket:&UdpSocket) {
    println!("i want to commit arson");

    let socket = socket.try_clone().unwrap();
    let mut buf = [0; 1024];
    loop {
        let (amount, src) = socket.recv_from(&mut buf).expect("Couldn't receive data");
        //println!("Received {} bytes", amount - 1);
        let message = String::from_utf8_lossy(&buf[..amount-1]); // -1 to cut off the \n
        //println!("Received message from {}: {}", src, message);
    }
}

fn keepalive_thread(socket:&UdpSocket, target:&SocketAddr) {
    loop {
        sleep(Duration::from_secs(2));
        send_to_pc(&socket, target, "keepalive");
    }
}

fn input_thread(socket:&UdpSocket, target:&SocketAddr) {
    let button = Ev3Button::new().unwrap();

    let mut up = false;
    let mut down = false;
    let mut left = false;
    let mut right = false;
    let mut enter = false;
    let mut back = false;

    loop {
        button.process();

        if up != button.is_up() {
            if button.is_up() {
                println!("never gonna give you UP");
                send_to_pc(&socket, target, "media?volup");
            }
        }
        if down != button.is_down() {
            if button.is_down() {
                println!("never gonna let you DOWN");
                send_to_pc(&socket, target, "media?voldown");
            }
        }
        if left != button.is_left() {
            if button.is_left() {
                println!("left");
                send_to_pc(&socket, target, "media?prev");
            }
        }
        if right != button.is_right() {
            if button.is_right() {
                println!("right");
                send_to_pc(&socket, target, "media?next");
            }
        }
        if enter != button.is_enter() {
            if button.is_enter() {
                println!("enter");
                send_to_pc(&socket, target, "media?pp");
            }
        }
        if back != button.is_backspace() {
            if button.is_backspace() {
                println!("Back");
                send_to_pc(&socket, target, "media?veryfunnyandhilariousmessagethatdefinitelydoesnotshutdownthewholefuckingsystemlmao");
            }
        }

        up = button.is_up();
        down = button.is_down();
        left = button.is_left();
        right = button.is_right();
        enter = button.is_enter();
        back = button.is_backspace();
    }
}

fn send_to_pc(socket: &&UdpSocket, addr: &SocketAddr, message: &str) {
    send(socket, addr, &*(OUTGOING_PREFIX.to_owned() + message));
}

fn send(socket:&UdpSocket, address:&SocketAddr, message:&str) {
    socket.send_to((message.to_owned() + "\n").as_bytes(), address).expect("Couldn't send data");
}