using System.Text.Json.Serialization;

namespace AegisMonitor;

/// <summary>
/// 单条传感器读数（JSON 字段名与 crates/services/src/overview/sensor_bridge.rs 的 SensorReading 逐字节对齐）。
/// 移植来源：Source/NexBox/monitor/SensorReader.cs（GPL-3.0 同源）。
/// </summary>
public class SensorReading
{
    [JsonPropertyName("hardware")]
    public string Hardware { get; set; } = string.Empty;

    [JsonPropertyName("hardwareType")]
    public string HardwareType { get; set; } = string.Empty;

    [JsonPropertyName("subHardware")]
    public string? SubHardware { get; set; }

    [JsonPropertyName("name")]
    public string Name { get; set; } = string.Empty;

    [JsonPropertyName("sensorType")]
    public string SensorType { get; set; } = string.Empty;

    [JsonPropertyName("value")]
    public float Value { get; set; }

    [JsonPropertyName("unit")]
    public string? Unit { get; set; }
}

public class SensorsResponse
{
    [JsonPropertyName("updatedAt")]
    public string UpdatedAt { get; set; } = string.Empty;

    [JsonPropertyName("sensors")]
    public List<SensorReading> Sensors { get; set; } = new();
}

public class ReadCommand
{
    [JsonPropertyName("cmd")]
    public string Cmd { get; set; } = string.Empty;
}
