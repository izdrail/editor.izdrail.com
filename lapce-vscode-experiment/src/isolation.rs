//! Linux worker policy. Fails closed on unsupported systems.
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
pub fn restrict() -> Result<(), String> {
    Err("Sandbox experiment currently requires Linux x86_64 seccomp".into())
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub fn restrict() -> Result<(), String> {
    // Only memory management, pipe I/O, signals and process exit. No file opens,
    // sockets, exec, clone, ptrace, namespace operations or filesystem mutations.
    let allowed = [
        libc::SYS_read,
        libc::SYS_write,
        libc::SYS_close,
        libc::SYS_fstat,
        libc::SYS_lseek,
        libc::SYS_fcntl,
        libc::SYS_mmap,
        libc::SYS_munmap,
        libc::SYS_mremap,
        libc::SYS_mprotect,
        libc::SYS_brk,
        libc::SYS_madvise,
        libc::SYS_rt_sigaction,
        libc::SYS_rt_sigprocmask,
        libc::SYS_rt_sigreturn,
        libc::SYS_sigaltstack,
        libc::SYS_futex,
        libc::SYS_sched_yield,
        libc::SYS_clock_gettime,
        libc::SYS_getrandom,
        libc::SYS_getpid,
        libc::SYS_gettid,
        libc::SYS_exit,
        libc::SYS_exit_group,
    ];
    let instruction = |code, jt, jf, k| libc::sock_filter { code, jt, jf, k };
    let mut filter = vec![
        instruction(0x20, 0, 0, 4),          // LD arch
        instruction(0x15, 1, 0, 0xc000003e), // x86_64 only
        instruction(0x06, 0, 0, 0x80000000), // KILL_PROCESS
        instruction(0x20, 0, 0, 0),          // LD syscall number
    ];
    for syscall in allowed {
        filter.push(instruction(0x15, 0, 1, syscall as u32));
        filter.push(instruction(0x06, 0, 0, 0x7fff0000)); // ALLOW
    }
    filter.push(instruction(0x06, 0, 0, 0x00050000 | libc::EPERM as u32));
    let program = libc::sock_fprog {
        len: filter.len() as u16,
        filter: filter.as_mut_ptr(),
    };
    // Safety: valid local BPF instruction storage survives both prctl calls.
    unsafe {
        // Do not leave editor-owned descriptors available to a compromised engine.
        if libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, 0u32) != 0 {
            return Err("Worker descriptor isolation unavailable".into());
        }
        for (resource, limit) in [
            (libc::RLIMIT_AS, 256 * 1024 * 1024),
            (libc::RLIMIT_CPU, 2),
            (libc::RLIMIT_CORE, 0),
        ] {
            let limit = libc::rlimit {
                rlim_cur: limit,
                rlim_max: limit,
            };
            if libc::setrlimit(resource, &limit) != 0 {
                return Err("Worker resource restriction failed".into());
            }
        }
        if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
            || libc::prctl(libc::PR_SET_SECCOMP, 2, &program, 0, 0) != 0
        {
            return Err("Worker syscall sandbox unavailable".into());
        }
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests {
    #[test]
    fn denies_file_network_and_process_syscalls() {
        // The sandbox is irreversible, so test it in a disposable child.
        unsafe {
            let pid = libc::fork();
            assert!(pid >= 0);
            if pid == 0 {
                if super::restrict().is_err() {
                    libc::_exit(2);
                }
                let checks = [
                    libc::syscall(
                        libc::SYS_openat,
                        libc::AT_FDCWD,
                        c"/etc/passwd".as_ptr(),
                        0,
                    ),
                    libc::syscall(
                        libc::SYS_socket,
                        libc::AF_INET,
                        libc::SOCK_STREAM,
                        0,
                    ),
                    libc::syscall(
                        libc::SYS_execve,
                        c"/bin/true".as_ptr(),
                        std::ptr::null::<u8>(),
                        std::ptr::null::<u8>(),
                    ),
                    libc::syscall(libc::SYS_clone, 0, 0, 0, 0, 0),
                ];
                libc::_exit(if checks.iter().all(|r| *r == -1) {
                    0
                } else {
                    3
                });
            }
            let mut status = 0;
            assert_eq!(libc::waitpid(pid, &mut status, 0), pid);
            assert_eq!(status, 0);
        }
    }
}
