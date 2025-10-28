#include "wrapper.hpp"

#include "include/lsplt.hpp"

#include <vector>
#include <string>
#include <cstring>

extern "C" {

lsplt_map_info_array_t lsplt_scan(const char* pid) {
    lsplt_map_info_array_t result = {nullptr, 0};
    
    try {
        std::string pid_str = pid ? pid : "self";
        auto cpp_result = lsplt::v2::MapInfo::Scan(pid_str);
        
        result.size = cpp_result.size();
        result.data = new lsplt_map_info_t[result.size];
        
        for (size_t i = 0; i < result.size; ++i) {
            const auto& src = cpp_result[i];
            auto& dest = result.data[i];
            
            dest.start = src.start;
            dest.end = src.end;
            dest.perms = src.perms;
            dest.is_private = src.is_private;
            dest.offset = src.offset;
            dest.dev = src.dev;
            dest.inode = src.inode;
            
            // Copy string
            if (!src.path.empty()) {
                dest.path = new char[src.path.size() + 1];
                std::strcpy(dest.path, src.path.c_str());
            } else {
                dest.path = nullptr;
            }
        }
    } catch (...) {
        // Handle exceptions
        result.data = nullptr;
        result.size = 0;
    }
    
    return result;
}

void lsplt_free_map_info_array(lsplt_map_info_array_t* array) {
    if (array && array->data) {
        for (size_t i = 0; i < array->size; ++i) {
            if (array->data[i].path) {
                delete[] array->data[i].path;
            }
        }
        delete[] array->data;
        array->data = nullptr;
        array->size = 0;
    }
}

bool lsplt_register_hook(dev_t dev, ino_t inode, const char* symbol, 
                        void* callback, void** backup) {
    if (!symbol) return false;
    
    try {
        return lsplt::v2::RegisterHook(dev, inode, std::string_view(symbol), 
                                      callback, backup);
    } catch (...) {
        return false;
    }
}

bool lsplt_register_hook_with_offset(dev_t dev, ino_t inode, uintptr_t offset,
                                   size_t size, const char* symbol,
                                   void* callback, void** backup) {
    if (!symbol) return false;
    
    try {
        return lsplt::v2::RegisterHook(dev, inode, offset, size, 
                                      std::string_view(symbol), callback, backup);
    } catch (...) {
        return false;
    }
}

bool lsplt_commit_hook(void) {
    try {
        return lsplt::v2::CommitHook();
    } catch (...) {
        return false;
    }
}

bool lsplt_invalidate_backup(void) {
    try {
        return lsplt::v2::InvalidateBackup();
    } catch (...) {
        return false;
    }
}

} // extern "C"