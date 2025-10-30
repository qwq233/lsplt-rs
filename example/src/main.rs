use log::{debug, info};
use lsplt_rs::MapInfo;

#[no_mangle]
extern "C" fn get_pid() -> i32 {
    debug!("get_pid called");
    2333
}

fn main() {
    init_logger();
    info!("Logger initialized");

    let map_info = MapInfo::scan("self");
    let prog_name = std::env::current_exe()
        .unwrap()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    info!("Current PID: {}", unsafe { libc::getpid() });

    let self_info = &map_info
        .iter()
        .find(|mi| {
            if let Some(path) = &mi.pathname {
                if mi.perms & (libc::PROT_EXEC as u8) == 0 && path.ends_with(&prog_name) {
                    return true;
                }
            }
            false
        })
        .expect("libc not found in memory maps");
    info!("libc info: {:?}", self_info);

    let mut original_ptr: *mut std::ffi::c_void = std::ptr::null_mut();

    lsplt_rs::register_hook(
        self_info.dev,
        self_info.inode,
        "getpid",
        get_pid as *mut std::ffi::c_void,
        Some(&mut original_ptr),
    )
    .unwrap();

    debug!("commit hook");
    lsplt_rs::commit_hook().unwrap();
    debug!("hook committed");

    info!("Current PID: {}", unsafe { libc::getpid() });
    info!("Original PID: {}", unsafe {
        if original_ptr.is_null() {
            panic!("Original function pointer is null\nWhich means the hook registration failed.");
        } else {
            let original_fn: extern "C" fn() -> i32 = std::mem::transmute(original_ptr);
            original_fn()
        }
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
