#include <stdint.h>
#include <stddef.h>
#include <sys/types.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// C wrapper for lsplt::v2::MapInfo
typedef struct {
    uintptr_t start;
    uintptr_t end;
    uint8_t perms;
    bool is_private;
    uintptr_t offset;
    dev_t dev;
    ino_t inode;
    char* path;  // C string instead of std::string
} lsplt_map_info_t;

// C wrapper for std::vector<MapInfo>
typedef struct {
    lsplt_map_info_t* data;
    size_t size;
} lsplt_map_info_array_t;

/**
 * @brief Scans /proc/pid/maps and returns memory mapping information
 * 
 * @param pid Process ID to scan, use "self" for current process
 * @return Array of map info structures. Caller must free with lsplt_free_map_info_array()
 */
lsplt_map_info_array_t lsplt_scan(const char* pid);

/**
 * @brief Free memory allocated by lsplt_scan()
 * 
 * @param array Array to free
 */
void lsplt_free_map_info_array(lsplt_map_info_array_t* array);

/**
 * @brief Register a hook by inode and device
 * 
 * @param dev Device number
 * @param inode Inode number
 * @param symbol Function symbol to hook
 * @param callback Callback function pointer
 * @param backup Optional pointer to store original function pointer
 * @return true if hook registration successful
 */
bool lsplt_register_hook(dev_t dev, ino_t inode, const char* symbol, 
                         void* callback, void** backup);

/**
 * @brief Register a hook by inode with offset range (for libraries in archives)
 * 
 * @param dev Device number
 * @param inode Inode number
 * @param offset File offset to library
 * @param size Upper bound size of library
 * @param symbol Function symbol to hook
 * @param callback Callback function pointer
 * @param backup Optional pointer to store original function pointer
 * @return true if hook registration successful
 */
bool lsplt_register_hook_with_offset(dev_t dev, ino_t inode, uintptr_t offset,
                                    size_t size, const char* symbol,
                                    void* callback, void** backup);

/**
 * @brief Commit all registered hooks
 * 
 * @return true if all hooks successfully committed
 */
bool lsplt_commit_hook(void);

/**
 * @brief Invalidate backup memory regions and apply hooks to original memory
 * 
 * @return true if all hooks successfully invalidated
 */
bool lsplt_invalidate_backup(void);

#ifdef __cplusplus
}
#endif