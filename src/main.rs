//! The Thinnest ever rust wrapper around SPDK. This is used for automated
//! and manual testing if running mayastor with all its bells and whistles
//! is not possible or desirable and all what is needed is to run SPDK with
//! particular configuration file (i.e. nvmf target for testing).
mod blobfs_init;

extern crate libc;
#[macro_use]
extern crate lazy_static;

use blobfs_init::blobfs_create;
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
    spdk_bs_dev, spdk_filesystem, spdk_fs_init, spdk_fs_load, spdk_fs_opts_init,
    spdk_fs_set_cache_size, spdk_json_decode_object, spdk_json_decode_string,
    spdk_json_object_decoder, spdk_json_val, spdk_jsonrpc_request, spdk_jsonrpc_send_bool_response,
    spdk_jsonrpc_send_error_response, spdk_parse_capacity, spdk_strerror,
    SPDK_APP_PARSE_ARGS_SUCCESS, SPDK_JSONRPC_ERROR_INTERNAL_ERROR,
    SPDK_JSONRPC_ERROR_INVALID_PARAMS, SPDK_JSON_VAL_STRING,
};

fn main() -> Result<(), std::io::Error> {
    let args = env::args()
        .map(|arg| CString::new(arg).unwrap())
        .collect::<Vec<CString>>();
    let mut c_args = args
        .iter()
        .map(|arg| arg.as_ptr())
        .collect::<Vec<*const c_char>>();
    c_args.push(std::ptr::null());

    let mut opts: spdk_app_opts = Default::default();

    unsafe {
        spdk_app_opts_init(
            &mut opts as *mut spdk_app_opts,
            std::mem::size_of::<spdk_app_opts>() as u64,
        );

        if spdk_app_parse_args(
            (c_args.len() as c_int) - 1,
            c_args.as_ptr() as *mut *mut c_char,
            &mut opts,
            null_mut(), // extra short options i.e. "f:S:"
            null_mut(), // extra long options
            None,       // extra options parse callback
            None,       // usage
        ) != SPDK_APP_PARSE_ARGS_SUCCESS
        {
            return Err(Error::new(ErrorKind::Other, "Parsing arguments failed"));
        }
    }

    opts.name = CString::new("spdk".to_owned()).unwrap().into_raw();
    opts.shutdown_cb = Some(spdk_shutdown_cb);

    let rc = unsafe {
        let rc = spdk_app_start(&mut opts, Some(blobfs_create), null_mut());
        // this will remove shm file in /dev/shm and do other cleanups
        spdk_app_fini();
        rc
    };
    if rc != 0 {
        Err(Error::new(
            ErrorKind::Other,
            format!("spdk failed with error {rc}"),
        ))
    } else {
        Ok(())
    }
}

extern "C" fn spdk_shutdown_cb() {
    unsafe { spdk_app_stop(0) };
}

// Callback function to handle the completion of the operation.
// extern "C" fn blobfs_mount_cb(cb_arg: *mut libc::c_void, fserrno: libc::c_int) {
//     // Handle the completion of the operation here.
//     if fserrno != 0 {
//         // Handle error.
//         println!("Operation failed with error code: {}", fserrno);
//     } else {
//         // Operation succeeded.
//         println!("Operation completed successfully.");
//     }
//     return;
// }

// fn blobfs_mount() {
//     let bdev_name = std::ffi::CString::new("Malloc0").expect("CString conversion failed");
//     let cluster_sz = 0; // Set an appropriate value for cluster_sz.
//     let cb_arg: *mut libc::c_void = std::ptr::null_mut(); // Set your callback argument if needed.
//     let mountpoint = "/mnt/test_mount";

//     // Call spdk_blobfs_bdev_mount.
//     unsafe {
//         spdk_blobfs_bdev_mount(
//             bdev_name.as_ptr(),
//             mountpoint.as_ptr() as *const i8,
//             Some(blobfs_mount_cb),
//             cb_arg,
//         );
//     }
// }

// Define the callback function that matches the signature of spdk_fs_op_with_handle_complete.
// extern "C" fn fs_op_complete_callback(
//     cb_arg: *mut ::std::os::raw::c_void,
//     fs: *mut spdk_fs,
//     fserrno: i32,
// ) {
//     // Handle the callback logic here, if needed.
//     println!("Filesystem loaded with errno: {}", fserrno);
// }

// Define the callback function that matches the signature of fs_send_request_fn.
// extern "C" fn send_request_callback(
//     cb_arg: *mut ::std::os::raw::c_void,
//     fs_req: *mut spdk_fs_request,
// ) {
//     // Handle the callback logic here, if needed.
//     println!("Send request callback called.");
// }
