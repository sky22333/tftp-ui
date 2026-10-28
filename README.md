# TFTP 服务端

Windows 上的轻量 TFTP 服务端 GUI。Rust + Slint，软件渲染，单文件 exe，免装运行库。

## 功能

- 监听地址可选单块 IPv4 网卡或全部网卡，地址列表可刷新，端口可改
- 任选根目录，只读对外提供文件
- 请求日志：显示读请求、文件已接受或请求被拒绝；不表示传输完成
- 静态 CRT，约 9.5 MB，运行内存约 15 MB

## 使用

1. 双击 `tftp-ui.exe`
2. 「监听地址」选接设备的那块网卡，必要时点击刷新图标；「根目录」选文件所在文件夹
3. 点「启动服务」
4. 设备端执行 `tftp <本机IP> GET <文件名>`；U-Boot / Breed 下为 `tftpboot`

## 构建

```bash
cargo build --release
```

产物 `target/release/tftp-ui.exe`。单文件依赖 `.cargo/config.toml` 中的 `+crt-static`
