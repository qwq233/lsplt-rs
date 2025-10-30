//! LSPlt - PLT hooking library for Rust
//!
//! This module provides a safe Rust interface to the LSPlt hooking functionality,
//! allowing for function hooking in shared libraries.

#[derive(Debug, Clone)]
/// An entry that describes a line in /proc/self/maps. You can obtain a list of these entries
/// by calling [`scan()`](MapInfo::scan) or [`scan_self()`](MapInfo::scan_self).
pub struct MapInfo {
    /// The start address of the memory region.
    pub start: usize,
    /// The end address of the memory region.
    pub end: usize,
    /// The permissions of the memory region. This is a bit mask of the following values:
    /// - PROT_READ
    /// - PROT_WRITE  
    /// - PROT_EXEC
    pub perms: u8,
    /// Whether the memory region is private.
    pub is_private: bool,
    /// The offset of the memory region.
    pub offset: usize,
    /// The device number of the memory region.
    /// Major can be obtained by `major()`
    /// Minor can be obtained by `minor()`
    pub dev: u64,
    /// The inode number of the memory region.
    pub inode: u64,
    /// The path of the memory region.
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

    /// Scans /proc/self/maps and returns a list of [`MapInfo`] entries.
    /// This is useful to find out the inode of the library to hook.
    pub fn scan_self() -> Vec<MapInfo> {
        Self::scan("self")
    }

    /// Scans /proc/[pid]/maps and returns a list of [`MapInfo`] entries.
    /// This is useful to find out the inode of the library to hook.
    ///
    /// # Arguments
    /// * `pid` - The process id to scan. Use "self" for the current process.
    ///
    /// # Returns
    /// A vector of [`MapInfo`] entries describing the memory maps.
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

/// Register a hook to a function by inode.
///
/// For shared objects within an archive, you should use
/// [`register_hook_with_offset`] instead.
///
/// # Arguments
/// * `dev` - The device number of the memory region.
/// * `inode` - The inode of the library to hook. You can obtain the inode by `stat()` or by finding
///             the library in the list returned by [`MapInfo::scan`].
/// * `symbol` - The function symbol to hook.
/// * `callback` - The callback function pointer to call when the function is called.
/// * `backup` - Optional backup function pointer which can call the original function.
///
/// # Returns
/// `Ok(())` if the hook was successfully registered, or an `io::Error` on failure.
///
/// # Notes
/// - This function is thread-safe.
/// - The backup will not be available until [`commit_hook`] is called.
/// - The backup will be `None` if the hook fails.
/// - You can unhook the function by calling this function with `callback` set to the backup
///   from a previous call.
/// - LSPlt will backup the hook memory region and restore it when the hook is restored to its
///   original function pointer to avoid dirty pages. LSPlt will do hooks on a copied memory region
///   so that the original memory region will not be modified. You can invalidate this behavior
///   and hook the original memory region by calling [`invalidate_backup`].
///
/// # See Also
/// - [`commit_hook`]
/// - [`invalidate_backup`]
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

/// Register a hook to a function by inode with offset range.
///
/// This is useful when hooking a library that is directly loaded from an archive without extraction.
///
/// # Arguments
/// * `dev` - The device number of the memory region.
/// * `inode` - The inode of the library to hook. You can obtain the inode by `stat()` or by finding
///             the library in the list returned by [`MapInfo::scan`].
/// * `offset` - The offset to the library in the file.
/// * `size` - The upper bound size to the library in the file.
/// * `symbol` - The function symbol to hook.
/// * `callback` - The callback function to call when the function is called.
/// * `backup` - Optional backup function pointer which can call the original function.
///
/// # Returns
/// `Ok(())` if the hook was successfully registered, or an `io::Error` on failure.
///
/// # Notes
/// - This function is thread-safe.
/// - The backup will not be available until [`commit_hook`] is called.
/// - The backup will be `None` if the hook fails.
/// - You can unhook the function by calling this function with `callback` set to the backup
///   from a previous call.
/// - LSPlt will backup the hook memory region and restore it when the hook is restored to its
///   original function pointer to avoid dirty pages. LSPlt will do hooks on a copied memory region
///   so that the original memory region will not be modified. You can invalidate this behavior
///   and hook the original memory region by calling [`invalidate_backup`].
/// - You can get the offset range of the library by getting its entry offset and size in the
///   zip file.
/// - According to the Android linker specification, the `offset` must be page aligned.
/// - The `offset` must be accurate, otherwise the hook may fail because the ELF header
///   cannot be found.
/// - The `size` can be inaccurate but should be larger or equal to the library size,
///   otherwise the hook may fail when the hook pointer is beyond the range.
/// - The behavior of this function is undefined if `offset + size` is larger than the
///   maximum value of `usize`.
///
/// # See Also
/// - [`commit_hook`]
/// - [`invalidate_backup`]
pub fn register_hook_with_offset(
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

/// Commit all registered hooks.
///
/// # Returns
/// `Ok(())` if all hooks were successfully committed, or an `io::Error` if any hook failed.
///
/// # Notes
/// - This function is thread-safe.
/// - The return value indicates whether all hooks are successfully committed. You can
///   determine which hook fails by checking the backup function pointer of [`register_hook`].
///
/// # See Also
/// - [`register_hook`]
/// - [`register_hook_with_offset`]
pub fn commit_hook() -> std::io::Result<()> {
    if unsafe {
        lsplt_sys::lsplt_commit_hook()
    } {
        Ok(())
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to commit hook"))
    }
}

/// Invalidate backup memory regions.
///
/// Normally LSPlt will backup the hooked memory region and do hook on a copied anonymous memory
/// region, and restore the original memory region when the hook is unregistered
/// (when the callback of [`register_hook`] is the original function). This function will restore
/// the backup memory region and do all existing hooks on the original memory region.
///
/// # Returns
/// `Ok(())` if all hooks were successfully invalidated, or an `io::Error` if any hook failed.
///
/// # Notes
/// - This function is thread-safe.
/// - This will be automatically called when the library is unloaded.
///
/// # See Also
/// - [`register_hook`]
pub fn invalidate_backup() -> std::io::Result<()> {
    if unsafe {
        lsplt_sys::lsplt_invalidate_backup()
    } {
        Ok(())
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to invalidate backup"))
    }
}