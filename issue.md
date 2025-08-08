在编译时遇到这样一个问题：

cargo build --release --bin main 

...

error[E0425]: cannot find function `__basepri_r` in module `crate::asm::inline`                                                                                    
  --> C:\Users\Administrator\.cargo\registry\src\mirrors.tuna.tsinghua.edu.cn-e791a3f93f26854f\cortex-m-0.7.7\src\register\basepri.rs:6:15
   |
6  |     call_asm!(__basepri_r() -> u8)
   |               ^^^^^^^^^^^ not found in `crate::asm::inline`
   |
  ::: C:\Users\Administrator\.cargo\registry\src\mirrors.tuna.tsinghua.edu.cn-e791a3f93f26854f\cortex-m-0.7.7\src\call_asm.rs:11:43
   |
11 |                 () => crate::asm::inline::$func($($args),*),
   |                                           ----- due to this macro variable

error[E0425]: cannot find function `__basepri_w` in module `crate::asm::inline`
  --> C:\Users\Administrator\.cargo\registry\src\mirrors.tuna.tsinghua.edu.cn-e791a3f93f26854f\cortex-m-0.7.7\src\register\basepri.rs:22:19
   |
22 |         call_asm!(__basepri_w(basepri: u8));
   |                   ^^^^^^^^^^^ not found in `crate::asm::inline`
   |
  ::: C:\Users\Administrator\.cargo\registry\src\mirrors.tuna.tsinghua.edu.cn-e791a3f93f26854f\cortex-m-0.7.7\src\call_asm.rs:11:43
   |
11 |                 () => crate::asm::inline::$func($($args),*),
   |                                           ----- due to this macro variable

error[E0425]: cannot find function `__basepri_max` in module `crate::asm::inline`
  --> C:\Users\Administrator\.cargo\registry\src\mirrors.tuna.tsinghua.edu.cn-e791a3f93f26854f\cortex-m-0.7.7\src\register\basepri_max.rs:19:19
   |
19 |         call_asm!(__basepri_max(basepri: u8));
   |                   ^^^^^^^^^^^^^ not found in `crate::asm::inline`
   |
  ::: C:\Users\Administrator\.cargo\registry\src\mirrors.tuna.tsinghua.edu.cn-e791a3f93f26854f\cortex-m-0.7.7\src\call_asm.rs:11:43
   |
11 |                 () => crate::asm::inline::$func($($args),*),
   |                                           ----- due to this macro variable

error[E0425]: cannot find function `__faultmask_r` in module `crate::asm::inline`
  --> C:\Users\Administrator\.cargo\registry\src\mirrors.tuna.tsinghua.edu.cn-e791a3f93f26854f\cortex-m-0.7.7\src\register\faultmask.rs:29:28
   |
29 |     let r: u32 = call_asm!(__faultmask_r() -> u32);
   |                            ^^^^^^^^^^^^^ not found in `crate::asm::inline`
   |
  ::: C:\Users\Administrator\.cargo\registry\src\mirrors.tuna.tsinghua.edu.cn-e791a3f93f26854f\cortex-m-0.7.7\src\call_asm.rs:11:43
   |
11 |                 () => crate::asm::inline::$func($($args),*),
   |                                           ----- due to this macro variable

error: invalid register `r0`: unknown register
   --> C:\Users\Administrator\.cargo\registry\src\mirrors.tuna.tsinghua.edu.cn-e791a3f93f26854f\cortex-m-0.7.7\src\..\asm\inline.rs:197:24
    |
197 |     asm!("bkpt #0xab", inout("r0") nr, in("r1") arg, options(nomem, nostack, preserves_flags));
    |                        ^^^^^^^^^^^^^^

error: invalid register `r1`: unknown register
   --> C:\Users\Administrator\.cargo\registry\src\mirrors.tuna.tsinghua.edu.cn-e791a3f93f26854f\cortex-m-0.7.7\src\..\asm\inline.rs:197:40
    |
197 |     asm!("bkpt #0xab", inout("r0") nr, in("r1") arg, options(nomem, nostack, preserves_flags));
    |                                        ^^^^^^^^^^^^

For more information about this error, try `rustc --explain E0425`.                                                                                                
error: could not compile `cortex-m` (lib) due to 6 previous errors
warning: build failed, waiting for other jobs to finish...


仔细检查之后发现，是忘记复制.cargo文件夹了。复制其在本目录之后，编译通过。

在编译ahrs时不通过，原因是默认使用std环境，toml 文件中加上 default-features = false 就好了

一切代码写完之后编译不通过，经检查是 mpu6050 和 ahrs 所依赖的 nalgebra 版本不一致，手动克隆它们的源码，然后强制修改依赖版本为最新版本，toml 文件中使用相对路径引入，重新编译通过

