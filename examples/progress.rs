use kyuri::Manager;

fn main() {
    const TEMPLATE: &str = "{msg}: {pos}/{total}";
    let manager = Manager::new(std::time::Duration::from_secs(1));

    let bar = manager.create_bar(std::u64::MAX, "Processing something", TEMPLATE);
    for i in 0..=std::u64::MAX {
        bar.set_pos(i);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
