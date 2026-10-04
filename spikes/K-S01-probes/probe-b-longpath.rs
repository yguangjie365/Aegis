// K-S01 测试 B：Rust 侧对长路径（>MAX_PATH）与 \\?\ 前缀归一化的实测
// 目的：确定"长路径"这条判定标准在 LongPathsEnabled=0 的机器上要怎么做才成立。
use std::fs;
use std::path::Path;

fn check(label: &str, p: &str) {
    let path = Path::new(p);
    let meta = fs::metadata(path);
    let readable = fs::read(path).map(|v| v.len()).ok();
    println!(
        "{:<28} len={:<4} metadata={:<8} read_bytes={:?}",
        label,
        p.chars().count(),
        match &meta {
            Ok(m) => format!("{}B", m.len()),
            Err(e) => format!("ERR({})", e.kind() as i32),
        },
        readable
    );
    if let Err(e) = &meta {
        println!("      └ os_error={:?}  msg={}", e.raw_os_error(), e);
    }
}

fn main() {
    // 由命令行传入实际样本路径（避免在源码里硬编码中文/emoji 造成转义歧义）
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("用法: probe-b <非ASCII路径> <跨盘路径> <长路径>");
        std::process::exit(2);
    }
    let (non_ascii, cross, long) = (&args[0], &args[1], &args[2]);

    println!("=== 当前进程长路径感知 ===");
    // Windows: 未声明 longPathAware 的进程，>MAX_PATH 的路径会失败（除非加 \\?\ 前缀）
    println!("  (Rust std 不加 \\\\?\\ 前缀，直接依赖 Win32 的 MAX_PATH 行为)");
    println!();

    println!("=== 直接使用（无前缀）===");
    check("非 ASCII + emoji", non_ascii);
    check("跨盘 (C:)", cross);
    check("长路径 >260", long);
    println!();

    println!("=== 加 \\\\?\\ 前缀后 ===");
    let prefixed = format!("\\\\?\\{}", long);
    check("长路径 + \\\\?\\ 前缀", &prefixed);
    println!();

    println!("=== 结论判据 ===");
    let direct = fs::metadata(Path::new(long)).is_ok();
    let with_prefix = fs::metadata(Path::new(&prefixed)).is_ok();
    println!("  长路径 直接可读        = {}", direct);
    println!("  长路径 \\\\?\\ 前缀可读   = {}", with_prefix);
    if !direct && with_prefix {
        println!("  ⇒ 🔴 结论：必须做 \\\\?\\ 归一化，否则 >260 字符路径一律失败");
    } else if direct {
        println!("  ⇒ 🟢 结论：本机可直接读长路径（可能因进程清单声明或策略已开）");
    } else {
        println!("  ⇒ 🔴 结论：两种方式都失败，长路径不可支持");
    }
}
