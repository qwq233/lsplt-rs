use log::{debug, info};
use lsplt_rs::MapInfo;

#[no_mangle]
extern "C" fn get_pid() -> i32 {
    2333
}

fn main() {
    init_logger();
    info!("Logger initialized");

    let map_info = MapInfo::scan("self");

    info!("Current PID: {}", unsafe { libc::getpid() });

    let libc = &map_info
        .iter()
        .find(|mi| {
            if let Some(path) = &mi.pathname {
                if mi.perms & (libc::PROT_EXEC as u8) == 0 && path.ends_with("libc.so")  {
                    return true;
                }
            }
            false
        })
        .expect("libc not found in memory maps");
    info!("libc info: {:?}", libc);

    let mut original_ptr: *mut std::ffi::c_void = std::ptr::null_mut();

    // Cast any function signature to a raw pointer for registration
    lsplt_rs::register_hook(
        libc.dev,
        libc.inode,
        "getpid",
        get_pid as *mut std::ffi::c_void,
        Some(&mut original_ptr),
    ).unwrap();

    lsplt_rs::commit_hook().unwrap();

    std::thread::sleep(std::time::Duration::from_secs(1));

    info!("Current PID: {}", unsafe { libc::getpid() });
    info!("Original PID: {}", unsafe {
        let original_fn: extern "C" fn() -> i32 = std::mem::transmute(original_ptr);
        original_fn()
    });
}

pub fn init_logger() {
    const PATTERN: &str = "{d(%Y-%m-%d %H:%M:%S %Z)(utc)} [{h({l})}] {M} - {m}{n}";
    let stdout = log4rs::append::console::ConsoleAppender::builder()
        .encoder(Box::new(log4rs::encode::pattern::PatternEncoder::new(
            PATTERN,
        )))
        .build();
    let root = log4rs::config::Root::builder()
        .appender("stdout")
        .build(log::LevelFilter::Debug);
    let config = log4rs::Config::builder()
        .appender(log4rs::config::Appender::builder().build("stdout", Box::new(stdout)))
        .build(root)
        .unwrap();
    log4rs::init_config(config).unwrap();
}
