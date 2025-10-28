
#[derive(Debug, Clone)]
pub struct MapInfo {
    pub start: usize,
    pub end: usize,
    pub perms: u8,
    pub is_private: bool,
    pub offset: usize,
    pub dev: u64,
    pub inode: u64,
    pub pathname: Option<String>,
}

impl MapInfo {
    fn new(
        start: usize,
        end: usize,
        perms: u8,
        is_private: bool,
        offset: usize,
        dev: u64,
        inode: u64,
        pathname: Option<String>,
    ) -> Self {
        MapInfo {
            start,
            end,
            perms,
            is_private,
            offset,
            dev,
            inode,
            pathname,
        }
    }

    fn from_map_info(mi: &lsplt_sys::lsplt_map_info_t) -> Self {
        let pathname = if mi.path.is_null() {
            None
        } else {
            unsafe {
                Some(std::ffi::CStr::from_ptr(mi.path).to_string_lossy().into_owned())
            }
        };
        MapInfo::new(
            mi.start,
            mi.end,
            mi.perms,
            mi.is_private,
            mi.offset,
            mi.dev,
            mi.inode,
            pathname,
        )
    }

    // Preserve this function for potential future use
    #[allow(dead_code)]
    fn to_map_info(&self) -> lsplt_sys::lsplt_map_info_t {
        lsplt_sys::lsplt_map_info_t {
            start: self.start,
            end: self.end,
            perms: self.perms,
            is_private: self.is_private,
            offset: self.offset,
            dev: self.dev,
            inode: self.inode,
            path: match &self.pathname {
                Some(s) => std::ffi::CString::new(s.as_str()).unwrap().into_raw(),
                None => std::ptr::null_mut(),
            },
        }
    }

    pub fn scan_self() -> Vec<MapInfo> {
        Self::scan("self")
    }

    pub fn scan(pid: &str) -> Vec<MapInfo> {
        let c_pid = std::ffi::CString::new(pid).unwrap();
        unsafe {
            let array = lsplt_sys::lsplt_scan(c_pid.as_ptr());
            let slice = std::slice::from_raw_parts(array.data, array.size);
            let mut result = Vec::with_capacity(array.size);
            for mi in slice {
                result.push(MapInfo::from_map_info(mi));
            }
            lsplt_sys::lsplt_free_map_info_array(&array as *const _ as *mut _);
            result
        }
    }
}

pub fn register_hook(
    dev: u64,
    inode: u64,
    symbol: &str,
    callback: *mut std::ffi::c_void,
    backup: Option<&mut *mut std::ffi::c_void>,
) -> std::io::Result<()> {
    let c_symbol = std::ffi::CString::new(symbol).unwrap();
    let result = unsafe {
        lsplt_sys::lsplt_register_hook(
            dev,
            inode,
            c_symbol.as_ptr(),
            callback,
            match backup {
                Some(b) => b as *mut *mut std::ffi::c_void,
                None => std::ptr::null_mut(),
            },
        )
    };

    if result {
        Ok(())
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to register hook"))
    }
}

pub fn register_hook_with_offest(
    dev: u64,
    inode: u64,
    offset: usize,
    size: usize,
    symbol: &str,
    callback: extern "C" fn(),
    backup: Option<&mut *mut std::ffi::c_void>,
) -> std::io::Result<()> {
    let c_symbol = std::ffi::CString::new(symbol).unwrap();
    let result = unsafe {
        lsplt_sys::lsplt_register_hook_with_offset(
            dev,
            inode,
            offset,
            size,
            c_symbol.as_ptr(),
            callback as usize as *mut std::ffi::c_void,
            match backup {
                Some(b) => b as *mut *mut std::ffi::c_void,
                None => std::ptr::null_mut(),
            },
        )
    };

    if result {
        Ok(())
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to register hook with offset"))
    }
}

pub fn commit_hook() -> std::io::Result<()> {
    if unsafe {
        lsplt_sys::lsplt_commit_hook()
    } {
        Ok(())
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to commit hook"))
    }
}

pub fn invalidate_backup() -> std::io::Result<()> {
    if unsafe {
        lsplt_sys::lsplt_invalidate_backup()
    } {
        Ok(())
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to invalidate backup"))
    }
}