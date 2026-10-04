//! NVIDIA NVML 只读规格通道（M-01 片2-b，2026-10-03）。
//!
//! **为什么在本 crate**：NVML 经 `libloading` 动态加载系统 `nvml.dll` 并做 C 互操作，属 FFI 类
//! 系统访问（04 分册 §一 准入表、02 册 KN-04）。
//!
//! **复用结论（DP-1）**：取数路径来自 `Source/NexBox/src-tauri/src/hardware.rs:741
//! get_nvidia_gpus_with_nvml()`（②档改造移植）。🔴 母本同一函数里的温度（`:769`）与负载
//! （`:773`）读取**不移植**，`catch_unwind`（`:1073` 等三处）也不移植——边界与理由见
//! [`DEPENDENCIES.md §五`](../../../../DEPENDENCIES.md)（K-004 单一传感器出口）。
//!
//! 🔴 **分发面**：本层只在运行期加载用户机器上随 NVIDIA 驱动已装好的 `nvml.dll`；
//! 工程内零 `nvml.h`、零 NVIDIA 二进制（每批取证 `find Aegis -iname 'nvml*'` 须 0 命中）。

/// 一张 NVIDIA 显卡的**静态规格项**（🔴 依 K-004，本结构刻意不含温度/负载/显存已用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NvmlGpuSpec {
    /// NVML 上报的显卡名（用于与 WMI `Win32_VideoController.Name` 对齐，匹配规则归 `services`）
    pub name: String,
    /// 显存总量（字节）；取不到为 `None`
    pub memory_total_bytes: Option<u64>,
    /// 系统驱动版本串；取不到为 `None`
    pub driver_version: Option<String>,
}

/// 枚举本机 NVIDIA 显卡的静态规格。
///
/// 无 NVIDIA 设备 / 无 `nvml.dll` / 初始化失败一律返回 `Err(原因)`，🔴 调用方不得当成
/// "本机没有独显"的结论（R-08：原因要能区分"没驱动"和"没这张卡"）。
///
/// # Errors
///
/// NVML 初始化或枚举失败时的用户可读原因。
pub fn nvidia_specs() -> Result<Vec<NvmlGpuSpec>, String> {
    use nvml_wrapper::Nvml;

    let nvml = Nvml::init().map_err(|e| format!("NVML 初始化失败: {e}"))?;
    let count = nvml
        .device_count()
        .map_err(|e| format!("NVML 枚举设备数失败: {e}"))?;

    let driver_version = nvml.sys_driver_version().ok();

    let mut specs = Vec::new();
    for index in 0..count {
        // 句柄取不到就整体判失败：本层无日志依赖，🔴 不得在此静默 `continue`（会把"部分缺失"
        // 伪装成"完整清单"）。失败原因交调用方按 R-08 走"未检测"降级，WMI 侧显卡行不受影响。
        let device = nvml
            .device_by_index(index)
            .map_err(|e| format!("NVML 第 {index} 张设备句柄获取失败: {e}"))?;
        let name = device
            .name()
            .map_err(|e| format!("NVML 取设备名失败: {e}"))?;
        // 🔴 只取 memory_info().total：母本还取了 temperature / utilization_rates，
        // 那两个指标在本工程唯一来源是 C# sidecar（K-004），此处不得顺手取。
        let memory_total_bytes = device.memory_info().ok().map(|info| info.total);
        specs.push(NvmlGpuSpec {
            name: name.trim().to_string(),
            memory_total_bytes,
            driver_version: driver_version.clone(),
        });
    }

    Ok(specs)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🔴 单测不得真起 NVML（与"单测不得真起 sidecar"同纪律：本机可能真有 NVIDIA 驱动，
    /// 结果随机器漂移；且真初始化会加载系统 dll）。此处只锁结构的相等语义与字段形状。
    #[test]
    fn spec_is_comparable_by_content() {
        let a = NvmlGpuSpec {
            name: "NVIDIA GeForce RTX 4070".to_string(),
            memory_total_bytes: Some(12_884_901_888),
            driver_version: Some("551.86".to_string()),
        };
        let b = a.clone();
        assert_eq!(a, b);
        assert_ne!(
            a,
            NvmlGpuSpec {
                name: a.name.clone(),
                memory_total_bytes: None,
                driver_version: a.driver_version.clone(),
            },
            "取不到与取到必须可区分（R-08）"
        );
    }
}
