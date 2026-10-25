# TFTP 服务端

Windows 上的轻量 TFTP 服务端 GUI。Rust + Slint，软件渲染，单文件 exe，免装运行库。

## 功能

- 监听地址可选单块网卡或全部网卡，端口可改
- 任选根目录，只读对外提供文件
- 实时日志：时间 / 客户端 / 读取或拒绝 / 文件 / 大小
- 静态 CRT，约 9.5 MB，运行内存约 33 MB

## 使用

1. 双击 `TFTP服务端.exe`
2. 「监听地址」选接设备的那块网卡，「根目录」选文件所在文件夹
3. 点「启动服务」
4. 设备端执行 `tftp <本机IP> GET <文件名>`；U-Boot / Breed 下为 `tftpboot`

## 只读

不提供写入。写入请求、路径穿越（`..`、绝对路径）一律拒绝并记入日志。

## 构建

```bash
cargo build --release
```

产物 `target/release/tftp-tool.exe`。单文件依赖 `.cargo/config.toml` 中的 `+crt-static`，勿删。

## 发布

Actions → 「发布 TFTP 服务端」→ Run workflow，填版本标签（如 `v1.0.0`）。流水线编译后校验静态 CRT，再创建同名 tag 与 Release。
