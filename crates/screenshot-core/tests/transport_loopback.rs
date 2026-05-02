use interprocess::local_socket::traits::ListenerExt;
use interprocess::local_socket::{GenericFilePath, ListenerOptions, ToFsName};
use screenshot_core::harness::transport::Transport;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::Duration;

fn tmp_socket_path() -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "screenshot-harness-test-{}-{}.sock",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    p
}

#[test]
fn transport_connects_and_round_trips_one_line() {
    let path = tmp_socket_path();
    let _ = std::fs::remove_file(&path);

    let name = path.as_path().to_fs_name::<GenericFilePath>().unwrap();

    let listener = ListenerOptions::new().name(name).create_sync().unwrap();

    let path2 = path.clone();
    let join = std::thread::spawn(move || {
        let conn = listener.incoming().next().unwrap().unwrap();
        let mut reader = BufReader::new(conn);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let mut conn = reader.into_inner();
        conn.write_all(b"pong\n").unwrap();
        conn.flush().unwrap();
        let _ = std::fs::remove_file(&path2);
    });

    std::thread::sleep(Duration::from_millis(100));
    let mut t = Transport::connect(&path).unwrap();
    t.write_line("ping").unwrap();
    let resp = loop {
        if let Some(line) = t.try_read_line().unwrap() {
            break line;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(resp, "pong");
    join.join().unwrap();
}
