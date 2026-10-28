//! 枚举本机可用 IPv4 地址，TFTP 刷机场景全部走 IPv4。

use std::net::{IpAddr, Ipv4Addr};

pub struct Adapter {
    pub label: String,
    pub ip: IpAddr,
}

pub fn all_addresses() -> Adapter {
    Adapter {
        label: "全部 IPv4 地址 · 0.0.0.0".to_string(),
        ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
    }
}

/// 首项为 0.0.0.0，即监听全部网卡。
pub fn list() -> std::io::Result<Vec<Adapter>> {
    let mut out = vec![all_addresses()];

    let ifaces = if_addrs::get_if_addrs()?;
    let mut items: Vec<(String, Ipv4Addr)> = Vec::new();

    for it in ifaces {
        let if_addrs::IfAddr::V4(addr) = it.addr else {
            continue;
        };
        if addr.ip == Ipv4Addr::LOCALHOST {
            continue;
        }
        items.push((it.name, addr.ip));
    }

    items.sort();
    items.dedup();

    for (name, ip) in items {
        out.push(Adapter {
            label: format!("{name} · {ip}"),
            ip: IpAddr::V4(ip),
        });
    }

    Ok(out)
}
