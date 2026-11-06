//! Several handles to one session log in one process: records never
//! interleave, and reopening never cuts a line another handle is writing.

mod support;

use support::{SESSION, new_session, temp_store};
use z_engine_protocol::SessionId;
use z_engine_store::{LogRecord, read_records};

#[test]
fn concurrent_handles_never_interleave_or_cut_records() {
    let (_dir, store) = temp_store();
    let id = SessionId::from(SESSION);
    let (log, _) = store.create(new_session(SESSION)).unwrap();
    let path = log.path().to_path_buf();
    drop(log);

    let writers: Vec<_> = (0..4)
        .map(|writer| {
            let (store, id) = (store.clone(), id.clone());
            std::thread::spawn(move || {
                for index in 0..25 {
                    let mut log = store.open_append(&id).unwrap();
                    let text = format!("{writer}:{index}:{}", "x".repeat(16 * 1024));
                    log.append(&LogRecord::Note { text }).unwrap();
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }

    let read = read_records(&path).unwrap();
    assert!(!read.torn_tail);
    assert_eq!(read.corrupt_lines, 0);
    assert_eq!(read.records.len(), 1 + 4 * 25);
}
