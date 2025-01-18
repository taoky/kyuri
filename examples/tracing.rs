use std::sync::Mutex;

use tracing::warn;
use tracing_subscriber;

fn main() {
    let manager = kyuri::Manager::new(std::time::Duration::from_secs(1));
    let writer = manager.create_writer();
    // Well, here tracing_subscriber does not support to just give a writer, so we need to wrap it with Mutex...
    let subscriber = tracing_subscriber::fmt()
        .with_writer(Mutex::new(writer))
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    // Create 10 threads, each with a progress bar
    std::thread::scope(|s| {
        let _: Vec<_> = (0..10)
            .map(|i| {
                let manager = &manager;
                s.spawn(move || {
                    let bar =
                        manager.create_bar(100, &format!("Thread {}", i), "{msg}: {bar}", true);
                    loop {
                        for j in 0..100 {
                            bar.set_pos(j);
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                    }
                })
            })
            .collect();
        s.spawn(|| loop {
            warn!("Something happened!");
            std::thread::sleep(std::time::Duration::from_millis(1100));
        });
    });
}
