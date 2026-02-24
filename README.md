# AoiPlate 操作手册（macOS）

AoiPlate 是一个为 **极简待办** 设计的 macOS 菜单栏应用。  
你可以在任何界面里通过 **双击字母键（默认 `J`）** 拉起“盘子爆炸舞台”，快速查看和处理任务。

## 1. 你能做什么
- 双击 `J` 打开/收起舞台
- 盘子大小按时间权重变化（越早创建越大）
- `Cmd + 单击` 任务盘子：标记完成并淡出
- 双击盘子文字：就地编辑
- 右键盘子：删除任务
- 按 `N`：新增任务
- 按 `M`：切换爆炸模式 / 列表模式

## 2. 快速开始（普通用户）
### 2.1 启动应用
在项目目录运行：

```bash
cargo run
```

看到菜单栏图标后，说明应用已常驻。

### 2.2 首次权限设置（非常重要）
如果双击 `J` 没反应，请在系统里开启权限：
- `输入监听`
- `辅助功能`

你可以通过菜单栏图标菜单里的“打开权限设置”直接跳转。

### 2.3 第一次打开舞台
1. 在任意界面双击 `J`
2. 舞台出现后按 `N` 新建任务
3. 回车保存

## 3. 日常使用说明
### 3.1 添加任务
- 打开舞台后按 `N`
- 输入一行文字，按回车保存

### 3.2 完成任务
- 按住 `Cmd`，单击任务盘子
- 盘子会淡出，剩余任务自动重排

### 3.3 编辑任务
- 双击盘子文字进入编辑
- `Enter` 保存，`Esc` 取消

### 3.4 删除任务
- 右键盘子，点击“删除此任务”

### 3.5 收起舞台
- 双击 `J`
- 或按 `Esc`
- 或点击空白区域

## 4. 数据与迁移
数据目录：
- `~/Library/Application Support/AoiPlate/todos.json`
- `~/Library/Application Support/AoiPlate/config.json`
- `~/Library/Application Support/AoiPlate/error.log`

迁移规则：
- 首次启动会自动尝试将旧版 `TodoLite` 的数据迁移到 `AoiPlate`
- 迁移失败不会阻塞启动，但会在日志里记录

## 5. 常见问题
### Q1：双击 `J` 没反应
- 先检查“输入监听”权限是否已开启
- 再检查菜单栏图标是否仍在（应用是否在运行）

### Q2：我担心数据丢失
- 所有数据都在本地 JSON 文件
- 可以直接备份 `~/Library/Application Support/AoiPlate/` 整个目录

### Q3：看不到任务盘子
- 先按 `N` 新建任务
- 若仍无显示，退出后重新 `cargo run`

## 6. 开发者命令
```bash
cargo check
cargo build --release
```

推荐的本地环境工具：
```bash
brew install mise direnv
cd /Users/shiwen/Desktop/Todo_lite
mise trust .mise.toml
direnv allow
```
