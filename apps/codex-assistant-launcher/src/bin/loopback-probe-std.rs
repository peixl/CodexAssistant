use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn main() {
    println!("[std-probe] start");
    let listener = match TcpListener::bind((Ipv4Addr::LOCALHOST, 0)) {
        Ok(l) => l,
        Err(e) => {
            println!("[std-probe] FAIL bind: {e}");
            return;
        }
    };
    let addr = listener.local_addr().expect("local_addr");
    println!("[std-probe] listening on {addr}");

    let stop = Arc::new(AtomicBool::new(false));
    let stop_srv = stop.clone();
    let server = std::thread::spawn(move || {
        listener.set_nonblocking(true).ok();
        let deadline = Instant::now() + Duration::from_millis(3500);
        loop {
            if stop_srv.load(Ordering::Relaxed) || Instant::now() >= deadline {
                return;
            }
            match listener.accept() {
                Ok((mut stream, peer)) => {
                    println!("[std-probe] accepted from {peer}");
                    let _ = stream.set_write_timeout(Some(Duration::from_millis(2000)));
                    let _ = stream.write_all(b"ok");
                    let _ = stream.flush();
                    return;
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => {
                    println!("[std-probe] accept error: {e}");
                    return;
                }
            }
        }
    });

    let target: SocketAddr = (Ipv4Addr::LOCALHOST, addr.port()).into();
    let started = Instant::now();
    println!("[std-probe] connecting to {target} (timeout 2500ms)");
    match TcpStream::connect_timeout(&target, Duration::from_millis(2500)) {
        Ok(mut stream) => {
            let elapsed = started.elapsed();
            println!("[std-probe] connected in {elapsed:?}");
            stream.set_read_timeout(Some(Duration::from_millis(2000))).ok();
            let mut buf = [0u8; 2];
            match stream.read_exact(&mut buf) {
                Ok(()) => println!(
                    "[std-probe] SUCCESS read={:?}",
                    std::str::from_utf8(&buf).unwrap_or("?")
                ),
                Err(e) => println!("[std-probe] FAIL read: {e}"),
            }
        }
        Err(e) => println!(
            "[std-probe] FAIL connect after {:?}: kind={:?} msg={e}",
            started.elapsed(),
            e.kind()
        ),
    }

    stop.store(true, Ordering::Relaxed);
    let _ = server.join();
    println!("[std-probe] exit");
}
