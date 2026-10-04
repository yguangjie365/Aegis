//! 剪贴板 FFI 的最小分步探针（2026-10-04）。
//!
//! 起因：`clipboard.rs` 的往返测试触发 `STATUS_HEAP_CORRUPTION`（退出码 0xc0000374）。
//! 推理未能定位，故用**分步打印 + 单独进程**把它夹出来：
//! 哪一行之后没有输出，问题就在那一步。
//!
//! 跑法：cargo run -p infra --example clipboard_probe
//!
//! 🔴 这个例子**故意不放进 tests/**：崩溃会带走整个测试进程，
//! 而探针的价值恰恰是"崩在哪一步"——必须能单独观察。

#[cfg(windows)]
fn main() {
    use std::io::Write;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{GetLastError, GlobalFree, HANDLE};
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable,
        OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };

    const CF_UNICODETEXT: u32 = 13;

    // 立刻刷新：崩溃时未刷新的 stdout 会丢，那就白跑了
    fn step(label: &str, extra: String) {
        println!("[step] {label} {extra}");
        let _ = std::io::stdout().flush();
    }

    let text = "Aegis 探针 🚀";
    let wide: Vec<u16> = std::ffi::OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let bytes = wide.len() * std::mem::size_of::<u16>();
    step("准备", format!("宽字符 {} 个，{bytes} 字节", wide.len()));

    // SAFETY: 探针按 Win32 文档次序逐步调用，每步打印；出问题的那一步会在日志里现形。
    unsafe {
        step("OpenClipboard 前", String::new());
        let opened = OpenClipboard(std::ptr::null_mut());
        step(
            "OpenClipboard 后",
            format!("返回值={opened} 错误={}", GetLastError()),
        );
        if opened == 0 {
            return;
        }

        step("EmptyClipboard 前", String::new());
        let emptied = EmptyClipboard();
        step(
            "EmptyClipboard 后",
            format!("返回值={emptied} 错误={}", GetLastError()),
        );

        step(
            "GlobalAlloc 前",
            format!("bytes={bytes} GMEM_MOVEABLE={GMEM_MOVEABLE}"),
        );
        let handle: HANDLE = GlobalAlloc(GMEM_MOVEABLE, bytes);
        step(
            "GlobalAlloc 后",
            format!("句柄={:p} 错误={}", handle, GetLastError()),
        );

        if handle.is_null() {
            let _ = CloseClipboard();
            return;
        }

        step("GlobalLock 前", String::new());
        let target = GlobalLock(handle);
        step(
            "GlobalLock 后",
            format!("指针={:p} 错误={}", target, GetLastError()),
        );

        if !target.is_null() {
            step("memcpy 前", String::new());
            std::ptr::copy_nonoverlapping(wide.as_ptr().cast::<u8>(), target.cast::<u8>(), bytes);
            step("memcpy 后", String::new());

            step("GlobalUnlock 前", String::new());
            let unlocked = GlobalUnlock(handle);
            step(
                "GlobalUnlock 后",
                format!("返回值={unlocked} 错误={}", GetLastError()),
            );
        }

        step("SetClipboardData 前", String::new());
        let set = SetClipboardData(CF_UNICODETEXT, handle);
        step(
            "SetClipboardData 后",
            format!("返回值={:p} 错误={}", set, GetLastError()),
        );
        if set.is_null() {
            // 移交失败 ⇒ 所有权仍在我们手上
            step("SetClipboardData 失败，GlobalFree", String::new());
            let freed = GlobalFree(handle);
            step("GlobalFree 后", format!("返回值={freed:p}"));
        }
        // 成功时**不得** GlobalFree：所有权已移交系统

        step("CloseClipboard 前", String::new());
        let closed = CloseClipboard();
        step(
            "CloseClipboard 后",
            format!("返回值={closed} 错误={}", GetLastError()),
        );
    }

    // ── 第二阶段：读回 ──
    // SAFETY: 同写侧；`GetClipboardData` 返回的句柄归系统所有，我们只读、不释放。
    unsafe {
        step("读：OpenClipboard 前", String::new());
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            step("读：OpenClipboard 失败", String::new());
            return;
        }
        step(
            "读：IsClipboardFormatAvailable",
            format!("{}", IsClipboardFormatAvailable(CF_UNICODETEXT)),
        );
        let handle = GetClipboardData(CF_UNICODETEXT);
        step("读：GetClipboardData", format!("句柄={:p}", handle));
        if !handle.is_null() {
            let pointer = GlobalLock(handle);
            step("读：GlobalLock", format!("指针={:p}", pointer));
            if !pointer.is_null() {
                let mut length = 0usize;
                let units = pointer.cast::<u16>();
                while *units.add(length) != 0 && length < 1024 {
                    length += 1;
                }
                step("读：扫描结束", format!("长度={length}"));
                let slice = std::slice::from_raw_parts(units, length);
                let back = String::from_utf16_lossy(slice);
                step("读：转字符串", format!("{back}"));
                let unlocked = GlobalUnlock(handle);
                step("读：GlobalUnlock", format!("返回值={unlocked}"));
            }
        }
        let _ = CloseClipboard();
        step("读：CloseClipboard 完毕", String::new());
    }
    step("全部完成", String::new());
}

#[cfg(not(windows))]
fn main() {
    println!("本探针仅实现 Windows");
}
