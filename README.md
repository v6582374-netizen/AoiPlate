# AoiPlate 1.1（macOS）操作手册

AoiPlate 是一款极简、轻量的菜单栏待办应用。  
你可以在任意界面通过双击触发键（默认 `J`）呼出“爆炸舞台”，用盘子直接管理任务。

## 1. 核心特性
- 全局双击 `J` 打开/收起舞台
- 舞台打开时显示 Dock 图标，收起后自动隐藏 Dock 图标
- 盘子尺寸整体放大，最小盘子半径较上一版提升到约 2 倍
- 盘子大小按时间权重自动变化（越早创建越大）
- `Cmd + 单击` 盘子：标记完成并淡出
- 双击盘子：短文本原位编辑，长文本弹出“盘子旁圆角编辑窗”
- 右键盘子：删除
- 按 `N`：在舞台底部打开新增输入栏
- 按 `M`：切换爆炸模式 / 列表模式
- 列表模式支持实时搜索未完成任务，长文本支持自动换行

## 2. 普通用户安装（推荐）
1. 从 GitHub Release 下载 `AoiPlate.dmg`
2. 双击打开 DMG
3. 将 `AoiPlate.app` 拖到 `Applications`
4. 从“应用程序”中启动 `AoiPlate`

备用包：
- `AoiPlate.app.zip`：解压后得到 `.app`
- `AoiPlate-macos-arm64.tar.gz`：终端用户使用

## 3. 首次启动与权限
如果双击 `J` 没反应，请在系统隐私设置中开启：
- 输入监听
- 辅助功能

可通过菜单栏图标菜单中的“打开权限设置”快速跳转。
若你暂时不想看到权限提示，可以点提示条左上角 `×` 关闭（本次运行有效，重启后若仍缺权限会再次提示）。

## 4. 日常使用
### 4.1 打开/关闭舞台
- 双击 `J`
- 或菜单栏点击“显示 / 隐藏 AoiPlate”
- `Esc` 或点击空白处也可收起

### 4.2 新增任务
- 舞台中按 `N`
- 底部输入栏输入内容
- 回车保存（或点“保存”）

### 4.3 完成任务
- 按住 `Cmd` 单击盘子
- 盘子 700ms 左右淡出，剩余任务自动重排

### 4.4 编辑任务
- 双击盘子进入编辑
- 若文本较长且盘子内显示不全，会在盘子旁弹出圆角编辑框
- `Enter` 保存，`Esc` 取消

### 4.5 列表模式与搜索
- 按 `M` 进入列表模式
- 右上角搜索框实时过滤“未完成任务”
- 再按 `M` 返回爆炸模式

## 5. 数据存储与迁移
数据目录：
- `~/Library/Application Support/AoiPlate/todos.json`
- `~/Library/Application Support/AoiPlate/config.json`
- `~/Library/Application Support/AoiPlate/error.log`

旧版本 `TodoLite` 目录会在首次启动时自动迁移到 `AoiPlate`。

## 6. 常见问题
### Q1：双击 `J` 无反应
- 检查输入监听/辅助功能权限
- 检查菜单栏图标是否存在（应用是否在运行）

### Q2：下载后双击 `AoiPlate-macos-arm64` 报“UTF-8 文本编码不适用”
- 这是 CLI 二进制，不是 Finder 双击启动的 GUI 包
- 请改用 `AoiPlate.dmg` 或 `AoiPlate.app.zip`

### Q3：任务看不到
- 先按 `N` 新建一个任务
- 或按 `M` 看列表模式是否有任务

## 7. 开发与构建
```bash
cargo check
cargo build --release
cargo bundle --release
```

推荐虚拟环境：
```bash
brew install mise direnv
cd /Users/shiwen/Desktop/Todo_lite
mise trust .mise.toml
direnv allow
```

## 8. 生成 1.1 Release 资产
一键构建：
```bash
./scripts/build_release_assets_macos.sh
```

产物位于 `dist/`：
- `AoiPlate.dmg`（主安装包）
- `AoiPlate.app.zip`
- `AoiPlate-macos-arm64.tar.gz`
- `SHA256SUMS.txt`

发布到 GitHub Release 时，建议将版本号设置为：`v1.1.0`。
