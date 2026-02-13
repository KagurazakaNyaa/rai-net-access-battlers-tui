# RaiNet AccessBattlers TUI

雷NET联线战士 TUI版

## 规则说明

参考 [雷NET联线战士规则说明](RULE.md)

## 计划特性

### 服务器模式特性

- [x] 可监听 unix domain socket（默认`/tmp/rainet.sock`，用于客户端连接）
- [x] 可监听 tcp 协议端口 （默认`0.0.0.0:2321`，用于客户端连接）
- [ ] 可监听 ssh/telnet 协议端口（默认`0.0.0.0:2322-2323`，用于远程连接，连接为客户端模式）

### 客户端模式特性

- [x] 可连接 unix domain socket
- [x] 可连接 tcp 协议端口
- [ ] 探测终端颜色支持
- [ ] 探测终端大小
- [ ] 探测终端是否支持 ANSI 转义序列

### 游戏特性

- [ ] 自动创建对战房间
- [ ] 基于房间ID加入房间
- [ ] 基于玩家名加入房间
- [ ] 保存和加载Replay

## 使用说明

### 启动服务器

默认同时监听 TCP 和 Unix socket：

```bash
cargo run -- server
```

仅监听 TCP：

```bash
cargo run -- server --mode tcp
```

仅监听 Unix socket（可指定路径）：

```bash
cargo run -- server --mode unix --unix /tmp/rainet.sock
```

同时监听（显式指定）：

```bash
cargo run -- server --mode both
```

默认 Unix socket 路径为 `/tmp/rainet.sock`，一般无需额外权限：

```

### 启动客户端

连接 TCP：

```bash
cargo run -- client --tcp 127.0.0.1:2321 --name P1
```

连接 Unix socket：

```bash
cargo run -- client --unix /tmp/rainet.sock --name P1
```
