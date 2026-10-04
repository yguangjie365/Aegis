// 追加测试：canonicalize 是否泄漏 \\?\ 前缀（会直接显示到界面上）
use std::fs;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    println!("=== canonicalize 前缀泄漏测试 ===");
    for a in &args {
        match fs::canonicalize(a) {
            Ok(p) => {
                let s = p.to_string_lossy().to_string();
                println!(
                    "  in : {}\n  out: {}\n  -> 含 \\\\?\\ 前缀 = {}   长度 {} -> {}",
                    a,
                    s,
                    s.starts_with("\\\\?\\"),
                    a.chars().count(),
                    s.chars().count()
                );
            }
            Err(e) => println!("  in : {}\n  ERR: {}", a, e),
        }
    }
    println!();
    println!("=== 显示用短路径（不做 canonicalize，仅 join）===");
    for a in &args {
        let p = std::path::PathBuf::from(a);
        println!(
            "  {}  ->  含前缀 = {}",
            p.display(),
            p.to_string_lossy().starts_with("\\\\?\\")
        );
    }
}
