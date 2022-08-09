mod utils;

extern crate ev3dev_lang_rust;

use std::net::{SocketAddr, UdpSocket};
use std::process::exit;
use std::str::FromStr;
use std::sync::Mutex;
use std::thread::sleep;
use std::time::{Duration, SystemTime};
use ev3dev_lang_rust::{Ev3Button, Led};
use ev3dev_lang_rust::sensors::{InfraredSensor, RemoteControl, SensorPort, TouchSensor};
use scoped_threadpool::Pool;
use crate::utils::PcConnection;

const INCOMING_PREFIX:&str = "pc2ev-";
const OUTGOING_PREFIX:&str = "ev2pc-";

fn main() {
    set_leds(true, true, "yellow");
    let connection:Mutex<PcConnection> = Mutex::new(PcConnection { connected: false, last_seen: 0 });

    let listen = "0.0.0.0:42069";
    let target = &SocketAddr::from_str("192.168.178.69:6969").unwrap();
    let mut pool = Pool::new(4);

    let socket = UdpSocket::bind(listen).expect("Couldn't bind to address");

    // this timeout has to be high enough so we can try to reconnect before it expires
    socket.set_read_timeout(Some(Duration::from_secs(69))).expect("Couldn't set read timeout");

    pool.scoped(|scope| {
        scope.execute(|| socket_thread(&socket, &connection)); // this thread is responsible for receiving messages
        scope.execute(|| init_connection_thread(&socket, target)); // this thread closes immediately
        scope.execute(|| input_thread(&socket, target)); // this thread is responsible for receiving input and sending messages
        scope.execute(|| keepalive_thread(&socket, &connection, target)); // this thread sends keepalive messages
        scope.join_all();
    });
}

fn set_leds(left:bool, right:bool, color:&str) {
    let led = Led::new().expect("Couldn't create LED");
    led.set_color(Led::COLOR_RED).expect("Couldn't set LED color");


    let col;

    match color {
        "green" => col = Led::COLOR_GREEN,
        "yellow" => col = Led::COLOR_YELLOW,
        "orange" => col = Led::COLOR_ORANGE,
        "amber" => col = Led::COLOR_AMBER,
        "red" => col = Led::COLOR_RED,
        _ => col = Led::COLOR_OFF,
    }

    if left && right {
        led.set_color(col).expect("Couldn't set LED color");
    } else {
        if left {
            led.set_color(col).expect("Couldn't set LED color");
        }
        if right {
            led.set_right_color(col).expect("Couldn't set LED color");
        }
    }
}

fn init_connection_thread(socket: &UdpSocket, target:&SocketAddr) {
    sleep(Duration::from_millis(1));
    send_to_pc(&socket, target, format!("connect?{}", hostname::get().unwrap().to_str().unwrap()).as_str());
}

fn socket_thread(socket:&UdpSocket, connection:&Mutex<PcConnection>) {
    println!("i want to commit arson");

    let socket = socket.try_clone().unwrap();
    let mut buf = [0; 1024];
    loop {
        let (amount, src) = socket.recv_from(&mut buf).unwrap_or_else(|e| {
            set_leds(true, true, "red");
            println!("Read timeout: {}", e);
            exit(1);
        });
        //println!("Received {} bytes", amount - 1);
        let message = String::from_utf8_lossy(&buf[..amount-1]); // -1 to cut off the \n
        if message.starts_with(INCOMING_PREFIX) {
            let message = message.to_string().strip_prefix(INCOMING_PREFIX).unwrap().to_string();

            println!("Received message from {}: {}", src, message);
            let split = message.split("?").collect::<Vec<&str>>();
            let command = split[0];
            match command {
                "connected" => {
                    let mut connection = connection.lock().unwrap();
                    if !connection.connected {
                        set_leds(true, true, "green");
                        connection.connected = true;
                        connection.last_seen = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
                        println!("Connected!");
                    }

                    drop(connection);
                }
                "keepalive" => {
                    let mut connection = connection.lock().unwrap();
                    if connection.connected {
                        connection.last_seen = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
                    }

                    drop(connection);
                }
                _ => {
                    println!("Unknown command: {}", command);
                }
            }
        }
    }
}

fn keepalive_thread(socket:&UdpSocket, connection:&Mutex<PcConnection>, target:&SocketAddr) {
    let mut no_response_count:i8 = 0;
    loop {
        sleep(Duration::from_secs(2));
        let mut connection = connection.lock().unwrap();
        if connection.connected {
            let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
            no_response_count = 0;
            if now - connection.last_seen > 5 {
                connection.connected = false;
                set_leds(true, true, "amber");
                println!("Disconnected!");
                // try reconnecting
                sleep(Duration::from_secs(3));
                println!("Trying to reconnect...");
                init_connection_thread(socket, target);
            } else {
                send_to_pc(&socket, target, "keepalive");
            }
        } else {
            no_response_count += 1;
            if no_response_count > 5 {
                set_leds(true, true, "red");
                println!("Failed to connect!");
                exit(1);
            }
        }
        drop(connection);
    }
}

fn input_thread(socket:&UdpSocket, target:&SocketAddr) {
    let button = Ev3Button::new().unwrap();

    let touch_sensor = TouchSensor::get(SensorPort::In1).unwrap();

    let infrared_sensor = InfraredSensor::get(SensorPort::In2).unwrap();
    let remote = RemoteControl::new(infrared_sensor, 1).unwrap();

    let mut ignore_touch = false;
    let mut ignore_enter = false;
    let mut ignore_remote = false;

    loop {
        button.process();
        remote.process().unwrap();

        let touch_state = touch_sensor.get_pressed_state().unwrap();
        let enter_state = button.is_enter();
        let remote_red_up_state = remote.is_red_up();

        if touch_state && !ignore_touch {
            ignore_touch = true;
            println!("down: touch");
            set_leds(true, true, "orange");
            send_to_pc(&socket, target, "kd");
        } else if !touch_state && ignore_touch {
            println!("up: touch");
            set_leds(true, true, "off");
            send_to_pc(&socket, target, "ku");
            ignore_touch = false;
        }

        if enter_state && !ignore_enter {
            ignore_enter = true;
            println!("down: enter");
            set_leds(true, true, "orange");
            send_to_pc(&socket, target, "kd");
        } else if !enter_state && ignore_enter {
            println!("up: enter");
            set_leds(true, true, "off");
            send_to_pc(&socket, target, "ku");
            ignore_enter = false;
        }

        if remote_red_up_state && !ignore_remote {
            ignore_remote = true;
            println!("down: remote");
            set_leds(true, true, "orange");
            send_to_pc(&socket, target, "kd");
        } else if !remote_red_up_state && ignore_remote {
            println!("up: remote");
            set_leds(true, true, "off");
            send_to_pc(&socket, target, "ku");
            ignore_remote = false;
        }
    }
}

fn send_to_pc(socket: &&UdpSocket, addr: &SocketAddr, message: &str) {
    send(socket, addr, &*(OUTGOING_PREFIX.to_owned() + message));
}

fn send(socket:&UdpSocket, address:&SocketAddr, message:&str) {
    socket.send_to((message.to_owned() + "\n").as_bytes(), address).expect("Couldn't send data");
}
