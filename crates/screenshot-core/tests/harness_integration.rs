use harness_protocol::{Command, Tier};
use interprocess::local_socket::{
    traits::{ListenerExt, Stream},
    GenericFilePath, ListenerOptions, ToFsName,
};
use interprocess::TryClone;
use screenshot_core::core::capture::MockCapture;
use screenshot_core::core::engine::Engine;
use screenshot_core::core::types::{Color, Rect};
use screenshot_core::harness::dispatch::{tick, DispatchState};
use screenshot_core::harness::server::HarnessServer;
use screenshot_core::harness::transport::Transport;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::{Duration, Instant};

fn temp_socket_path() -> String {
    let dir = std::env::temp_dir();
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    dir.join(format!("scrn-harness-{pid}-{nanos}.sock"))
        .to_string_lossy()
        .into_owned()
}

fn run_engine_side(socket_path: String, ready: Sender<()>) {
    let name = socket_path.clone().to_fs_name::<GenericFilePath>().unwrap();
    let listener = ListenerOptions::new().name(name).create_sync().unwrap();
    ready.send(()).unwrap();
    let stream = listener.incoming().next().unwrap().unwrap();
    let transport = Transport::from_stream(stream).unwrap();
    let mut server = HarnessServer::new(transport);
    server
        .send_hello(1, format!("pid={}", std::process::id()))
        .unwrap();

    let mut engine = Engine::new(
        "/tmp/x.png".into(),
        "png".into(),
        90,
        Color::new(255, 0, 0, 255),
        3.0,
        8.0,
    );
    let cap = MockCapture::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
    engine.start(&cap);
    let mut state = DispatchState::default();

    let start = Instant::now();
    let deadline = start + Duration::from_secs(10);
    while Instant::now() < deadline {
        tick(&mut engine, &mut server, &mut state);
        if engine.should_close {
            break;
        }
        if engine.state_label() == "Idle" && start.elapsed().as_millis() > 100 {
            break;
        }
        thread::sleep(Duration::from_millis(8));
    }
}

#[test]
fn end_to_end_drag_and_save() {
    let socket = temp_socket_path();
    let socket_for_thread = socket.clone();
    let (tx, rx) = channel();
    let handle = thread::spawn(move || run_engine_side(socket_for_thread, tx));
    rx.recv_timeout(Duration::from_secs(2)).unwrap();

    let stream = interprocess::local_socket::Stream::connect(
        socket.clone().to_fs_name::<GenericFilePath>().unwrap(),
    )
    .unwrap();
    let (read_half, mut write_half) = (BufReader::new(stream.try_clone().unwrap()), stream);
    let mut lines = read_half.lines();

    let hello = lines.next().unwrap().unwrap();
    assert!(hello.contains("\"type\":\"hello\""), "got {hello}");

    let cmds = [
        Command::MouseDown {
            seq: 1,
            x: 50.0,
            y: 60.0,
            button: harness_protocol::MouseButton::Left,
            modifiers: vec![],
            mode: Tier::Scripted,
        },
        Command::MouseUp {
            seq: 2,
            x: 150.0,
            y: 160.0,
            button: harness_protocol::MouseButton::Left,
            modifiers: vec![],
            mode: Tier::Scripted,
        },
    ];
    for c in &cmds {
        let line = serde_json::to_string(c).unwrap();
        writeln!(write_half, "{line}").unwrap();
        write_half.flush().unwrap();
    }

    let mut got_region_selected = false;
    let region_deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < region_deadline && !got_region_selected {
        if let Some(Ok(line)) = lines.next() {
            if line.contains("\"type\":\"engine_event\"")
                && line.contains("\"type\":\"regionSelected\"")
            {
                got_region_selected = true;
            }
        }
    }
    assert!(got_region_selected, "no regionSelected within 3s");

    let out_path = PathBuf::from(format!("/tmp/m1-it-{}.png", std::process::id()));
    let _ = std::fs::remove_file(&out_path);
    let composite = Command::CompositeRequest {
        seq: 3,
        save_path: out_path.to_string_lossy().into_owned(),
        format: "png".into(),
    };
    writeln!(write_half, "{}", serde_json::to_string(&composite).unwrap()).unwrap();
    write_half.flush().unwrap();

    let composite_deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < composite_deadline && !out_path.exists() {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(out_path.exists(), "composite png never written");
    let img = image::open(&out_path).unwrap();
    assert_eq!(img.width(), 100);
    assert_eq!(img.height(), 100);

    let cancel = Command::Cancel {
        seq: 4,
        mode: Tier::Scripted,
    };
    writeln!(write_half, "{}", serde_json::to_string(&cancel).unwrap()).unwrap();
    write_half.flush().unwrap();

    thread::sleep(Duration::from_millis(100));

    let _ = std::fs::remove_file(&out_path);
    let _ = std::fs::remove_file(&socket);
    handle.join().unwrap();
}
