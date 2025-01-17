use kyuri::Manager;

fn main() {
    const TEMPLATE: &str = "{msg}: {bar} ({pos}/{len})";
    let manager = Manager::new(std::time::Duration::from_secs(1));

    let bar = manager.create_bar(10000, "Processing something but in a bar", TEMPLATE);
    for i in 0..=10000 {
        bar.set_pos(i);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    bar.finish();
}
