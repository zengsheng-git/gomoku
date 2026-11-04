// 授权码发码工具(仅开发者使用):
//   cargo run --bin license-keygen -- genkey [--out license-private.key]
//     生成密钥对: 私钥写入文件(绝不入库), 公钥打印出来填入 src/license.rs 的 PUBLIC_KEY
//   cargo run --bin license-keygen -- sign --key <私钥文件> --product <0|1|2> --name <名称> [--expire <yyyymmdd>]
//     product: 0=两版通用 1=象棋(xqlink) 2=揭棋(jieqilink); --expire 省略 = 永久授权; 打印可直接粘贴的授权码
use std::fs;
use std::path::PathBuf;

use wzqlink_lib::license::keygen;
use wzqlink_lib::license::parse_date_days;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("genkey") => cmd_genkey(flag_value(&args, "--out")),
        Some("sign") => cmd_sign(
            flag_value(&args, "--key").unwrap_or_else(|| "license-private.key".into()),
            flag_value(&args, "--product"),
            flag_value(&args, "--name"),
            flag_value(&args, "--expire"),
            flag_value(&args, "--machine").unwrap_or_default(),
        ),
        _ => {
            eprintln!("用法:");
            eprintln!("  license-keygen genkey [--out license-private.key]");
            eprintln!("  license-keygen sign --key <私钥文件> --product <0|1|2> --name <名称> [--expire <yyyymmdd>]");
            eprintln!("    (--expire 省略 = 永久授权)");
            std::process::exit(1);
        }
    }
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1).cloned())
}

fn cmd_genkey(out: Option<String>) {
    let path = PathBuf::from(out.unwrap_or_else(|| "license-private.key".into()));
    if path.exists() {
        eprintln!("私钥文件已存在, 拒绝覆盖: {}", path.display());
        std::process::exit(1);
    }
    match keygen::genkey() {
        Ok((priv_hex, pub_hex)) => {
            fs::write(&path, priv_hex).expect("写私钥失败");
            println!("私钥已写入: {} (请妥善备份, 丢失后无法再签发同系列授权码; 切勿提交到 git)", path.display());
            println!("公钥(hex): {pub_hex}");
            println!("请将以下内容填入 server/src/license.rs 的 PUBLIC_KEY:");
            let bytes: Vec<String> = (0..32)
                .map(|i| format!("0x{}", &pub_hex[i * 2..i * 2 + 2]))
                .collect();
            for chunk in bytes.chunks(8) {
                println!("    {};", chunk.join(", "));
            }
        }
        Err(e) => {
            eprintln!("生成失败: {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_sign(key_file: String, product: Option<String>, name: Option<String>, expire: Option<String>, machine: String) {
    let product = match product.as_deref().unwrap_or("0") {
        "0" | "all" => 0u8,
        "1" | "chess" => 1,
        "2" | "jieqi" => 2,
        "3" | "gomoku" => 3,
        other => {
            eprintln!("未知产品编号: {other} (可用: 0=通用 1=象棋 2=揭棋)");
            std::process::exit(1);
        }
    };
    let name = name.unwrap_or_default();
    if name.is_empty() || name.len() > 60 {
        eprintln!("--name 必填(1~60 字节)");
        std::process::exit(1);
    }
    // --expire 省略 = 永久授权(payload 中 expire_days=0)
    let expire_days = match expire.as_deref().map(parse_date_days) {
        Some(Ok(d)) => d,
        Some(Err(e)) => {
            eprintln!("--expire 格式错误: {e}");
            std::process::exit(1);
        }
        None => 0,
    };
    let expire_label = match expire_days {
        0 => "永久".to_string(),
        _ => expire.clone().unwrap_or_default(),
    };
    let priv_hex = fs::read_to_string(&key_file).unwrap_or_else(|e| {
        eprintln!("读取私钥失败({e}): {}", key_file);
        std::process::exit(1);
    });
    match keygen::sign(priv_hex.trim(), product, expire_days, &name, &machine) {
        Ok(code) => {
            let label = match product {
                0 => "通用",
                1 => "象棋",
                2 => "揭棋",
                _ => "五子棋",
            };
            println!("授权给: {name} ({label}, 至 {expire_label})");
            println!("授权码:");
            println!("{code}");
        }
        Err(e) => {
            eprintln!("签发失败: {e}");
            std::process::exit(1);
        }
    }
}
