use kyuri::{Manager};

fn main() {
    let manager = Manager::new(std::time::Duration::from_secs(1));

    let manager_0 = manager.clone();
    let t1 = std::thread::spawn(move || {
        let bar_1 = manager_0.create_bar(100, "Downloading", "template");
        for i in 0..100 {
            bar_1.set_pos(i);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });
    let manager_1 = manager.clone();
    let t2 = std::thread::spawn(move || {
        let bar_2 = manager_1.create_bar(200, "Uploading", "template");
        for i in 0..200 {
            bar_2.set_pos(i);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });

    t1.join().unwrap();
    t2.join().unwrap();
}
