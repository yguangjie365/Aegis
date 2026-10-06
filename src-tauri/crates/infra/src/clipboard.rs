//! 系统剪贴板的文本读写。
//!
//! ## 规格依据
//!
//! * **F-FT-33**：文本卡片"从剪贴板读入"；剪贴板**被占用或为空**时要如实提示
//!   （不是笼统的"读取失败"）。
//! * **F-FT-34**：电脑侧接收文本的**成功定义就是剪贴板写入成功**——
//!   写不进去必须回 `CLIPBOARD_WRITE_FAILED`，**而不是回"已送达"**。
//!   这条把"剪贴板能不能写"从一个实现细节提升成了**交付语义本身**。
//!
//! ## 复用结论（DP-1）
//!
//! ③思路复用 ← `Source/SystemToolkit/.../FileTransfer/` 的剪贴板读写
//! （GPL-3.0 同源，ADR-003 五步登记见 `Aegis/DEPENDENCIES.md §三`）。
//! 零 XAML/C# 代码复制，只借"先 Open 再 Empty 再 Set 最后 Close"的调用次序。
//!
//! 🔴 FFI 归 `infra`（02 册 KN-04）：故这是本工程**唯一**碰剪贴板的地方。
//!
//! ## 🔴 为什么要一把进程级互斥锁（这不是防御性编程，是必需）
//!
//! Windows 剪贴板是**操作系统里唯一一份**的全局资源。两个调用交错时，
//! 一方的 `EmptyClipboard` 会释放另一方刚交给系统的内存块。
//!
//! 这是**实测结论**，不是推测：最初版本的往返测试**并行**跑两个用例，
//! 直接以 `STATUS_HEAP_CORRUPTION`（0xc0000374）崩掉整个测试进程；
//! 而**同一套调用序列**在单进程逐步探针（`examples/clipboard_probe.rs`）里完全正常。
//! 故本模块的每一次剪贴板操作都在同一把锁内完成，**串行化**。

use std::fmt;
use std::sync::{Mutex, MutexGuard, OnceLock};
// 🔴 只被 `tests` 模块的重试用到（2026-10-07）：生产代码路径不引用它，
//   放在顶层会让**非测试构建**报 unused import（`cargo check` 的唯一 warning）。
#[cfg(test)]
use std::time::Duration;

/// 剪贴板错误。
///
/// 🔴 刻意把"**被别的进程占用**"单独成一档：F-FT-33 要求这种情形有专门话术，
/// 而它与"没有文本""系统调用失败"对用户是**三件不同的事**。
#[derive(Debug)]
pub enum ClipboardError {
    /// 打不开剪贴板：通常是被别的进程占用（重试即可）
    Busy,
    /// 剪贴板里没有文本（或不是文本格式）
    Empty,
    /// 其它系统调用失败（含内存分配失败）
    Failed(String),
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => write!(f, "剪贴板被其它程序占用，请稍后再试。"),
            Self::Empty => write!(f, "剪贴板里没有文本内容。"),
            Self::Failed(detail) => write!(f, "剪贴板操作失败：{detail}"),
        }
    }
}

impl std::error::Error for ClipboardError {}

/// 串行化所有剪贴板访问（见模块头"为什么要一把进程级互斥锁"）。
fn lock() -> MutexGuard<'static, ()> {
    static GATE: OnceLock<Mutex<()>> = OnceLock::new();
    // 毒锁恢复：一次 panic 不该让剪贴板功能永久失效
    GATE.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 把 `text` 写入系统剪贴板（替换既有内容）。
///
/// # Errors
/// 剪贴板被占用 → [`ClipboardError::Busy`]；其它失败 → [`ClipboardError::Failed`]。
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn set_text(text: &str) -> Result<(), ClipboardError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };

    /// `CF_UNICODETEXT`（Win32 常量；不在 windows-sys 的导出里，故就地取字面量）
    const CF_UNICODETEXT: u32 = 13;

    let _gate = lock();

    // UTF-16LE + NUL 结尾（Win32 剪贴板文本的约定）
    let wide: Vec<u16> = std::ffi::OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let bytes = wide.len() * std::mem::size_of::<u16>();

    // SAFETY: 以下调用序列是 Win32 剪贴板的既定用法；每次 Open 都有配对的 Close，
    // 每个 GlobalAlloc 的所有权在 SetClipboardData 成功后移交给系统（故不再释放）。
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            // 🔴 打不开多半是被占用——单独成一档，不混进"操作失败"
            return Err(ClipboardError::Busy);
        }
        let result = (|| -> Result<(), ClipboardError> {
            if EmptyClipboard() == 0 {
                return Err(ClipboardError::Failed("EmptyClipboard 失败".to_owned()));
            }
            let handle = GlobalAlloc(GMEM_MOVEABLE, bytes);
            if handle.is_null() {
                return Err(ClipboardError::Failed("分配剪贴板内存失败".to_owned()));
            }
            let target = GlobalLock(handle);
            if target.is_null() {
                // 分配成功但锁不上：这块内存归我们，必须释放
                GlobalFree(handle);
                return Err(ClipboardError::Failed("锁定剪贴板内存失败".to_owned()));
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr().cast::<u8>(), target.cast::<u8>(), bytes);
            // 🔴 这里的返回值 0 表示"锁计数未归零"，**不是失败**：只要 GetLastError 为 0 就没事。
            //    实测确认（探针里同样返回 0 而一切正常）。
            let _ = GlobalUnlock(handle);
            if SetClipboardData(CF_UNICODETEXT, handle).is_null() {
                // 移交失败 ⇒ 所有权仍在我们手上，必须释放，否则泄漏
                GlobalFree(handle);
                return Err(ClipboardError::Failed("SetClipboardData 失败".to_owned()));
            }
            // 成功：所有权已移交系统，**不得**再 GlobalFree
            Ok(())
        })();
        let _ = CloseClipboard();
        result
    }
}

/// 读系统剪贴板的文本。
///
/// # Errors
/// 被占用 → [`ClipboardError::Busy`]；没有文本 → [`ClipboardError::Empty`]。
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn get_text() -> Result<String, ClipboardError> {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalUnlock};

    const CF_UNICODETEXT: u32 = 13;
    /// 扫描上限：剪贴板内容异常时的兜底（F-FT-32 的单条上限是 256 KiB，这里给足余量）
    const MAX_UNITS: usize = 1024 * 1024;

    let _gate = lock();

    // SAFETY: 同 `set_text`：每次 Open 必有配对 Close；`GetClipboardData` 返回的句柄
    // **归系统所有**，我们只读、不释放。
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return Err(ClipboardError::Busy);
        }
        let result = (|| -> Result<String, ClipboardError> {
            // 🔴 先问格式再取数据：直接 Get 失败时无法区分"没有文本"与"取不到"
            if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
                return Err(ClipboardError::Empty);
            }
            let handle = GetClipboardData(CF_UNICODETEXT);
            if handle.is_null() {
                return Err(ClipboardError::Empty);
            }
            let pointer = GlobalLock(handle);
            if pointer.is_null() {
                return Err(ClipboardError::Failed("锁定剪贴板内存失败".to_owned()));
            }
            let units = pointer.cast::<u16>();
            let mut length = 0usize;
            while length < MAX_UNITS && *units.add(length) != 0 {
                length += 1;
            }
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(units, length));
            let _ = GlobalUnlock(handle);
            Ok(text)
        })();
        let _ = CloseClipboard();
        result
    }
}

/// 非 Windows 平台的占位（本工程只发布 Windows，规格 §一）。
///
/// # Errors
/// 恒返回 [`ClipboardError::Failed`]——不给"静默当成功"的机会。
#[cfg(not(windows))]
pub fn set_text(_text: &str) -> Result<(), ClipboardError> {
    Err(ClipboardError::Failed(
        "剪贴板目前仅实现 Windows".to_owned(),
    ))
}

/// 非 Windows 平台的占位。
///
/// # Errors
/// 恒返回 [`ClipboardError::Failed`]。
#[cfg(not(windows))]
pub fn get_text() -> Result<String, ClipboardError> {
    Err(ClipboardError::Failed(
        "剪贴板目前仅实现 Windows".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 写剪贴板，遇到"被别的程序占用"时**短重试**。
    ///
    /// 🔴 为什么要有这个：剪贴板是**系统级共享资源**，任何程序都可能临时占住它。
    /// 这条测试因此**天生可能撞上 Busy**——不重试就会偶发失败，
    /// 而偶发失败会把整套门禁的可信度一起拖下水（实测撞到过一次）。
    ///
    /// 返回 `false` = 重试后仍被占用（调用方应**跳过**并说明原因，而不是判失败）。
    fn set_with_retry(text: &str) -> bool {
        for _ in 0..8 {
            match set_text(text) {
                Ok(()) => return true,
                Err(ClipboardError::Busy) => std::thread::sleep(Duration::from_millis(120)),
                // 其它错误不是"被占用"，照旧暴露出来
                Err(other) => panic!("写剪贴板失败（非占用）：{other}"),
            }
        }
        false
    }
    /// 🔴 往返测试**会动到用户当前的剪贴板**，故先存后还原：
    /// 一个测试不该把别人正在用的剪贴板内容弄丢。
    ///
    /// 与别的用例并发也不怕：本模块每次操作都在同一把进程级锁内（见模块头）。
    #[test]
    fn set_then_get_round_trips_and_restores_the_users_clipboard() {
        let saved = get_text().ok();
        let probe = "Aegis 剪贴板往返测试 🚀 / tab\t/ 换行\n";
        if !set_with_retry(probe) {
            eprintln!("跳过：剪贴板被其它程序持续占用（环境相关，不是实现缺陷）");
            return;
        }
        assert_eq!(
            get_text().expect("读剪贴板"),
            probe,
            "写进去与读出来必须一致"
        );
        if let Some(previous) = saved {
            let _ = set_text(&previous);
        }
    }

    #[test]
    fn empty_text_is_writable_and_readable() {
        let saved = get_text().ok();
        if !set_with_retry("") {
            eprintln!("跳过：剪贴板被其它程序持续占用（环境相关，不是实现缺陷）");
            return;
        }
        assert_eq!(get_text().expect("读回"), "");
        if let Some(previous) = saved {
            let _ = set_text(&previous);
        }
    }

    #[test]
    fn error_text_distinguishes_busy_from_empty() {
        // 三档文案必须互不相同：F-FT-33 要求"被占用"与"为空"给不同话术
        let busy = ClipboardError::Busy.to_string();
        let empty = ClipboardError::Empty.to_string();
        let failed = ClipboardError::Failed("x".to_owned()).to_string();
        assert_ne!(busy, empty);
        assert_ne!(busy, failed);
        assert_ne!(empty, failed);
        assert!(busy.contains("占用"));
        assert!(empty.contains("没有文本"));
    }
}
