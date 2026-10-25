//! 本机网络地址枚举。只保留可用的 IPv4 地址，TFTP 刷机场景全部走 IPv4。

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr};

#[derive(Clone)]
pub struct Adapter {
    /// 下拉框里显示的文本
    pub label: String,
    /// 绑定时使用的地址
    pub ip: IpAddr,
}

/// 第一项固定为通配地址，表示监听全部网卡。
pub fn list() -> Vec<Adapter> {
    let mut out = vec![Adapter {
        label: "全部网卡 · 0.0.0.0".to_string(),
        ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
    }];

    let Ok(ifaces) = if_addrs::get_if_addrs() else {
        return out;
    };

    let mut seen = HashSet::new();
    let mut items: Vec<(String, IpAddr)> = Vec::new();

    for it in ifaces {
        let ip = match it.addr {
            if_addrs::IfAddr::V4(a) => IpAddr::V4(a.ip),
            if_addrs::IfAddr::V6(a) => IpAddr::V6(a.ip),
        };

        // 只要 IPv4，跳过回环与 169.254 自动地址
        let IpAddr::V4(v4) = ip else { continue };
        if v4.is_loopback() || v4.is_link_local() || v4.is_unspecified() {
            continue;
        }
        if !seen.insert(v4) {
            continue;
        }

        items.push((it.name.to_string(), ip));
    }

    items.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, ip) in items {
        out.push(Adapter {
            label: format!("{name} · {ip}"),
            ip,
        });
    }

    out
}
