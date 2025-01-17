use kyuri::Manager;

fn main() {
    const TEMPLATE: &str = "{msg}\n[{elapsed}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})";
    let manager = Manager::new(std::time::Duration::from_secs(1));
    let bar_1 = manager.create_bar(100, "Downloading at thread 1", TEMPLATE, false);
    let bar_2 = manager.create_bar(200, "Uploading at thread 2", TEMPLATE, false);
    let t1 = std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        bar_1.set_visible(true);
        for i in 0..100 {
            bar_1.set_pos(i);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        bar_1.set_visible(false);
    });
    let t2 = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(1));
        bar_2.set_visible(true);
        for i in 0..200 {
            bar_2.set_pos(i);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        bar_2.set_visible(false);
    });

    t1.join().unwrap();
    t2.join().unwrap();
}
