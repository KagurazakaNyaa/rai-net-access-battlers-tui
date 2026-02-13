# RaiNet AccessBattlers TUI

雷NET联线战士 TUI版

## 规则说明

参考 [雷NET联线战士规则说明](RULE.md)

## 计划特性

### 服务器模式特性

- [ ] 可监听 unix domain socket（默认`/var/run/rainet/rainet.sock`，用于客户端连接）
- [ ] 可监听 tcp 协议端口 （默认`0.0.0.0:2321`，用于客户端连接）
- [ ] 可监听 ssh/telnet 协议端口（默认`0.0.0.0:2322-2323`，用于远程连接，连接为客户端模式）

### 客户端模式特性

- [ ] 可连接 unix domain socket
- [ ] 可连接 tcp 协议端口
- [ ] 探测终端颜色支持
- [ ] 探测终端大小
- [ ] 探测终端是否支持 ANSI 转义序列

### 游戏特性

- [ ] 自动创建对战房间
- [ ] 基于房间ID加入房间
- [ ] 基于玩家名加入房间
- [ ] 保存和加载Replay
