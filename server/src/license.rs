// 授权验证: 离线 Ed25519 签名授权码(无需服务器)
// 码格式: [payload_len u8][payload][ed25519签名 64B] → Base32(Crockford) 每 6 字符一组 '-' 分隔
// payload: [ver u8][product u8][expire_days u32 BE][name_len u8][name utf8][machine_len u8][machine utf8]
//   ver=1; product: 1=象棋(xqlink) 2=揭棋(jieqilink) 0=两版通用
//   machine 为设备绑定预留字段, 当前版本收到非空值直接拒绝(需要发码方配合生成空值)
// 试用期: 首次运行记入 license.json, 7 天内全功能; 超过后引擎提示档位强制锁定新手(1), 授权后解锁

use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use ed25519_dalek::Signature;
use ed25519_dalek::Verifier as _;
use ed25519_dalek::VerifyingKey;
use serde::Deserialize;
use serde::Serialize;

// Ed25519 公钥, 由 tools(`cargo run --bin license-keygen -- genkey`) 生成后填入
// 私钥绝不入库: 私钥文件已在 .gitignore 中
pub const PUBLIC_KEY: [u8; 32] = [
    0xab, 0x7b, 0xfc, 0x35, 0x53, 0x71, 0x70, 0x7e, 0x4e, 0xd5, 0x32, 0xec, 0x45, 0xe4, 0x83, 0xaa, 0x19, 0xc6, 0xfb,
    0x79, 0x8d, 0xcf, 0x9c, 0xf1, 0x82, 0xac, 0xf1, 0x31, 0x4e, 0x12, 0x1e, 0xa9,
];

const PRODUCT: u8 = 3; // 本应用的产品编号: 五子棋
const TRIAL_DAYS: u64 = 7;
const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

#[derive(Serialize, Deserialize, Clone)]
struct LicenseStore {
    // 首次运行时间(天数), 试用期从此时起算
    first_run_day: u64,
    // 激活成功的原始授权码, 未激活为空
    code: String,
}

#[derive(Serialize, Clone)]
pub struct LicenseStatus {
    pub licensed: bool,       // 已激活且未过期
    pub name: String,         // 授权给谁(未授权为空)
    pub expire: String,       // 授权到期日 yyyymmdd(未授权为空)
    pub trial_days_left: u64, // 剩余试用天数(已授权为 0)
    pub trial_expired: bool,  // 试用是否已结束
}

struct State {
    first_run_day: u64,
    info: Option<LicenseInfo>,
}

pub struct LicenseInfo {
    pub name: String,
    pub expire_days: u32,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

fn today_day() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / 86400
}

// days since epoch -> yyyymmdd (Howard Hinnant civil_from_days)
fn day_to_ymd(days: u32) -> String {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}{:02}{:02}", y, m, d)
}

// 验签并解析授权码; 产品不匹配/过期/设备绑定码均视为无效
pub fn verify_code(code: &str) -> Result<LicenseInfo, String> {
    let raw = decode_code(code)?;
    let payload_len = raw[0] as usize;
    if raw.len() != 1 + payload_len + 64 {
        return Err("授权码长度不正确".into());
    }
    let payload = &raw[1..1 + payload_len];
    let sig = Signature::from_slice(&raw[1 + payload_len..])
        .map_err(|_| "授权码格式错误".to_string())?;
    let key = VerifyingKey::from_bytes(&PUBLIC_KEY).map_err(|_| "内置公钥无效".to_string())?;
    key.verify(payload, &sig).map_err(|_| "授权码无效".to_string())?;

    if payload.len() < 7 || payload[0] != 1 {
        return Err("授权码版本不受支持".into());
    }
    let product = payload[1];
    if product != 0 && product != PRODUCT {
        return Err("此授权码不适用于本应用".into());
    }
    let expire_days = u32::from_be_bytes([payload[2], payload[3], payload[4], payload[5]]);
    // expire_days = 0 表示永久授权; 非 0 时检查是否过期
    if expire_days != 0 && (expire_days as u64) < today_day() {
        return Err("此授权码已过期".into());
    }
    let mut i = 6;
    let name_len = payload[i] as usize;
    i += 1;
    if payload.len() < i + name_len + 1 {
        return Err("授权码内容不完整".into());
    }
    let name = String::from_utf8(payload[i..i + name_len].to_vec())
        .map_err(|_| "授权码内容不完整".to_string())?;
    i += name_len;
    let machine_len = payload[i];
    if machine_len > 0 {
        return Err("此授权码为设备绑定版, 当前版本不支持".into());
    }
    Ok(LicenseInfo { name, expire_days })
}

// ===== Base32 Crockford =====

fn encode_code(raw: &[u8]) -> String {
    let mut out = String::new();
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &b in raw {
        buf = (buf << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[(buf >> bits) as usize & 31] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[(buf << (5 - bits)) as usize & 31] as char);
    }
    // 每 6 字符一组
    let mut grouped = String::new();
    for (i, c) in out.chars().enumerate() {
        if i > 0 && i % 6 == 0 {
            grouped.push('-');
        }
        grouped.push(c);
    }
    grouped
}

fn decode_code(code: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = code
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .map(|c| match c {
            'I' | 'L' => '1',
            'O' => '0',
            c => c,
        })
        .collect();
    let mut out = Vec::with_capacity(cleaned.len() * 5 / 8);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for c in cleaned.chars() {
        let v = ALPHABET
            .iter()
            .position(|&a| a as char == c)
            .ok_or_else(|| format!("授权码含有无效字符: {c}"))? as u32;
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

// ===== 状态与试用期 =====

fn state_file_path(base: &Path) -> std::path::PathBuf {
    base.join("wzqlink").join("license.json")
}

// 启动时调用: 加载/初始化 license.json, 并对已存授权码重新验签(过期/文件被改均降级为未授权)
pub fn init(base: &Path) {
    let path = state_file_path(base);
    let store: LicenseStore = fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(LicenseStore { first_run_day: today_day(), code: String::new() });
    if store.code.is_empty() {
        save_store(&path, &store);
    }
    let info = if store.code.is_empty() {
        None
    } else {
        verify_code(&store.code).ok().map(|i| LicenseInfo { name: i.name, expire_days: i.expire_days })
    };
    let _ = STATE.set(Mutex::new(State { first_run_day: store.first_run_day, info }));
}

fn save_store(path: &Path, store: &LicenseStore) {
    let json = serde_json::to_string_pretty(store).unwrap();
    let _ = fs::write(path, json);
}

fn with_state<T>(f: impl FnOnce(&State) -> T) -> T {
    let state = STATE.get().expect("license 未初始化").lock().unwrap();
    f(&state)
}

pub fn status() -> LicenseStatus {
    let today = today_day();
    with_state(|s| {
        let licensed = match &s.info {
            Some(info) => info.expire_days as u64 >= today,
            None => false,
        };
        let elapsed = today.saturating_sub(s.first_run_day);
        LicenseStatus {
            licensed,
            name: s.info.as_ref().map(|i| i.name.clone()).unwrap_or_default(),
            expire: s
                .info
                .as_ref()
                .map(|i| if i.expire_days == 0 { "永久".into() } else { day_to_ymd(i.expire_days) })
                .unwrap_or_default(),
            trial_days_left: TRIAL_DAYS.saturating_sub(elapsed),
            trial_expired: !licensed && elapsed >= TRIAL_DAYS,
        }
    })
}

// 激活: 验签成功才写入文件(同时更新内存态); 失败返回错误文案给前端展示
pub fn activate(base: &Path, code: &str) -> Result<LicenseInfo, String> {
    let info = verify_code(code)?;
    {
        let mut state = STATE.get().expect("license 未初始化").lock().unwrap();
        state.info = Some(LicenseInfo { name: info.name.clone(), expire_days: info.expire_days });
    }
    let path = state_file_path(base);
    let mut store: LicenseStore = fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(LicenseStore { first_run_day: today_day(), code: String::new() });
    store.code = code.trim().to_string();
    save_store(&path, &store);
    Ok(info)
}

// 引擎档位闸门: 已授权或试用期内 → 按用户设置; 试用结束未授权 → 锁定小白档(免费)
pub fn effective_hint_level(configured: i32) -> i32 {
    let s = status();
    if s.licensed || !s.trial_expired {
        configured.clamp(0, 5)
    } else {
        1
    }
}

// 免费档锁定: 试用结束且未授权。引擎侧据此把深度压到 1 并关闭云库(比付费新手档更弱)
pub fn locked() -> bool {
    let s = status();
    !s.licensed && s.trial_expired
}

// ===== Tauri 命令 =====

#[tauri::command]
pub async fn get_license_status() -> LicenseStatus {
    status()
}

#[tauri::command]
pub async fn activate_license(app: tauri::AppHandle, code: String) -> Result<LicenseStatus, String> {
    use tauri::Manager as _;
    let base = app.path().config_dir().unwrap();
    let info = activate(&base, &code)?;
    let _ = info;
    Ok(status())
}

// ===== 发码工具入口(cargo run --bin license-keygen) =====

pub mod keygen {
    use ed25519_dalek::Signer as _;
    use ed25519_dalek::SigningKey;

    use super::encode_code;

    pub fn genkey() -> Result<(String, String), String> {
        let mut secret = [0u8; 32];
        rand_core::OsRng.fill_bytes(&mut secret);
        let signing = SigningKey::from_bytes(&secret);
        let priv_hex: String = secret.iter().map(|b| format!("{:02x}", b)).collect();
        let pub_hex: String = signing.verifying_key().to_bytes().iter().map(|b| format!("{:02x}", b)).collect();
        Ok((priv_hex, pub_hex))
    }

    pub fn sign(priv_hex: &str, product: u8, expire_days: u32, name: &str, machine: &str) -> Result<String, String> {
        let mut secret = [0u8; 32];
        for (i, b) in priv_hex.as_bytes().chunks_exact(2).enumerate() {
            secret[i] = u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16)
                .map_err(|e| format!("私钥格式错误: {e}"))?;
        }
        let signing = SigningKey::from_bytes(&secret);
        let mut payload = vec![1u8, product];
        payload.extend_from_slice(&expire_days.to_be_bytes());
        payload.push(name.len() as u8);
        payload.extend_from_slice(name.as_bytes());
        payload.push(machine.len() as u8);
        payload.extend_from_slice(machine.as_bytes());
        let mut raw = vec![payload.len() as u8];
        raw.extend_from_slice(&payload);
        raw.extend_from_slice(&signing.sign(&payload).to_bytes());
        Ok(encode_code(&raw))
    }

    use rand_core::RngCore as _;
}

// yyyy-mm-dd 或 yyyymmdd -> days since epoch (Howard Hinnant days_from_civil)
pub fn parse_date_days(s: &str) -> Result<u32, String> {
    let clean: String = s.chars().filter(|c| *c != '-').collect();
    if clean.len() != 8 || !clean.chars().all(|c| c.is_ascii_digit()) {
        return Err("日期格式应为 yyyymmdd".into());
    }
    let y: i64 = clean[0..4].parse().map_err(|_| "日期格式错误".to_string())?;
    let m: u32 = clean[4..6].parse().map_err(|_| "日期格式错误".to_string())?;
    let d: u32 = clean[6..8].parse().map_err(|_| "日期格式错误".to_string())?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return Err("日期数值无效".into());
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp as i64 + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Ok(days as u32)
}

// ===== 单元测试: 自签自验闭环(测试密钥对独立于生产密钥) =====

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::Signer as _;

    fn test_verify_with(pub_key: [u8; 32], code: &str) -> Result<LicenseInfo, String> {
        // 测试辅助: 临时替换公钥的验签路径
        // 由于 PUBLIC_KEY 是 const, 这里直接用内部函数重放验签逻辑
        let raw = decode_code(code)?;
        let payload_len = raw[0] as usize;
        assert_eq!(raw.len(), 1 + payload_len + 64);
        let payload = &raw[1..1 + payload_len];
        let sig = Signature::from_slice(&raw[1 + payload_len..]).unwrap();
        let key = VerifyingKey::from_bytes(&pub_key).unwrap();
        key.verify(payload, &sig).map_err(|_| "授权码无效".to_string())?;
        let expire_days = u32::from_be_bytes([payload[2], payload[3], payload[4], payload[5]]);
        let mut i = 6;
        let name_len = payload[i] as usize;
        i += 1;
        let name = String::from_utf8(payload[i..i + name_len].to_vec()).unwrap();
        Ok(LicenseInfo { name, expire_days })
    }

    #[test]
    fn test_sign_verify_roundtrip() {
        use ed25519_dalek::SigningKey;
        let signing = SigningKey::from_bytes(&[42u8; 32]);
        let pub_key = signing.verifying_key().to_bytes();

        let payload_exp = parse_date_days("2030-01-01").unwrap();
        let name = "张三";
        let mut payload = vec![1u8, 1];
        payload.extend_from_slice(&payload_exp.to_be_bytes());
        payload.push(name.len() as u8);
        payload.extend_from_slice(name.as_bytes());
        payload.push(0);
        let mut raw = vec![payload.len() as u8];
        raw.extend_from_slice(&payload);
        raw.extend_from_slice(&signing.sign(&payload).to_bytes());
        let code = encode_code(&raw);

        let info = test_verify_with(pub_key, &code).unwrap();
        assert_eq!(info.name, "张三");
        assert_eq!(info.expire_days, payload_exp);
    }

    #[test]
    fn test_tampered_code_rejected() {
        use ed25519_dalek::SigningKey;
        let signing = SigningKey::from_bytes(&[42u8; 32]);
        let pub_key = signing.verifying_key().to_bytes();

        let mut payload = vec![1u8, 1];
        payload.extend_from_slice(&40000u32.to_be_bytes());
        payload.push(2);
        payload.extend_from_slice("ab".as_bytes());
        payload.push(0);
        let mut raw = vec![payload.len() as u8];
        raw.extend_from_slice(&payload);
        raw.extend_from_slice(&signing.sign(&payload).to_bytes());
        // 篡改 payload 内容后验签必须失败
        raw[2] ^= 0xff;
        let code = encode_code(&raw);
        assert!(test_verify_with(pub_key, &code).is_err());
    }

    #[test]
    fn test_base32_roundtrip() {
        let data: Vec<u8> = (0..=255u8).collect();
        let decoded = decode_code(&encode_code(&data)).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_parse_date() {
        assert_eq!(parse_date_days("2027-12-31").unwrap(), parse_date_days("20271231").unwrap());
        assert!(parse_date_days("2027-13-01").is_err());
    }

    // 真实密钥签发的样例码(生产公钥+发码工具产物): 验证 verify_code 全路径
    #[test]
    fn test_real_code_accepted() {
        let code = "2W0G60-000000-ZS5TJK-JTV476-ME5YF5-58WT4B-E00A84-QJSYEQ-FP5DX6-YS0SVC-ERQAB3-01ZFXD-PFAP2Y-9Z4MN6-FE7X4A-4G3AV4-XT4DT5-KA0A14-2Q0WV0-04SNK6-MEHXZX-0G8CVA-D4EVBQ-4GR";
        let info = verify_code(code).unwrap();
        assert_eq!(info.name, "五子棋用户");
        assert_eq!(info.expire_days, 0); // 永久授权
    }

    #[test]
    fn test_wrong_product_rejected() {
        // 象棋(product=1)真实签发的授权码不能激活五子棋(product=3)
        let code = "2G0G20-000000-SSNNHF-MAZ5F7-JJMED2-5Q03W4-8VQFDP-MY4KQH-EXC57Q-6ZS2MX-5JYGCY-TS9KRR-ZFP3K3-9P1BNE-T07E6R-08NCXV-FZ5XZ6-FFTGJS-GH6Z24-8AVJY0-NZGDM9-E0BD3K-1S8D";
        assert!(verify_code(code).is_err());
    }
}
