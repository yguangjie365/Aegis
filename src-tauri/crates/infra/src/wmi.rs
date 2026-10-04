//! WMI 只读查询通道（M-01 片2-b，2026-10-03）。
//!
//! **为什么在本 crate**：WMI 经 COM（`IWbemServices`）访问，属 FFI 类系统访问，
//! 准入表明文"FFI 只出现在 infra 与 elevated-helper"（[04 分册 §一](../../../../Docs/开发文档/03-开发规范/04-文件与目录规范.md)、02 册 KN-04）。
//!
//! **复用结论（DP-1）**：移植自 `Source/NexBox/src-tauri/src/wmi_query.rs`（107 行，②档改造移植，
//! 登记见 `DEPENDENCIES.md §三`）。WQL 语句本身逐条来自母本 `hardware.rs`，不在本文件。
//!
//! 🔴 本层**零业务判断**：只负责连接、执行 WQL、把 `wmi::Variant` 折叠成自有 [`Value`]。
//! 折叠的目的是让 `wmi` crate 的类型不外泄——否则域逻辑被迫依赖 FFI 层类型，`services` 也就
//! 不得不直接声明该依赖（触上面那条准入表）。
//!
//! 🔴 **单测不得真查 WMI**（与"单测不得真起 sidecar"同一条纪律）：本文件的测试只测取值折叠与
//! 提取，全部用手工构造的 [`Value`]。

use std::collections::HashMap;
use wmi::{COMLibrary, Variant, WMIConnection};

/// WMI 属性的归一取值（`Variant` 的窄化镜像，只留本域实际会碰到的形态）。
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Text(String),
    Signed(i64),
    Unsigned(u64),
    Real(f64),
    Flag(bool),
    List(Vec<Value>),
}

/// 一行查询结果：属性名 → 归一取值。
#[derive(Debug, Clone)]
pub struct Row {
    cells: HashMap<String, Value>,
}

impl Row {
    /// 由键值对构造一行。
    ///
    /// 🔴 只给单测与跨层装配用（真实行只从 [`query`] / [`query_ns`] 来）：概览域的各采集器要能
    /// 在**不真查 WMI** 的前提下验证取值逻辑，这是唯一的注入口。
    #[must_use]
    pub fn from_pairs(pairs: &[(&str, Value)]) -> Self {
        Self {
            cells: pairs
                .iter()
                .map(|(key, value)| ((*key).to_string(), value.clone()))
                .collect(),
        }
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.cells.get(key)
    }

    /// 提取为字符串（空白串视为取不到，与母本 `v_str` 同口径）。
    #[must_use]
    pub fn text(&self, key: &str) -> Option<String> {
        match self.get(key)? {
            Value::Text(s) => {
                let trimmed = s.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            }
            // 母本把数值型属性也当字符串读（BIOS 版本、驱动日期等形态不一），此处保持同语义
            Value::Signed(n) => Some(n.to_string()),
            Value::Unsigned(n) => Some(n.to_string()),
            Value::Real(n) => Some(format!("{n}")),
            Value::Flag(b) => Some(b.to_string()),
            Value::List(_) => None,
        }
    }

    #[must_use]
    pub fn u16(&self, key: &str) -> Option<u16> {
        u64_to(self.u64(key)?)
    }

    #[must_use]
    pub fn u32(&self, key: &str) -> Option<u32> {
        u64_to(self.u64(key)?)
    }

    /// 提取为 `u64`（字符串形态的纯数字也接受，母本 `v_u64` 同口径）。
    ///
    /// 🔴 与母本的一处**有意偏离**：母本用 `as` 跨宽度截断（`UI4 → u16` 等），溢出时静默得到错值；
    /// 本层按 R-08"没有证据的数字绝不展示"改判 `None`（取不到就是取不到，不交一个截断值）。
    #[must_use]
    pub fn u64(&self, key: &str) -> Option<u64> {
        match self.get(key)? {
            Value::Unsigned(n) => Some(*n),
            Value::Signed(n) => u64::try_from(*n).ok(),
            Value::Real(n) => u64::try_from(real_to_i64(*n)?).ok(),
            Value::Flag(b) => Some(u64::from(*b)),
            Value::Text(s) => s.trim().parse::<u64>().ok(),
            Value::List(_) => None,
        }
    }

    /// 带符号取值（`Speed` 一类属性对"未知"报 `-1`，🔴 不得折叠成 0 或大正数）。
    #[must_use]
    pub fn i64(&self, key: &str) -> Option<i64> {
        match self.get(key)? {
            Value::Signed(n) => Some(*n),
            Value::Unsigned(n) => i64::try_from(*n).ok(),
            Value::Real(n) => Some(real_to_i64(*n)?),
            Value::Flag(b) => Some(i64::from(*b)),
            Value::Text(s) => s.trim().parse::<i64>().ok(),
            Value::List(_) => None,
        }
    }

    /// 布尔属性（WMI 的 `PhysicalAdapter` / `NetEnabled` 只认 `Variant::Bool`，
    /// 母本同样用 `matches!(.., Bool(true))`；缺失即 `None`，不当 `false` 用）。
    #[must_use]
    pub fn flag(&self, key: &str) -> Option<bool> {
        match self.get(key)? {
            Value::Flag(b) => Some(*b),
            _ => None,
        }
    }

    /// `u16` 数组属性（`ChassisTypes`），以及母本 `v_u16_arr` 同样接受的逗号分隔串形态。
    #[must_use]
    pub fn u16_list(&self, key: &str) -> Vec<u16> {
        match self.get(key) {
            Some(Value::List(items)) => items.iter().filter_map(value_to_u16).collect(),
            Some(Value::Text(s)) => s
                .split(',')
                .filter_map(|part| part.trim().parse::<u16>().ok())
                .collect(),
            Some(value) => value_to_u16(value).map(|n| vec![n]).unwrap_or_default(),
            None => Vec::new(),
        }
    }
}

/// `Real` → 整数：先四舍五入再判宽度溢出，🔴 溢出或非有限值即 `None`（与 `u64_to` 同一条 R-08 口径）。
///
/// 值域检查通过后才做 `as`，此时整数部分已在 `i64` 内，转换不再丢量级，故放行截断类 lint。
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn real_to_i64(value: f64) -> Option<i64> {
    if !value.is_finite() {
        return None;
    }
    let rounded = value.round();
    if rounded > i64::MAX as f64 || rounded < i64::MIN as f64 {
        return None;
    }
    Some(rounded as i64)
}

fn value_to_u16(value: &Value) -> Option<u16> {
    let n = match value {
        Value::Unsigned(n) => *n,
        Value::Signed(n) => u64::try_from(*n).ok()?,
        Value::Text(s) => s.trim().parse::<u64>().ok()?,
        _ => return None,
    };
    u64_to(n)
}

fn u64_to<N: TryFrom<u64>>(n: u64) -> Option<N> {
    N::try_from(n).ok()
}

fn fold(variant: Variant) -> Option<Value> {
    let value = match variant {
        Variant::String(s) => Value::Text(s),
        Variant::I1(n) => Value::Signed(i64::from(n)),
        Variant::I2(n) => Value::Signed(i64::from(n)),
        Variant::I4(n) => Value::Signed(i64::from(n)),
        Variant::I8(n) => Value::Signed(n),
        Variant::UI1(n) => Value::Unsigned(u64::from(n)),
        Variant::UI2(n) => Value::Unsigned(u64::from(n)),
        Variant::UI4(n) => Value::Unsigned(u64::from(n)),
        Variant::UI8(n) => Value::Unsigned(n),
        Variant::R4(n) => Value::Real(f64::from(n)),
        Variant::R8(n) => Value::Real(n),
        Variant::Bool(b) => Value::Flag(b),
        Variant::Array(items) => {
            let folded: Vec<Value> = items.into_iter().filter_map(fold).collect();
            Value::List(folded)
        }
        // Empty / Null / Unknown / Object：本域不消费对象引用，一律当"取不到"
        _ => return None,
    };
    Some(value)
}

/// 在默认命名空间（`ROOT\CIMV2`）执行 WQL。
///
/// # Errors
///
/// 返回用户可读的失败原因（COM 初始化 / 连接 / 查询三段各自命名），🔴 禁静默失败。
pub fn query(wql: &str) -> Result<Vec<Row>, String> {
    run_query(None, wql)
}

/// 在指定命名空间执行 WQL（如 `ROOT\WMI` 的 `WmiMonitorID`，随片2-c 使用）。
///
/// # Errors
///
/// 同上。
pub fn query_ns(namespace: &str, wql: &str) -> Result<Vec<Row>, String> {
    run_query(Some(namespace), wql)
}

fn run_query(namespace: Option<&str>, wql: &str) -> Result<Vec<Row>, String> {
    // 每次调用新建连接：`COMLibrary::new()` 会 `CoInitializeEx`，而调用线程可能已被他人以
    // 不同套间模型初始化过（此时复用长生命周期连接会失败）。母本同形，调用侧改用独立线程隔离。
    let com = COMLibrary::new().map_err(|e| format!("COM 初始化失败: {e}"))?;
    let con = match namespace {
        None => WMIConnection::new(com),
        Some(ns) => WMIConnection::with_namespace_path(ns, com),
    }
    .map_err(|e| format!("WMI 连接失败: {e}"))?;

    let raw: Vec<HashMap<String, Variant>> = con
        .raw_query(wql)
        .map_err(|e| format!("WMI 查询失败: {e}"))?;

    Ok(raw
        .into_iter()
        .map(|cells| Row {
            cells: cells
                .into_iter()
                .filter_map(|(key, variant)| fold(variant).map(|value| (key, value)))
                .collect(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pairs: &[(&str, Value)]) -> Row {
        Row {
            cells: pairs
                .iter()
                .map(|(k, v)| ((*k).to_string(), v.clone()))
                .collect(),
        }
    }

    #[test]
    fn text_trims_and_treats_blank_as_missing() {
        let r = row(&[
            ("Model", Value::Text("  WD Black  ".to_string())),
            ("Empty", Value::Text("   ".to_string())),
        ]);
        assert_eq!(r.text("Model").as_deref(), Some("WD Black"));
        assert_eq!(r.text("Empty"), None, "空白串必须当取不到");
        assert_eq!(r.text("Absent"), None);
    }

    #[test]
    fn numeric_cells_are_readable_as_text() {
        let r = row(&[("Rev", Value::Unsigned(27)), ("Neg", Value::Signed(-1))]);
        assert_eq!(r.text("Rev").as_deref(), Some("27"));
        assert_eq!(r.text("Neg").as_deref(), Some("-1"));
    }

    #[test]
    fn unsigned_overflow_returns_none_instead_of_truncating() {
        // 母本用 `as u16` 会把 70000 截成 4464；本层判 None（R-08）
        let r = row(&[("Big", Value::Unsigned(70_000))]);
        assert_eq!(r.u16("Big"), None);
        assert_eq!(r.u32("Big"), Some(70_000));
        assert_eq!(r.u64("Big"), Some(70_000));
    }

    #[test]
    fn negative_cell_is_not_a_u64_but_is_an_i64() {
        let r = row(&[("Speed", Value::Signed(-1))]);
        assert_eq!(r.u64("Speed"), None);
        assert_eq!(r.i64("Speed"), Some(-1));
    }

    #[test]
    fn numeric_strings_are_parsed() {
        let r = row(&[("Cap", Value::Text("17179869184".to_string()))]);
        assert_eq!(r.u64("Cap"), Some(17_179_869_184));
    }

    #[test]
    fn flag_only_accepts_true_bool() {
        let yes = row(&[("NetEnabled", Value::Flag(true))]);
        let no = row(&[("NetEnabled", Value::Flag(false))]);
        let txt = row(&[("NetEnabled", Value::Text("true".to_string()))]);
        assert_eq!(yes.flag("NetEnabled"), Some(true));
        assert_eq!(no.flag("NetEnabled"), Some(false));
        assert_eq!(txt.flag("NetEnabled"), None, "字符串形态的 true 不当布尔用");
        assert_eq!(row(&[]).flag("NetEnabled"), None, "缺失即 None");
    }

    #[test]
    fn list_and_comma_text_both_fold_to_u16_list() {
        let list = row(&[(
            "ChassisTypes",
            Value::List(vec![Value::Unsigned(9), Value::Unsigned(3)]),
        )]);
        assert_eq!(list.u16_list("ChassisTypes"), vec![9, 3]);
        let text = row(&[("ChassisTypes", Value::Text("9, 3".to_string()))]);
        assert_eq!(text.u16_list("ChassisTypes"), vec![9, 3]);
        let single = row(&[("ChassisTypes", Value::Unsigned(3))]);
        assert_eq!(single.u16_list("ChassisTypes"), vec![3]);
        assert_eq!(row(&[]).u16_list("ChassisTypes"), Vec::<u16>::new());
    }
}
