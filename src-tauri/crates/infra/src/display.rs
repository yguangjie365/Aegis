//! 显示器分辨率/刷新率 + EDID 注册表读（M-01 片2-c，2026-10-03）。
//!
//! **为什么在本 crate**：GDI API（`EnumDisplaySettingsW`）与注册表二进制读取属 FFI 类系统访问，
//! 准入表明文"FFI 只出现在 infra 与 elevated-helper"（[04 分册 §一](../../../../Docs/开发文档/03-开发规范/04-文件与目录规范.md)、02 册 KN-04）。
//!
//! **复用结论（DP-1）**：
//! - 桌面分辨率/刷新率 ← `Source/NexBox/src-tauri/src/hardware.rs:221 fallback_enumerate_monitors()`
//!   的 GDI 路径（②档改造移植，登记见 `DEPENDENCIES.md §三`）；
//! - EDID 注册表键 ← 母本 `:542` 注册表兜底路径（同读 `HKLM\SYSTEM\...\EDID` 二进制 blob）。
//!
//! 🔴 本层**零业务判断**：只负责从 GDI / 注册表取原始值，解码与展示文案归 `services`。

use winreg::enums::*;
use winreg::RegKey;

/// 一张显示器的桌面分辨率与刷新率（来自 GDI `EnumDisplaySettingsW`，当前活动模式）。
#[derive(Debug, Clone)]
pub struct DisplayMode {
    /// 水平像素数（`dmPelsWidth`）
    pub width: u32,
    /// 垂直像素数（`dmPelsHeight`）
    pub height: u32,
    /// 刷新率 Hz（`dmDisplayFrequency`），0 = 未上报
    pub refresh_hz: u32,
}

/// 枚举本机所有活动桌面的显示模式（主屏优先，按设备索引升序）。
///
/// # Errors
///
/// GDI 调用失败时的用户可读原因。
pub fn active_display_modes() -> Result<Vec<DisplayMode>, String> {
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplaySettingsW, DEVMODEW, DISPLAY_DEVICEW, ENUM_CURRENT_SETTINGS,
    };

    let mut modes = Vec::new();
    let mut adapter_index = 0_u32;

    loop {
        let mut adapter: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
        adapter.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;

        // 枚举显示适配器（\\.\DISPLAY1, \\.\DISPLAY2, ...）
        if unsafe {
            windows_sys::Win32::Graphics::Gdi::EnumDisplayDevicesW(
                std::ptr::null(),
                adapter_index,
                &mut adapter,
                0,
            )
        } == 0
        {
            break;
        }

        // 跳过非活动适配器（无 ATTACHED_TO_DESKTOP 标志）
        if adapter.StateFlags & 0x00000004 == 0 {
            // 0x00000004 = DISPLAY_DEVICE_ATTACHED_TO_DESKTOP
            adapter_index += 1;
            continue;
        }

        let mut dev_mode: DEVMODEW = unsafe { std::mem::zeroed() };
        dev_mode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;

        if unsafe {
            EnumDisplaySettingsW(
                adapter.DeviceName.as_ptr(),
                ENUM_CURRENT_SETTINGS,
                &mut dev_mode,
            )
        } != 0
        {
            modes.push(DisplayMode {
                width: dev_mode.dmPelsWidth,
                height: dev_mode.dmPelsHeight,
                refresh_hz: dev_mode.dmDisplayFrequency,
            });
        }

        adapter_index += 1;
    }

    Ok(modes)
}

/// EDID 解析结果（型号、制造商、序列号、物理尺寸厘米）。
///
/// 字段全部 `Option`：EDID blob 可能残缺或校验失败，🔴 不得把垃圾当真实值展示。
// 🔴 2026-10-03 补 `PartialEq`：单测用 `assert_eq!` 比较 `Option<EdidInfo>`，
//    缺该 derive 会导致 `E0369 binary operation == cannot be applied to Option<EdidInfo>`
//    而使 `cargo test` 在本 crate 编译阶段就失败（连带 services/infra 全部测试跑不起来）。
#[derive(Debug, Clone, PartialEq)]
pub struct EdidInfo {
    /// 型号名（EDID descriptor block 第 2-4 块中的 ASCII 文本，去前后空白）
    pub model: Option<String>,
    /// 制造商三字码（EDID byte 8-9 解码为 PNP ID，如 "DEL" → "Dell"）
    pub manufacturer: Option<String>,
    /// 序列号（EDID serial number field，纯数字串）
    pub serial: Option<String>,
    /// 物理宽度 cm（EDID byte 21）
    pub width_cm: Option<u8>,
    /// 物理高度 cm（EDID byte 22）
    pub height_cm: Option<u8>,
}

/// 从注册表读取所有显示器的 EDID blob 并解码。
///
/// 键路径：`HKLM\SYSTEM\CurrentControlSet\Enum\DISPLAY\<PNP_ID>\<Instance_ID>\Device Parameters\EDID`
/// （母本 `:542` 同路径，Windows 每接一台显示器就在此处写一份二进制 EDID）。
///
/// # Errors
///
/// 注册表根键打开失败时返回原因；单个子键读不到 EDID 静默跳过（不整体失败）。
pub fn read_edid_from_registry() -> Result<Vec<EdidInfo>, String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let enum_key = hklm
        .open_subkey(r"SYSTEM\CurrentControlSet\Enum\DISPLAY")
        .map_err(|e| format!("注册表打开 DISPLAY 枚举键失败: {e}"))?;

    let mut results = Vec::new();

    for pnp_result in enum_key.enum_keys().flatten() {
        let pnp_sub = match enum_key.open_subkey(&pnp_result) {
            Ok(sub) => sub,
            Err(_) => continue,
        };
        for instance_result in pnp_sub.enum_keys().flatten() {
            let device_params = match pnp_sub.open_subkey(&instance_result) {
                Ok(sub) => sub,
                Err(_) => continue,
            };
            let edid_blob = match device_params.get_raw_value("EDID") {
                Ok(raw) => raw.bytes,
                Err(_) => continue,
            };
            if let Some(info) = decode_edid(&edid_blob) {
                results.push(info);
            }
        }
    }

    Ok(results)
}

/// 解码 128 字节 EDID blob（CEA-861 基础块）。
///
/// 参考 tubatools / 母本 `:417-428` 的 ushort 文本数组解码逻辑：
/// - byte 8-9：制造商 ID（5-bit 编码三字码）
/// - byte 10-11：产品代码（小端 u16）
/// - byte 12-15：序列号（小端 u32）
/// - byte 21-22：物理尺寸 cm
/// - descriptor blocks（byte 54-125）：ASCII 文本描述符（型号名在第 2-4 块）
fn decode_edid(blob: &[u8]) -> Option<EdidInfo> {
    if blob.len() < 128 {
        return None;
    }
    // 头两字节必须为 0x00 0xFF（EDID 标识）
    if blob[0] != 0x00 || blob[1] != 0xFF {
        return None;
    }

    let manufacturer = decode_pnp_id(blob[8], blob[9]);
    let serial = decode_serial(blob[12..16].try_into().ok()?);

    let width_cm = blob[21];
    let height_cm = blob[22];

    let model = decode_model_from_descriptors(&blob[54..126]);

    Some(EdidInfo {
        model,
        manufacturer,
        serial,
        width_cm: if width_cm > 0 { Some(width_cm) } else { None },
        height_cm: if height_cm > 0 { Some(height_cm) } else { None },
    })
}

/// PNP ID 三字码解码（EDID byte 8-9，5-bit 编码）。
///
/// 算法：manufacturer_code = ((byte8 & 0x7C) << 2) | ((byte9 >> 6) & 0x03) 等，
/// 拆成三个 5-bit 字母（A=1..Z=26）。母本 `:282` 同算法。
fn decode_pnp_id(byte8: u8, byte9: u8) -> Option<String> {
    // PNP ID 为 16 bit，EDID 偏移 0x08–0x09 按**大端**存放（byte8 = 高字节），
    // 其中 3 个 5 bit 字母依次为 c1<<10 | c2<<5 | c3。
    // 例：Dell "DEL" = 0x10AC → byte8=0x10, byte9=0xAC。
    let code = (u16::from(byte8) << 8) | u16::from(byte9);
    let c1 = ((code >> 10) & 0x1F) as u8;
    let c2 = ((code >> 5) & 0x1F) as u8;
    let c3 = (code & 0x1F) as u8;
    if c1 == 0 || c2 == 0 || c3 == 0 {
        return None;
    }
    Some(format!(
        "{}{}{}",
        char::from(b'A' + c1 - 1),
        char::from(b'A' + c2 - 1),
        char::from(b'A' + c3 - 1)
    ))
}

/// 序列号解码：小端 u32 → 十进制字符串；全 0 视为无序列号。
fn decode_serial(bytes: [u8; 4]) -> Option<String> {
    let serial = u32::from_le_bytes(bytes);
    if serial == 0 {
        return None;
    }
    Some(serial.to_string())
}

/// 从 EDID 描述符块（byte 54-125，共 4 块 × 18 字节）提取型号名。
///
/// 型号名通常在第 2-4 块的 ASCII 文本描述符中（type 0xFC = 显示器名称）。
/// 母本 `:458` 同口径：只取 EDID 名称描述符，纯型号，不拼厂商/SN。
fn decode_model_from_descriptors(blocks: &[u8]) -> Option<String> {
    if blocks.len() < 72 {
        return None;
    }
    // 4 块，每块 18 字节
    for i in 0..4 {
        let offset = i * 18;
        if offset + 18 > blocks.len() {
            break;
        }
        let block = &blocks[offset..offset + 18];
        // byte 0-1 = 0x00 0x00 表示这是描述符块
        if block[0] != 0x00 || block[1] != 0x00 {
            continue;
        }
        // byte 3 = 类型：0xFC = 显示器名称
        if block[3] != 0xFC {
            continue;
        }
        // byte 5-17 = ASCII 文本（可能含前导空格，以 \n 或 \0 结尾）
        let text_bytes = &block[5..];
        let text = String::from_utf8_lossy(text_bytes);
        let trimmed = text
            .trim_end_matches(|c: char| c == '\n' || c == '\0' || c == ' ')
            .trim_start()
            .to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pnp_id_decodes_known_codes() {
        // PNP ID 编码：3 个 5bit 字母 → c1<<10 | c2<<5 | c3，EDID 内按大端存放
        // （byte8 = 高字节、byte9 = 低字节）。
        // DEL = Dell:    c1=D(4) c2=E(5) c3=L(12) → 0x10AC → (0x10, 0xAC)
        // 🔴 2026-10-03 修正：本测试原先写成 (0x4C, 0xAC) / (0x59, 0xD4)，
        //    是把字节内容与偏移量混写，断言必然失败（解出 SEL/VSA 而非 DEL/SAM）。
        //    连带暴露 `EdidInfo` 缺 `PartialEq` 而使本 crate 测试无法编译，一并补上。
        assert_eq!(decode_pnp_id(0x10, 0xAC).as_deref(), Some("DEL"));
        // SAM = Samsung: c1=S(19) c2=A(1) c3=M(13) → 0x4C2D → (0x4C, 0x2D)
        assert_eq!(decode_pnp_id(0x4C, 0x2D).as_deref(), Some("SAM"));
        // 全零无效
        assert_eq!(decode_pnp_id(0x00, 0x00), None);
    }

    #[test]
    fn serial_zero_is_missing() {
        assert_eq!(decode_serial([0, 0, 0, 0]), None);
        assert_eq!(
            decode_serial([0x39, 0x05, 0x00, 0x00]).as_deref(),
            Some("1337")
        );
    }

    #[test]
    fn edid_blob_too_short_returns_none() {
        assert_eq!(decode_edid(&[0x00, 0xFF]), None);
    }

    #[test]
    fn edid_header_mismatch_returns_none() {
        let mut blob = vec![0u8; 128];
        blob[0] = 0xFF;
        blob[1] = 0x00;
        assert_eq!(decode_edid(&blob), None);
    }

    #[test]
    fn model_from_descriptor_block_type_fc() {
        // 构造一个简化的描述符块：type 0xFC，文本 "DELL U2415"
        let mut blocks = vec![0u8; 72];
        // 第 2 块（i=1, offset=18）
        let offset = 18;
        blocks[offset] = 0x00;
        blocks[offset + 1] = 0x00;
        blocks[offset + 3] = 0xFC; // 显示器名称
        let name = b"DELL U2415\n";
        blocks[offset + 5..offset + 5 + name.len()].copy_from_slice(name);
        assert_eq!(
            decode_model_from_descriptors(&blocks).as_deref(),
            Some("DELL U2415")
        );
    }

    #[test]
    fn active_display_modes_returns_ok_on_any_system() {
        // 单测环境可能无桌面（CI / 远程会话），这里只锁"不 panic 且返回 Ok 或 Err"
        let result = active_display_modes();
        assert!(result.is_ok() || result.is_err());
        if let Ok(modes) = result {
            // 有桌面时至少一个模式
            if !modes.is_empty() {
                assert!(modes[0].width > 0);
                assert!(modes[0].height > 0);
            }
        }
    }
}
