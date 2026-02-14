# 协议

基于行的文本协议，使用 TCP/Unix Socket 传输，所有消息以换行结尾。

## 客户端 → 服务器

握手：

```
HELLO <client_id> <name>
```

房间指令：

```
OP ROOM LIST
OP ROOM CREATE <name> <auto:0|1>
OP ROOM JOIN <room_id>
OP ROOM SPECTATE <room_id>
OP ROOM AUTO
OP ROOM LEAVE
```

游戏操作（仅当前回合玩家有效）：

```
OP SETUP L <row> <col>
OP SETUP V <row> <col>
OP REMOVE <row> <col>
OP MOVE <from_row> <from_col> <to_row> <to_col>
OP BOOST <from_row> <from_col> <to_row> <to_col>
OP ENTER <from_row> <from_col> <reveal:0|1> <stack:L|V>
OP LINEBOOST ATTACH <row> <col>
OP LINEBOOST DETACH <row> <col>
OP VIRUSCHECK <row> <col>
OP FIREWALL PLACE <row> <col>
OP FIREWALL REMOVE <row> <col>
OP NOTFOUND <row1> <col1> <row2> <col2> <swap:0|1>
OP ENDTURN
```

## 服务器 → 客户端

角色分配：

```
YOU P1|P2|SPEC|LOBBY
```

房间列表：

```
ROOMS_BEGIN
ROOMS <n>
ROOM <id> <name> <players> <spectators> <auto:0|1> <WAITING|PLAYING>
...
ROOMS_END
```

状态快照：

```
STATE_BEGIN
PHASE SETUP <P1|P2>
PHASE PLAYING
PHASE GAMEOVER <P1|P2>
CURRENT <P1|P2>
PENDING <row> <col> | PENDING NONE
PLAYER P1 SETUP_LINKS <n> SETUP_VIRUSES <n> SETUP_PLACED <n>
PLAYER P2 SETUP_LINKS <n> SETUP_VIRUSES <n> SETUP_PLACED <n>
PLAYER P1 LINEBOOST <row,col|-> <row,col|->
PLAYER P2 LINEBOOST <row,col|-> <row,col|->
PLAYER P1 FIREWALL <row,col|-> <row,col|->
PLAYER P2 FIREWALL <row,col|-> <row,col|->
PLAYER P1 VIRUSCHECK <0|1> <0|1>
PLAYER P2 VIRUSCHECK <0|1> <0|1>
PLAYER P1 NOTFOUND <0|1> <0|1>
PLAYER P2 NOTFOUND <0|1> <0|1>
STACKS P1 LINK <n> VIRUS <n>
STACKS P2 LINK <n> VIRUS <n>
CARDS <n>
CARD <row> <col> <P1|P2> <L|V> <revealed:0|1> <boost:0|1>
FIREWALLS <n>
FW <row> <col> <P1|P2>
NAMES <p1_name> <p2_name>
ROOMPLAYERS <name1,name2,...>
ROOMSPECTATORS <name1,name2,...>
STATE_END
```

错误：

```
ERR <CODE>
ERR INVALID_OP <GAME_ERROR_CODE>
ERR JOIN_FAILED <io error>
ERR SPECTATE_FAILED <io error>
ERR AUTO_JOIN_FAILED <io error>
```

### 错误码

通用：

- `NOT_IN_ROOM`
- `NOT_A_PLAYER`
- `ROOM_NOT_FOUND`
- `ROOM_NOT_READY`
- `NOT_YOUR_TURN`

游戏错误码（来自 `GameError`）：

- `OUT_OF_BOUNDS`
- `NOT_ADJACENT`
- `NO_CARD`
- `NOT_YOUR_CARD`
- `OCCUPIED_BY_OWN_CARD`
- `OWN_EXIT_BLOCKED`
- `OPPONENT_FIREWALL`
- `INVALID_SETUP_POSITION`
- `SETUP_EXHAUSTED`
- `SETUP_NOT_CURRENT_PLAYER`
- `NOT_IN_SETUP_PHASE`
- `NOT_IN_PLAYING_PHASE`
- `NOT_ON_OPPONENT_EXIT`
- `FIREWALL_ON_EXIT`
- `TERMINAL_CARD_USED`
- `INVALID_TARGET`
- `PENDING_BOOST_MOVE`
- `NO_PENDING_BOOST_MOVE`
- `CANNOT_ENTER_SERVER_WITH_BOOST`
