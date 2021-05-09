// Copyright 2016-2020 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
#[error("Error calling {method}: {source}")]
pub struct RaiseLimitError {
    method: &'static str,
    source: io::Error,
}

/// Raise the soft open file descriptor resource limit to the smaller of the
/// kernel limit and the hard resource limit.
///
/// Returns [`Ok(Some(u64))`] with the new limit.
///
///
/// darwin_fd_limit exists to work around an issue where launchctl on Mac OS X
/// defaults the rlimit maxfiles to 256/unlimited. The default soft limit of 256
/// ends up being far too low for our multithreaded scheduler testing, depending
/// on the number of cores available.
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[allow(non_camel_case_types)]
pub fn raise_fd_limit() -> Result<Option<u64>, RaiseLimitError> {
    use std::cmp;
    use std::mem::size_of_val;
    use std::ptr::null_mut;

    unsafe {
        static CTL_KERN: libc::c_int = 1;
        static KERN_MAXFILESPERPROC: libc::c_int = 29;

        // The strategy here is to fetch the current resource limits, read the
        // kern.maxfilesperproc sysctl value, and bump the soft resource limit for
        // maxfiles up to the sysctl value.

        // Fetch the kern.maxfilesperproc value
        let mut mib: [libc::c_int; 2] = [CTL_KERN, KERN_MAXFILESPERPROC];
        let mut maxfiles: libc::c_int = 0;
        let mut size: libc::size_t = size_of_val(&maxfiles) as libc::size_t;
        if libc::sysctl(
            &mut mib[0],
            2,
            &mut maxfiles as *mut _ as *mut _,
            &mut size,
            null_mut(),
            0,
        ) != 0
        {
            let err = io::Error::last_os_error();
            return Err(RaiseLimitError {
                method: "libc",
                source: err,
            });
        }

        // Fetch the current resource limits
        let mut rlim = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut rlim) != 0 {
            let err = io::Error::last_os_error();
            return Err(RaiseLimitError {
                method: "getrlimit",
                source: err,
            });
        }

        // Bump the soft limit to the smaller of kern.maxfilesperproc and the hard
        // limit
        rlim.rlim_cur = cmp::min(maxfiles as libc::rlim_t, rlim.rlim_max);

        // Set our newly-increased resource limit
        if libc::setrlimit(libc::RLIMIT_NOFILE, &rlim) != 0 {
            let err = io::Error::last_os_error();
            return Err(RaiseLimitError {
                method: "setrlimit",
                source: err,
            });
        }

        Ok(Some(rlim.rlim_cur))
    }
}

/// Raise the soft open file descriptor resource limit to the hard resource
/// limit.
///
/// Returns [`Ok(Some(u64))`] with the new limit.
///
#[cfg(any(target_os = "linux"))]
#[allow(non_camel_case_types)]
pub fn raise_fd_limit() -> Result<Option<u64>, RaiseLimitError> {
    unsafe {
        // Fetch the current resource limits
        let mut rlim = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut rlim) != 0 {
            let err = io::Error::last_os_error();
            return Err(RaiseLimitError {
                method: "getrlimit",
                source: err,
            });
        }

        // Set soft limit to hard imit
        rlim.rlim_cur = rlim.rlim_max;

        // Set our newly-increased resource limit
        if libc::setrlimit(libc::RLIMIT_NOFILE, &rlim) != 0 {
            let err = io::Error::last_os_error();
            return Err(RaiseLimitError {
                method: "setrlimit",
                source: err,
            });
        }

        Ok(Some(rlim.rlim_cur.into()))
    }
}

/// Returns [`Ok(None)`].
#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "linux")))]
pub fn raise_fd_limit() -> Result<u64, RaiseLimitError> {
    Ok(None)
}

#[cfg(test)]
pub mod test {
    use crate::raise_fd_limit;

    #[test]
    fn test_raise_limit() {
        matches::assert_matches!(raise_fd_limit(), Ok(Some(_)))
    }
}
