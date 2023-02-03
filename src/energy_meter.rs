use crate::*;
use std::fs::File;
use std::io::{BufWriter, Error, ErrorKind, Result, Write};
use std::net::{SocketAddr, TcpStream};
use std::ops::Add;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flate2::write::GzEncoder;
use flate2::Compression;

fn handle_requests(
    do_run: Arc<AtomicBool>,
    pac: Pac2200,
    interval: Duration,
    new_sockets: Receiver<(TcpStream, SocketAddr)>,
    log_to: Option<PathBuf>,
    log_interval: Duration,
    log_to_console: bool,
) {
    let mut sockets: Vec<(TcpStream, SocketAddr)> = Vec::new();

    let mut logger: Box<dyn Write> = if let Some(log_to) = log_to {
        let curr_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let file = File::create(log_to.join(format!("pac2200log-{}.json.gz", curr_secs)))
            .expect("Open logfile");
        let file = BufWriter::with_capacity(1024000, file);
        Box::new(GzEncoder::new(file, Compression::best()))
    } else {
        Box::new(DevNullFile::new())
    };

    let mut cur_values;
    loop {
        if let Some(values) = pac.get_current_params() {
            cur_values = values;
            break;
        }
    }
    let mut sockets_to_remove = Vec::new();
    let mut next_flush = SystemTime::now().add(log_interval);
    while do_run.load(Ordering::Relaxed) {
        if next_flush < SystemTime::now() {
            eprintln!("Flushing logfile...");
            logger.flush().expect("flush");
            next_flush = SystemTime::now().add(log_interval);
        }
        if let Some(values) = pac.get_current_params() {
            cur_values = values;
            if log_to_console {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&cur_values).expect("serde_json pretty")
                );
            }
        }
        let mut load = serde_json::to_string(&cur_values).expect("serde_json");
        load.push('\n');
        let load_bytes = load.as_bytes();
        logger.write_all(load_bytes).expect("Write to file");
        for (i, (socket, socket_peer_addr)) in sockets.iter_mut().enumerate() {
            if let Err(e) = socket.write_all(load_bytes) {
                eprintln!(
                    "Socket {} ({:?}) has an error: {:?} Removing from list of sockets",
                    i, socket_peer_addr, e
                );
                sockets_to_remove.push(i);
            }
        }
        // Use pop here, we need to start from the end of the sockets array
        while let Some(socket_index) = sockets_to_remove.pop() {
            sockets.remove(socket_index);
        }
        while let Ok(new_socket) = new_sockets.try_recv() {
            sockets.push(new_socket);
        }
        std::thread::sleep(interval);
    }
}

pub fn energy_meter(emp: EnergyMeterParams) -> Result<()> {
    if emp.polling_interval.is_some() && emp.polling_interval.unwrap() < 1000 {
        return Err(Error::new(
            ErrorKind::Other,
            format!("A polling interval smaller than one second is too low"),
        ));
    }
    if emp.log_flush_interval.is_some() && emp.log_flush_interval.unwrap() < 10 {
        return Err(Error::new(
            ErrorKind::Other,
            format!("A flushing interval smaller than ten seconds is too low"),
        ));
    }
    let polling_interval = emp.polling_interval.unwrap_or(1000);
    let polling_interval = Duration::from_millis(polling_interval);
    let pac2200 = Pac2200::new(
        &emp.meter_host,
        emp.meter_port.unwrap_or(502),
        polling_interval,
    )?;

    let bind_to = emp.bind_to.unwrap_or(String::from("localhost:1723"));
    let listener = std::net::TcpListener::bind(bind_to)?;
    listener.set_nonblocking(false)?;
    let (send_socket, recv_socket) = channel();
    let do_run = Arc::new(AtomicBool::new(true));
    let do_run_clone = do_run.clone();
    std::thread::spawn(move || {
        handle_requests(
            do_run_clone,
            pac2200,
            polling_interval,
            recv_socket,
            emp.log_to,
            emp.log_flush_interval
                .map(|i| Duration::from_secs(i))
                .unwrap_or(Duration::from_secs(3600)),
            emp.write_to_console.unwrap_or(false),
        )
    });
    for socket in listener.incoming() {
        if let Ok(socket) = socket {
            match socket.peer_addr() {
                Ok(peer_addr) => {
                    eprintln!("New connection from {:?}", peer_addr);
                    if let Err(e) = socket.set_nonblocking(true) {
                        eprintln!(
                            "Error: Cannot set socket into non-blocking mode ({:?}); ignoring socket!", e
                        );
                    } else {
                        send_socket.send((socket, peer_addr)).expect("Channel");
                    }
                }
                Err(e) => {
                    eprintln!("Error: Unable to read socket's peer address: {:?}", e);
                }
            }
        }
    }
    Ok(())
}
