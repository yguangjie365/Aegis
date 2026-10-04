//! `discovery` 的测试。
//!
//! 大部分是**纯逻辑**（对端表的状态迁移），因为发现功能最容易出错的地方全在状态迁移上；
//! 末尾有一条**真 socket 环回**用例兜住收发管路（绑定 `:0` 由系统分配端口，不会与其它测试抢端口）。

use super::*;
use std::net::UdpSocket;

const T0: i64 = 1_760_000_000_000;

fn addr(port: u16) -> SocketAddr {
    format!("192.168.1.24:{port}").parse().unwrap()
}

fn announcement(id: &str, name: &str) -> Announcement {
    Announcement {
        device_id: id.to_owned(),
        device_name: name.to_owned(),
        transfer_port: 18_889,
        sent_at_ms: T0,
    }
}

// ── 报文编解码 ────────────────────────────────────────────────────────────

#[test]
fn encode_decode_round_trips() {
    let original = announcement("desktop#abc", "书房台式机");
    let bytes = encode(&original).unwrap();
    assert_eq!(&bytes[..4], b"AGFD", "魔数在前");
    assert_eq!(bytes[4], DISCOVERY_VERSION, "版本紧随其后");
    assert_eq!(decode(&bytes).unwrap(), original);
}

/// 名称在**解码时**就被清理：调用方拿到的一定是可显示的字符串。
#[test]
fn decode_cleans_the_device_name() {
    let mut raw = announcement("desktop#abc", "书\u{200B}房\u{FEFF}台式机");
    raw.device_name = "  书\u{200B}房  ".to_owned();
    let cleaned = decode(&encode(&raw).unwrap()).unwrap();
    assert_eq!(cleaned.device_name, "书房");

    // 全是不可见字符 / 空白 → 回退
    let mut blank = announcement("desktop#abc", "");
    blank.device_name = "\u{200B}   ".to_owned();
    assert_eq!(
        decode(&encode(&blank).unwrap()).unwrap().device_name,
        UNKNOWN_DEVICE_NAME
    );
}

#[test]
fn clean_device_name_trims_and_falls_back() {
    assert_eq!(clean_device_name("  台式机  "), "台式机");
    assert_eq!(clean_device_name("a\u{202E}b"), "ab", "方向控制符也要去掉");
    assert_eq!(clean_device_name(""), UNKNOWN_DEVICE_NAME);
    assert_eq!(clean_device_name("   "), UNKNOWN_DEVICE_NAME);
    assert_eq!(clean_device_name("\u{200B}"), UNKNOWN_DEVICE_NAME);
    // 正常名字不受影响
    assert_eq!(clean_device_name("我的机器 🚀"), "我的机器 🚀");
}

#[test]
fn decode_rejects_garbage_without_panicking_or_echoing_it() {
    // 超上限：**未解析即拒**
    let oversized = vec![0x41_u8; MAX_DATAGRAM_BYTES + 1];
    assert_eq!(
        decode(&oversized),
        Err(DiscoveryError::TooLarge(MAX_DATAGRAM_BYTES + 1))
    );
    // 过短
    assert_eq!(decode(b"AGF"), Err(DiscoveryError::TooShort(3)));
    assert_eq!(decode(b""), Err(DiscoveryError::TooShort(0)));
    // 魔数不符（误连到本端口的路人）
    assert_eq!(decode(b"XXXX\x01{}"), Err(DiscoveryError::BadMagic));
    // 版本不符（不做降级协商）
    assert_eq!(
        decode(b"AGFD\x09{}"),
        Err(DiscoveryError::UnsupportedVersion(9))
    );
    // 头对但体不是 JSON
    assert_eq!(decode(b"AGFD\x01not json"), Err(DiscoveryError::Malformed));
    // 空设备标识
    let body = br#"{"deviceName":"x","transferPort":18889}"#;
    let mut packet = b"AGFD\x01".to_vec();
    packet.extend_from_slice(body);
    assert_eq!(
        decode(&packet),
        Err(DiscoveryError::InvalidField("deviceId"))
    );
    // 端口非法（低于特权段下界）
    let body = br#"{"deviceId":"a#b","deviceName":"x","transferPort":80}"#;
    let mut packet = b"AGFD\x01".to_vec();
    packet.extend_from_slice(body);
    assert_eq!(
        decode(&packet),
        Err(DiscoveryError::InvalidField("transferPort"))
    );

    // 🔴 错误文案不得回显对端可控内容
    let text = DiscoveryError::BadMagic.to_string();
    assert!(!text.contains("XXXX"));
    assert_eq!(text, "宣告魔数不符");
}

/// 前向兼容：新增字段不该让旧端解析失败（缺失字段取默认值）。
#[test]
fn decode_tolerates_missing_optional_fields() {
    let body = br#"{"deviceId":"a#b","transferPort":18889}"#;
    let mut packet = b"AGFD\x01".to_vec();
    packet.extend_from_slice(body);
    let decoded = decode(&packet).unwrap();
    assert_eq!(decoded.device_id, "a#b");
    assert_eq!(decoded.device_name, UNKNOWN_DEVICE_NAME, "缺名回退");
    assert_eq!(decoded.sent_at_ms, 0);
}

// ── 本机标识 ──────────────────────────────────────────────────────────────

#[test]
fn self_identity_is_stable_across_restarts_when_the_token_persists() {
    let (first, generated) = SelfIdentity::load_or_create("DESKTOP-1", None);
    let token = generated.expect("首次必须生成令牌并要求持久化");
    assert_eq!(token.len(), 32, "uuid v4 simple 形式");
    assert_eq!(first.token(), token);

    // 模拟重启：同一个令牌读回来 → **同一个标识**
    let (second, again) = SelfIdentity::load_or_create("DESKTOP-1", Some(&token));
    assert_eq!(again, None, "已有令牌不得再生成");
    assert_eq!(first.device_id(), second.device_id(), "重启后标识必须稳定");

    // 令牌为空串等同缺失
    let (third, regen) = SelfIdentity::load_or_create("DESKTOP-1", Some("   "));
    assert!(regen.is_some());
    assert_ne!(third.device_id(), first.device_id());

    // 机器名不同 → 标识不同
    let (other, _) = SelfIdentity::load_or_create("DESKTOP-2", Some(&token));
    assert_ne!(other.device_id(), first.device_id());
}

#[test]
fn self_announcement_carries_cleaned_name_and_own_id() {
    let identity = SelfIdentity::new("  DESKTOP-1\u{200B}  ", "tok");
    let a = identity.announcement(18_889, T0);
    assert_eq!(
        a.device_id, "  DESKTOP-1\u{200B}  #tok",
        "标识用原名，保证稳定"
    );
    assert_eq!(a.device_name, "DESKTOP-1", "显示名要清理");
    assert_eq!(a.transfer_port, 18_889);
    assert_eq!(a.sent_at_ms, T0);
}

// ── 对端表状态迁移 ────────────────────────────────────────────────────────

#[test]
fn observe_reports_appear_refresh_and_address_change() {
    let mut table = PeerTable::new("self#tok", OFFLINE_AFTER);
    let a = announcement("peer#1", "书房台式机");

    assert_eq!(
        table.observe(&a, addr(40000), T0),
        PeerEvent::Appeared {
            device_id: "peer#1".to_owned()
        }
    );
    assert_eq!(table.len(), 1);

    assert_eq!(
        table.observe(&a, addr(40000), T0 + 3_000),
        PeerEvent::Refreshed {
            device_id: "peer#1".to_owned()
        }
    );
    assert_eq!(table.len(), 1, "重复听到不得增条");

    // 同一台机换地址（多网卡 / 换网段）→ 更新地址而非新增
    let changed = table.observe(&a, addr(40001), T0 + 6_000);
    assert_eq!(
        changed,
        PeerEvent::AddressChanged {
            device_id: "peer#1".to_owned(),
            from: addr(40000),
            to: addr(40001),
        }
    );
    assert_eq!(table.len(), 1, "🔴 换地址不得变成两条");
    assert_eq!(table.get("peer#1").unwrap().address, addr(40001));
}

#[test]
fn self_announcements_are_filtered_out() {
    let mut table = PeerTable::new("self#tok", OFFLINE_AFTER);
    let own = announcement("self#tok", "本机");
    assert_eq!(
        table.observe(&own, addr(40000), T0),
        PeerEvent::SelfAnnouncement
    );
    assert!(table.is_empty(), "🔴 自己的宣告绝不能进列表");
}

/// R-6："移除离线设备按**对象身份**而非仅按地址（避免同名/换地址误删）"。
#[test]
fn peers_are_keyed_by_identity_not_by_name_or_address() {
    let mut table = PeerTable::new("self#tok", OFFLINE_AFTER);
    // 两台**同名**但标识不同的机器 → 必须各占一条
    table.observe(&announcement("peer#1", "台式机"), addr(40000), T0);
    table.observe(&announcement("peer#2", "台式机"), addr(40000), T0);
    assert_eq!(table.len(), 2, "同名不同机不得合并");

    // 同一台机器从两个地址宣告 → 只有一条
    table.observe(&announcement("peer#3", "笔记本"), addr(40010), T0);
    table.observe(&announcement("peer#3", "笔记本"), addr(40011), T0);
    assert_eq!(table.len(), 3, "同机多网卡只一条");
}

// ── 离线判定（R-6 的时钟自洽） ────────────────────────────────────────────

#[test]
fn sweep_removes_peers_after_ten_seconds_of_silence() {
    let mut table = PeerTable::new("self#tok", OFFLINE_AFTER);
    table.observe(&announcement("peer#1", "A"), addr(40000), T0);
    table.observe(&announcement("peer#2", "B"), addr(40001), T0);

    // 9.999 秒：都还在
    assert_eq!(
        table.sweep(T0 + 9_999),
        Vec::<Peer>::new(),
        "9.999 秒时不该有人离线"
    );
    assert_eq!(table.len(), 2);

    // 第 10 秒：peer#1 到期（peer#2 在 T0+5000 又听到过）
    table.observe(&announcement("peer#2", "B"), addr(40001), T0 + 5_000);
    let removed = table.sweep(T0 + 10_000);
    assert_eq!(removed.len(), 1);
    assert_eq!(
        removed[0].device_id, "peer#1",
        "🔴 返回的是**对象本身**（上层要据此发离线事件）"
    );
    assert_eq!(table.len(), 1);

    // 再扫一次不会重复报同一条
    assert_eq!(
        table.sweep(T0 + 10_001),
        Vec::<Peer>::new(),
        "同一条不得重复报离线"
    );
}

/// 🔴 R-6 的核心：**离线判定用本机接收时刻**。
///
/// 对端时钟无论快慢（1970 年或 2099 年）都不得影响在线判定——
/// 否则"对端时钟快 1 小时"会立刻把它判成离线。
#[test]
fn offline_judgement_ignores_the_peers_own_clock() {
    let mut table = PeerTable::new("self#tok", OFFLINE_AFTER);

    for (tag, skew) in [
        ("对端时钟停在 1970", 0_i64),
        ("对端时钟跑到 2099", 4_070_908_800_000_i64),
        ("对端时钟快 1 小时（母本会误判的那种）", T0 + 3_600_000),
    ] {
        let mut a = announcement("peer#1", "A");
        a.sent_at_ms = skew;
        table.observe(&a, addr(40000), T0);

        // 声明时刻再离谱，只要**本机**刚收到，就必须在线
        assert!(
            table.sweep(T0 + 1_000).is_empty(),
            "[{tag}] 刚收到不得判离线"
        );
        assert_eq!(table.len(), 1, "[{tag}]");
        // 而且是本机时刻在推进才判离线
        let removed = table.sweep(T0 + 10_000);
        assert_eq!(removed.len(), 1, "[{tag}] 超过阈值才离线");
        assert_eq!(
            removed[0].announced_at_ms, skew,
            "[{tag}] 对端自报时刻只作展示，原样保留"
        );
        table.observe(&a, addr(40000), T0);
    }
}

#[test]
fn online_snapshot_is_sorted_for_stable_ui_and_tests() {
    let mut table = PeerTable::new("self#tok", OFFLINE_AFTER);
    for id in ["peer#c", "peer#a", "peer#b"] {
        table.observe(&announcement(id, id), addr(40000), T0);
    }
    let ids: Vec<String> = table.online().into_iter().map(|p| p.device_id).collect();
    assert_eq!(ids, vec!["peer#a", "peer#b", "peer#c"]);
}

// ── 真 socket 环回（兜住收发管路） ────────────────────────────────────────

/// 用真实 UDP 环回走一遍"编码 → 发送 → 接收 → 解码 → 进对端表"。
///
/// 端口用 `:0` 由系统分配，避免与其它测试或本机服务抢端口。
#[test]
fn udp_loopback_delivers_and_registers_an_announcement() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("绑定接收端");
    let sender = UdpSocket::bind("127.0.0.1:0").expect("绑定发送端");
    receiver
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("设超时");

    let original = announcement("peer#loop", "环回测试机");
    let payload = encode(&original).unwrap();
    sender
        .send_to(&payload, receiver.local_addr().unwrap())
        .expect("发送宣告");

    let mut buffer = vec![0_u8; MAX_DATAGRAM_BYTES];
    let (read, from) = receiver.recv_from(&mut buffer).expect("应收到宣告");
    let decoded = decode(&buffer[..read]).unwrap();
    assert_eq!(decoded, original, "环回后内容必须一致");

    let mut table = PeerTable::new("self#tok", OFFLINE_AFTER);
    assert_eq!(
        table.observe(&decoded, from, T0),
        PeerEvent::Appeared {
            device_id: "peer#loop".to_owned()
        }
    );
    // 记下的是**真实来源地址**（本机环回 + 系统分配的端口）
    assert_eq!(table.get("peer#loop").unwrap().address, from);
    assert_eq!(from.ip().to_string(), "127.0.0.1");
}

/// 垃圾数据报不得让接收循环崩掉（真实网络上一定会遇到）。
#[test]
fn udp_loopback_survives_garbage_datagrams() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("绑定接收端");
    let sender = UdpSocket::bind("127.0.0.1:0").expect("绑定发送端");
    receiver
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("设超时");
    let to = receiver.local_addr().unwrap();

    sender.send_to(b"not our protocol at all", to).unwrap();
    sender.send_to(&[0xFF_u8; 64], to).unwrap();

    let mut table = PeerTable::new("self#tok", OFFLINE_AFTER);
    let mut buffer = vec![0_u8; MAX_DATAGRAM_BYTES];
    for _ in 0..2 {
        let (read, from) = receiver.recv_from(&mut buffer).expect("应收到数据报");
        assert!(decode(&buffer[..read]).is_err(), "垃圾必须被拒");
        // 拒了就不该进表
        assert!(table.is_empty());
        let _ = from;
    }

    // 垃圾之后仍能正常收下一条合法宣告（循环没被打断）
    let good = encode(&announcement("peer#ok", "正常机")).unwrap();
    sender.send_to(&good, to).unwrap();
    let (read, from) = receiver.recv_from(&mut buffer).expect("应收到合法宣告");
    let decoded = decode(&buffer[..read]).unwrap();
    assert_eq!(
        table.observe(&decoded, from, T0),
        PeerEvent::Appeared {
            device_id: "peer#ok".to_owned()
        }
    );
}
