using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using LibreHardwareMonitor.Hardware;
using System.Runtime.ExceptionServices;

namespace AegisMonitor;

/// <summary>
/// 传感器采集 sidecar（🔴 本工程唯一传感器出口，ADR-001 §2.4）。
/// 协议：stdin 收一行 JSON 命令（{"cmd":"read"} / {"cmd":"exit"}），stdout 回一行 JSON 结果，
/// stderr 只走诊断文本，由宿主（crates/infra/src/sidecar.rs）排空后转日志。
/// 移植来源：Source/NexBox/monitor/Program.cs（GPL-3.0 同源，登记见 DEPENDENCIES.md §三）。
/// </summary>
class Program
{
    private static Computer? _computer;
    private static readonly SensorVisitor _visitor = new(_ => { });

    // JSON 兜底：允许 NaN / Infinity 序列化为字面量，避免整体序列化崩溃
    private static readonly JsonSerializerOptions _jsonOptions = new()
    {
        NumberHandling = JsonNumberHandling.AllowNamedFloatingPointLiterals
    };

    static int Main(string[] args)
    {
        // stderr 显式切 UTF-8：中文 Windows 控制台默认 OEM 代码页（GBK），宿主（infra::sidecar）
        // 按 UTF-8 逐行读，非 UTF-8 字节会成乱码 → 诊断信息等同于没留（违 R-08）。
        // 唯一失败场景是流已被关闭/重定向到无编码能力的句柄，此时无处可报，只能放过。
        try
        {
            Console.SetError(new StreamWriter(Console.OpenStandardError(), new UTF8Encoding(false))
            {
                AutoFlush = true,
            });
        }
        catch (Exception)
        {
        }

        // 捕获非托管崩溃（AccessViolationException 等 SEH 异常）
        AppDomain.CurrentDomain.FirstChanceException += (_, e) =>
        {
            if (e.Exception is AccessViolationException || e.Exception is NullReferenceException)
            {
                Console.Error.WriteLine($"[AegisMonitor] 首次触发异常: {e.Exception.GetType().Name}: {e.Exception.Message}");
            }
        };

        try
        {
            _computer = new Computer
            {
                IsCpuEnabled = true,
                IsGpuEnabled = true,
                IsMemoryEnabled = true,
                IsMotherboardEnabled = true,
                IsStorageEnabled = true,
                IsNetworkEnabled = false,
                IsControllerEnabled = false,
                IsPsuEnabled = false,
            };

            // Computer.Open() 放独立线程并设 15 秒上限（规格 01 §六 N-04：驱动挂死不得拖死采集）。
            // 超时时进程仍然存活，已初始化成功的硬件可照常读取——这是降级路径而非故障。
            // 🔴 与 NexBox 的差别：本进程不安装任何第三方内核驱动，部分温度/SMART 读数因此在
            //    缺驱动的环境下天然缺失，由宿主按 R-08 显式标注"未检测"（见 DEPENDENCIES.md §四）。
            var openTask = Task.Run(() =>
            {
                try
                {
                    _computer.Open();
                    SafeAccept(_computer);
                    Console.Error.WriteLine("[AegisMonitor] Computer.Open() 完成");
                }
                catch (Exception ex)
                {
                    Console.Error.WriteLine($"[AegisMonitor] 硬件初始化异常: {ex.GetType().Name}: {ex.Message}");
                }
            });

            if (!openTask.Wait(TimeSpan.FromSeconds(15)))
            {
                Console.Error.WriteLine("[AegisMonitor] Computer.Open() 超时(15s)，传感器通道可能不可用（缺内核驱动或硬件不支持）");
            }

            var input = Console.In;
            var output = Console.Out;

            while (true)
            {
                var line = input.ReadLine();
                if (line == null)
                    break; // 宿主关闭了 stdin

                line = line.Trim();
                if (line.Length == 0)
                    continue;

                try
                {
                    var cmd = JsonSerializer.Deserialize<ReadCommand>(line);
                    if (cmd?.Cmd == "read")
                    {
                        SafeUpdateAndCollect(output);
                    }
                    else if (cmd?.Cmd == "exit")
                    {
                        break;
                    }
                }
                catch (JsonException)
                {
                    Console.Error.WriteLine("[AegisMonitor] 命令 JSON 解析失败，已忽略该行");
                }
            }
        }
        catch (Exception ex)
        {
            var error = JsonSerializer.Serialize(new { error = $"{ex.GetType().Name}: {ex.Message}" });
            try { Console.WriteLine(error); Console.Out.Flush(); } catch { }
            return 1;
        }
        finally
        {
            try { _computer?.Close(); } catch { }
        }

        return 0;
    }

    /// <summary>
    /// 安全执行 Accept：某些硬件在 Accept 时可能触发 AccessViolation
    /// </summary>
    static void SafeAccept(Computer computer)
    {
        try
        {
            computer.Accept(_visitor);
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"[AegisMonitor] Accept 异常: {ex.GetType().Name}: {ex.Message}");
        }
    }

    /// <summary>
    /// 安全执行 Update + Collect，任何硬件崩溃都不会拖垮进程
    /// </summary>
    static void SafeUpdateAndCollect(TextWriter output)
    {
        // 首次：暴力更新
        UpdateHardware(_computer!.Hardware);

        // 第二次 Accept（有时 Accept 比 Update 更容易触发崩溃，改用 SafeAccept）
        SafeAccept(_computer);

        var sensors = new List<SensorReading>();
        SafeCollectSensors(_computer.Hardware, sensors);

        var response = new SensorsResponse
        {
            UpdatedAt = DateTime.UtcNow.ToString("o"),
            Sensors = sensors
        };

        var json = JsonSerializer.Serialize(response, _jsonOptions);
        output.WriteLine(json);
        output.Flush();
    }

    static void CollectSensors(IEnumerable<IHardware> hardwareList, List<SensorReading> sensors)
    {
        foreach (var hw in hardwareList)
        {
            if (hw.SubHardware.Length > 0)
            {
                CollectSensors(hw.SubHardware, sensors);
            }

            foreach (var sensor in hw.Sensors)
            {
                if (!sensor.Value.HasValue)
                    continue;

                var value = sensor.Value.Value;
                // 过滤 NaN / Infinity / -Infinity：这些值无法被 JSON 表达，且对展示无意义
                // 注：net48 上没有 float.IsFinite，用 IsNaN/IsInfinity 组合判断
                if (float.IsNaN(value) || float.IsInfinity(value))
                    continue;

                sensors.Add(new SensorReading
                {
                    Hardware = hw.Name,
                    HardwareType = hw.HardwareType.ToString(),
                    SubHardware = hw.SubHardware.Length > 0 ? hw.Name : null,
                    Name = sensor.Name,
                    SensorType = sensor.SensorType.ToString(),
                    Value = value,
                    Unit = GetUnit(sensor.SensorType)
                });
            }
        }
    }

    /// <summary>
    /// 安全遍历传感器：整体包在 try/catch 中，
    /// 防止某个传感器枚举时触发 AccessViolationException 导致进程退出
    /// </summary>
    static void SafeCollectSensors(IEnumerable<IHardware> hardwareList, List<SensorReading> sensors)
    {
        try
        {
            CollectSensors(hardwareList, sensors);
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"[AegisMonitor] CollectSensors 异常: {ex.GetType().Name}: {ex.Message}");
        }
    }

    static void UpdateHardware(IEnumerable<IHardware> hardwareList)
    {
        foreach (var hw in hardwareList)
        {
            try
            {
                hw.Update();
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"[AegisMonitor] 硬件 \"{hw.Name}\" ({hw.HardwareType}) 更新失败: {ex.GetType().Name}: {ex.Message}");
            }

            if (hw.SubHardware.Length > 0)
            {
                try
                {
                    UpdateHardware(hw.SubHardware);
                }
                catch (Exception ex)
                {
                    Console.Error.WriteLine($"[AegisMonitor] 子硬件 \"{hw.Name}\" 更新失败: {ex.GetType().Name}: {ex.Message}");
                }
            }
        }
    }

    static string? GetUnit(SensorType type)
    {
        return type switch
        {
            SensorType.Voltage => "V",
            SensorType.Clock => "MHz",
            SensorType.Temperature => "°C",
            SensorType.Load => "%",
            SensorType.Fan => "RPM",
            SensorType.Flow => "L/h",
            SensorType.Control => "%",
            SensorType.Level => "%",
            SensorType.Power => "W",
            SensorType.Data => "GB",
            SensorType.SmallData => "MB",
            SensorType.Factor => "",
            SensorType.Frequency => "Hz",
            SensorType.Throughput => "B/s",
            SensorType.Current => "A",
            SensorType.Energy => "mWh",
            SensorType.Noise => "dBA",
            SensorType.Humidity => "%",
            SensorType.TimeSpan => "s",
            _ => null
        };
    }
}
