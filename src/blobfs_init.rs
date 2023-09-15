//! The Thinnest ever rust wrapper around SPDK. This is used for automated
//! and manual testing if running mayastor with all its bells and whistles
//! is not possible or desirable and all what is needed is to run SPDK with
//! particular configuration file (i.e. nvmf target for testing).
extern crate libc;

use std::{
    env,
    ffi::{c_void, CStr, CString},
    io::{Error, ErrorKind},
    iter::Iterator,
    mem,
    os::raw::{c_char, c_int},
    ptr,
    ptr::null_mut,
    sync::{Arc, Mutex},
    vec::Vec,
};

use spdk_rs::libspdk::{
    spdk_app_fini, spdk_app_opts, spdk_app_opts_init, spdk_app_parse_args, spdk_app_start,
    spdk_app_stop, spdk_bdev, spdk_bdev_create_bs_dev_ext, spdk_bdev_event_type, spdk_bdev_module,
    spdk_blobfs_bdev_mount, spdk_blobfs_bdev_op_complete, spdk_blobfs_opts, spdk_bs_bdev_claim,
    spdk_bs_dev, spdk_filesystem, spdk_fs_alloc_thread_ctx, spdk_fs_create_file, spdk_fs_init,
    spdk_fs_load, spdk_fs_opts_init, spdk_fs_set_cache_size, spdk_fs_thread_ctx,
    spdk_json_decode_object, spdk_json_decode_string, spdk_json_object_decoder, spdk_json_val,
    spdk_jsonrpc_request, spdk_jsonrpc_send_bool_response, spdk_jsonrpc_send_error_response,
    spdk_parse_capacity, spdk_strerror, SPDK_APP_PARSE_ARGS_SUCCESS,
    SPDK_JSONRPC_ERROR_INTERNAL_ERROR, SPDK_JSONRPC_ERROR_INVALID_PARAMS, SPDK_JSON_VAL_STRING,
    spdk_event_call, spdk_event_allocate, fs_request_fn,spdk_fs_create_file_async,
};

// Define a custom struct to capture the callback context
struct BlobfsBdevCreateContext {
    bdev_name: CString,
    cb_fn: spdk_blobfs_bdev_op_complete,
    cb_arg: *mut c_void,
    bdev: *mut spdk_bs_dev,
}

extern "C" fn spdk_bdev_create_bs_dev_ext_complete(
    event_type: spdk_bdev_event_type,
    bdev: *mut spdk_bdev,
    ctx: *mut c_void,
) {
    println!("BlobFS Bdev event type: {:?}", event_type);
    return;
}

pub fn spdk_fs_alloc_thread_ctx_wrapper(fs: *mut spdk_filesystem) -> *mut spdk_fs_thread_ctx {
    unsafe { spdk_fs_alloc_thread_ctx(fs) }
}

pub fn spdk_fs_create_file_wrapper(
    fs: *mut spdk_filesystem,
    ctx: *mut spdk_fs_thread_ctx,
    name: *const c_char,
) -> c_int {
    unsafe { spdk_fs_create_file(fs, ctx, name) }
}



extern "C" fn spdk_fs_init_complete(ctx: *mut c_void, fs: *mut spdk_filesystem, fserrno: c_int) {
    if fserrno != 0 {
        // Handle error.
        println!("Init operation failed with error code: {}", fserrno);
    } else {
        // Operation succeeded.
        println!("Filesystem init operation completed. Context: {:?}", ctx);
        println!("Filesystem pointer: {:?}", fs);
        
        unsafe {
            spdk_fs_load(
                (*(ctx as *mut BlobfsBdevCreateContext)).bdev,
                Some(_send_request),
                Some(spdk_fs_load_complete),
                ctx,
            )
        }
        
    }
    return;
}

extern "C" fn spdk_fs_load_complete(ctx: *mut c_void, fs: *mut spdk_filesystem, fserrno: c_int) {
    if fserrno != 0 {
        // Handle error.
        println!("Load operation failed with error code: {}", fserrno);
    } else {
        // Operation succeeded.
        println!("Filesystem load operation completed. Context: {:?}", ctx);
        println!("Filesystem pointer: {:?}", fs);
        let file_name = "example";
        let file_name_cstr = std::ffi::CString::new(file_name).unwrap();
        print!("Creating file: {:?}\n", file_name);
        

        
       let name = "example_file.txt".as_ptr() as *const c_char;
       
       unsafe {spdk_fs_create_file_async(fs, name, Some(file_create_cb), std::ptr::null_mut());
       
        }
    }
    return;
}

unsafe extern "C" fn file_create_cb(ctx: *mut ::std::os::raw::c_void, fserrno: c_int) {
    if fserrno == 0 {
        println!("File creation successful!");
    } else {
        println!("File creation failed with error code: {}", fserrno);
    }
}

// Define the blobfs_bdev_create function in Rust
fn blobfs_bdev_create(
    bdev_name: &str,
    cluster_sz: u32,
    cb_fn: spdk_blobfs_bdev_op_complete,
    cb_arg: *mut c_void,
) {
    let mut bs_dev: *mut *mut spdk_bs_dev = &mut std::ptr::null_mut();
    let mut blobfs_opt: spdk_blobfs_opts = Default::default();
    // Create a callback context
    let context = Box::new(BlobfsBdevCreateContext {
        bdev_name: CString::new(bdev_name).expect("CString creation failed"),
        cb_fn,
        cb_arg,
        bdev: unsafe { *bs_dev },
    });

    let ctx_ptr = Box::into_raw(context);
    // Allocate memory for the SPDK context

    let rc = unsafe {
        // Create a blobstore block device from the bdev
        spdk_bdev_create_bs_dev_ext(
            (*ctx_ptr).bdev_name.as_ptr(),
            Some(spdk_bdev_create_bs_dev_ext_complete),
            ctx_ptr as *mut c_void,
            bs_dev,
        )
    };

    if rc != 0 {
        // Handle the error
        eprintln!(
            "Failed to create a blobstore block device from bdev ({})",
            bdev_name
        );
        // Call the callback function with an error code
        unsafe {
            // TODO: Fix this
            // cb_fn((*ctx_ptr).cb_arg, -libc::ENOMEM);
        }
        // Free the allocated memory
        unsafe { Box::from_raw(ctx_ptr) };
        return;
    }

    let blobfs_bdev_module = spdk_bdev_module {
        name: "blobfs\0".as_ptr() as *const i8, // Null-terminated C string
        // Fill in other fields as needed
        ..spdk_bdev_module::default()
    };
    let blobfs_bdev_module_ptr: *const spdk_bdev_module = &blobfs_bdev_module;

    let rc = unsafe {
        // Claim the blobfs base bdev
        spdk_bs_bdev_claim(*bs_dev, blobfs_bdev_module_ptr as *mut spdk_bdev_module)
    };

    if rc != 0 {
        // Handle the error
        eprintln!("Blobfs base bdev already claimed by another bdev");
        // Call the callback function with an error code
        unsafe {
            // TODO: Fix this
            // cb_fn((*ctx_ptr).cb_arg, -libc::EBUSY);
        }
        // Free the allocated memory
        unsafe { Box::from_raw(ctx_ptr) };
        return;
    }

    unsafe {
        // Initialize the blobfs options
        spdk_fs_opts_init(&mut blobfs_opt);
    }

    if cluster_sz != 0 {
        // Set the cluster size if provided
        blobfs_opt.cluster_sz = cluster_sz;
    }

    unsafe {
        spdk_fs_load
            ( *bs_dev,
            Some(_send_request),
            Some(spdk_fs_load_complete),
            ctx_ptr as *mut c_void,
        
        )
    }


    // Free the allocated memory
    unsafe { Box::from_raw(ctx_ptr) };
    return;
}

// Callback function to handle the completion of the operation.
extern "C" fn blobfs_bdev_create_complete(cb_arg: *mut libc::c_void, fserrno: libc::c_int) {
    // Handle the completion of the operation here.
    if fserrno != 0 {
        // Handle error.
        println!("Operation failed with error code: {}", fserrno);
    } else {
        // Operation succeeded.
        println!("Operation completed successfully.");
    }
    return;
}

// Creates a blobfs file system on top of the given bdev.
pub extern "C" fn blobfs_create(_arg: *mut c_void) {
    let bdev_name = "Malloc0";
    let cluster_sz = 0; // Set an appropriate value for cluster_sz.
    let cb_arg: *mut libc::c_void = std::ptr::null_mut(); // Set your callback argument if needed.

    // Call blobfs_bdev_create.
    unsafe {
        blobfs_bdev_create(
            bdev_name,
            cluster_sz,
            Some(blobfs_bdev_create_complete),
            cb_arg,
        );
    }
}


extern "C" fn call_fn(arg1: *mut libc::c_void, arg2: *mut libc::c_void) {
    unsafe{
        //let fn_ptr = arg1 as fs_request_fn;
        let fn_ptr = unsafe { std::mem::transmute::<*mut std::ffi::c_void, fs_request_fn>(arg1) };
     //   let fn_ptr = fn_ptr.unwrap();
       // let fn_ptr = unsafe { std::mem::transmute::<*mut std::ffi::c_void, fs_request_fn>(arg1) };
    if let Some(call_fn) = fn_ptr {
        call_fn(arg2);
    }
    }  // fn_ptr(arg2);
    }
    
    extern "C" fn _send_request(__fn: fs_request_fn, arg: *mut c_void)
    { unsafe{
        let event = spdk_event_allocate(0, Some(call_fn), std::mem::transmute(__fn), arg);
    spdk_event_call(event);
            }
    }
