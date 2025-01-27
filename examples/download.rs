use kyuri::Manager;
use rand::Rng;

fn main() {
    const TEMPLATE: &str =
        "{msg}\n[{elapsed}] {state_emoji} {bytes}/{total_bytes} ({bytes_per_sec}, {eta})";
    let manager = Manager::new(std::time::Duration::from_secs(1));

    std::thread::scope(|s| {
        let manager_0 = &manager;
        s.spawn(move || {
            let mut rng = rand::thread_rng();
            let mut cnt = 0;
            loop {
                let size = rng.gen::<u16>() as u64;
                let interval_micros = rng.gen_range(1..100);
                let bar_1 = manager_0.create_bar(
                    size,
                    &format!("Downloading {} (thread 1)", cnt),
                    TEMPLATE,
                    true,
                );
                for i in 0..=size {
                    bar_1.set_pos(i);
                    std::thread::sleep(std::time::Duration::from_micros(interval_micros));
                }
                cnt += 1;
            }
        });
        let manager_1 = &manager;
        s.spawn(move || {
            let mut rng = rand::thread_rng();
            let mut cnt = 0;
            loop {
                let size = rng.gen::<u16>() as u64;
                let interval_micros = rng.gen_range(1..100);
                let bar_2 = manager_1.create_bar(
                    size,
                    &format!("Downloading {} (thread 2)", cnt),
                    TEMPLATE,
                    true,
                );
                for i in 0..=size {
                    bar_2.set_pos(i);
                    if i == size {
                        bar_2.finish();
                    }
                    std::thread::sleep(std::time::Duration::from_micros(interval_micros));
                }
                cnt += 1;
            }
        });
    });
}
