# 雷NET联线战士 TUI

雷NET联线战士 TUI 版

## 规则说明

参考 [雷NET联线战士规则说明](RULE.md)

## 计划特性

### 服务器模式特性

- [x] 可监听 Unix Domain Socket（默认`/tmp/rainet.sock`，用于客户端连接）
- [x] 可监听 TCP 协议端口（默认`0.0.0.0:2321`，用于客户端连接）
- [ ] 可监听 ssh/telnet 协议端口（默认`0.0.0.0:2322-2323`，用于远程连接，连接为客户端模式）

### 客户端模式特性

- [x] 可连接 Unix Domain Socket
- [x] 可连接 TCP 协议端口
- [ ] 探测终端颜色支持
- [ ] 探测终端大小
- [ ] 探测终端是否支持 ANSI 转义序列

### 游戏特性

- [x] 自动创建对战房间
- [x] 基于房间ID加入房间
- [ ] 基于玩家名加入房间
- [ ] 保存和加载Replay

## 使用说明

### 启动服务器

默认同时监听 TCP 和 Unix Socket：

```bash
cargo run -- server
```

仅监听 TCP：

```bash
cargo run -- server --mode tcp
```

仅监听 Unix Socket（可指定路径）：

```bash
cargo run -- server --mode unix --unix /tmp/rainet.sock
```

同时监听（显式指定）：

```bash
cargo run -- server --mode both
```

默认 Unix Socket 路径为 `/tmp/rainet.sock`，一般无需额外权限。

可指定日志文件：

```bash
cargo run -- server --log server.log
```

### 启动客户端

连接 TCP：

```bash
cargo run -- client --tcp 127.0.0.1:2321 --name P1
```

连接 Unix Socket：

```bash
cargo run -- client --unix /tmp/rainet.sock --name P1
```

客户端 ID（可选，未提供则自动生成）：

```bash
cargo run -- client --tcp 127.0.0.1:2321 --name P1 --id client-123
```

语言选择（可选，未传入则使用系统 LANG，无法判断则回落英文）：

```bash
cargo run -- client --tcp 127.0.0.1:2321 --name P1 --lang zh-CN
```

## 房间系统

### 房间列表与加入

客户端启动后进入大厅：

- ↑/↓：选择房间
- Enter：加入选中房间
- S：观战选中房间
- J：输入房间 ID 加入
- C：创建房间（输入名称）
- A：自动加入（加入可自动加入的房间，否则创建）
- T：切换“创建房间时自动加入”开关

### 房间容量

- 玩家：固定 2 人
- 观战：若干（不参与操作）

### 协议

完整协议见 [PROTOCOL.md](PROTOCOL.md)

## 快捷键

### 全局

- `Q`：退出
- `H`：显示帮助

### 通用移动

- 方向键：移动光标
- `H/J/K/L`：移动光标（vim）

### 布局（Setup）

- `L`：放置 Link
- `V`：放置 Virus
- `Backspace`：移除

### 移动/战斗

- `Enter`：选择/确认
- `Esc`：取消/返回
- `T`：打开终端
- `E`：进入服务器

### 终端（Terminal）

- `1`：LineBoost
- `2`：VirusCheck
- `3`：Firewall
- `4`：404

### LineBoost

- `Enter`：附加/拆卸
- `Esc`：返回

### VirusCheck

- `Enter`：揭示
- `Esc`：返回

### Firewall

- `Enter`：放置/移除
- `Esc`：返回

### 404

- `Enter`：选择
- `Y`：交换
- `N`：不交换

### 服务器（Server）

- `Y`：显示
- `N`：隐藏
- `L`：Link 栈
- `V`：Virus 栈
