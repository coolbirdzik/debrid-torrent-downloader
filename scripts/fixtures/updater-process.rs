#![windows_subsystem = "windows"]

// A real Windows process for testing executable locks, replacement and relaunch in an isolated folder.
fn main() {
    let exe = std::env::current_exe().unwrap();
    let folder = exe.parent().unwrap();
    let version = if cfg!(old_version) { "old" } else { "new" };
    std::fs::write(folder.join("started.log"), format!("{version}\n{}", std::process::id())).unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !folder.join(format!("exit-{version}")).exists() && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
