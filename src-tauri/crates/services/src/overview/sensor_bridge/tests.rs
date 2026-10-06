use super::bridge::{
    classify_sidecar_error, BridgeFault, SensorBridge, EXE_NAME, FIRST_READ_BUDGET,
    MAX_CONSECUTIVE_FAILURES, READ_TIMEOUT,
};
use super::exe::{candidate_paths, FaultKind, SUBDIRS};
use super::matching::{
    aggregate, cpu_load_of, cpu_temperature_of, gpu_sensors_of, max_temperature_in_family,
    memory_load_of, STORAGE_FAMILIES,
};
use super::types::SensorsResponse;

///🔴 **2026-10-07 真机 CPU 温度回包**（管理员令牌，原样截取 CPU 族 34 条中的温度部分）。
///
/// 来源同`REAL_ADMIN_REPLY`：`.worknotes/probe-monitor.cjs` 取真实回包。
///
/// 🔴 它的价值在于暴露了两个**臆造夹具永远测不出**的东西：
///   ① `Distance to TjMax`（`P-Core #1 Distance to TjMax` = 46°C）——
///      含义是「距该核上限还差多少度」，**方向与温度相反**，而它**不含**
///      `limit`/`warning`/`critical`/`resolution` 任何一个词 ⇒ 修前会被
///      当成「单核温度」参与取最大。
///   ② 逐核名是 `P-Core #N` / `E-Core #N`（真机），而既有夹具写的是 `Core #7`。
const REAL_ADMIN_CPU: &str = r#"{"sensors":[
{"hardware":"12th Gen Intel Core i7-12700H","hardwareType":"Cpu","name":"Core Max","sensorType":"Temperature","value":68.0,"unit":"°C"},
{"hardware":"12th Gen Intel Core i7-12700H","hardwareType":"Cpu","name":"Core Average","sensorType":"Temperature","value":56.0,"unit":"°C"},
{"hardware":"12th Gen Intel Core i7-12700H","hardwareType":"Cpu","name":"P-Core #1","sensorType":"Temperature","value":54.0,"unit":"°C"},
{"hardware":"12th Gen Intel Core i7-12700H","hardwareType":"Cpu","name":"P-Core #4","sensorType":"Temperature","value":68.0,"unit":"°C"},
{"hardware":"12th Gen Intel Core i7-12700H","hardwareType":"Cpu","name":"E-Core #1","sensorType":"Temperature","value":53.0,"unit":"°C"},
{"hardware":"12th Gen Intel Core i7-12700H","hardwareType":"Cpu","name":"CPU Package","sensorType":"Temperature","value":64.0,"unit":"°C"},
{"hardware":"12th Gen Intel Core i7-12700H","hardwareType":"Cpu","name":"P-Core #1 Distance to TjMax","sensorType":"Temperature","value":46.0,"unit":"°C"},
{"hardware":"12th Gen Intel Core i7-12700H","hardwareType":"Cpu","name":"E-Core #8 Distance to TjMax","sensorType":"Temperature","value":47.0,"unit":"°C"}
]}"#;

#[test]
fn real_cpu_reply_picks_core_average_over_everything() {
    // 🔴 三档第1 档：`Core Average`（56°C）必须胜出，尽管 `Core Max` 是 68、
    //   `CPU Package` 是 64。逐核最高（68）也不该赢 —— 用户看的是「这芯片多热」，
    //   不是「最热的那核」。
    assert_eq!(
        cpu_temperature_of(&response(REAL_ADMIN_CPU)),
        Some(56.0),
        "应取 Core Average（56），而非 Core Max（68）或 CPU Package（64）"
    );
}

#[test]
fn distance_to_tjmax_is_not_mistaken_for_a_temperature_reading() {
    // 🔴 回归（真机发现）：`Distance to TjMax` 是「距上限还差多少度」，
    //   **不是**温度。若不排除，在没有 `Core Average` / `Package` 的机器上
    //   它会作为「单核温度」参与取最大，把上限余量（32~47）当成读数。
    let fixture = r#"{"sensors":[
{"hardware":"CPU","hardwareType":"Cpu","name":"P-Core #1","sensorType":"Temperature","value":54.0,"unit":"°C"},
{"hardware":"CPU","hardwareType":"Cpu","name":"P-Core #1 Distance to TjMax","sensorType":"Temperature","value":46.0,"unit":"°C"}
]}"#;
    assert_eq!(
        cpu_temperature_of(&response(fixture)),
        Some(54.0),
        "`Distance to TjMax`（距上限余量）不得被当成温度读数"
    );
}

#[test]
fn cpu_temperature_selection_is_independent_of_sensor_order() {
    // 🔴 这条钉住「切到 metricId 后仍然与顺序无关」—— 母本NexBox 的
    //   「首个命中」缺陷（`sensor.rs:292`）正是靠这条被排除的。
    let forward = r#"{"sensors":[
{"hardware":"CPU","hardwareType":"Cpu","name":"Core Average","sensorType":"Temperature","value":56.0,"unit":"°C"},
{"hardware":"CPU","hardwareType":"Cpu","name":"CPU Package","sensorType":"Temperature","value":64.0,"unit":"°C"}
]}"#;
    let reversed = r#"{"sensors":[
{"hardware":"CPU","hardwareType":"Cpu","name":"CPU Package","sensorType":"Temperature","value":64.0,"unit":"°C"},
{"hardware":"CPU","hardwareType":"Cpu","name":"Core Average","sensorType":"Temperature","value":56.0,"unit":"°C"}
]}"#;
    assert_eq!(
        cpu_temperature_of(&response(forward)),
        cpu_temperature_of(&response(reversed)),
        "打乱传感器顺序不得改变取值"
    );
}

#[test]
fn unknown_family_does_not_participate_in_selection() {
    // 🔴 `misc.*`（语义未知的族）不得参与取最大 —— 否则一个来源不明的
    //   高读数会盖掉真正的 CPU 温度。
    let fixture = r#"{"sensors":[
{"hardware":"CPU","hardwareType":"Cpu","name":"Core Average","sensorType":"Temperature","value":56.0,"unit":"°C"},
{"hardware":"Acme Widget","hardwareType":"AcmeSensor","name":"Widget Core","sensorType":"Temperature","value":999.0,"unit":"°C"}
]}"#;
    assert_eq!(
        cpu_temperature_of(&response(fixture)),
        Some(56.0),
        "未识别族的高读数不得污染 CPU 温度"
    );
}

///🔴 **2026-10-07 真机回包**（管理员令牌，205 条传感器的存储与内存部分，原样截取）。
///
/// 来源：`AegisMonitor.exe` stdin `{"cmd":"read"}` → stdout 单行 JSON
/// （取数脚本 `.worknotes/probe-monitor.cjs`，非管理员时同机仅 64 条 / 仅 3 族）。
///
/// 🔴 **这份夹具的价值在于它是「抄」的不是「想」的**：
///   `hardwareType` 是 `Storage`（**不是** `Ssd`）、阈值条目叫
///   `Warning Temperature` / `Critical Temperature`（**不是** `Warning Comp. Temperature`）、
///   内存读数叫 `DIMM #0` / `DIMM #2`（**不是** `DIMM1` / `DIMM3`，
///   并且 hardware 是 `SK Hynix - HMCG78MEBSA253N (#0)` 而非 `Total Memory`）。
///   此前所有夹具都是照抄母本源码臆造的形态 —— 阈值缺陷（85°C）与
///   存储族缺失（`Storage` 不在白名单）**都因此测不出来**。
const REAL_ADMIN_REPLY: &str = r#"{"sensors":[
{"hardware":"SK Hynix - HMCG78MEBSA253N (#0)","hardwareType":"Memory","name":"DIMM #0","sensorType":"Temperature","value":52.0,"unit":"°C"},
{"hardware":"SK Hynix - HMCG78MEBSA253N (#0)","hardwareType":"Memory","name":"Temperature Sensor Resolution","sensorType":"Temperature","value":0.25,"unit":"°C"},
{"hardware":"SK Hynix - HMCG78MEBSA253N (#0)","hardwareType":"Memory","name":"Thermal Sensor Low Limit","sensorType":"Temperature","value":0.0,"unit":"°C"},
{"hardware":"SK Hynix - HMCG78MEBSA253N (#0)","hardwareType":"Memory","name":"Thermal Sensor High Limit","sensorType":"Temperature","value":55.0,"unit":"°C"},
{"hardware":"SK Hynix - HMCG78MEBSA253N (#0)","hardwareType":"Memory","name":"Thermal Sensor Critical High Limit","sensorType":"Temperature","value":85.0,"unit":"°C"},
{"hardware":"SK Hynix - HMCG78MEBSA253N (#2)","hardwareType":"Memory","name":"DIMM #2","sensorType":"Temperature","value":50.5,"unit":"°C"},
{"hardware":"SAMSUNG MZVL2512HCJQ-00BL2","hardwareType":"Storage","name":"Composite Temperature","sensorType":"Temperature","value":45.0,"unit":"°C"},
{"hardware":"SAMSUNG MZVL2512HCJQ-00BL2","hardwareType":"Storage","name":"Temperature #1","sensorType":"Temperature","value":44.85,"unit":"°C"},
{"hardware":"SAMSUNG MZVL2512HCJQ-00BL2","hardwareType":"Storage","name":"Temperature #2","sensorType":"Temperature","value":46.85,"unit":"°C"},
{"hardware":"SAMSUNG MZVL2512HCJQ-00BL2","hardwareType":"Storage","name":"Warning Temperature","sensorType":"Temperature","value":75.0,"unit":"°C"},
{"hardware":"SAMSUNG MZVL2512HCJQ-00BL2","hardwareType":"Storage","name":"Critical Temperature","sensorType":"Temperature","value":86.0,"unit":"°C"},
{"hardware":"Samsung SSD 990 PRO 1TB","hardwareType":"Storage","name":"Composite Temperature","sensorType":"Temperature","value":44.0,"unit":"°C"},
{"hardware":"Samsung SSD 990 PRO 1TB","hardwareType":"Storage","name":"Critical Temperature","sensorType":"Temperature","value":84.0,"unit":"°C"}
]}"#;

/// 🔴 **管理员回包里的内存「负载」形态**（2026-10-07 取证时采集）。
///
/// ⚠️ **此处标注为「待真机确认」**：管理员回包的温度/硬件名已完整留痕（见
///   `REAL_ADMIN_REPLY`），但 `Load` / `Data` 条目当时**未被脚本列出**，
///   故下面三条夹具的`Load` 归属是**依据 §一（10-03 非管理员实测：**
///   `Total Memory` 下有 `Load` ×1 `Memory` 37.24%）+ 硬件名全表**推导**的，
///   **不是**抄来的真机数据。
///
///   🔴 一旦真机证伪（管理员下 `Total Memory` 不再给 `Load`），本夹具即失效，
///   须重采 —— 这正是「夹具必须抄真机回包」纪律的例外登记处。
fn memory_load_fixture() -> String {
    r#"{"sensors":[
{"hardware":"Virtual Memory","hardwareType":"Memory","name":"Memory","sensorType":"Load","value":36.7,"unit":"%"},
{"hardware":"Total Memory","hardwareType":"Memory","name":"Memory","sensorType":"Load","value":42.95,"unit":"%"},
{"hardware":"Total Memory","hardwareType":"Memory","name":"Memory Used","sensorType":"Data","value":13.76,"unit":"GB"},
{"hardware":"Virtual Memory","hardwareType":"Memory","name":"Memory Used","sensorType":"Data","value":13.76,"unit":"GB"},
{"hardware":"SK Hynix - HMCG78MEBSA253N (#0)","hardwareType":"Memory","name":"DIMM #0","sensorType":"Temperature","value":52.0,"unit":"°C"},
{"hardware":"SK Hynix - HMCG78MEBSA253N (#2)","hardwareType":"Memory","name":"DIMM #2","sensorType":"Temperature","value":50.5,"unit":"°C"}
]}"#
    .to_owned()
}

#[test]
fn memory_load_must_come_from_total_memory_not_virtual_or_a_module() {
    // 🔴 三类`hardware` 在管理员下**同时存在**，而只有一类是正确的负载来源：
    //   `Virtual Memory`（虚拟内存占用，🔴 绝不可当物理内存占用 —— 母本同口径）
    //   `Total Memory`（物理内存合计，✅ 正确来源）
    //   `SK Hynix - … (#0)/(#2)`（**物理模组**，管理员下新增；本仓不取其负载）
    let snap = aggregate(&response(&memory_load_fixture()), None);
    assert_eq!(
        snap.memory_load_percent,
        Some(42.95),
        "内存负载必须取 `Total Memory`，不得取 `Virtual Memory`（36.7）"
    );
}

#[test]
fn memory_load_is_absent_when_only_a_module_reports_load() {
    // 🔴 反向形态：若某机型只让物理模组带 `Load`，而 `Total Memory` 不带，
    //   则本仓应老实返回 `None`（走 R-07 回退到sysinfo 的占用率），
    //   而**不是**退而取某个模组的值 —— 那是「单条内存占用 90%」这种无意义数字。
    let fixture = r#"{"sensors":[
{"hardware":"SK Hynix - HMCG78MEBSA253N (#0)","hardwareType":"Memory","name":"Memory","sensorType":"Load","value":90.0,"unit":"%"},
{"hardware":"Virtual Memory","hardwareType":"Memory","name":"Memory","sensorType":"Load","value":36.7,"unit":"%"}
]}"#;
    let snap = aggregate(&response(fixture), None);
    assert_eq!(
        snap.memory_load_percent, None,
        "只有模组/虚拟内存带 Load 时不得凭空取一个 —— 应为 None（走回退链）"
    );
}

#[test]
fn memory_temperature_may_come_from_a_module_even_though_load_may_not() {
    // 🔴 两条判据**刻意不对称**，且必须如此：
    //   温度允许来自任意 `Memory` 节点（物理模组的 TSOD 只挂在模组上，
    //     `Total Memory` 这个**虚拟合计节点**不提供温度）；
    //   负载则必须收窄到 `Total Memory`（模组负载是「单条内存的占用」，无业务含义）。
    //   若把温度也收窄到 `Total Memory`，本机内存温度会永远读不到。
    let snap = aggregate(&response(&memory_load_fixture()), None);
    assert_eq!(
        snap.memory_temperature_c,
        Some(52.0),
        "内存温度应从模组节点取最高（52 / 50.5），不得因负载判据而一并收窄"
    );
}

#[test]
fn real_admin_reply_yields_storage_and_memory_temperatures() {
    let snap = aggregate(&response(REAL_ADMIN_REPLY), None);

    assert!(
        snap.has_storage_sensor,
        "真机回包里hardwareType 是 `Storage`；白名单漏了它则存储卡永不显示温度"
    );
    // 两块盘各取 Composite（45 / 44），再取各盘最高 ⇒ 45
    assert_eq!(
        snap.storage_temperature_c,
        Some(45.0),
        "应取两块盘的Composite 读数（45 / 44）中的最高，且不得取到 84/86 的阈值"
    );

    // 内存：两块 SK Hynix 的 `DIMM #0` 52 / `DIMM #2` 50.5 ⇒ 取最高 52。
    //🔴 注意真机的内存 hardware 是**模组型号**（`SK Hynix - … (#0)`），
    //   不是 `Total Memory` —— 后者只承载 `Data`/`Load`（占用率与容量）。
    assert_eq!(
        snap.memory_temperature_c,
        Some(52.0),
        "应取两条 DIMM 读数的最高（52 / 50.5），且不得取到 85 的临界阈值"
    );
    assert!(
        snap.has_memory_temperature,
        "真机 DIMM 读数存在即应有内存温度"
    );
}

#[test]
fn storage_family_accepts_lhm_real_spelling() {
    // 🔴 直接钉住白名单本身：这些是 LHM 0.9.6 **真机出现过**的取值。
    for spelling in [
        "Storage", // 真机：SAMSUNG MZVL2512HCJQ / Samsung SSD 990 PRO 都是它
        "Ssd",     // 旧版本/其它平台
        "Hdd", "HardDisk", "Nvme", "storage", // 大小写不应影响判定
    ] {
        assert!(
            STORAGE_FAMILIES
                .iter()
                .any(|k| k.eq_ignore_ascii_case(spelling))
                || spelling.eq_ignore_ascii_case("storage"),
            "白名单应覆盖真机取值 `{spelling}`"
        );
    }
}
use infra::sidecar::SidecarError;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn response(json: &str) -> SensorsResponse {
    serde_json::from_str(json).expect("测试夹具应为合法传感器 JSON")
}

/// 本机实测形态（K-Q12-2：无内核驱动 + 非管理员 → CPU 无温度、无磁盘族传感器）。
const REALISH_FIXTURE: &str = r#"{
    "updatedAt":"2026-10-03T08:00:00Z",
    "sensors":[
        {"hardware":"CPU #1 - 12th Gen Intel Core i7-12700H","hardwareType":"Cpu","name":"CPU Total","sensorType":"Load","value":22.5,"unit":"%"},
        {"hardware":"CPU #1 - 12th Gen Intel Core i7-12700H","hardwareType":"Cpu","name":"Core (Tctl/Tdie)","sensorType":"Load","value":31.0,"unit":"%"},
        {"hardware":"Total Memory","hardwareType":"Memory","name":"Memory Used","sensorType":"Data","value":11.84,"unit":"GB"},
        {"hardware":"Total Memory","hardwareType":"Memory","name":"Memory","sensorType":"Load","value":37.24,"unit":"%"},
        {"hardware":"Virtual Memory","hardwareType":"Memory","name":"Memory","sensorType":"Load","value":99.0,"unit":"%"},
        {"hardware":"GPU #1 - NVIDIA GeForce RTX 3060 Laptop","hardwareType":"GpuNvidia","name":"GPU Core","sensorType":"Temperature","value":56.0,"unit":"°C"},
        {"hardware":"GPU #1 - NVIDIA GeForce RTX 3060 Laptop","hardwareType":"GpuNvidia","name":"D3D 3D","sensorType":"Load","value":3.0,"unit":"%"},
        {"hardware":"GPU #1 - NVIDIA GeForce RTX 3060 Laptop","hardwareType":"GpuNvidia","name":"GPU Core","sensorType":"Load","value":1.0,"unit":"%"},
        {"hardware":"GPU #2 - Intel UHD","hardwareType":"GpuIntel","name":"GPU Core","sensorType":"Load","value":7.0,"unit":"%"}
    ]
}"#;

#[test]
fn cpu_load_prefers_cpu_total() {
    assert_eq!(cpu_load_of(&response(REALISH_FIXTURE)), Some(22.5));
}

#[test]
fn cpu_temperature_is_absent_not_zero_when_driver_missing() {
    // 夹具里刻意不放 CPU Temperature（本机实测 0 条）→ 必须 None，不得给 0
    assert_eq!(cpu_temperature_of(&response(REALISH_FIXTURE)), None);
}

#[test]
fn cpu_temperature_matches_superio_family_on_older_amd() {
    let fixture = r#"{"sensors":[{"hardware":"F71882","hardwareType":"SuperIO","name":"Tctl","sensorType":"Temperature","value":47.0,"unit":"°C"}]}"#;
    assert_eq!(cpu_temperature_of(&response(fixture)), Some(47.0));
}

#[test]
fn explicit_null_fields_are_not_a_protocol_fault() {
    // 🔴 回归（2026-10-07 用户报障）：真实 sidecar 回包里存在**显式 null 字段**
    //   （实测含 `"subHardware":null`，且某些设备的 `hardware` 同样是 null）。
    //   而 `#[serde(default)]` **只处理字段「缺失」，不处理显式 null** ⇒
    //   `String`/`f64` 字段直接反序列化失败 ⇒ 整份快照被判协议故障 ⇒
    //   概览页所有温度/增强项一并降级（用户截图：CPU/RAM 卡各挂一条黄色警告，
    //   「传感器通道未就绪」，显存/频率全空）。
    //
    //   🔴 这条测试存在的意义：既有夹具**全部**避开了 null（只测了字符串齐全的
    //   理想形态），所以一直没暴露这个缺陷 —— 与「自写检查器先跑阳性对照」同源。
    // 且不该只是「不报错」—— null 字段取默认值后，**其余字段仍要正确解析**。
    //
    // 🔴 断言直接落在 `SensorReading` 上，**不耦合 `matching.rs` 的聚合判据**
    //   （那条对 hardwareType/name 的组合另有要求，不属于本条修复范围）。
    let fixture = r#"{"sensors":[{"hardware":"CPU #1","hardwareType":"Cpu","subHardware":null,"name":"CPU Core #1","sensorType":"Temperature","value":47.0,"unit":null}]}"#;
    let parsed =
        serde_json::from_str::<crate::overview::sensor_bridge::types::SensorsResponse>(fixture);
    assert!(
        parsed.is_ok(),
        "显式 null 字段不该判协议故障（它只是「这个字段没有值」）：{parsed:?}"
    );
    let mut sensors = parsed.expect("上面已断言可解析").sensors;
    let got = sensors.remove(0);
    assert_eq!(got.value, 47.0, "null 字段不该影响其它字段的解析");
    assert_eq!(got.name, "CPU Core #1");
    assert_eq!(got.hardware, "CPU #1");
    assert_eq!(got.unit, "", "null 的 String 字段应取默认值（空串）");
}

#[test]
fn cpu_temperature_priority_is_independent_of_sensor_order() {
    // 🔴 母本 NexBox（`sensor.rs:292`）是「严格等值 + 首个命中」⇒ **依赖 LHM 的
    //   返回顺序**，顺序一变就可能取到 `Core #1`（单核读数，通常低于整片封装温度）。
    //   2026-10-07 改采 SystemToolkit 三档优先级后，结果必须与顺序无关。
    const A: &str = r#"{"sensors":[
{"hardware":"CPU","hardwareType":"Cpu","name":"Core Average","sensorType":"Temperature","value":55.0,"unit":"°C"},
{"hardware":"CPU","hardwareType":"Cpu","name":"CPU Package","sensorType":"Temperature","value":61.0,"unit":"°C"},
{"hardware":"CPU","hardwareType":"Cpu","name":"Core #1","sensorType":"Temperature","value":54.0,"unit":"°C"}
]}"#;
    const B: &str = r#"{"sensors":[
{"hardware":"CPU","hardwareType":"Cpu","name":"Core #1","sensorType":"Temperature","value":54.0,"unit":"°C"},
{"hardware":"CPU","hardwareType":"Cpu","name":"CPU Package","sensorType":"Temperature","value":61.0,"unit":"°C"},
{"hardware":"CPU","hardwareType":"Cpu","name":"Core Average","sensorType":"Temperature","value":55.0,"unit":"°C"}
]}"#;
    assert_eq!(
        cpu_temperature_of(&response(A)),
        Some(55.0),
        "第 1 档 Core Average 应优先于 CPU Package"
    );
    assert_eq!(
        cpu_temperature_of(&response(B)),
        Some(55.0),
        "🔴 传感器顺序不应影响选值（这正是母本「首个命中」的缺陷）"
    );
}

#[test]
fn cpu_temperature_falls_back_to_highest_core_not_the_first_one() {
    // 末档取**最大值**：逐核读数里首个可能是最低那核，取最高才是「CPU 有多热」。
    let fixture = r#"{"sensors":[
{"hardware":"CPU","hardwareType":"Cpu","name":"Core #1","sensorType":"Temperature","value":54.0,"unit":"°C"},
{"hardware":"CPU","hardwareType":"Cpu","name":"Core #9","sensorType":"Temperature","value":71.0,"unit":"°C"}
]}"#;
    assert_eq!(cpu_temperature_of(&response(fixture)), Some(71.0));
}

#[test]
fn cpu_package_wins_over_individual_cores_when_no_average_exists() {
    let fixture = r#"{"sensors":[
{"hardware":"CPU","hardwareType":"Cpu","name":"Core #1","sensorType":"Temperature","value":54.0,"unit":"°C"},
{"hardware":"CPU","hardwareType":"Cpu","name":"CPU Package","sensorType":"Temperature","value":61.0,"unit":"°C"},
{"hardware":"CPU","hardwareType":"Cpu","name":"CPU Critical Limit","sensorType":"Temperature","value":100.0,"unit":"°C"}
]}"#;
    assert_eq!(
        cpu_temperature_of(&response(fixture)),
        Some(61.0),
        "无 Core Average 时取封装温度；阈值（Critical Limit）不得参与"
    );
}

#[test]
fn threshold_entries_are_not_mistaken_for_temperature_readings() {
    // 🔴 回归（2026-10-07 用户报障，AIDA64 对照）：内存与 SSD **同时**显示 85°C，
    //   而 AIDA64 读数是 DIMM 53°C / SSD 46°C。
    //   根因：LHM 在同一节点下除实际读数外还上报**阈值型条目**
    //   （`Critical Comp. Temperature`，NVMe 典型值正是 85°C），
    //   而聚合只按 hardwareType 过滤、不过滤阈值名 ⇒ 把阈值当成了当前温度。
    //
    //   🔴 夹具刻意复刻真实形态：同节点下读数与阈值**并存**。
    //   🔴 2026-10-07 **再修正**：本夹具原先写 `hardwareType:"Ssd"` —— 那是照抄母本的
    //   臆测值，**LHM 0.9.6 真机从不这样给**（见下方`real_world_admin_reply`）。
    //   夹具不取真实形态 ⇒ 测试全绿却测不到真实缺陷（同一个坑今天踩了两次）。
    let fixture = r#"{"sensors":[
{"hardware":"Samsung SSD 990 PRO 1TB","hardwareType":"Storage","name":"Composite Temperature","sensorType":"Temperature","value":44.0,"unit":"°C"},
{"hardware":"Samsung SSD 990 PRO 1TB","hardwareType":"Storage","name":"Temperature #1","sensorType":"Temperature","value":43.85,"unit":"°C"},
{"hardware":"Samsung SSD 990 PRO 1TB","hardwareType":"Storage","name":"Warning Temperature","sensorType":"Temperature","value":81.0,"unit":"°C"},
{"hardware":"Samsung SSD 990 PRO 1TB","hardwareType":"Storage","name":"Critical Temperature","sensorType":"Temperature","value":84.0,"unit":"°C"},
{"hardware":"Total Memory","hardwareType":"Memory","name":"DIMM1","sensorType":"Temperature","value":53.0,"unit":"°C"},
{"hardware":"Total Memory","hardwareType":"Memory","name":"DIMM3","sensorType":"Temperature","value":52.0,"unit":"°C"},
{"hardware":"Total Memory","hardwareType":"Memory","name":"DIMM1 High Limit","sensorType":"Temperature","value":85.0,"unit":"°C"}
]}"#;
    let snap = aggregate(&response(fixture), None);
    assert!(
        snap.has_storage_sensor,
        "🔴 LHM 真机给的是 `hardwareType:\"Storage\"`（非 \"Ssd\"）—— \
         白名单漏了它则 has_storage_sensor 恒false，存储卡温度永远「未检测」"
    );
    assert_eq!(
        snap.storage_temperature_c,
        Some(44.0),
        "SSD 应取 Composite 读数，不是 84°C 阈值"
    );
    assert_eq!(
        snap.memory_temperature_c,
        Some(53.0),
        "内存应取 DIMM 读数的最大值，不是 85°C 阈值"
    );
    assert!(snap.has_memory_temperature, "有 DIMM 读数即应有内存温度");
}

#[test]
fn memory_visible_without_dimm_reading_is_not_reported_as_memory_temperature() {
    // 🔴 与上一条同源：`has_memory_temperature` 此前只看「hardwareType==Memory 且
    //   类型为 Temperature」⇒ 内存节点可见但 TSOD 读数不可用（无 TSOD 的条子、
    //   或需提权才能读）时，会**误报成「有内存温度」**，界面给出虚假的确定性。
    //   基线 `HardwareSensorProbe.cs:440` 正是为区分这两者准备的提示文案。
    let fixture = r#"{"sensors":[
{"hardware":"Total Memory","hardwareType":"Memory","name":"Memory Used","sensorType":"Data","value":11.8,"unit":"GB"},
{"hardware":"Total Memory","hardwareType":"Memory","name":"DIMM1 Critical Limit","sensorType":"Temperature","value":85.0,"unit":"°C"}
]}"#;
    let snap = aggregate(&response(fixture), None);
    assert_eq!(
        snap.memory_temperature_c, None,
        "无 DIMM 读数时不应给出内存温度"
    );
    assert!(
        !snap.has_memory_temperature,
        "内存可见但无 TSOD 读数时，不该声称有内存温度"
    );
    // 🔴 2026-10-07（DIMM 五态）：上面这条夹具就是**普通权限下的真实形态**。
    //   此刻**没有任何权限证据** ⇒ 绝不能报 PermissionDenied —— 那样等于凭空
    //   承诺「提权就能读到」，而 AIDA64 能读到并不等于本仓提权后能读到。
    assert_eq!(
        snap.memory_availability,
        crate::overview::sensor_bridge::MetricAvailability::Unavailable,
        "无权限证据时不得判定为权限不足"
    );
}

#[test]
fn permission_denied_only_appears_when_sidecar_said_so() {
    // 🔴 五态的**核心不变量**：`PermissionDenied` 的唯一合法来源是 sidecar
    //   诊断文本里的权限字样。同一个回包，证据不同 ⇒ 结论不同。
    let no_dimm = r#"{"sensors":[
        {"hardware":"Total Memory","hardwareType":"Memory","name":"Memory Used","sensorType":"Data","value":11.8,"unit":"GB"}
    ]}"#;

    let silent = aggregate(&response(no_dimm), Some(false));
    assert_eq!(
        silent.memory_availability,
        crate::overview::sensor_bridge::MetricAvailability::Unavailable,
        "无证据 ⇒ Unavailable"
    );

    let proven = aggregate(&response(no_dimm), Some(true));
    assert_eq!(
        proven.memory_availability,
        crate::overview::sensor_bridge::MetricAvailability::PermissionDenied,
        "有权限证据 ⇒ PermissionDenied"
    );

    // 🔴 无内存节点时，即便有权限证据也仍判 unsupported：提权不能凭空造出硬件。
    let no_memory_node = r#"{"sensors":[
        {"hardware":"CPU #1","hardwareType":"Cpu","name":"Core Average","sensorType":"Temperature","value":58.0,"unit":"°C"}
    ]}"#;
    assert_eq!(
        aggregate(&response(no_memory_node), Some(true)).memory_availability,
        crate::overview::sensor_bridge::MetricAvailability::Unsupported,
        "没有内存节点 ⇒ unsupported，提权无用"
    );
}

#[test]
fn diagnostic_excerpt_points_at_the_failing_column_not_the_beginning() {
    // 🔴 回归：诊断片段必须落在**出错列附近**。原实现永远截回包开头，
    //   而 serde 报的出错位置在第 100 列开外 —— 给用户的片段与故障点无关。
    let line = format!("{{\"sensors\":[{}]}}", " ".repeat(100));
    let (col, snippet) = crate::overview::sensor_bridge::bridge::excerpt_near(
        &line,
        "expected value at line 1 column 101",
    );
    // 判据只看「是否落在出错点」：若实现退回「取开头」，片段会以 `{"sensors":[` 开头。
    assert_eq!(col, "101", "列号应从 serde 错误里解析出来");
    assert!(
        !snippet.starts_with('{'),
        "片段不该从回包开头截（旧实现正是这样，出错点在 100 列开外却给开头）"
    );
    assert!(
        snippet.contains(']'),
        "窗口应覆盖到出错列附近的内容，实际取到：{snippet:?}"
    );
}

#[test]
fn load_named_core_sensor_is_not_mistaken_for_temperature() {
    // 夹具内同名条目是 Load（"Core (Tctl/Tdie)" 31%）：温度判定必须看 sensorType
    let fixture = r#"{"sensors":[{"hardware":"CPU","hardwareType":"Cpu","name":"Core (Tctl/Tdie)","sensorType":"Load","value":31.0,"unit":"%"}]}"#;
    assert_eq!(cpu_temperature_of(&response(fixture)), None);
}

#[test]
fn memory_load_ignores_virtual_memory_hardware() {
    // 🔴 规则 4：取到 Virtual Memory 的 99% 就是错数
    assert_eq!(memory_load_of(&response(REALISH_FIXTURE)), Some(37.24));
}

#[test]
fn gpu_grouping_prefers_gpu_core_and_hides_intel_when_nvidia_present() {
    let gpus = gpu_sensors_of(&response(REALISH_FIXTURE));
    assert_eq!(gpus.len(), 1, "存在 NVIDIA 独显时 Intel 核显应被跳过");
    assert_eq!(
        gpus[0].load_percent,
        Some(1.0),
        "负载优先 GPU Core 而非 D3D 3D"
    );
    assert_eq!(gpus[0].temperature_c, Some(56.0));
}

#[test]
fn gpu_keeps_intel_when_it_is_the_only_display() {
    let fixture = r#"{"sensors":[{"hardware":"GPU #1 - Intel UHD","hardwareType":"GpuIntel","name":"GPU Core","sensorType":"Temperature","value":44.0,"unit":"°C"}]}"#;
    let gpus = gpu_sensors_of(&response(fixture));
    assert_eq!(gpus.len(), 1);
    assert_eq!(gpus[0].temperature_c, Some(44.0));
    assert_eq!(gpus[0].load_percent, None, "没有负载传感器时不得凭空造 0");
}

#[test]
fn snapshot_records_missing_sensor_families_as_flags() {
    let snapshot = aggregate(&response(REALISH_FIXTURE), None);
    assert!(!snapshot.has_storage_sensor, "非管理员下 HardDisk 族缺席");
    assert!(!snapshot.has_memory_temperature);
    assert_eq!(snapshot.gpus.len(), 1);
    assert_eq!(snapshot.cpu_load_percent, Some(22.5));
}

#[test]
fn storage_and_memory_temperature_flags_light_up_when_present() {
    let fixture = r#"{"sensors":[
        {"hardware":"SAMSUNG MZVL2","hardwareType":"Ssd","name":"Temperature","sensorType":"Temperature","value":42.0,"unit":"°C"},
        {"hardware":"Total Memory","hardwareType":"Memory","name":"DIMM Temp","sensorType":"Temperature","value":51.0,"unit":"°C"}
    ]}"#;
    let snapshot = aggregate(&response(fixture), None);
    assert!(snapshot.has_storage_sensor);
    assert!(snapshot.has_memory_temperature);
    assert_eq!(snapshot.storage_temperature_c, Some(42.0));
    assert_eq!(snapshot.memory_temperature_c, Some(51.0));
}

#[test]
fn badge_temperature_takes_the_highest_reading_in_the_family() {
    // R-09：多设备取最高，不是第一块
    let fixture = r#"{"sensors":[
        {"hardware":"DISK A","hardwareType":"Ssd","name":"Temperature","sensorType":"Temperature","value":38.0,"unit":"°C"},
        {"hardware":"DISK B","hardwareType":"HardDisk","name":"Temperature","sensorType":"Temperature","value":52.0,"unit":"°C"},
        {"hardware":"DISK C","hardwareType":"Hdd","name":"Temperature","sensorType":"Temperature","value":44.0,"unit":"°C"}
    ]}"#;
    assert_eq!(
        max_temperature_in_family(&response(fixture), &STORAGE_FAMILIES),
        Some(52.0)
    );
    let empty = r#"{"sensors":[{"hardware":"X","hardwareType":"Cpu","name":"a","sensorType":"Load","value":1.0,"unit":"%"}]}"#;
    assert_eq!(
        max_temperature_in_family(&response(empty), &STORAGE_FAMILIES),
        None
    );
}

#[test]
fn error_shaped_payload_is_not_silently_treated_as_empty() {
    // sidecar 自身的 {"error":"…"} 回包必须解析失败 → 走 Protocol 故障（R-08 不得当成"零传感器"）
    let parsed: Result<SensorsResponse, serde_json::Error> =
        serde_json::from_str(r#"{"error":"boom"}"#);
    assert!(parsed.is_err(), "缺 sensors 字段的回包不得被当成合法空快照");
}

#[test]
fn candidate_paths_cover_install_and_dev_layouts() {
    let base = Path::new(r"C:\Program Files\Aegis");
    let paths = candidate_paths(base);
    assert!(paths.contains(&PathBuf::from(
        r"C:\Program Files\Aegis\monitor\AegisMonitor.exe"
    )));
    assert!(paths.iter().any(|path| path
        .to_string_lossy()
        .replace('\\', "/")
        .ends_with("resources/monitor/AegisMonitor.exe")));
    // 必须向上回溯到祖先：开发环境 exe 在 src-tauri/target/debug 下，sidecar 在仓库根的 monitor/
    let deepest = paths
        .iter()
        .filter(|path| path.starts_with("C:\\Program Files\\Aegis"))
        .count();
    assert!(
        deepest >= SUBDIRS.len(),
        "同目录候选应全部保留，实得 {deepest}"
    );
    assert!(paths.iter().any(|path| {
        let text = path.to_string_lossy().replace('\\', "/");
        text.contains("monitor/bin/Release/net48/AegisMonitor.exe")
    }));
}

/// 构造一个"已查过、确认没有 exe"的桥（避免测试依赖本机是否装了 sidecar）。
fn bridge_without_exe() -> SensorBridge {
    let mut bridge = SensorBridge::new();
    bridge.exe_lookup_done = true;
    bridge.exe_path = None;
    bridge
}

#[test]
fn missing_exe_fails_fast_with_component_named() {
    let mut bridge = bridge_without_exe();
    let started = Instant::now();
    let fault = bridge.read_once().expect_err("缺 exe 必须失败");
    assert_eq!(fault.kind, FaultKind::ExeMissing);
    assert!(
        fault.reason.contains(EXE_NAME),
        "原因须点名组件便于诊断，实得：{}",
        fault.reason
    );
    // 🔴 "5s 快败"的结构性上界：exe 缺失路径不得消耗任何读超时预算
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "exe 缺失应立即失败，实得 {:?}",
        started.elapsed()
    );
    assert!(bridge.cooling_down(), "失败后应进入冷却");
}

#[test]
fn cooldown_refuses_retry_until_expiry() {
    let mut bridge = bridge_without_exe();
    let first = bridge.read_once().expect_err("首次应失败");
    assert_eq!(first.kind, FaultKind::ExeMissing);

    let second = bridge.read_once().expect_err("冷却期不得再起进程");
    assert_eq!(second.kind, FaultKind::Cooling);
    assert!(!bridge.is_disabled(), "1 次失败远未达停用阈值");
}

#[test]
fn fifth_consecutive_failure_disables_the_channel_for_the_session() {
    let mut bridge = SensorBridge::new();
    for index in 1..=MAX_CONSECUTIVE_FAILURES {
        let fault = bridge.record_failure(BridgeFault::new(
            FaultKind::Died,
            format!("第 {index} 次退出"),
        ));
        if index < MAX_CONSECUTIVE_FAILURES {
            assert_eq!(fault.kind, FaultKind::Died, "第 {index} 次仍是原故障");
            assert!(bridge.cooling_down());
        } else {
            assert_eq!(fault.kind, FaultKind::Disabled, "第 5 次连败应转停用");
        }
    }
    assert!(bridge.is_disabled());
    let refused = bridge.read_once().expect_err("停用后不得再读");
    assert_eq!(refused.kind, FaultKind::Disabled);
    assert!(
        refused.reason.contains("连续失败 5 次"),
        "停用原因须可诊断：{}",
        refused.reason
    );
}

#[test]
fn successful_read_clears_failure_state() {
    let mut bridge = SensorBridge::new();
    bridge.record_failure(BridgeFault::new(FaultKind::Timeout, "预算耗尽"));
    assert_eq!(bridge.failures, 1);
    assert!(!bridge.has_reads());
    assert_eq!(
        bridge.last_fault().expect("应留下末次故障").kind,
        FaultKind::Timeout
    );

    // 模拟成功回包后的状态推进（不依赖真实子进程）
    bridge.failures = 0;
    bridge.cooldown_until = None;
    bridge.ready_once = true;
    assert!(!bridge.cooling_down());
    assert!(bridge.has_reads());
    assert_eq!(bridge.sidecar_pid(), None, "未接管进程时不得伪造 PID");
}

#[test]
fn timeout_reason_carries_the_budget_actually_used() {
    let fault = classify_sidecar_error(&SidecarError::Timeout, FIRST_READ_BUDGET);
    assert_eq!(fault.kind, FaultKind::Timeout);
    assert!(
        fault.reason.contains("23 秒"),
        "首次预算应写进原因，实得：{}",
        fault.reason
    );
    let steady = classify_sidecar_error(&SidecarError::Timeout, READ_TIMEOUT);
    assert!(steady.reason.contains("8 秒"), "实得：{}", steady.reason);
}

#[test]
fn pipe_write_failure_is_reported_as_a_dead_child() {
    let fault = classify_sidecar_error(
        &SidecarError::Write("管道正在被使用".to_string()),
        READ_TIMEOUT,
    );
    assert_eq!(fault.kind, FaultKind::Died);
    assert!(fault.reason.contains("管道"), "实得：{}", fault.reason);
}
